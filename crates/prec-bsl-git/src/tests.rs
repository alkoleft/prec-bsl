use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

#[test]
fn git_index_collects_staged_added_modified_deleted_and_cyrillic_paths() {
    let repo = TempGitRepo::new("collects_staged_statuses");
    write_file(
        repo.path().join("src/Модуль.bsl"),
        "Процедура Тест()\nКонецПроцедуры\n",
    );
    write_file(repo.path().join("src/Удаляемый.bsl"), "Удалить\n");
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    write_file(
        repo.path().join("src/Модуль.bsl"),
        "Процедура Тест()\n    Сообщить(\"изменено\");\nКонецПроцедуры\n",
    );
    write_file(repo.path().join("src/Новый Модуль \"Тест\".bsl"), "Новый\n");
    fs::remove_file(repo.path().join("src/Удаляемый.bsl")).unwrap();
    run_git(repo.path(), ["add", "."]);

    let files = collect_staged_files(repo.path()).unwrap();

    assert_staged(&files, StagedStatus::Modified, "src/Модуль.bsl");
    assert_staged(&files, StagedStatus::Added, "src/Новый Модуль \"Тест\".bsl");
    let deleted = assert_staged(&files, StagedStatus::Deleted, "src/Удаляемый.bsl");
    assert!(
        !repo.path().join(&deleted.path).exists(),
        "deleted staged files must not require working-tree contents"
    );
}

#[test]
fn git_index_collection_uses_no_renames_as_commit_mode_contract() {
    let repo = TempGitRepo::new("uses_no_renames");
    write_file(repo.path().join("src/Старый.bsl"), "Текст\n");
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    fs::rename(
        repo.path().join("src/Старый.bsl"),
        repo.path().join("src/Новый.bsl"),
    )
    .unwrap();
    run_git(repo.path(), ["add", "."]);

    let files = collect_staged_files(repo.path()).unwrap();

    assert_staged(&files, StagedStatus::Deleted, "src/Старый.bsl");
    assert_staged(&files, StagedStatus::Added, "src/Новый.bsl");
    assert!(
        files
            .iter()
            .all(|file| file.status != StagedStatus::Renamed),
        "--no-renames must keep rename processing as delete plus add"
    );
}

#[test]
fn git_index_parser_maps_renamed_copied_and_unknown_statuses() {
    let files = parse_name_status_output(
        "R100\told/Модуль.bsl\tnew/Модуль.bsl\nC075\tsrc/Base.bsl\tsrc/Copy.bsl\nX\tstrange.file\n",
    )
    .unwrap();

    assert_eq!(
        files,
        vec![
            StagedFile::with_original(StagedStatus::Renamed, "old/Модуль.bsl", "new/Модуль.bsl"),
            StagedFile::with_original(StagedStatus::Copied, "src/Base.bsl", "src/Copy.bsl"),
            StagedFile::new(StagedStatus::Unknown("X".to_owned()), "strange.file"),
        ]
    );
}

#[test]
fn git_index_restages_modified_and_generated_paths() {
    let repo = TempGitRepo::new("restages_paths");
    write_file(repo.path().join("src/Модуль.bsl"), "Исходный\n");
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    write_file(repo.path().join("src/Модуль.bsl"), "Измененный\n");
    write_file(repo.path().join("src/Сгенерированный.bsl"), "Новый\n");

    restage_paths(
        repo.path(),
        &[
            PathBuf::from("src/Модуль.bsl"),
            PathBuf::from("src/Сгенерированный.bsl"),
        ],
    )
    .unwrap();

    let files = collect_staged_files(repo.path()).unwrap();

    assert_staged(&files, StagedStatus::Modified, "src/Модуль.bsl");
    assert_staged(&files, StagedStatus::Added, "src/Сгенерированный.bsl");
}

#[test]
fn git_index_restaging_treats_paths_as_literals_not_pathspec_magic() {
    let repo = TempGitRepo::new("literal_pathspecs");
    write_file(repo.path().join("src/Normal.bsl"), "baseline\n");
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    write_file(
        repo.path().join("src/Normal.bsl"),
        "modified but not restaged\n",
    );
    write_file(repo.path().join(":(top)*"), "literal magic-looking path\n");

    restage_paths(repo.path(), &[PathBuf::from(":(top)*")]).unwrap();

    let files = collect_staged_files(repo.path()).unwrap();

    assert_staged(&files, StagedStatus::Added, ":(top)*");
    assert!(
        files
            .iter()
            .all(|file| file.path != PathBuf::from("src/Normal.bsl")),
        "literal restaging must not expand pathspec magic and stage unrelated paths: {files:#?}"
    );
}

#[test]
fn git_index_failures_are_available_as_blocking_diagnostics() {
    let repo = TempGitRepo::new("blocking_diagnostics_without_git_repo");
    let missing_repo = repo.path().join("missing-repo");

    let error = collect_staged_files(&missing_repo).unwrap_err();
    let diagnostic = error.blocking_diagnostic();

    assert_eq!(diagnostic.severity, GitIndexDiagnosticSeverity::Blocking);
    assert!(
        diagnostic.message.contains("git diff --name-status"),
        "diagnostic must name the failed Git boundary, got: {}",
        diagnostic.message
    );
}

fn assert_staged<'a>(files: &'a [StagedFile], status: StagedStatus, path: &str) -> &'a StagedFile {
    files
        .iter()
        .find(|file| file.status == status && file.path == PathBuf::from(path))
        .unwrap_or_else(|| panic!("missing staged entry {status:?} {path}; got: {files:#?}"))
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

#[derive(Debug)]
struct TempGitRepo {
    path: PathBuf,
}

impl TempGitRepo {
    fn new(name: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join("git-index-tests")
            .join(format!("{}-{name}-{timestamp}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        run_raw_git(&path, ["init"]);
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

fn run_raw_git<const N: usize>(cwd: &Path, args: [&str; N]) {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git command failed in {}: {}\nstdout: {}\nstderr: {}",
        cwd.display(),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
