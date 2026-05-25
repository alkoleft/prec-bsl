use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use prec_bsl_git::{StagedFile, StagedStatus};
use serde::Deserialize;

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

pub fn discover_source_roots_from_staged_files(
    repo_root: &Path,
    staged_files: &[StagedFile],
) -> SourceRootResolution {
    if let Some(resolution) = discover_source_roots_from_v8project(repo_root, staged_files) {
        return resolution;
    }

    discover_source_roots_from_staged_markers(repo_root, staged_files)
}

fn discover_source_roots_from_v8project(
    repo_root: &Path,
    staged_files: &[StagedFile],
) -> Option<SourceRootResolution> {
    let manifest = match read_v8project_manifest(repo_root)? {
        Ok(manifest) => manifest,
        Err(message) => {
            return Some(SourceRootResolution {
                roots: Vec::new(),
                diagnostics: vec![SourceRootDiagnostic {
                    severity: SourceRootDiagnosticSeverity::Blocking,
                    message,
                }],
            });
        }
    };
    let source_dirs = manifest_source_dirs(&manifest);
    if source_dirs.is_empty() {
        return Some(SourceRootResolution {
            roots: Vec::new(),
            diagnostics: vec![SourceRootDiagnostic {
                severity: SourceRootDiagnosticSeverity::Blocking,
                message: "v8project.yaml does not define supported source-set entries".to_owned(),
            }],
        });
    }

    let staged_source_dirs = source_dirs_for_staged_paths(&source_dirs, staged_files);
    let unmatched_diagnostics = unmatched_manifest_source_diagnostics(&source_dirs, staged_files);
    if !unmatched_diagnostics.is_empty() {
        return Some(SourceRootResolution {
            roots: Vec::new(),
            diagnostics: unmatched_diagnostics,
        });
    }

    if staged_source_dirs.is_empty() {
        return Some(SourceRootResolution {
            roots: Vec::new(),
            diagnostics: Vec::new(),
        });
    }

    Some(resolve_source_roots(repo_root, &staged_source_dirs))
}

fn discover_source_roots_from_staged_markers(
    repo_root: &Path,
    staged_files: &[StagedFile],
) -> SourceRootResolution {
    let mut discovered_roots = Vec::<PathBuf>::new();
    let mut diagnostics = Vec::new();

    for staged_file in staged_files {
        let kind = classify_path(&staged_file.path);
        if kind == SourceFileKind::Unsupported {
            continue;
        }

        if let Some(source_root) = discover_source_root_for_path(repo_root, &staged_file.path) {
            if !discovered_roots.contains(&source_root) {
                discovered_roots.push(source_root);
            }
            continue;
        }

        if requires_metadata_source_root(&staged_file.path, kind) {
            diagnostics.push(SourceRootDiagnostic {
                severity: SourceRootDiagnosticSeverity::Blocking,
                message: format!(
                    "unable to discover source root for {}; pass --source-dir explicitly",
                    staged_file.path.display()
                ),
            });
            continue;
        }

        if !discovered_roots.contains(&PathBuf::from(".")) {
            discovered_roots.push(PathBuf::from("."));
        }
    }

    if discovered_roots.is_empty() {
        return SourceRootResolution {
            roots: Vec::new(),
            diagnostics,
        };
    }

    let mut resolution = resolve_source_roots(repo_root, &discovered_roots);
    diagnostics.append(&mut resolution.diagnostics);
    resolution.diagnostics = diagnostics;
    resolution
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct V8ProjectManifest {
    #[serde(default, rename = "basePath")]
    base_path: Option<PathBuf>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default, rename = "source-set")]
    source_sets: Vec<V8ProjectSourceSet>,
}

#[derive(Debug, Deserialize)]
struct V8ProjectSourceSet {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    path: PathBuf,
}

fn read_v8project_manifest(repo_root: &Path) -> Option<Result<V8ProjectManifest, String>> {
    let path = repo_root.join("v8project.yaml");
    let input = match fs::read_to_string(&path) {
        Ok(input) => input,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            return Some(Err(format!(
                "failed to read v8project.yaml source-set manifest: {error}"
            )));
        }
    };
    Some(
        serde_yml::from_str(&input).map_err(|error| {
            format!("failed to parse v8project.yaml source-set manifest: {error}")
        }),
    )
}

fn manifest_source_dirs(manifest: &V8ProjectManifest) -> Vec<PathBuf> {
    let base_path = manifest
        .base_path
        .as_deref()
        .unwrap_or_else(|| Path::new(""));
    let mut source_dirs = Vec::new();
    for source_set in &manifest.source_sets {
        if !is_supported_source_set_kind(source_set.kind.as_deref()) {
            continue;
        }

        let mut root = base_path.join(&source_set.path);
        if is_edt_manifest(manifest) {
            root.push("src");
        }
        if !source_dirs.contains(&root) {
            source_dirs.push(root);
        }
    }
    source_dirs
}

fn is_supported_source_set_kind(kind: Option<&str>) -> bool {
    match kind {
        None => true,
        Some(kind) => matches!(
            kind.to_ascii_uppercase().as_str(),
            "CONFIGURATION" | "EXTENSION" | "DESIGNER"
        ),
    }
}

