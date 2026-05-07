use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::composition_sort::{
    COMPOSITION_SORT_RULE, CompositionSortSettings, CompositionSorting, sort_composition_text,
};
use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioRegistry, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{SourceFileKind, classify_repo_path, resolve_source_roots};

#[test]
fn composition_sort_sorts_edt_configuration_and_is_idempotent() {
    let repo = temp_repo("edt_configuration");
    let repo_path = PathBuf::from("src/Configuration/Configuration.mdo");
    write_file(repo.join(&repo_path), unsorted_edt_configuration());

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = composition_sort_config("");

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
        sorted_edt_configuration()
    );
    assert_eq!(first_report.results.len(), 1);
    assert_eq!(first_report.results[0].rule_id, COMPOSITION_SORT_RULE);
    assert_eq!(
        first_report.results[0].status,
        ScenarioResultStatus::Modified
    );
    assert_eq!(first_report.modified_paths(), vec![repo_path]);
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
fn composition_sort_sorts_designer_child_objects() {
    let input = concat!(
        "<MetaDataObject>\n",
        "  <Configuration>\n",
        "    <ChildObjects>\n",
        "      <CommonModule>ЯМодуль</CommonModule>\n",
        "      <CommonModule>АМодуль</CommonModule>\n",
        "      <Catalog>Товары</Catalog>\n",
        "      <Catalog>Контрагенты</Catalog>\n",
        "      <Subsystem>Ядро</Subsystem>\n",
        "      <Subsystem>Администрирование</Subsystem>\n",
        "    </ChildObjects>\n",
        "  </Configuration>\n",
        "</MetaDataObject>\n",
    );
    let expected = concat!(
        "<MetaDataObject>\n",
        "  <Configuration>\n",
        "    <ChildObjects>\n",
        "      <CommonModule>АМодуль</CommonModule>\n",
        "      <CommonModule>ЯМодуль</CommonModule>\n",
        "      <Catalog>Контрагенты</Catalog>\n",
        "      <Catalog>Товары</Catalog>\n",
        "      <Subsystem>Ядро</Subsystem>\n",
        "      <Subsystem>Администрирование</Subsystem>\n",
        "    </ChildObjects>\n",
        "  </Configuration>\n",
        "</MetaDataObject>\n",
    );

    let result = sort_composition_text(
        Path::new("Configuration.xml"),
        SourceFileKind::XmlMetadata,
        input,
        &CompositionSortSettings::from_settings(None).unwrap(),
    );

    assert_eq!(result, CompositionSorting::Modified(expected.to_owned()));
}

#[test]
fn composition_sort_honors_prefix_buckets_after_non_prefixed_references() {
    let input = concat!(
        "<mdclass:Configuration>\n",
        "  <commonModules>CommonModule.РатБ</commonModules>\n",
        "  <commonModules>CommonModule.АА</commonModules>\n",
        "  <commonModules>CommonModule.ЮТБ</commonModules>\n",
        "  <commonModules>CommonModule.РатА</commonModules>\n",
        "  <commonModules>CommonModule.ЮТА</commonModules>\n",
        "</mdclass:Configuration>\n",
    );
    let expected = concat!(
        "<mdclass:Configuration>\n",
        "  <commonModules>CommonModule.АА</commonModules>\n",
        "  <commonModules>CommonModule.РатА</commonModules>\n",
        "  <commonModules>CommonModule.РатБ</commonModules>\n",
        "  <commonModules>CommonModule.ЮТА</commonModules>\n",
        "  <commonModules>CommonModule.ЮТБ</commonModules>\n",
        "</mdclass:Configuration>\n",
    );
    let settings = CompositionSortSettings::from_settings(Some(&serde_json::json!({
        "УчитываяПрефикс": ["Рат", "ЮТ"]
    })))
    .unwrap();

    let result = sort_composition_text(
        Path::new("Configuration/Configuration.mdo"),
        SourceFileKind::ConfigurationMetadata,
        input,
        &settings,
    );

    assert_eq!(result, CompositionSorting::Modified(expected.to_owned()));
}

