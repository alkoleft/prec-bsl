use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use prec_bsl_git::{StagedFile, StagedStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRoot {
    pub configured_path: PathBuf,
    pub absolute_path: PathBuf,
    pub repo_relative_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub source_root: SourceRoot,
    pub repo_path: PathBuf,
    pub source_relative_path: PathBuf,
    pub kind: SourceFileKind,
    pub staged_status: Option<StagedStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFileKind {
    BslModule,
    ConfigurationMetadata,
    EdtMetadata,
    EdtForm,
    XmlMetadata,
    ExternalArtifact,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRootResolution {
    pub roots: Vec<SourceRoot>,
    pub diagnostics: Vec<SourceRootDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRootDiagnostic {
    pub severity: SourceRootDiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRootDiagnosticSeverity {
    Blocking,
}

#[derive(Debug)]
pub enum SourceFileError {
    ReadDir {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for SourceFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadDir { path, source } => {
                write!(
                    formatter,
                    "failed to read source directory {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for SourceFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadDir { source, .. } => Some(source),
        }
    }
}

pub fn parse_source_dir_list(value: &str) -> Vec<PathBuf> {
    value
        .split(',')
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect()
}

pub fn resolve_source_roots(repo_root: &Path, source_dirs: &[PathBuf]) -> SourceRootResolution {
    let canonical_repo_root =
        fs::canonicalize(repo_root).unwrap_or_else(|_| repo_root.to_path_buf());
    let configured_roots = if source_dirs.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        source_dirs.to_vec()
    };

    let mut roots = Vec::new();
    let mut diagnostics = Vec::new();
    for configured_path in configured_roots {
        let absolute_path = resolve_repo_path(repo_root, &configured_path);
        if !absolute_path.is_dir() {
            diagnostics.push(SourceRootDiagnostic {
                severity: SourceRootDiagnosticSeverity::Blocking,
                message: format!("missing source root: {}", absolute_path.display()),
            });
            continue;
        }
        let canonical_root =
            fs::canonicalize(&absolute_path).unwrap_or_else(|_| absolute_path.clone());
        if !canonical_root.starts_with(&canonical_repo_root) {
            diagnostics.push(SourceRootDiagnostic {
                severity: SourceRootDiagnosticSeverity::Blocking,
                message: format!(
                    "source root is outside repository: {}",
                    canonical_root.display()
                ),
            });
            continue;
        }

        let repo_relative_path = repo_relative_path(&canonical_repo_root, &canonical_root);
        roots.push(SourceRoot {
            configured_path,
            absolute_path: canonical_root,
            repo_relative_path,
        });
    }

    SourceRootResolution { roots, diagnostics }
}

pub fn classify_path(path: &Path) -> SourceFileKind {
    if path
        .file_name()
        .is_some_and(|name| name == "Configuration.mdo")
    {
        return SourceFileKind::ConfigurationMetadata;
    }

    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("bsl") => SourceFileKind::BslModule,
        Some(extension) if extension.eq_ignore_ascii_case("mdo") => SourceFileKind::EdtMetadata,
        Some(extension) if extension.eq_ignore_ascii_case("form") => SourceFileKind::EdtForm,
        Some(extension) if extension.eq_ignore_ascii_case("xml") => SourceFileKind::XmlMetadata,
        Some(extension)
            if extension.eq_ignore_ascii_case("epf")
                || extension.eq_ignore_ascii_case("erf")
                || extension.eq_ignore_ascii_case("cfe") =>
        {
            SourceFileKind::ExternalArtifact
        }
        _ => SourceFileKind::Unsupported,
    }
}

pub fn classify_staged_files(roots: &[SourceRoot], staged_files: &[StagedFile]) -> Vec<SourceFile> {
    staged_files
        .iter()
        .filter_map(|staged_file| {
            let source_root = matching_source_root(roots, &staged_file.path)?;
            Some(classify_repo_path_with_root(
                source_root,
                staged_file.path.clone(),
                Some(staged_file.status.clone()),
            ))
        })
        .collect()
}

pub fn classify_repo_path(
    roots: &[SourceRoot],
    repo_path: impl Into<PathBuf>,
    staged_status: Option<StagedStatus>,
) -> Option<SourceFile> {
    let repo_path = repo_path.into();
    let source_root = matching_source_root(roots, &repo_path)?;
    Some(classify_repo_path_with_root(
        source_root,
        repo_path,
        staged_status,
    ))
}

pub fn collect_source_files(roots: &[SourceRoot]) -> Result<Vec<SourceFile>, SourceFileError> {
    let mut files = Vec::new();
    for source_root in roots {
        collect_source_files_into(source_root, &source_root.absolute_path, &mut files)?;
    }
    files.sort_by(|left, right| left.repo_path.cmp(&right.repo_path));
    Ok(files)
}

fn collect_source_files_into(
    source_root: &SourceRoot,
    directory: &Path,
    files: &mut Vec<SourceFile>,
) -> Result<(), SourceFileError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|source| SourceFileError::ReadDir {
            path: directory.to_path_buf(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| SourceFileError::ReadDir {
            path: directory.to_path_buf(),
            source,
        })?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|source| SourceFileError::ReadDir {
                path: entry.path(),
                source,
            })?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_source_files_into(source_root, &path, files)?;
        } else if file_type.is_file() {
            let repo_path = source_root.repo_relative_path.join(
                path.strip_prefix(&source_root.absolute_path)
                    .unwrap_or(&path),
            );
            files.push(classify_repo_path_with_root(
                source_root,
                repo_path.clone(),
                None,
            ));
        }
    }

    Ok(())
}

fn classify_repo_path_with_root(
    source_root: &SourceRoot,
    repo_path: PathBuf,
    staged_status: Option<StagedStatus>,
) -> SourceFile {
    let kind = classify_path(&repo_path);
    let source_relative_path = if source_root.repo_relative_path.as_os_str().is_empty() {
        repo_path.clone()
    } else {
        repo_path
            .strip_prefix(&source_root.repo_relative_path)
            .unwrap_or(&repo_path)
            .to_path_buf()
    };

    SourceFile {
        source_root: source_root.clone(),
        repo_path,
        source_relative_path,
        kind,
        staged_status,
    }
}

fn matching_source_root<'a>(roots: &'a [SourceRoot], repo_path: &Path) -> Option<&'a SourceRoot> {
    roots
        .iter()
        .filter(|root| {
            root.repo_relative_path.as_os_str().is_empty()
                || repo_path.starts_with(&root.repo_relative_path)
        })
        .max_by_key(|root| root.repo_relative_path.components().count())
}

fn resolve_repo_path(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn repo_relative_path(repo_root: &Path, absolute_path: &Path) -> PathBuf {
    absolute_path
        .strip_prefix(repo_root)
        .map(normalize_repo_relative_path)
        .unwrap_or_else(|_| absolute_path.to_path_buf())
}

fn normalize_repo_relative_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => normalized.push(value),
            Component::ParentDir => normalized.push(".."),
            Component::Prefix(_) | Component::RootDir => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests;
