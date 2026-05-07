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
mod tests;
