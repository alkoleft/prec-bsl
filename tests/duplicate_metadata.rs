use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::duplicate_metadata::{
    DUPLICATE_METADATA_RULE, DuplicateMetadataRemoval, remove_duplicate_metadata_text,
};
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{SourceFileKind, classify_repo_path, resolve_source_roots};

#[test]
fn duplicate_metadata_removes_edt_duplicates_and_is_idempotent() {
    let repo = temp_repo("edt_configuration");
    let repo_path = PathBuf::from("src/Configuration/Configuration.mdo");
    write_file(repo.join(&repo_path), edt_configuration_with_duplicates());

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = duplicate_metadata_config();

    let first_report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file.clone()],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        edt_configuration_without_duplicates()
    );
    assert_eq!(first_report.results.len(), 1);
    assert_eq!(first_report.results[0].rule_id, DUPLICATE_METADATA_RULE);
    assert_eq!(
        first_report.results[0].status,
        ScenarioResultStatus::Modified
    );
    assert_eq!(first_report.modified_paths(), vec![repo_path]);
    assert_eq!(first_report.hook_exit_code(), 1);

    let second_report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert!(second_report.results.is_empty());
    assert_eq!(second_report.hook_exit_code(), 0);
}

#[test]
fn duplicate_metadata_removes_designer_child_object_duplicates() {
    let input = concat!(
        "<MetaDataObject>\n",
        "  <Configuration>\n",
        "    <ChildObjects>\n",
        "      <CommonModule>Модуль</CommonModule>\n",
        "      <Catalog>Товары</Catalog>\n",
        "      <CommonModule>Модуль</CommonModule>\n",
        "      <Catalog>Товары</Catalog>\n",
        "      <Subsystem>Ядро</Subsystem>\n",
        "    </ChildObjects>\n",
        "  </Configuration>\n",
        "</MetaDataObject>\n",
    );
    let expected = concat!(
        "<MetaDataObject>\n",
        "  <Configuration>\n",
        "    <ChildObjects>\n",
        "      <CommonModule>Модуль</CommonModule>\n",
        "      <Catalog>Товары</Catalog>\n",
        "      <Subsystem>Ядро</Subsystem>\n",
        "    </ChildObjects>\n",
        "  </Configuration>\n",
        "</MetaDataObject>\n",
    );

    let result = remove_duplicate_metadata_text(
        Path::new("Configuration.xml"),
        SourceFileKind::XmlMetadata,
        input,
    );

    assert_eq!(
        result,
        DuplicateMetadataRemoval::Modified(expected.to_owned())
    );
}

#[test]
fn duplicate_metadata_preserves_last_duplicate_occurrence() {
    let input = concat!(
        "<MetaDataObject>\n",
        "  <Configuration>\n",
        "    <ChildObjects>\n",
        "      <CommonModule>А</CommonModule>\n",
        "      <Catalog>Б</Catalog>\n",
        "      <CommonModule>А</CommonModule>\n",
        "    </ChildObjects>\n",
        "  </Configuration>\n",
        "</MetaDataObject>\n",
    );
    let expected = concat!(
        "<MetaDataObject>\n",
        "  <Configuration>\n",
        "    <ChildObjects>\n",
        "      <Catalog>Б</Catalog>\n",
        "      <CommonModule>А</CommonModule>\n",
        "    </ChildObjects>\n",
        "  </Configuration>\n",
        "</MetaDataObject>\n",
    );

    assert_eq!(
        remove_duplicate_metadata_text(
            Path::new("Configuration.xml"),
            SourceFileKind::XmlMetadata,
            input
        ),
        DuplicateMetadataRemoval::Modified(expected.to_owned())
    );
}

#[test]
fn duplicate_metadata_preserves_edt_entries_before_languages() {
    let input = concat!(
        "<mdclass:Configuration>\n",
        "  <commonModules>CommonModule.А</commonModules>\n",
        "  <commonModules>CommonModule.А</commonModules>\n",
        "  <languages>Language.Русский</languages>\n",
        "  <commonModules>CommonModule.Б</commonModules>\n",
        "  <commonModules>CommonModule.Б</commonModules>\n",
        "</mdclass:Configuration>\n",
    );
    let expected = concat!(
        "<mdclass:Configuration>\n",
        "  <commonModules>CommonModule.А</commonModules>\n",
        "  <commonModules>CommonModule.А</commonModules>\n",
        "  <languages>Language.Русский</languages>\n",
        "  <commonModules>CommonModule.Б</commonModules>\n",
        "</mdclass:Configuration>\n",
    );

    assert_eq!(
        remove_duplicate_metadata_text(
            Path::new("Configuration/Configuration.mdo"),
            SourceFileKind::ConfigurationMetadata,
            input,
        ),
        DuplicateMetadataRemoval::Modified(expected.to_owned())
    );
}

