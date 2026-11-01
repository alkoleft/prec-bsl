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
    // The whole stdout must be a single valid JSON document: the text diff
    // trailer must not be appended after the JSON report.
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|error| panic!("stdout must parse as JSON: {error}\n{stdout}"));
    assert_eq!(json["mode"], "check");
    let results = json["results"].as_array().unwrap();
    let modified = results
        .iter()
        .find(|result| result["severity"] == "modified")
        .expect("dry-run JSON must include the would-be modification");
    assert_eq!(
        modified["proposed_content"],
        "Процедура Тест()\nКонецПроцедуры\n"
    );
    assert!(
        !stdout.contains("Предлагаемые"),
        "JSON output must not be followed by the text diff trailer:\n{stdout}"
    );
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

/// Warning diagnostics from checker scenarios are violations for `check`:
/// exit code must be 1, unlike the hook contract where warnings never block.
#[test]
fn check_counts_warning_diagnostics_as_violations() {
    let repo = TempGitRepo::new("check_warning_violation");
    let module = PathBuf::from("src/Плохой.bsl");
    write_file(
        repo.path().join(&module),
        "Процедура Тест()\n    Сообщить(\"плохоеСлово\");\nКонецПроцедуры\n",
    );
    write_file(repo.path().join("НецензурныеСлова.txt"), "плохоеСлово\n");
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
            "ПроверкаНецензурныхСлов",
            "--mode",
            "all",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("найдено слово из словаря 'плохоеСлово' в строке 2"),
        "check must report the profanity warning:\n{stdout}"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        "Процедура Тест()\n    Сообщить(\"плохоеСлово\");\nКонецПроцедуры\n",
        "check must not modify files for warning scenarios"
    );
}

