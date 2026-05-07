use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::form_change_permission::{
    DISABLE_FORM_CHANGE_RULE, FormChangePermissionDisabling, disable_form_change_permission_text,
};
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioRegistry, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{SourceFileKind, classify_repo_path, resolve_source_roots};

#[test]
fn disable_form_change_disables_edt_form_and_is_idempotent() {
    let repo = temp_repo("edt_form");
    let repo_path = PathBuf::from("src/Catalogs/Товары/Forms/Форма/Form.form");
    write_file(repo.join(&repo_path), edt_form_with_permission());

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = disable_form_change_config();

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
        edt_form_without_permission()
    );
    assert_eq!(first_report.results.len(), 1);
    assert_eq!(first_report.results[0].rule_id, DISABLE_FORM_CHANGE_RULE);
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
fn disable_form_change_updates_designer_customizable_flag() {
    let input = concat!(
        "<Form>\n",
        "  <WindowOpeningMode>LockOwnerWindow</WindowOpeningMode>\n",
        "  <Customizable>true</Customizable>\n",
        "</Form>\n",
    );
    let expected = concat!(
        "<Form>\n",
        "  <WindowOpeningMode>LockOwnerWindow</WindowOpeningMode>\n",
        "  <Customizable>false</Customizable>\n",
        "</Form>\n",
    );

    let result = disable_form_change_permission_text(
        Path::new("src/Forms/Form.xml"),
        SourceFileKind::XmlMetadata,
        input,
    );

    assert_eq!(
        result,
        FormChangePermissionDisabling::Modified(expected.to_owned())
    );
}

#[test]
fn disable_form_change_inserts_missing_designer_customizable_after_window_opening_mode() {
    let input = concat!(
        "<Form>\n",
        "\t<WindowOpeningMode>LockOwnerWindow</WindowOpeningMode>\r\n",
        "\t<Title>Форма</Title>\r\n",
        "</Form>\r\n",
    );
    let expected = concat!(
        "<Form>\n",
        "\t<WindowOpeningMode>LockOwnerWindow</WindowOpeningMode>\r\n",
        "\t<Customizable>false</Customizable>\r\n",
        "\t<Title>Форма</Title>\r\n",
        "</Form>\r\n",
    );

    let result = disable_form_change_permission_text(
        Path::new("src/Forms/Form.xml"),
        SourceFileKind::XmlMetadata,
        input,
    );

    assert_eq!(
        result,
        FormChangePermissionDisabling::Modified(expected.to_owned())
    );
}

#[test]
fn disable_form_change_keeps_non_form_xml_clean() {
    let non_form_xml = disable_form_change_permission_text(
        Path::new("src/Settings.xml"),
        SourceFileKind::XmlMetadata,
        concat!(
            "<Settings>\n",
            "  <WindowOpeningMode>LockOwnerWindow</WindowOpeningMode>\n",
            "  <Customizable>true</Customizable>\n",
            "</Settings>\n",
        ),
    );
    let non_form_edt = disable_form_change_permission_text(
        Path::new("src/Forms/Форма/Вложенная.form"),
        SourceFileKind::EdtForm,
        "<form:Form><allowFormCustomize>true</allowFormCustomize></form:Form>",
    );

    assert_eq!(non_form_xml, FormChangePermissionDisabling::Clean);
    assert_eq!(non_form_edt, FormChangePermissionDisabling::Clean);
}

#[test]
fn disable_form_change_keeps_existing_false_values_clean() {
    let edt = disable_form_change_permission_text(
        Path::new("src/Forms/Форма/Form.form"),
        SourceFileKind::EdtForm,
        edt_form_without_permission(),
    );
    let designer = disable_form_change_permission_text(
        Path::new("src/Forms/Form.xml"),
        SourceFileKind::XmlMetadata,
        concat!(
            "<Form>\n",
            "  <WindowOpeningMode>LockOwnerWindow</WindowOpeningMode>\n",
            "  <Customizable>false</Customizable>\n",
            "</Form>\n",
        ),
    );

    assert_eq!(edt, FormChangePermissionDisabling::Clean);
    assert_eq!(designer, FormChangePermissionDisabling::Clean);
}