#[test]
fn duplicate_metadata_uses_source_root_relative_configuration_identity() {
    let repo = temp_repo("source_relative_configuration");
    let nested_path = PathBuf::from("src/Nested/Configuration/Configuration.mdo");
    write_file(
        repo.join(&nested_path),
        minimal_edt_duplicate_configuration(),
    );

    let broad_roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let broad_file = classify_repo_path(&broad_roots, nested_path.clone(), None).unwrap();
    let broad_report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &broad_roots,
            config: &duplicate_metadata_config(),
            files: vec![broad_file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(broad_report.results.len(), 1);
    assert_eq!(
        broad_report.results[0].status,
        ScenarioResultStatus::Skipped
    );
    assert_eq!(
        fs::read_to_string(repo.join(&nested_path)).unwrap(),
        minimal_edt_duplicate_configuration()
    );

    let nested_roots = resolve_source_roots(&repo, &[PathBuf::from("src/Nested")]).roots;
    let nested_file = classify_repo_path(&nested_roots, nested_path.clone(), None).unwrap();
    let nested_report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &nested_roots,
            config: &duplicate_metadata_config(),
            files: vec![nested_file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(nested_report.results.len(), 1);
    assert_eq!(
        nested_report.results[0].status,
        ScenarioResultStatus::Modified
    );
    assert_eq!(
        fs::read_to_string(repo.join(&nested_path)).unwrap(),
        minimal_edt_clean_configuration()
    );
}

#[test]
fn duplicate_metadata_skips_non_configuration_and_preserves_uid_like_references() {
    assert_eq!(
        remove_duplicate_metadata_text(
            Path::new("CommonModules/Модуль/Модуль.mdo"),
            SourceFileKind::EdtMetadata,
            "<mdclass:CommonModule/>",
        ),
        DuplicateMetadataRemoval::Skipped(
            "scenario handles only Configuration.mdo and Configuration.xml".to_owned()
        )
    );

    let input = concat!(
        "<mdclass:Configuration>\n",
        "  <commonModules>CommonModule.UID-123</commonModules>\n",
        "  <commonModules>CommonModule.UID-123</commonModules>\n",
        "</mdclass:Configuration>\n",
    );
    assert_eq!(
        remove_duplicate_metadata_text(
            Path::new("Configuration/Configuration.mdo"),
            SourceFileKind::ConfigurationMetadata,
            input,
        ),
        DuplicateMetadataRemoval::Clean
    );
}

#[test]
fn duplicate_metadata_preserves_same_reference_with_different_source_shape() {
    let input = concat!(
        "<mdclass:Configuration>\n",
        "  <commonModules>CommonModule.А</commonModules>\n",
        "\t<commonModules>CommonModule.А</commonModules>\n",
        "</mdclass:Configuration>\n",
    );

    assert_eq!(
        remove_duplicate_metadata_text(
            Path::new("Configuration/Configuration.mdo"),
            SourceFileKind::ConfigurationMetadata,
            input,
        ),
        DuplicateMetadataRemoval::Clean
    );
}

#[test]
fn duplicate_metadata_reports_malformed_xml() {
    let result = remove_duplicate_metadata_text(
        Path::new("Configuration/Configuration.mdo"),
        SourceFileKind::ConfigurationMetadata,
        "<mdclass:Configuration><commonModules></mdclass:Configuration>",
    );

    assert!(
        matches!(result, DuplicateMetadataRemoval::Failed(message) if message.contains("failed to parse XML/EDT file"))
    );
}

fn duplicate_metadata_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["УдалениеДублейМетаданных.os"]
            }
        }"#,
    )
    .unwrap()
}

fn edt_configuration_with_duplicates() -> &'static str {
    concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<mdclass:Configuration>\n",
        "  <name>Demo</name>\n",
        "  <languages>Language.Русский</languages>\n",
        "  <commonModules>CommonModule.Модуль</commonModules>\n",
        "  <catalogs>Catalog.Товары</catalogs>\n",
        "  <commonModules>CommonModule.Модуль</commonModules>\n",
        "  <catalogs>Catalog.Товары</catalogs>\n",
        "  <subsystems>Subsystem.Ядро</subsystems>\n",
        "</mdclass:Configuration>\n",
    )
}

fn edt_configuration_without_duplicates() -> &'static str {
    concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<mdclass:Configuration>\n",
        "  <name>Demo</name>\n",
        "  <languages>Language.Русский</languages>\n",
        "  <commonModules>CommonModule.Модуль</commonModules>\n",
        "  <catalogs>Catalog.Товары</catalogs>\n",
        "  <subsystems>Subsystem.Ядро</subsystems>\n",
        "</mdclass:Configuration>\n",
    )
}

fn minimal_edt_duplicate_configuration() -> &'static str {
    concat!(
        "<mdclass:Configuration>\n",
        "  <languages>Language.Русский</languages>\n",
        "  <commonModules>CommonModule.А</commonModules>\n",
        "  <commonModules>CommonModule.А</commonModules>\n",
        "</mdclass:Configuration>\n",
    )
}

fn minimal_edt_clean_configuration() -> &'static str {
    concat!(
        "<mdclass:Configuration>\n",
        "  <languages>Language.Русский</languages>\n",
        "  <commonModules>CommonModule.А</commonModules>\n",
        "</mdclass:Configuration>\n",
    )
}

fn temp_repo(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    path.push(format!("prec-bsl-duplicate-metadata-{name}-{unique}"));
    fs::create_dir_all(&path).expect("temporary duplicate metadata test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}
