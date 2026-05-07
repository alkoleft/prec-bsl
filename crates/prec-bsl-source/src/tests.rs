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
        classify_path(Path::new("External/Отчет.epf")),
        SourceFileKind::ExternalArtifact
    );
    assert_eq!(
        classify_path(Path::new("External/Обработка.erf")),
        SourceFileKind::ExternalArtifact
    );
    assert_eq!(
        classify_path(Path::new("External/Расширение.cfe")),
        SourceFileKind::ExternalArtifact
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
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
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
