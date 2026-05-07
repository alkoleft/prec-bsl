use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::composition_sort::{
    COMPOSITION_SORT_RULE, CompositionSortScope, CompositionSortSettings, CompositionSorting,
    METADATA_TREE_SORT_RULE, SUBSYSTEM_COMPOSITION_SORT_RULE, sort_composition_text,
};
use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioResultStatus, run_pipeline,
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
        CompositionSortScope::All,
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
        CompositionSortScope::All,
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
        &prec_bsl::reference_registry(),
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
            CompositionSortScope::All,
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
            CompositionSortScope::All,
        ),
        CompositionSorting::Skipped(
            "scenario handles only configuration and subsystem metadata description files"
                .to_owned()
        )
    );
    assert_eq!(
        sort_composition_text(
            Path::new("Subsystems/Демо/Демо.mdo"),
            SourceFileKind::EdtMetadata,
            unsorted_edt_subsystem(),
            &disabled,
            CompositionSortScope::SubsystemComposition,
        ),
        CompositionSorting::Skipped(
            "subsystem composition sorting is disabled by scenario settings".to_owned()
        )
    );
}

#[test]
fn compatibility_metadata_tree_rule_sorts_only_configuration_tree() {
    let repo = temp_repo("metadata_tree_alias");
    let configuration_path = PathBuf::from("src/Configuration/Configuration.mdo");
    let subsystem_path = PathBuf::from("src/Subsystems/Демо/Демо.mdo");
    write_file(
        repo.join(&configuration_path),
        unsorted_minimal_edt_configuration(),
    );
    write_file(repo.join(&subsystem_path), unsorted_edt_subsystem());

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let configuration_file = classify_repo_path(&roots, configuration_path.clone(), None).unwrap();
    let subsystem_file = classify_repo_path(&roots, subsystem_path.clone(), None).unwrap();
    let config = config_with_rules(&[METADATA_TREE_SORT_RULE]);
    let report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![configuration_file, subsystem_file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(
        fs::read_to_string(repo.join(&configuration_path)).unwrap(),
        sorted_minimal_edt_configuration()
    );
    assert_eq!(
        fs::read_to_string(repo.join(&subsystem_path)).unwrap(),
        unsorted_edt_subsystem()
    );
    assert_eq!(
        report
            .results
            .iter()
            .filter(|result| result.status == ScenarioResultStatus::Modified)
            .map(|result| result.rule_id.as_str())
            .collect::<Vec<_>>(),
        vec![METADATA_TREE_SORT_RULE]
    );
}

#[test]
fn compatibility_subsystem_rule_sorts_edt_and_designer_subsystem_content() {
    let repo = temp_repo("subsystem_alias");
    let edt_path = PathBuf::from("src/Subsystems/Демо/Демо.mdo");
    let designer_path = PathBuf::from("src-designer/Subsystems/Демо.xml");
    write_file(repo.join(&edt_path), unsorted_edt_subsystem());
    write_file(repo.join(&designer_path), unsorted_designer_subsystem());

    let roots = resolve_source_roots(
        &repo,
        &[PathBuf::from("src"), PathBuf::from("src-designer")],
    )
    .roots;
    let edt_file = classify_repo_path(&roots, edt_path.clone(), None).unwrap();
    let designer_file = classify_repo_path(&roots, designer_path.clone(), None).unwrap();
    let config = config_with_rules(&[SUBSYSTEM_COMPOSITION_SORT_RULE]);

    let first_report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![edt_file.clone(), designer_file.clone()],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(
        fs::read_to_string(repo.join(&edt_path)).unwrap(),
        sorted_edt_subsystem()
    );
    assert_eq!(
        fs::read_to_string(repo.join(&designer_path)).unwrap(),
        sorted_designer_subsystem()
    );
    assert_eq!(
        first_report.modified_paths(),
        vec![edt_path.clone(), designer_path.clone()]
    );

    let second_report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![edt_file, designer_file],
            mode: PipelineMode::Hook,
        },
    );

    assert!(second_report.results.is_empty());
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
        &prec_bsl::reference_registry(),
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
        &prec_bsl::reference_registry(),
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
        CompositionSortScope::All,
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

fn config_with_rules(rules: &[&str]) -> prec_bsl::config::ResolvedConfig {
    let rules = rules
        .iter()
        .map(|rule| format!(r#""{rule}.os""#))
        .collect::<Vec<_>>()
        .join(", ");
    parse_config_str(&format!(
        r#"{{
            "Precommt4onecСценарии": {{
                "ГлобальныеСценарии": [{rules}]
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

fn unsorted_edt_subsystem() -> &'static str {
    concat!(
        "<mdclass:Subsystem>\n",
        "  <name>Демо</name>\n",
        "  <content>Document.Заказ</content>\n",
        "  <content>Catalog.Номенклатура</content>\n",
        "  <content>CommonModule.Сервис</content>\n",
        "  <content>CommonModule.Адаптер</content>\n",
        "</mdclass:Subsystem>\n",
    )
}

fn sorted_edt_subsystem() -> &'static str {
    concat!(
        "<mdclass:Subsystem>\n",
        "  <name>Демо</name>\n",
        "  <content>Catalog.Номенклатура</content>\n",
        "  <content>CommonModule.Адаптер</content>\n",
        "  <content>CommonModule.Сервис</content>\n",
        "  <content>Document.Заказ</content>\n",
        "</mdclass:Subsystem>\n",
    )
}

fn unsorted_designer_subsystem() -> &'static str {
    concat!(
        "<MetaDataObject>\n",
        "  <Subsystem>\n",
        "    <Properties>\n",
        "      <Name>Демо</Name>\n",
        "      <Content>\n",
        "        <xr:Item xsi:type=\"xr:MDObjectRef\">Document.Заказ</xr:Item>\n",
        "        <xr:Item xsi:type=\"xr:MDObjectRef\">Catalog.Номенклатура</xr:Item>\n",
        "        <xr:Item xsi:type=\"xr:MDObjectRef\">CommonModule.Сервис</xr:Item>\n",
        "        <xr:Item xsi:type=\"xr:MDObjectRef\">CommonModule.Адаптер</xr:Item>\n",
        "      </Content>\n",
        "    </Properties>\n",
        "  </Subsystem>\n",
        "</MetaDataObject>\n",
    )
}

fn sorted_designer_subsystem() -> &'static str {
    concat!(
        "<MetaDataObject>\n",
        "  <Subsystem>\n",
        "    <Properties>\n",
        "      <Name>Демо</Name>\n",
        "      <Content>\n",
        "        <xr:Item xsi:type=\"xr:MDObjectRef\">Catalog.Номенклатура</xr:Item>\n",
        "        <xr:Item xsi:type=\"xr:MDObjectRef\">CommonModule.Адаптер</xr:Item>\n",
        "        <xr:Item xsi:type=\"xr:MDObjectRef\">CommonModule.Сервис</xr:Item>\n",
        "        <xr:Item xsi:type=\"xr:MDObjectRef\">Document.Заказ</xr:Item>\n",
        "      </Content>\n",
        "    </Properties>\n",
        "  </Subsystem>\n",
        "</MetaDataObject>\n",
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
