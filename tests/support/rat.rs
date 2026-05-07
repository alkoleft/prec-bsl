use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub const RAT_REPO: &str = "/home/alko/develop/open-source/rat";
pub const RAT_SOURCE_ROOTS: &[&str] = &["fixtures/configuration", "exts/rat", "tests"];
pub const RAT_PARSER_ROOTS: &[&str] = &["fixtures/configuration/src", "exts/rat/src", "tests/src"];

pub fn rat_repo() -> Option<&'static Path> {
    let path = Path::new(RAT_REPO);
    path.join("v8config.json").is_file().then_some(path)
}

pub fn git_status_short(repo: &Path) -> io::Result<String> {
    let output = Command::new("git")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .arg("-C")
        .arg(repo)
        .arg("status")
        .arg("--short")
        .output()?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git status failed for {}: {}",
            repo.display(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn copy_required_source_roots(repo: &Path, destination: &Path) -> io::Result<Vec<PathBuf>> {
    RAT_SOURCE_ROOTS
        .iter()
        .map(|root| {
            let source = repo.join(root);
            let target = destination.join(root);
            copy_dir_all(&source, &target)?;
            Ok(target)
        })
        .collect()
}

#[derive(Debug)]
pub struct TempRatCopy {
    path: PathBuf,
}

impl TempRatCopy {
    pub fn new() -> io::Result<Self> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| io::Error::other(format!("system clock before unix epoch: {error}")))?
            .as_nanos();
        let path = std::env::current_dir()?
            .join("target")
            .join("rat-acceptance")
            .join(format!("{}-{timestamp}", std::process::id()));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRatCopy {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn collect_source_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_source_files_into(root, &mut files)?;
    files.sort();
    Ok(files)
}

pub fn collect_bsl_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_bsl_files_into(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn copy_dir_all(source: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let child_target = target.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &child_target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), child_target)?;
        }
    }
    Ok(())
}

fn collect_source_files_into(root: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_source_files_into(&path, files)?;
        } else if file_type.is_file() && is_source_file(path.as_path()) {
            files.push(path);
        }
    }
    Ok(())
}

fn collect_bsl_files_into(root: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_bsl_files_into(&path, files)?;
        } else if file_type.is_file() && has_extension(path.as_path(), "bsl") {
            files.push(path);
        }
    }
    Ok(())
}

fn is_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some("bsl" | "mdo" | "form")
    ) || path.file_name() == Some(OsStr::new("Configuration.mdo"))
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|value| value == extension)
}