fn is_edt_manifest(manifest: &V8ProjectManifest) -> bool {
    manifest
        .format
        .as_deref()
        .is_some_and(|format| format.eq_ignore_ascii_case("EDT"))
}

fn source_dirs_for_staged_paths(
    source_dirs: &[PathBuf],
    staged_files: &[StagedFile],
) -> Vec<PathBuf> {
    let mut staged_source_dirs = Vec::new();
    for staged_file in staged_files {
        if classify_path(&staged_file.path) == SourceFileKind::Unsupported {
            continue;
        }
        let Some(source_dir) = matching_source_dir(source_dirs, &staged_file.path) else {
            continue;
        };
        let source_dir = source_dir.clone();
        if !staged_source_dirs.contains(&source_dir) {
            staged_source_dirs.push(source_dir);
        }
    }
    staged_source_dirs
}

fn matching_source_dir<'a>(source_dirs: &'a [PathBuf], repo_path: &Path) -> Option<&'a PathBuf> {
    source_dirs
        .iter()
        .filter(|source_dir| {
            source_dir.as_os_str().is_empty()
                || source_dir.as_path() == Path::new(".")
                || repo_path.starts_with(source_dir)
        })
        .max_by_key(|source_dir| source_dir.components().count())
}

fn unmatched_manifest_source_diagnostics(
    source_dirs: &[PathBuf],
    staged_files: &[StagedFile],
) -> Vec<SourceRootDiagnostic> {
    staged_files
        .iter()
        .filter_map(|staged_file| {
            let kind = classify_path(&staged_file.path);
            if kind == SourceFileKind::Unsupported
                || matching_source_dir(source_dirs, &staged_file.path).is_some()
            {
                return None;
            }
            if !requires_metadata_source_root(&staged_file.path, kind) {
                return None;
            }
            Some(SourceRootDiagnostic {
                severity: SourceRootDiagnosticSeverity::Blocking,
                message: format!(
                    "staged source file is outside v8project.yaml source roots: {}",
                    staged_file.path.display()
                ),
            })
        })
        .collect()
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

fn discover_source_root_for_path(repo_root: &Path, repo_path: &Path) -> Option<PathBuf> {
    if let Some(source_root) = direct_configuration_source_root(repo_path) {
        return Some(source_root);
    }

    for candidate in ancestor_candidates(repo_path) {
        if repo_root
            .join(&candidate)
            .join("Configuration")
            .join("Configuration.mdo")
            .is_file()
            || repo_root
                .join(&candidate)
                .join("Configuration.xml")
                .is_file()
        {
            return Some(candidate);
        }
    }

    None
}

fn direct_configuration_source_root(repo_path: &Path) -> Option<PathBuf> {
    if repo_path
        .file_name()
        .is_some_and(|name| name == "Configuration.xml")
    {
        return Some(
            repo_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(".")),
        );
    }

    if !repo_path
        .file_name()
        .is_some_and(|name| name == "Configuration.mdo")
    {
        return None;
    }

    let configuration_dir = repo_path.parent()?;
    if configuration_dir
        .file_name()
        .is_some_and(|name| name == "Configuration")
    {
        return Some(
            configuration_dir
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(".")),
        );
    }

    None
}

fn ancestor_candidates(repo_path: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut current = repo_path.parent();
    while let Some(candidate) = current {
        candidates.push(if candidate.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            candidate.to_path_buf()
        });
        current = candidate.parent();
    }
    candidates
}

fn requires_metadata_source_root(repo_path: &Path, kind: SourceFileKind) -> bool {
    matches!(
        kind,
        SourceFileKind::ConfigurationMetadata
            | SourceFileKind::EdtMetadata
            | SourceFileKind::EdtForm
            | SourceFileKind::XmlMetadata
    ) || path_contains_metadata_directory(repo_path)
}

fn path_contains_metadata_directory(path: &Path) -> bool {
    path.components().any(|component| {
        let value = component.as_os_str();
        [
            "AccumulationRegisters",
            "AccountingRegisters",
            "BusinessProcesses",
            "CalculationRegisters",
            "Catalogs",
            "ChartsOfAccounts",
            "ChartsOfCalculationTypes",
            "ChartsOfCharacteristicTypes",
            "CommandGroups",
            "CommonCommands",
            "CommonForms",
            "CommonModules",
            "CommonPictures",
            "CommonTemplates",
            "Constants",
            "DataProcessors",
            "DefinedTypes",
            "DocumentJournals",
            "DocumentNumerators",
            "Documents",
            "Enums",
            "EventSubscriptions",
            "ExchangePlans",
            "ExternalDataSources",
            "FunctionalOptions",
            "FunctionalOptionsParameters",
            "HTTPServices",
            "InformationRegisters",
            "IntegrationServices",
            "Languages",
            "Reports",
            "Roles",
            "ScheduledJobs",
            "Sequences",
            "SessionParameters",
            "SettingsStorages",
            "Subsystems",
            "Tasks",
            "WebServices",
            "XDTOPackages",
        ]
        .iter()
        .any(|directory| value == *directory)
    })
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
