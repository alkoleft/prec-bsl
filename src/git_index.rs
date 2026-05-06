use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedFile {
    pub status: StagedStatus,
    pub path: PathBuf,
    pub original_path: Option<PathBuf>,
}

impl StagedFile {
    fn new(status: StagedStatus, path: impl Into<PathBuf>) -> Self {
        Self {
            status,
            path: path.into(),
            original_path: None,
        }
    }

    fn with_original(
        status: StagedStatus,
        original_path: impl Into<PathBuf>,
        path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            status,
            path: path.into(),
            original_path: Some(original_path.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StagedStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Unknown(String),
}

#[derive(Debug)]
pub enum GitIndexError {
    Io {
        operation: &'static str,
        repo_root: PathBuf,
        source: std::io::Error,
    },
    CommandFailed {
        operation: &'static str,
        repo_root: PathBuf,
        status: Option<i32>,
        stderr: String,
    },
    InvalidOutput {
        operation: &'static str,
        repo_root: PathBuf,
        message: String,
    },
}

impl GitIndexError {
    pub fn blocking_diagnostic(&self) -> GitIndexDiagnostic {
        GitIndexDiagnostic {
            severity: GitIndexDiagnosticSeverity::Blocking,
            message: self.to_string(),
        }
    }
}

impl fmt::Display for GitIndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                repo_root,
                source,
            } => write!(
                formatter,
                "git {operation} failed for {}: {source}",
                repo_root.display()
            ),
            Self::CommandFailed {
                operation,
                repo_root,
                status,
                stderr,
            } => write!(
                formatter,
                "git {operation} failed for {} with status {}: {}",
                repo_root.display(),
                status
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "terminated by signal".to_owned()),
                stderr.trim()
            ),
            Self::InvalidOutput {
                operation,
                repo_root,
                message,
            } => write!(
                formatter,
                "git {operation} returned invalid output for {}: {message}",
                repo_root.display()
            ),
        }
    }
}

