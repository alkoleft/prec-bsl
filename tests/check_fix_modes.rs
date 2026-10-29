use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

/// Matrix of `--mode` file selection: staged change, unstaged change,
/// combined staged+unstaged change (`MM`/`AM`), untracked file, and clean
/// file. Each mode must process the correct set.
#[test]
fn check_fix_modes_select_expected_file_sets() {
    for mode in ["all", "changed", "index", "worktree"] {
        let repo = TempGitRepo::new(&format!("mode_{mode}"));
        let staged = PathBuf::from("src/Стадированный.bsl");
        let unstaged = PathBuf::from("src/Нестадированный.bsl");
        let untracked = PathBuf::from("src/Новый.bsl");
        let clean = PathBuf::from("src/Чистый.bsl");
        let combined = PathBuf::from("src/Комбинированный.bsl");
        let combined_new = PathBuf::from("src/КомбинированныйНовый.bsl");

        write_file(repo.path().join(&staged), DIRTY_MODULE);
        write_file(repo.path().join(&unstaged), DIRTY_MODULE);
        write_file(repo.path().join(&clean), CLEAN_MODULE);
        write_file(repo.path().join(&combined), DIRTY_MODULE);
        run_git(repo.path(), ["add", "."]);
        run_git(repo.path(), ["commit", "-m", "baseline"]);

        // staged modification
        write_file(repo.path().join(&staged), DIRTYER_MODULE);
        run_git(repo.path(), ["add", "src/Стадированный.bsl"]);
        // unstaged modification
        write_file(repo.path().join(&unstaged), DIRTYER_MODULE);
        // untracked file
        write_file(repo.path().join(&untracked), DIRTYER_MODULE);
        // combined modification (staged edit + further unstaged edit): MM
        write_file(repo.path().join(&combined), DIRTYER_MODULE);
        run_git(repo.path(), ["add", "src/Комбинированный.bsl"]);
        write_file(repo.path().join(&combined), DIRTYEST_MODULE);
        // combined addition (staged new file + further unstaged edit): AM
        write_file(repo.path().join(&combined_new), DIRTYER_MODULE);
        run_git(repo.path(), ["add", "src/КомбинированныйНовый.bsl"]);
        write_file(repo.path().join(&combined_new), DIRTYEST_MODULE);

        let output = run_prec_bsl(
            repo.path(),
            [
                "check",
                repo.path().to_str().unwrap(),
                "--source-dir",
                "src",
                "--rules",
                "УдалениеЛишнихКонцевыхПробелов",
                "--mode",
                mode,
            ],
        );

        assert_eq!(
            output.status.code(),
            Some(1),
            "mode {mode}: {}",
            output_text(&output)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let expected_files = match mode {
            "all" => vec![
                &staged,
                &unstaged,
                &untracked,
                &clean,
                &combined,
                &combined_new,
            ],
            "changed" => vec![&staged, &unstaged, &untracked, &combined, &combined_new],
            "index" => vec![&staged, &combined, &combined_new],
            "worktree" => vec![&unstaged, &untracked, &combined, &combined_new],
            _ => unreachable!(),
        };
        let expected_count = expected_files.len();
        assert!(
            stdout.contains(&format!(
                "prec-bsl check: обработано файлов: {expected_count}"
            )),
            "mode {mode}: unexpected stdout:\n{stdout}"
        );
        for path in &[&staged, &unstaged, &untracked, &combined, &combined_new] {
            let must_appear = expected_files.contains(path);
            if must_appear {
                assert!(
                    stdout.contains(path.to_string_lossy().as_ref()),
                    "mode {mode}: expected {} in report:\n{stdout}",
                    path.display()
                );
            }
        }
        let clean_mentions = stdout.matches("src/Чистый.bsl").count();
        assert_eq!(
            clean_mentions, 0,
            "mode {mode}: clean file must not appear in check report:\n{stdout}"
        );

        // check must never write
        for path in [
            &staged,
            &unstaged,
            &untracked,
            &clean,
            &combined,
            &combined_new,
        ] {
            assert_eq!(
                fs::read_to_string(repo.path().join(path)).unwrap(),
                if path == &clean {
                    CLEAN_MODULE
                } else if path == &combined || path == &combined_new {
                    DIRTYEST_MODULE
                } else {
                    DIRTYER_MODULE
                },
                "mode {mode}: check must not modify {}",
                path.display()
            );
        }
    }
}

#[test]
fn check_never_writes_files_and_reports_diff() {
    let repo = TempGitRepo::new("check_no_write");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(
        repo.path().join(&module),
        "Процедура Тест()   \nКонецПроцедуры\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);
    write_file(
        repo.path().join(&module),
        "Процедура Тест()   \nКонецПроцедуры\t\n",
    );

    let output = run_prec_bsl(
        repo.path(),
        [
            "check",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
            "--mode",
            "changed",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Предлагаемые изменения (не записаны):"));
    assert!(stdout.contains("-Процедура Тест()   "));
    assert!(stdout.contains("+Процедура Тест()"));
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        "Процедура Тест()   \nКонецПроцедуры\t\n",
        "check must not modify the file"
    );
}

#[test]
fn fix_writes_files_and_exits_one() {
    let repo = TempGitRepo::new("fix_applies");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(
        repo.path().join(&module),
        "Процедура Тест()   \nКонецПроцедуры\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);
    write_file(
        repo.path().join(&module),
        "Процедура Тест()   \nКонецПроцедуры\t\n",
    );

    let output = run_prec_bsl(
        repo.path(),
        [
            "fix",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
            "--mode",
            "changed",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        "Процедура Тест()\nКонецПроцедуры\n",
        "fix must apply the fixer"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("Предлагаемые изменения"));
}

#[test]
fn fix_dry_run_does_not_write_and_exits_one() {
    let repo = TempGitRepo::new("fix_dry_run");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(
        repo.path().join(&module),
        "Процедура Тест()   \nКонецПроцедуры\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);
    write_file(
        repo.path().join(&module),
        "Процедура Тест()   \nКонецПроцедуры\t\n",
    );

    let output = run_prec_bsl(
        repo.path(),
        [
            "fix",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
            "--mode",
            "changed",
            "--dry-run",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        "Процедура Тест()   \nКонецПроцедуры\t\n",
        "fix --dry-run must not modify the file"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Предлагаемые изменения (не записаны):"));
    assert!(stdout.contains("+Процедура Тест()"));
}

#[test]
fn check_and_fix_exit_zero_on_clean_repository() {
    let repo = TempGitRepo::new("clean_repo");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(repo.path().join(&module), CLEAN_MODULE);
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    for command in ["check", "fix"] {
        let output = run_prec_bsl(
            repo.path(),
            [
                command,
                repo.path().to_str().unwrap(),
                "--source-dir",
                "src",
                "--rules",
                "УдалениеЛишнихКонцевыхПробелов",
                "--mode",
                "changed",
            ],
        );

        assert_eq!(
            output.status.code(),
            Some(0),
            "{command}: {}",
            output_text(&output)
        );
    }
}

#[test]
fn check_reports_checker_diagnostics_without_modifications() {
    let repo = TempGitRepo::new("checker_diagnostics");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(
        repo.path().join(&module),
        concat!(
            "Процедура Плохой()\n",
            "    Перейти ~Метка;\n",
            "~Метка:\n",
            "КонецПроцедуры\n",
        ),
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    let output = run_prec_bsl(
        repo.path(),
        [
            "check",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            "ЗапретИспользованияПерейти",
            "--mode",
            "all",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("оператор Перейти запрещен"));
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        concat!(
            "Процедура Плохой()\n",
            "    Перейти ~Метка;\n",
            "~Метка:\n",
            "КонецПроцедуры\n",
        ),
        "check must not modify files for checker scenarios"
    );
}

#[test]
fn modes_skip_deleted_files_with_existing_message() {
    let repo = TempGitRepo::new("deleted_files");
    let module = PathBuf::from("src/Удаленный.bsl");
    write_file(repo.path().join(&module), CLEAN_MODULE);
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);
    fs::remove_file(repo.path().join(&module)).unwrap();
    run_git(repo.path(), ["add", "-A"]);

    for mode in ["changed", "index"] {
        let output = run_prec_bsl(
            repo.path(),
            [
                "check",
                repo.path().to_str().unwrap(),
                "--source-dir",
                "src",
                "--rules",
                "УдалениеЛишнихКонцевыхПробелов",
                "--mode",
                mode,
            ],
        );

        assert!(
            output.status.success(),
            "mode {mode}: {}",
            output_text(&output)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("сценарий не обрабатывает удаленные файлы"),
            "mode {mode}: deleted files must be skipped with the existing message:\n{stdout}"
        );
    }
}

#[test]
fn exec_rules_warns_about_deprecation_but_keeps_behavior() {
    let repo = TempGitRepo::new("exec_rules_deprecated");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(
        repo.path().join(&module),
        "Процедура Тест()   \nКонецПроцедуры\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    let output = run_prec_bsl(
        repo.path(),
        [
            "exec-rules",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
        ],
    );

    // exec-rules keeps its historic exit codes: success when only fixers ran.
    assert!(output.status.success(), "{}", output_text(&output));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("exec-rules устарела"),
        "exec-rules must warn about deprecation: {stderr}"
    );
    assert!(stderr.contains("fix --mode all"));
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        "Процедура Тест()\nКонецПроцедуры\n",
        "exec-rules must keep writing files"
    );
}

#[test]
fn check_and_fix_exit_two_on_execution_errors() {
    let output = run_prec_bsl(
        std::env::current_dir().unwrap(),
        [
            "check",
            "/nonexistent-prec-bsl-repo",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
        ],
    );

    assert_eq!(output.status.code(), Some(2), "{}", output_text(&output));

    let fix_output = run_prec_bsl(
        std::env::current_dir().unwrap(),
        [
            "fix",
            "/nonexistent-prec-bsl-repo",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
        ],
    );

    assert_eq!(
        fix_output.status.code(),
        Some(2),
        "{}",
        output_text(&fix_output)
    );
}

#[test]
fn fix_json_output_includes_proposed_content_for_dry_run() {
    let repo = TempGitRepo::new("json_dry_run");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(
        repo.path().join(&module),
        "Процедура Тест()   \nКонецПроцедуры\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);
    write_file(
        repo.path().join(&module),
        "Процедура Тест()   \nКонецПроцедуры\t\n",
    );

    let output = run_prec_bsl(
        repo.path(),
        [
            "fix",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
            "--mode",
            "changed",
            "--dry-run",
            "--format",
            "json",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    // fix --dry-run runs the pipeline in check mode: diagnostics only.
    assert!(stdout.contains("\"mode\": \"check\""));
    assert!(stdout.contains("proposed_content"));
}

/// `fix --restage` on a file with a combined `MM` status: the fixed working
/// tree must land in the Git index too, exactly like the hook restaging step.
#[test]
fn fix_restage_updates_index_of_combined_status_file() {
    let repo = TempGitRepo::new("fix_restage_mm");
    let module = PathBuf::from("src/Комбинированный.bsl");
    write_file(repo.path().join(&module), DIRTY_MODULE);
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    // staged edit, then a further unstaged edit: status becomes MM
    write_file(repo.path().join(&module), DIRTYER_MODULE);
    run_git(repo.path(), ["add", "src/Комбинированный.bsl"]);
    write_file(repo.path().join(&module), DIRTYEST_MODULE);

    let output = run_prec_bsl(
        repo.path(),
        [
            "fix",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
            "--mode",
            "index",
            "--restage",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        CLEAN_MODULE,
        "fix --restage must fix the working-tree copy"
    );
    assert_eq!(
        run_git_stdout(repo.path(), ["show", ":src/Комбинированный.bsl"]),
        CLEAN_MODULE,
        "fix --restage must put the fixed content into the index"
    );
    assert_eq!(
        run_git_stdout(repo.path(), ["diff", "--name-only"]),
        "",
        "after --restage the working tree must match the index"
    );
}

/// Default `fix` without `--restage` keeps the staged content untouched:
/// only the working copy is fixed (backward-compatible contract).
#[test]
fn fix_without_restage_leaves_index_untouched() {
    let repo = TempGitRepo::new("fix_no_restage_mm");
    let module = PathBuf::from("src/Комбинированный.bsl");
    write_file(repo.path().join(&module), DIRTY_MODULE);
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    write_file(repo.path().join(&module), DIRTYER_MODULE);
    run_git(repo.path(), ["add", "src/Комбинированный.bsl"]);
    write_file(repo.path().join(&module), DIRTYEST_MODULE);

    let output = run_prec_bsl(
        repo.path(),
        [
            "fix",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
            "--mode",
            "index",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        CLEAN_MODULE,
        "fix must fix the working-tree copy"
    );
    assert_eq!(
        run_git_stdout(repo.path(), ["show", ":src/Комбинированный.bsl"]),
        DIRTYER_MODULE,
        "fix without --restage must not touch the staged content"
    );
}

/// `--restage` together with `--dry-run` is a warning, not an error: the
/// dry-run still runs and no file is written or staged.
#[test]
fn fix_dry_run_with_restage_warns_and_keeps_files_untouched() {
    let repo = TempGitRepo::new("fix_dry_run_restage");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(repo.path().join(&module), DIRTY_MODULE);
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);
    write_file(repo.path().join(&module), DIRTYER_MODULE);

    let output = run_prec_bsl(
        repo.path(),
        [
            "fix",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
            "--mode",
            "changed",
            "--dry-run",
            "--restage",
        ],
    );

    // dry-run semantics are unchanged: exit 1 means "there is something to
    // apply", not an error.
    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--restage"),
        "fix --dry-run --restage must warn about the useless flag: {stderr}"
    );
    assert!(
        stderr.contains("предупреждение"),
        "fix --dry-run --restage warning must be marked as a warning: {stderr}"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        DIRTYER_MODULE,
        "fix --dry-run --restage must not modify the file"
    );
    assert_eq!(
        run_git_stdout(repo.path(), ["show", ":src/Модуль.bsl"]),
        DIRTY_MODULE,
        "fix --dry-run --restage must not touch the index"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Предлагаемые изменения (не записаны):"));
}

/// `check` does not accept `--restage`: it is a parse error like any other
/// foreign flag, exiting with the CLI error code.
#[test]
fn check_rejects_restage_as_cli_error() {
    let repo = TempGitRepo::new("check_restage_rejected");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(repo.path().join(&module), DIRTY_MODULE);
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    let output = run_prec_bsl(
        repo.path(),
        [
            "check",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
            "--mode",
            "changed",
            "--restage",
        ],
    );

    assert_eq!(output.status.code(), Some(2), "{}", output_text(&output));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("неизвестный параметр для check: --restage"),
        "check must reject --restage with a clear parse error: {stderr}"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        DIRTY_MODULE,
        "rejected check run must not modify files"
    );
}

const DIRTY_MODULE: &str = "Процедура Тест()   \nКонецПроцедуры\n";
const DIRTYER_MODULE: &str = "Процедура Тест()   \nКонецПроцедуры\t\n";
const DIRTYEST_MODULE: &str = "Процедура Тест()   \nКонецПроцедуры \t \n";
const CLEAN_MODULE: &str = "Процедура Тест()\nКонецПроцедуры\n";

struct TempGitRepo {
    path: PathBuf,
}

impl TempGitRepo {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after UNIX_EPOCH")
            .as_nanos();
        let path = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("check-fix-modes-tests")
            .join(format!("{name}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        run_git(&path, ["init"]);
        run_git(&path, ["config", "user.email", "prec-bsl@example.invalid"]);
        run_git(&path, ["config", "user.name", "prec-bsl tests"]);
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempGitRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_prec_bsl<const N: usize>(cwd: impl AsRef<Path>, args: [&str; N]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_prec-bsl"))
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap()
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn run_git<const N: usize>(repo: &Path, args: [&str; N]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("-c")
        .arg("core.quotePath=false")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git command failed in {}: {}\nstdout: {}\nstderr: {}",
        repo.display(),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_git_stdout<const N: usize>(repo: &Path, args: [&str; N]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("-c")
        .arg("core.quotePath=false")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git command failed in {}: {}\nstdout: {}\nstderr: {}",
        repo.display(),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn output_text(output: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}