#[test]
fn disable_form_change_skips_non_form_files_in_pipeline() {
    let repo = temp_repo("skip_cases");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    write_file(repo.join(&repo_path), "Процедура Тест()\nКонецПроцедуры\n");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();
    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &disable_form_change_config(),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, DISABLE_FORM_CHANGE_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
    assert_eq!(
        report.results[0].message,
        "scenario handles only XML form description files"
    );
    assert_eq!(report.hook_exit_code(), 0);
}

#[test]
fn disable_form_change_reports_invalid_property_values_and_malformed_xml() {
    let invalid_value = disable_form_change_permission_text(
        Path::new("src/Forms/Форма/Form.form"),
        SourceFileKind::EdtForm,
        "<form:Form><allowFormCustomize>yes</allowFormCustomize></form:Form>",
    );
    let cdata_value = disable_form_change_permission_text(
        Path::new("src/Forms/Form.xml"),
        SourceFileKind::XmlMetadata,
        "<Form><Customizable><![CDATA[true]]></Customizable></Form>",
    );
    let empty_value = disable_form_change_permission_text(
        Path::new("src/Forms/Form.xml"),
        SourceFileKind::XmlMetadata,
        concat!(
            "<Form>",
            "<WindowOpeningMode>LockOwnerWindow</WindowOpeningMode>",
            "<Customizable></Customizable>",
            "</Form>",
        ),
    );
    let malformed = disable_form_change_permission_text(
        Path::new("src/Forms/Форма/Form.form"),
        SourceFileKind::EdtForm,
        "<form:Form><allowFormCustomize>true</form:Form>",
    );
    let mixed_designer = disable_form_change_permission_text(
        Path::new("src/Forms/Form.xml"),
        SourceFileKind::XmlMetadata,
        "<Form><Customizable>true<Extra/></Customizable></Form>",
    );
    let mixed_edt = disable_form_change_permission_text(
        Path::new("src/Forms/Форма/Form.form"),
        SourceFileKind::EdtForm,
        "<form:Form><allowFormCustomize>true<extra/></allowFormCustomize></form:Form>",
    );

    assert!(matches!(
        invalid_value,
        FormChangePermissionDisabling::Failed(message)
            if message.contains("allowFormCustomize must contain true or false")
    ));
    assert!(matches!(
        cdata_value,
        FormChangePermissionDisabling::Failed(message)
            if message.contains("Customizable supports only plain XML text values")
    ));
    assert!(matches!(
        empty_value,
        FormChangePermissionDisabling::Failed(message)
            if message.contains("Customizable must contain true or false plain text")
    ));
    assert!(matches!(
        malformed,
        FormChangePermissionDisabling::Failed(message)
            if message.contains("failed to parse XML/EDT file")
    ));
    assert!(matches!(
        mixed_designer,
        FormChangePermissionDisabling::Failed(message)
            if message.contains("Customizable must contain true or false plain text")
    ));
    assert!(matches!(
        mixed_edt,
        FormChangePermissionDisabling::Failed(message)
            if message.contains("allowFormCustomize must contain true or false plain text")
    ));
}

fn disable_form_change_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ОтключениеРазрешенияИзменятьФорму.os"]
            }
        }"#,
    )
    .unwrap()
}

fn edt_form_with_permission() -> &'static str {
    concat!(
        "<form:Form xmlns:form=\"http://g5.1c.ru/v8/dt/form\">\n",
        "  <items/>\n",
        "  <allowFormCustomize>true</allowFormCustomize>\n",
        "  <extInfo/>\n",
        "</form:Form>\n",
    )
}

fn edt_form_without_permission() -> &'static str {
    concat!(
        "<form:Form xmlns:form=\"http://g5.1c.ru/v8/dt/form\">\n",
        "  <items/>\n",
        "  <allowFormCustomize>false</allowFormCustomize>\n",
        "  <extInfo/>\n",
        "</form:Form>\n",
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
        .join("disable-form-change-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary disable form change test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