#[test]
fn composition_sort_skips_disabled_configuration_and_non_configuration_files() {
    let repo = temp_repo("missing_non_configuration");
    fs::create_dir_all(repo.join("src")).unwrap();
    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let missing_file =
        classify_repo_path(&roots, "src/CommonModules/Модуль/Модуль.mdo", None).unwrap();
    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &composition_sort_config(""),
            files: vec![missing_file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
    assert_eq!(report.hook_exit_code(), 0);

    let disabled = CompositionSortSettings::from_settings(Some(&serde_json::json!({
        "ОтключенныеОбъекты": "Подсистема, конфигурация"
    })))
    .unwrap();
    assert_eq!(
        sort_composition_text(
            Path::new("Configuration/Configuration.mdo"),
            SourceFileKind::ConfigurationMetadata,
            unsorted_edt_configuration(),
            &disabled,
        ),
        CompositionSorting::Skipped(
            "configuration composition sorting is disabled by scenario settings".to_owned()
        )
    );
    assert_eq!(
        sort_composition_text(
            Path::new("CommonModules/Модуль/Модуль.mdo"),
            SourceFileKind::EdtMetadata,
            "<mdclass:CommonModule/>",
            &disabled,
        ),
        CompositionSorting::Skipped(
            "scenario handles only Configuration.mdo and Configuration.xml".to_owned()
        )
    );
}

#[test]
fn composition_sort_uses_source_root_relative_configuration_identity() {
    let repo = temp_repo("source_relative_configuration");
    let nested_path = PathBuf::from("src/Nested/Configuration/Configuration.mdo");
    write_file(
        repo.join(&nested_path),
        unsorted_minimal_edt_configuration(),
    );

    let broad_roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let broad_file = classify_repo_path(&broad_roots, nested_path.clone(), None).unwrap();
    let broad_report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &broad_roots,
            config: &composition_sort_config(""),
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
        unsorted_minimal_edt_configuration()
    );

    let nested_roots = resolve_source_roots(&repo, &[PathBuf::from("src/Nested")]).roots;
    let nested_file = classify_repo_path(&nested_roots, nested_path.clone(), None).unwrap();
    let nested_report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &nested_roots,
            config: &composition_sort_config(""),
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
        sorted_minimal_edt_configuration()
    );
}

#[test]
fn composition_sort_reports_invalid_settings_and_malformed_xml() {
    let invalid_settings = CompositionSortSettings::from_settings(Some(&serde_json::json!({
        "УчитываяПрефикс": [1]
    })))
    .unwrap_err();
    assert!(invalid_settings.contains("must contain only strings"));

    let result = sort_composition_text(
        Path::new("Configuration/Configuration.mdo"),
        SourceFileKind::ConfigurationMetadata,
        "<mdclass:Configuration><commonModules></mdclass:Configuration>",
        &CompositionSortSettings::from_settings(None).unwrap(),
    );

    assert!(
        matches!(result, CompositionSorting::Failed(message) if message.contains("failed to parse XML/EDT file"))
    );
}

fn composition_sort_config(settings: &str) -> prec_bsl::config::ResolvedConfig {
    let settings = if settings.trim().is_empty() {
        "{}".to_owned()
    } else {
        format!("{{{settings}}}")
    };
    parse_config_str(&format!(
        r#"{{
            "Precommt4onecСценарии": {{
                "ГлобальныеСценарии": ["СортировкаСостава.os"],
                "НастройкиСценариев": {{
                    "СортировкаСостава": {settings}
                }}
            }}
        }}"#
    ))
    .unwrap()
}

fn unsorted_edt_configuration() -> &'static str {
    concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<mdclass:Configuration>\n",
        "  <name>Demo</name>\n",
        "  <languages>Language.Русский</languages>\n",
        "  <commonModules>CommonModule.ЯМодуль</commonModules>\n",
        "  <commonModules>CommonModule.АМодуль</commonModules>\n",
        "  <subsystems>Subsystem.Ядро</subsystems>\n",
        "  <subsystems>Subsystem.Администрирование</subsystems>\n",
        "  <commonTemplates>CommonTemplate.Шаблон</commonTemplates>\n",
        "  <commonTemplates>CommonTemplate.АШаблон</commonTemplates>\n",
        "  <commonModules>CommonModule.UID-123</commonModules>\n",
        "</mdclass:Configuration>\n",
    )
}

fn sorted_edt_configuration() -> &'static str {
    concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<mdclass:Configuration>\n",
        "  <name>Demo</name>\n",
        "  <languages>Language.Русский</languages>\n",
        "  <commonModules>CommonModule.АМодуль</commonModules>\n",
        "  <commonModules>CommonModule.ЯМодуль</commonModules>\n",
        "  <subsystems>Subsystem.Ядро</subsystems>\n",
        "  <subsystems>Subsystem.Администрирование</subsystems>\n",
        "  <commonTemplates>CommonTemplate.АШаблон</commonTemplates>\n",
        "  <commonTemplates>CommonTemplate.Шаблон</commonTemplates>\n",
        "  <commonModules>CommonModule.UID-123</commonModules>\n",
        "</mdclass:Configuration>\n",
    )
}

fn unsorted_minimal_edt_configuration() -> &'static str {
    concat!(
        "<mdclass:Configuration>\n",
        "  <commonModules>CommonModule.Я</commonModules>\n",
        "  <commonModules>CommonModule.А</commonModules>\n",
        "</mdclass:Configuration>\n",
    )
}

fn sorted_minimal_edt_configuration() -> &'static str {
    concat!(
        "<mdclass:Configuration>\n",
        "  <commonModules>CommonModule.А</commonModules>\n",
        "  <commonModules>CommonModule.Я</commonModules>\n",
        "</mdclass:Configuration>\n",
    )
}

fn temp_repo(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    path.push(format!("prec-bsl-composition-sort-{name}-{unique}"));
    fs::create_dir_all(&path).expect("temporary composition sort test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}
