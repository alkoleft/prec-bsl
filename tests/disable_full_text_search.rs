use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::full_text_search::{
    DISABLE_FULL_TEXT_SEARCH_RULE, FullTextSearchDisabling, FullTextSearchExclusions,
    disable_full_text_search_text,
};
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioRegistry, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{SourceFileKind, classify_repo_path, resolve_source_roots};

#[test]
fn disable_full_text_search_disables_edt_metadata_and_is_idempotent() {
    let repo = temp_repo("edt_metadata");
    let repo_path = PathBuf::from("src/Catalogs/Товары/Object.mdo");
    write_file(repo.join(&repo_path), edt_metadata_with_full_text_search());

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = full_text_search_config("");

    let first_report = run_pipeline(
        &ScenarioRegistry::reference(),
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
        corrected_edt_metadata()
    );
    assert_eq!(first_report.results.len(), 1);
    assert_eq!(
        first_report.results[0].rule_id,
        DISABLE_FULL_TEXT_SEARCH_RULE
    );
    assert_eq!(
        first_report.results[0].status,
        ScenarioResultStatus::Modified
    );
    assert_eq!(first_report.modified_paths(), vec![repo_path.clone()]);
    assert_eq!(first_report.hook_exit_code(), 1);

    let second_report = run_pipeline(
        &ScenarioRegistry::reference(),
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
fn disable_full_text_search_handles_designer_and_xr_metadata_names() {
    let input = concat!(
        "<MetaDataObject>\n",
        "  <Attribute name=\"Description\">\n",
        "    <xr:FullTextSearch>Use</xr:FullTextSearch>\n",
        "  </Attribute>\n",
        "  <Attribute name=\"Code\">\n",
        "    <FullTextSearch>use</FullTextSearch>\n",
        "  </Attribute>\n",
        "</MetaDataObject>\n",
    );
    let expected = concat!(
        "<MetaDataObject>\n",
        "  <Attribute name=\"Description\">\n",
        "    <xr:FullTextSearch>DontUse</xr:FullTextSearch>\n",
        "  </Attribute>\n",
        "  <Attribute name=\"Code\">\n",
        "    <FullTextSearch>DontUse</FullTextSearch>\n",
        "  </Attribute>\n",
        "</MetaDataObject>\n",
    );

    let result = disable_full_text_search_text(
        Path::new("src/Catalogs/Товары.xml"),
        SourceFileKind::XmlMetadata,
        input,
        &FullTextSearchExclusions::DisableAll,
    );

    assert_eq!(
        result,
        FullTextSearchDisabling::Modified(expected.to_owned())
    );
}

#[test]
fn disable_full_text_search_preserves_configured_attribute_exclusions() {
    let input = concat!(
        "<mdclass:Catalog>\n",
        "  <attributes>\n",
        "    <name>Сохранить</name>\n",
        "    <fullTextSearch>Use</fullTextSearch>\n",
        "  </attributes>\n",
        "  <attributes>\n",
        "    <name>Отключить</name>\n",
        "    <fullTextSearch>Use</fullTextSearch>\n",
        "  </attributes>\n",
        "  <tabularSections>\n",
        "    <name>ТЧ</name>\n",
        "    <attributes>\n",
        "      <name>Поле</name>\n",
        "      <fullTextSearch>Use</fullTextSearch>\n",
        "    </attributes>\n",
        "    <attributes>\n",
        "      <name>ДругоеПоле</name>\n",
        "      <fullTextSearch>Use</fullTextSearch>\n",
        "    </attributes>\n",
        "  </tabularSections>\n",
        "</mdclass:Catalog>\n",
    );
    let expected = concat!(
        "<mdclass:Catalog>\n",
        "  <attributes>\n",
        "    <name>Сохранить</name>\n",
        "    <fullTextSearch>Use</fullTextSearch>\n",
        "  </attributes>\n",
        "  <attributes>\n",
        "    <name>Отключить</name>\n",
        "    <fullTextSearch>DontUse</fullTextSearch>\n",
        "  </attributes>\n",
        "  <tabularSections>\n",
        "    <name>ТЧ</name>\n",
        "    <attributes>\n",
        "      <name>Поле</name>\n",
        "      <fullTextSearch>Use</fullTextSearch>\n",
        "    </attributes>\n",
        "    <attributes>\n",
        "      <name>ДругоеПоле</name>\n",
        "      <fullTextSearch>DontUse</fullTextSearch>\n",
        "    </attributes>\n",
        "  </tabularSections>\n",
        "</mdclass:Catalog>\n",
    );
    let exclusions = FullTextSearchExclusions::from_settings(
        Some(&serde_json::json!({
            "МетаданныеДляИсключения": {
                "/src/Catalogs/Товары/Object.mdo": ["Сохранить", "ТЧ.Поле"]
            }
        })),
        Path::new("src/Catalogs/Товары/Object.mdo"),
    )
    .unwrap();

    let result = disable_full_text_search_text(
        Path::new("src/Catalogs/Товары/Object.mdo"),
        SourceFileKind::EdtMetadata,
        input,
        &exclusions,
    );

    assert_eq!(
        result,
        FullTextSearchDisabling::Modified(expected.to_owned())
    );
}

#[test]
fn disable_full_text_search_does_not_preserve_by_ancestor_names_only() {
    let input = concat!(
        "<mdclass:Catalog>\n",
        "  <name>Товары</name>\n",
        "  <fullTextSearch>Use</fullTextSearch>\n",
        "  <attributes>\n",
        "    <name>Описание</name>\n",
        "    <fullTextSearch>Use</fullTextSearch>\n",
        "  </attributes>\n",
        "  <tabularSections>\n",
        "    <name>ТЧ</name>\n",
        "    <attributes>\n",
        "      <name>Поле</name>\n",
        "      <fullTextSearch>Use</fullTextSearch>\n",
        "    </attributes>\n",
        "  </tabularSections>\n",
        "</mdclass:Catalog>\n",
    );
    let expected = concat!(
        "<mdclass:Catalog>\n",
        "  <name>Товары</name>\n",
        "  <fullTextSearch>DontUse</fullTextSearch>\n",
        "  <attributes>\n",
        "    <name>Описание</name>\n",
        "    <fullTextSearch>DontUse</fullTextSearch>\n",
        "  </attributes>\n",
        "  <tabularSections>\n",
        "    <name>ТЧ</name>\n",
        "    <attributes>\n",
        "      <name>Поле</name>\n",
        "      <fullTextSearch>DontUse</fullTextSearch>\n",
        "    </attributes>\n",
        "  </tabularSections>\n",
        "</mdclass:Catalog>\n",
    );
    let exclusions = FullTextSearchExclusions::from_settings(
        Some(&serde_json::json!({
            "МетаданныеДляИсключения": {
                "src/Catalogs/Товары/Object.mdo": ["Товары", "ТЧ"]
            }
        })),
        Path::new("src/Catalogs/Товары/Object.mdo"),
    )
    .unwrap();

    let result = disable_full_text_search_text(
        Path::new("src/Catalogs/Товары/Object.mdo"),
        SourceFileKind::EdtMetadata,
        input,
        &exclusions,
    );

    assert_eq!(
        result,
        FullTextSearchDisabling::Modified(expected.to_owned())
    );
}

#[test]
fn disable_full_text_search_skips_empty_path_exclusion_and_non_metadata_files() {
    let repo = temp_repo("skip_cases");
    let metadata_path = PathBuf::from("src/Catalogs/Товары/Object.mdo");
    let form_path = PathBuf::from("src/Catalogs/Товары/Forms/Форма/Form.form");
    write_file(
        repo.join(&metadata_path),
        edt_metadata_with_full_text_search(),
    );
    write_file(
        repo.join(&form_path),
        "<form:Form><fullTextSearch>Use</fullTextSearch></form:Form>",
    );

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let metadata_file = classify_repo_path(&roots, metadata_path.clone(), None).unwrap();
    let form_file = classify_repo_path(&roots, form_path.clone(), None).unwrap();
    let config = full_text_search_config(
        r#""МетаданныеДляИсключения": {
            "src/Catalogs/Товары/Object.mdo": []
        }"#,
    );

    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![metadata_file, form_file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(
        fs::read_to_string(repo.join(&metadata_path)).unwrap(),
        edt_metadata_with_full_text_search()
    );
    assert_eq!(report.results.len(), 2);
    assert!(
        report
            .results
            .iter()
            .all(|result| result.status == ScenarioResultStatus::Skipped)
    );
    assert_eq!(report.hook_exit_code(), 0);
}

#[test]
fn disable_full_text_search_reports_invalid_settings_and_malformed_xml() {
    let invalid_settings = FullTextSearchExclusions::from_settings(
        Some(&serde_json::json!({
            "МетаданныеДляИсключения": {
                "src/Catalogs/Товары/Object.mdo": ["Сохранить", 1]
            }
        })),
        Path::new("src/Catalogs/Товары/Object.mdo"),
    )
    .unwrap_err();
    assert!(invalid_settings.contains("must contain only strings"));

    let result = disable_full_text_search_text(
        Path::new("src/Catalogs/Товары/Object.mdo"),
        SourceFileKind::EdtMetadata,
        "<mdclass:Catalog><fullTextSearch>Use</mdclass:Catalog>",
        &FullTextSearchExclusions::DisableAll,
    );

    assert!(
        matches!(result, FullTextSearchDisabling::Failed(message) if message.contains("failed to parse XML/EDT file"))
    );
}

fn full_text_search_config(settings: &str) -> prec_bsl::config::ResolvedConfig {
    let settings = if settings.trim().is_empty() {
        "{}".to_owned()
    } else {
        format!("{{{settings}}}")
    };
    parse_config_str(&format!(
        r#"{{
            "Precommt4onecСценарии": {{
                "ГлобальныеСценарии": ["ОтключениеПолнотекстовогоПоиска.os"],
                "НастройкиСценариев": {{
                    "ОтключениеПолнотекстовогоПоиска": {settings}
                }}
            }}
        }}"#
    ))
    .unwrap()
}

