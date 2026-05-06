use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::git_index::{StagedFile, StagedStatus};

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
        _ => SourceFileKind::Unsupported,
    }
}

pub fn classify_staged_files(roots: &[SourceRoot], staged_files: &[StagedFile]) -> Vec<SourceFile> {
    staged_files
        .iter()
        .filter_map(|staged_file| {
            let source_root = matching_source_root(roots, &staged_file.path)?;
            Some(source_file(
                source_root,
                staged_file.path.clone(),
                classify_path(&staged_file.path),
                Some(staged_file.status.clone()),
            ))
        })
        .collect()
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
            files.push(source_file(
                source_root,
                repo_path.clone(),
                classify_path(&repo_path),
                None,
            ));
        }
    }

    Ok(())
}

fn source_file(
    source_root: &SourceRoot,
    repo_path: PathBuf,
    kind: SourceFileKind,
    staged_status: Option<StagedStatus>,
) -> SourceFile {
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
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn source_root_resolves_explicit_single_root_and_reports_missing_roots() {
        let repo = temp_repo("explicit_single_root");
        fs::create_dir_all(repo.join("configuration")).unwrap();

        let resolution = resolve_source_roots(
            &repo,
            &[
                PathBuf::from("configuration"),
                PathBuf::from("missing/Источник"),
            ],
        );

        assert_eq!(resolution.roots.len(), 1);
        assert_eq!(
            resolution.roots[0].repo_relative_path,
            PathBuf::from("configuration")
        );
        assert_eq!(
            resolution.diagnostics,
            vec![SourceRootDiagnostic {
                severity: SourceRootDiagnosticSeverity::Blocking,
                message: format!(
                    "missing source root: {}",
                    repo.join("missing/Источник").display()
                ),
            }]
        );
    }

    #[test]
    fn source_root_defaults_to_repository_root_for_staged_files() {
        let repo = temp_repo("default_repository_root");
        fs::create_dir_all(repo.join("src")).unwrap();
        let resolution = resolve_source_roots(&repo, &[]);
        let staged_files = vec![StagedFile {
            status: StagedStatus::Added,
            path: PathBuf::from("src/Модуль.bsl"),
            original_path: None,
        }];

        let files = classify_staged_files(&resolution.roots, &staged_files);

        assert_eq!(resolution.roots[0].repo_relative_path, PathBuf::new());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].source_root.repo_relative_path, PathBuf::new());
        assert_eq!(
            files[0].source_relative_path,
            PathBuf::from("src/Модуль.bsl")
        );
    }

    #[test]
    fn source_root_normalizes_existing_roots_before_preserving_context() {
        let repo = temp_repo("normalizes_existing_roots");
        fs::create_dir_all(repo.join("configuration")).unwrap();
        fs::create_dir_all(repo.join("src")).unwrap();
        let resolution = resolve_source_roots(&repo, &[PathBuf::from("configuration/..")]);
        let staged_files = vec![StagedFile {
            status: StagedStatus::Added,
            path: PathBuf::from("src/Модуль.bsl"),
            original_path: None,
        }];

        let files = classify_staged_files(&resolution.roots, &staged_files);

        assert!(resolution.diagnostics.is_empty());
        assert_eq!(resolution.roots[0].repo_relative_path, PathBuf::new());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].repo_path, PathBuf::from("src/Модуль.bsl"));
    }

    #[test]
    fn source_root_reports_existing_roots_outside_repository() {
        let repo = temp_repo("reports_outside_roots_repo");
        let outside = temp_repo("reports_outside_roots_external");

        let resolution = resolve_source_roots(&repo, &[outside.clone()]);

        assert!(resolution.roots.is_empty());
        assert_eq!(
            resolution.diagnostics,
            vec![SourceRootDiagnostic {
                severity: SourceRootDiagnosticSeverity::Blocking,
                message: format!(
                    "source root is outside repository: {}",
                    fs::canonicalize(outside).unwrap().display()
                ),
            }]
        );
    }

    #[test]
    fn source_root_preserves_multiple_exec_rules_roots_per_file() {
        let repo = temp_repo("multiple_exec_rules_roots");
        write_file(repo.join("configuration/src/ОбщийМодуль.bsl"), "");
        write_file(repo.join("extensions/Расширение/src/Object.mdo"), "");

        let resolution = resolve_source_roots(
            &repo,
            &[
                PathBuf::from("configuration"),
                PathBuf::from("extensions/Расширение"),
            ],
        );
        let files = collect_source_files(&resolution.roots).unwrap();

        let configuration_file = files
            .iter()
            .find(|file| file.repo_path == PathBuf::from("configuration/src/ОбщийМодуль.bsl"))
            .unwrap();
        assert_eq!(
            configuration_file.source_root.repo_relative_path,
            PathBuf::from("configuration")
        );
        assert_eq!(
            configuration_file.source_relative_path,
            PathBuf::from("src/ОбщийМодуль.bsl")
        );

        let extension_file = files
            .iter()
            .find(|file| file.repo_path == PathBuf::from("extensions/Расширение/src/Object.mdo"))
            .unwrap();
        assert_eq!(
            extension_file.source_root.repo_relative_path,
            PathBuf::from("extensions/Расширение")
        );
        assert_eq!(extension_file.kind, SourceFileKind::EdtMetadata);
    }

    #[test]
    fn file_classification_covers_bsl_edt_designer_xml_and_unsupported_files() {
        assert_eq!(
            classify_path(Path::new("src/Модуль.bsl")),
            SourceFileKind::BslModule
        );
        assert_eq!(
            classify_path(Path::new("Configuration.mdo")),
            SourceFileKind::ConfigurationMetadata
        );
        assert_eq!(
            classify_path(Path::new("Catalogs/Товары/Ext/Object.mdo")),
            SourceFileKind::EdtMetadata
        );
        assert_eq!(
            classify_path(Path::new("Forms/ФормаЭлемента.form")),
            SourceFileKind::EdtForm
        );
        assert_eq!(
            classify_path(Path::new("Designer/ConfigDumpInfo.xml")),
            SourceFileKind::XmlMetadata
        );
        assert_eq!(
            classify_path(Path::new("README.md")),
            SourceFileKind::Unsupported
        );
    }

    #[test]
    fn file_classification_preserves_deleted_staged_files_without_contents() {
        let repo = temp_repo("deleted_without_contents");
        fs::create_dir_all(repo.join("configuration")).unwrap();
        let resolution = resolve_source_roots(&repo, &[PathBuf::from("configuration")]);
        let staged_files = vec![StagedFile {
            status: StagedStatus::Deleted,
            path: PathBuf::from("configuration/src/УдаленныйМодуль.bsl"),
            original_path: None,
        }];

        let files = classify_staged_files(&resolution.roots, &staged_files);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].kind, SourceFileKind::BslModule);
        assert_eq!(files[0].staged_status, Some(StagedStatus::Deleted));
        assert_eq!(
            files[0].source_relative_path,
            PathBuf::from("src/УдаленныйМодуль.bsl")
        );
        assert!(
            !repo.join(&files[0].repo_path).exists(),
            "classification must not require deleted file contents"
        );
    }

    #[test]
    fn source_root_list_parses_comma_separated_cli_value() {
        assert_eq!(
            parse_source_dir_list("configuration, exts/Расширение ,, tests/src"),
            vec![
                PathBuf::from("configuration"),
                PathBuf::from("exts/Расширение"),
                PathBuf::from("tests/src"),
            ]
        );
    }

    fn temp_repo(test_name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after UNIX_EPOCH")
            .as_nanos();
        let path = std::env::current_dir()
            .expect("current dir must be available")
            .join("target")
            .join("source-root-tests")
            .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
        fs::create_dir_all(&path).expect("temporary source-root test repo must be created");
        path
    }

    fn write_file(path: impl AsRef<Path>, content: &str) {
        let path = path.as_ref();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}