impl std::error::Error for GitIndexError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::CommandFailed { .. } | Self::InvalidOutput { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitIndexDiagnostic {
    pub severity: GitIndexDiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitIndexDiagnosticSeverity {
    Blocking,
}

pub fn collect_staged_files(repo_root: &Path) -> Result<Vec<StagedFile>, GitIndexError> {
    let operation = "diff --name-status --staged --no-renames";
    let output = git_command(repo_root)
        .args(["diff", "--name-status", "--staged", "--no-renames"])
        .output()
        .map_err(|source| GitIndexError::Io {
            operation,
            repo_root: repo_root.to_path_buf(),
            source,
        })?;

    if !output.status.success() {
        return Err(GitIndexError::CommandFailed {
            operation,
            repo_root: repo_root.to_path_buf(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let stdout =
        String::from_utf8(output.stdout).map_err(|error| GitIndexError::InvalidOutput {
            operation,
            repo_root: repo_root.to_path_buf(),
            message: format!("stdout is not valid UTF-8: {error}"),
        })?;

    parse_name_status_output(&stdout).map_err(|message| GitIndexError::InvalidOutput {
        operation,
        repo_root: repo_root.to_path_buf(),
        message,
    })
}

pub fn restage_paths(repo_root: &Path, paths: &[PathBuf]) -> Result<(), GitIndexError> {
    if paths.is_empty() {
        return Ok(());
    }

    let operation = "add --";
    let output = git_command(repo_root)
        .env("GIT_LITERAL_PATHSPECS", "1")
        .arg("add")
        .arg("--")
        .args(paths)
        .output()
        .map_err(|source| GitIndexError::Io {
            operation,
            repo_root: repo_root.to_path_buf(),
            source,
        })?;

    if output.status.success() {
        Ok(())
    } else {
        Err(GitIndexError::CommandFailed {
            operation,
            repo_root: repo_root.to_path_buf(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

fn git_command(repo_root: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo_root)
        .arg("-c")
        .arg("core.quotePath=false");
    command
}

fn parse_name_status_output(output: &str) -> Result<Vec<StagedFile>, String> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .map(parse_name_status_line)
        .collect()
}

fn parse_name_status_line(line: &str) -> Result<StagedFile, String> {
    let fields = line.split('\t').collect::<Vec<_>>();
    let Some(status_code) = fields.first().copied() else {
        return Err("empty name-status line".to_owned());
    };

    match status_code.as_bytes().first().copied() {
        Some(b'A') => parse_single_path_status(StagedStatus::Added, &fields, line),
        Some(b'M') => parse_single_path_status(StagedStatus::Modified, &fields, line),
        Some(b'D') => parse_single_path_status(StagedStatus::Deleted, &fields, line),
        Some(b'R') => parse_two_path_status(StagedStatus::Renamed, &fields, line),
        Some(b'C') => parse_two_path_status(StagedStatus::Copied, &fields, line),
        Some(_) => {
            let path = fields
                .last()
                .filter(|path| !path.is_empty())
                .ok_or_else(|| format!("missing path in name-status line: {line}"))?;
            Ok(StagedFile::new(
                StagedStatus::Unknown(status_code.to_owned()),
                decode_git_path(path)?,
            ))
        }
        None => Err(format!("missing status in name-status line: {line}")),
    }
}

fn parse_single_path_status(
    status: StagedStatus,
    fields: &[&str],
    line: &str,
) -> Result<StagedFile, String> {
    match fields {
        [_, path] if !path.is_empty() => Ok(StagedFile::new(status, decode_git_path(path)?)),
        _ => Err(format!("expected one path in name-status line: {line}")),
    }
}

fn parse_two_path_status(
    status: StagedStatus,
    fields: &[&str],
    line: &str,
) -> Result<StagedFile, String> {
    match fields {
        [_, original_path, path] if !original_path.is_empty() && !path.is_empty() => {
            Ok(StagedFile::with_original(
                status,
                decode_git_path(original_path)?,
                decode_git_path(path)?,
            ))
        }
        _ => Err(format!(
            "expected source and target paths in name-status line: {line}"
        )),
    }
}

fn decode_git_path(path: &str) -> Result<PathBuf, String> {
    if !path.starts_with('"') {
        return Ok(PathBuf::from(path));
    }

    if !path.ends_with('"') || path.len() < 2 {
        return Err(format!("unterminated quoted Git path: {path}"));
    }

    let mut decoded = String::new();
    let mut chars = path[1..path.len() - 1].chars().peekable();
    while let Some(character) = chars.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }

        let Some(escaped) = chars.next() else {
            return Err(format!("unterminated escape in quoted Git path: {path}"));
        };

        match escaped {
            '\\' => decoded.push('\\'),
            '"' => decoded.push('"'),
            'n' => decoded.push('\n'),
            'r' => decoded.push('\r'),
            't' => decoded.push('\t'),
            '0'..='7' => {
                let mut octal = String::from(escaped);
                for _ in 0..2 {
                    if chars.peek().is_some_and(|next| matches!(next, '0'..='7')) {
                        octal.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                let byte = u8::from_str_radix(&octal, 8)
                    .map_err(|error| format!("invalid octal escape in Git path {path}: {error}"))?;
                decoded.push(char::from(byte));
            }
            other => decoded.push(other),
        }
    }

    Ok(PathBuf::from(decoded))
}

#[cfg(test)]
mod tests {
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
                StagedFile::with_original(
                    StagedStatus::Renamed,
                    "old/Модуль.bsl",
                    "new/Модуль.bsl"
                ),
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

    fn assert_staged<'a>(
        files: &'a [StagedFile],
        status: StagedStatus,
        path: &str,
    ) -> &'a StagedFile {
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
            let path = std::env::current_dir()
                .unwrap()
                .join("target")
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
}