/// Warning diagnostics from checker scenarios are violations for `fix` too.
#[test]
fn fix_counts_warning_diagnostics_as_violations() {
    let repo = TempGitRepo::new("fix_warning_violation");
    let module = PathBuf::from("src/Плохой.bsl");
    write_file(
        repo.path().join(&module),
        "Процедура Тест()\n    Сообщить(\"плохоеСлово\");\nКонецПроцедуры\n",
    );
    write_file(repo.path().join("НецензурныеСлова.txt"), "плохоеСлово\n");
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    let output = run_prec_bsl(
        repo.path(),
        [
            "fix",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            "ПроверкаНецензурныхСлов",
            "--mode",
            "all",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    // A pure checker scenario has nothing to apply: the file stays intact.
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        "Процедура Тест()\n    Сообщить(\"плохоеСлово\");\nКонецПроцедуры\n",
        "fix must not modify files for warning-only scenarios"
    );
}

/// The user-visible regression from the yaxunit repository: a `v8config.json`
/// without the `ГлобальныеСценарии` key used to resolve to an empty scenario
/// set, so `fix`/`check` silently processed files with zero rules and exited
/// `0`. The missing key must fall back to the built-in default scenario set,
/// so a file with trailing-whitespace violations gets fixed.
#[test]
fn fix_applies_default_scenarios_when_config_omits_global_scenarios_key() {
    let repo = TempGitRepo::new("fix_default_scenarios");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(
        repo.path().join(&module),
        "Процедура Тест()   \nКонецПроцедуры\n",
    );
    write_file(
        repo.path().join("v8config.json"),
        concat!(
            "{\n",
            "  \"GLOBAL\": {\"version\": \"2.1\"},\n",
            "  \"Precommt4onecСценарии\": {\n",
            "    \"ИспользоватьСценарииРепозитория\": false\n",
            "  }\n",
            "}\n",
        ),
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    let output = run_prec_bsl(
        repo.path(),
        [
            "fix",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--mode",
            "all",
        ],
    );

    // Before the fix this exited 0 with zero executed rules; now the default
    // scenario set runs, the fixer applies, and the contract exit code is 1.
    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        "Процедура Тест()\nКонецПроцедуры\n",
        "fix must apply the default trailing-whitespace fixer"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("УдалениеЛишнихКонцевыхПробелов"),
        "the default scenario set must include the trailing-whitespace rule:\n{stdout}"
    );
}

/// The same config shape must make `check` report violations instead of a
/// clean exit: diagnostics over zero rules hid problems before.
#[test]
fn check_reports_violations_with_default_scenarios_when_key_omitted() {
    let repo = TempGitRepo::new("check_default_scenarios");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(
        repo.path().join(&module),
        "Процедура Тест()   \nКонецПроцедуры\n",
    );
    write_file(
        repo.path().join("v8config.json"),
        concat!(
            "{\n",
            "  \"Precommt4onecСценарии\": {\n",
            "    \"ИспользоватьСценарииРепозитория\": false\n",
            "  }\n",
            "}\n",
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
            "--mode",
            "all",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        "Процедура Тест()   \nКонецПроцедуры\n",
        "check must not modify files"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("УдалениеЛишнихКонцевыхПробелов"),
        "check must run the default scenario set:\n{stdout}"
    );
}

/// The explicit empty array stays the deliberate "all scenarios off" switch:
/// no rules run, files are untouched, and the exit code is 0.
#[test]
fn check_respects_explicit_empty_global_scenarios_array() {
    let repo = TempGitRepo::new("empty_global_scenarios");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(
        repo.path().join(&module),
        "Процедура Тест()   \nКонецПроцедуры\n",
    );
    write_file(
        repo.path().join("v8config.json"),
        concat!(
            "{\n",
            "  \"Precommt4onecСценарии\": {\n",
            "    \"ГлобальныеСценарии\": []\n",
            "  }\n",
            "}\n",
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
            "--mode",
            "all",
        ],
    );

    assert_eq!(output.status.code(), Some(0), "{}", output_text(&output));
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        "Процедура Тест()   \nКонецПроцедуры\n",
        "the explicit empty array must disable every scenario"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("УдалениеЛишнихКонцевыхПробелов"),
        "no rule must run for the explicit empty array:\n{stdout}"
    );
}

/// `--exclude-rules` subtracts the listed rules from the resolved default
/// set: with no `v8config.json` every supported scenario would run, and the
/// fixture triggers violations from two of them (the trailing-whitespace
/// fixer `B` and the duplicate-methods checker `A`). Excluding `B` keeps
/// the checker violation of `A` and drops the fixer modification of `B`.
#[test]
fn check_exclude_rules_removes_rule_from_default_set() {
    let repo = TempGitRepo::new("exclude_defaults");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(repo.path().join(&module), DOUBLE_DIRTY_MODULE);
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    let output = run_prec_bsl(
        repo.path(),
        [
            "check",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--mode",
            "all",
            "--exclude-rules",
            "УдалениеЛишнихКонцевыхПробелов",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ПроверкаДублейПроцедурИФункций"),
        "the non-excluded checker must still report violations:\n{stdout}"
    );
    assert!(
        stdout.contains("duplicate procedure or function definition: Тест"),
        "the duplicate-methods violation must stay in the report:\n{stdout}"
    );
    assert!(
        !stdout.contains("УдалениеЛишнихКонцевыхПробелов"),
        "the excluded fixer must not appear in the report:\n{stdout}"
    );
    assert!(
        !stdout.contains("удалены лишние пробелы или табы в конце строк"),
        "the excluded fixer's modification must not be proposed:\n{stdout}"
    );
}

/// `--rules` and `--exclude-rules` combine: the explicit set `A,B` minus
/// `B` runs only `A`. This guards against the exclusion being wiped by the
/// disabled-scenarios cleanup that an explicit `--rules` override performs.
#[test]
fn check_exclude_rules_with_explicit_rules() {
    let repo = TempGitRepo::new("exclude_explicit_rules");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(repo.path().join(&module), DOUBLE_DIRTY_MODULE);
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    let output = run_prec_bsl(
        repo.path(),
        [
            "check",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--mode",
            "all",
            "--rules",
            "ПроверкаДублейПроцедурИФункций,УдалениеЛишнихКонцевыхПробелов",
            "--exclude-rules",
            "УдалениеЛишнихКонцевыхПробелов",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ПроверкаДублейПроцедурИФункций"),
        "the non-excluded rule from --rules must report violations:\n{stdout}"
    );
    assert!(
        !stdout.contains("УдалениеЛишнихКонцевыхПробелов"),
        "the excluded rule must not run even with an explicit --rules set:\n{stdout}"
    );
}

/// `--exclude-rules` unions with `ОтключенныеСценарии` from `v8config.json`:
/// the config disables one of the two rules triggered by the fixture, the
/// CLI excludes the other, so no violation survives and `check` exits 0.
#[test]
fn check_exclude_rules_union_with_disabled() {
    let repo = TempGitRepo::new("exclude_union_disabled");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(repo.path().join(&module), DOUBLE_DIRTY_MODULE);
    write_file(
        repo.path().join("v8config.json"),
        concat!(
            "{\n",
            "  \"Precommt4onecСценарии\": {\n",
            "    \"ОтключенныеСценарии\": [\"УдалениеЛишнихКонцевыхПробелов\"]\n",
            "  }\n",
            "}\n",
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
            "--mode",
            "all",
            "--exclude-rules",
            "ПроверкаДублейПроцедурИФункций",
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "config-disabled and CLI-excluded rules must leave no violations: {}",
        output_text(&output)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("УдалениеЛишнихКонцевыхПробелов"),
        "the config-disabled rule must not run:\n{stdout}"
    );
    assert!(
        !stdout.contains("ПроверкаДублейПроцедурИФункций"),
        "the CLI-excluded rule must not run:\n{stdout}"
    );
    assert!(
        !stdout.contains("duplicate procedure or function definition"),
        "no duplicate-methods violation may survive the union:\n{stdout}"
    );
    assert!(
        !stdout.contains("удалены лишние пробелы или табы в конце строк"),
        "no trailing-whitespace modification may survive the union:\n{stdout}"
    );
}

/// A typo in `--exclude-rules` is an execution error, not a silent no-op:
/// the unknown name would otherwise bring the rule back into the run.
#[test]
fn check_exclude_rules_unknown_name_fails() {
    let repo = TempGitRepo::new("exclude_unknown_name");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(repo.path().join(&module), CLEAN_MODULE);
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    let output = run_prec_bsl(
        repo.path(),
        [
            "check",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--exclude-rules",
            "НесуществующийСценарий",
        ],
    );

    assert_eq!(output.status.code(), Some(2), "{}", output_text(&output));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("НесуществующийСценарий"),
        "the error must name the unknown scenario: {stderr}"
    );
    assert!(
        stderr.contains("unsupported"),
        "the error must classify the name as unsupported: {stderr}"
    );
}

/// The `fix` wiring honors `--exclude-rules`: `fix --dry-run` with the fixer
/// rule excluded reports the would-be modification only from the remaining
/// rules, and the excluded fixer's edit is absent from the proposed diff.
#[test]
fn fix_exclude_rules_dry_run_removes_rule_from_default_set() {
    let repo = TempGitRepo::new("exclude_fix_dry_run");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(repo.path().join(&module), DOUBLE_DIRTY_MODULE);
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    let output = run_prec_bsl(
        repo.path(),
        [
            "fix",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--mode",
            "all",
            "--dry-run",
            "--exclude-rules",
            "ПроверкаДублейПроцедурИФункций",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        DOUBLE_DIRTY_MODULE,
        "fix --dry-run must not modify the file"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("УдалениеЛишнихКонцевыхПробелов"),
        "the non-excluded fixer must still propose its edit:\n{stdout}"
    );
    assert!(
        stdout.contains("Предлагаемые изменения (не записаны):"),
        "the dry-run diff must be shown:\n{stdout}"
    );
    assert!(
        stdout.contains("+Процедура Тест()"),
        "the proposed diff must include the trailing-whitespace fix:\n{stdout}"
    );
    assert!(
        !stdout.contains("ПроверкаДублейПроцедурИФункций"),
        "the excluded checker must not appear in the report:\n{stdout}"
    );
    assert!(
        !stdout.contains("duplicate procedure or function definition"),
        "the excluded checker must not report violations:\n{stdout}"
    );
}

const DIRTY_MODULE: &str = "Процедура Тест()   \nКонецПроцедуры\n";
const DIRTYER_MODULE: &str = "Процедура Тест()   \nКонецПроцедуры\t\n";
const DIRTYEST_MODULE: &str = "Процедура Тест()   \nКонецПроцедуры \t \n";
const CLEAN_MODULE: &str = "Процедура Тест()\nКонецПроцедуры\n";
/// One module triggering exactly two default-set violations: a trailing
/// whitespace line (fixed by `УдалениеЛишнихКонцевыхПробелов`) and two
/// procedure definitions with the same name (reported by
/// `ПроверкаДублейПроцедурИФункций`).
const DOUBLE_DIRTY_MODULE: &str = concat!(
    "Процедура Тест()   \n",
    "    Сообщить(\"а\");\n",
    "КонецПроцедуры\n",
    "\n",
    "Процедура Тест()\n",
    "    Сообщить(\"б\");\n",
    "КонецПроцедуры\n",
);

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