fn edt_metadata_with_full_text_search() -> &'static str {
    concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<mdclass:Catalog>\n",
        "  <name>Товары</name>\n",
        "  <fullTextSearchOnInputByString>Use</fullTextSearchOnInputByString>\n",
        "  <attributes>\n",
        "    <name>Описание</name>\n",
        "    <fullTextSearch>Use</fullTextSearch>\n",
        "  </attributes>\n",
        "  <attributes>\n",
        "    <name>Картинка</name>\n",
        "    <fullTextSearch>DontUse</fullTextSearch>\n",
        "  </attributes>\n",
        "</mdclass:Catalog>\n",
    )
}

fn corrected_edt_metadata() -> &'static str {
    concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<mdclass:Catalog>\n",
        "  <name>Товары</name>\n",
        "  <fullTextSearchOnInputByString>Use</fullTextSearchOnInputByString>\n",
        "  <attributes>\n",
        "    <name>Описание</name>\n",
        "    <fullTextSearch>DontUse</fullTextSearch>\n",
        "  </attributes>\n",
        "  <attributes>\n",
        "    <name>Картинка</name>\n",
        "    <fullTextSearch>DontUse</fullTextSearch>\n",
        "  </attributes>\n",
        "</mdclass:Catalog>\n",
    )
}

fn temp_repo(test_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after UNIX_EPOCH")
        .as_nanos();
    let path = std::env::current_dir()
        .expect("current dir must be available")
        .join("target")
        .join("disable-full-text-search-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary full-text-search test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory must be created");
    }
    fs::write(path, content).expect("test fixture must be written");
}
