use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn prek_hook_uses_git_index_ignores_passed_filenames_and_restages_modifications() {
    let repo = TempGitRepo::new("uses_git_index");
    let staged_path = PathBuf::from("src/Стадированный.bsl");
    let passed_path = PathBuf::from("src/Переданный.bsl");

    write_file(
        repo.path().join(&staged_path),
        "Процедура Тест()\nКонецПроцедуры\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    write_file(
        repo.path().join(&staged_path),
        "Процедура Тест()\n    Сообщить(\"изменено\");   \nКонецПроцедуры\t\n",
    );
    write_file(
        repo.path().join(&passed_path),
        "Процедура НеСтадированный()   \nКонецПроцедуры\n",
    );
    run_git(repo.path(), ["add", "src/Стадированный.bsl"]);

    let output = run_prec_bsl(
        repo.path().join("src"),
        [
            "prek-hook",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
            "src/Переданный.bsl",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("prec-bsl hook: processed 1 file(s)"));
    assert!(stdout.contains("Modified files:"));
    assert!(stdout.contains("src/Стадированный.bsl"));
    assert!(!stdout.contains("src/Переданный.bsl"));

    assert_eq!(
        fs::read_to_string(repo.path().join(&staged_path)).unwrap(),
        "Процедура Тест()\n    Сообщить(\"изменено\");\nКонецПроцедуры\n"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join(&passed_path)).unwrap(),
        "Процедура НеСтадированный()   \nКонецПроцедуры\n"
    );
    assert_eq!(run_git_stdout(repo.path(), ["diff", "--name-only"]), "");
    assert_eq!(
        run_git_stdout(repo.path(), ["diff", "--cached", "--name-only"]),
        "src/Стадированный.bsl\n"
    );
    assert_eq!(
        run_git_stdout(repo.path(), ["show", ":src/Стадированный.bsl"]),
        "Процедура Тест()\n    Сообщить(\"изменено\");\nКонецПроцедуры\n"
    );
}

#[test]
fn prek_hook_passes_deleted_files_only_to_deleted_file_capable_scenarios() {
    let repo = TempGitRepo::new("deleted_files");
    let deleted_path = PathBuf::from("src/Удаленный.bsl");
    write_file(
        repo.path().join(&deleted_path),
        "Процедура Тест()\nКонецПроцедуры\n",
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    fs::remove_file(repo.path().join(&deleted_path)).unwrap();
    run_git(repo.path(), ["add", "-A"]);

    let output = run_prec_bsl(
        repo.path(),
        ["prek-hook", "--rules", "УдалениеЛишнихКонцевыхПробелов"],
    );

    assert!(output.status.success(), "{}", output_text(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("prec-bsl hook: processed 1 file(s)"));
    assert!(stdout.contains("scenario does not handle deleted files"));
    assert_eq!(
        run_git_stdout(repo.path(), ["diff", "--cached", "--name-status"]),
        "D\tsrc/Удаленный.bsl\n"
    );
}

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
            .join("prek-hook-tests")
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
