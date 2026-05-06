use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioRegistry, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};
use prec_bsl::xml_forms::{
    FormElement, XML_FORM_CORRECTION_RULE, XmlFormCorrection, correct_edt_form_text,
};

#[test]
fn xml_forms_corrects_duplicate_ids_and_is_idempotent() {
    let repo = temp_repo("duplicate_ids");
    let repo_path = PathBuf::from("src/DataProcessors/Обработка/Forms/Форма/Form.form");
    write_file(repo.join(&repo_path), duplicate_ids_form());

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = xml_forms_config();

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
        corrected_duplicate_ids_form()
    );
    assert_eq!(first_report.results.len(), 1);
    assert_eq!(first_report.results[0].rule_id, XML_FORM_CORRECTION_RULE);
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
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        corrected_duplicate_ids_form()
    );
    assert_eq!(second_report.hook_exit_code(), 0);
}

#[test]
fn xml_forms_preserves_base_form_ids_and_reassigns_non_borrowed_duplicates() {
    let repo = temp_repo("base_form");
    let form_path = PathBuf::from("src/Extensions/Расширение/Forms/Форма/Form.form");
    let base_path = PathBuf::from("src/Extensions/Расширение/Forms/Форма/BaseForm/Form.form");
    write_file(repo.join(&base_path), base_form());
    write_file(
        repo.join(&form_path),
        extension_form_with_duplicate_base_id(),
    );

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, form_path.clone(), None).unwrap();
    let config = xml_forms_config();

    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(
        fs::read_to_string(repo.join(&base_path)).unwrap(),
        base_form()
    );
    assert_eq!(
        fs::read_to_string(repo.join(&form_path)).unwrap(),
        corrected_extension_form_with_base_id()
    );
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Modified);
    assert_eq!(report.modified_paths(), vec![form_path]);
}

#[test]
fn xml_forms_reports_ambiguous_base_form_matches_as_hard_failure() {
    let current = concat!(
        "<form:Form>\n",
        "  <items>\n",
        "    <name>Общий</name>\n",
        "    <id>1</id>\n",
        "  </items>\n",
        "  <items>\n",
        "    <name>Общий</name>\n",
        "    <id>2</id>\n",
        "  </items>\n",
        "</form:Form>\n",
    );
    let base_elements = vec![FormElement {
        path: "form:Form.items.Общий".to_owned(),
        name: "Общий".to_owned(),
        id: 5,
    }];
    let result = correct_edt_form_text(Path::new("Form.form"), current, Some(&base_elements));

    assert_eq!(
        result,
        XmlFormCorrection::Failed(
            "base form element match is ambiguous: form:Form.items.Общий".to_owned()
        )
    );
}

#[test]
fn xml_forms_reports_malformed_xml_as_hard_failure() {
    let result = correct_edt_form_text(
        Path::new("src/DataProcessors/Обработка/Forms/Форма/Form.form"),
        "<form:Form><items></form:Form>",
        None,
    );

    assert!(
        matches!(result, XmlFormCorrection::Failed(message) if message.contains("failed to parse XML/EDT file"))
    );
}

#[test]
fn xml_forms_corrects_compact_valid_xml_layout() {
    let input = concat!(
        "<form:Form>",
        "<items><name>Первый</name><id>1</id></items>",
        "<items><name>Второй</name><id>1</id></items>",
        "<items><name>Третий</name><id>3</id></items>",
        "</form:Form>",
    );
    let expected = concat!(
        "<form:Form>",
        "<items><name>Первый</name><id>2</id></items>",
        "<items><name>Второй</name><id>1</id></items>",
        "<items><name>Третий</name><id>3</id></items>",
        "</form:Form>",
    );

    let result = correct_edt_form_text(Path::new("Form.form"), input, None);

    assert_eq!(result, XmlFormCorrection::Modified(expected.to_owned()));
}

#[test]
fn xml_forms_reports_modified_sibling_base_form_path() {
    let repo = temp_repo("modified_base_form");
    let form_path = PathBuf::from("src/Extensions/Расширение/Forms/Форма/Form.form");
    let base_path = PathBuf::from("src/Extensions/Расширение/Forms/Форма/BaseForm/Form.form");
    write_file(repo.join(&base_path), base_form_with_duplicate_ids());
    write_file(repo.join(&form_path), extension_form_matching_base());

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, form_path.clone(), None).unwrap();
    let config = xml_forms_config();

    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(
        fs::read_to_string(repo.join(&base_path)).unwrap(),
        corrected_base_form_with_duplicate_ids()
    );
    assert_eq!(
        fs::read_to_string(repo.join(&form_path)).unwrap(),
        corrected_extension_form_matching_base()
    );
    assert_eq!(report.results.len(), 2);
    assert_eq!(report.modified_paths(), vec![base_path, form_path]);
}

#[test]
fn xml_forms_reports_invalid_ids_as_hard_failures() {
    for invalid_id in ["0", "abc", "18446744073709551616"] {
        let input =
            format!("<form:Form><items><name>Поле</name><id>{invalid_id}</id></items></form:Form>");

        let result = correct_edt_form_text(Path::new("Form.form"), &input, None);

        assert!(
            matches!(result, XmlFormCorrection::Failed(message) if message == format!("form element id must be a positive integer: {invalid_id}"))
        );
    }
}

#[test]
fn xml_forms_allocates_free_ids_lazily_across_multiple_duplicate_groups() {
    let input = concat!(
        "<form:Form>",
        "<items><name>A</name><id>1</id></items>",
        "<items><name>B</name><id>1</id></items>",
        "<items><name>C</name><id>2</id></items>",
        "<items><name>D</name><id>2</id></items>",
        "</form:Form>",
    );
    let expected = concat!(
        "<form:Form>",
        "<items><name>A</name><id>3</id></items>",
        "<items><name>B</name><id>1</id></items>",
        "<items><name>C</name><id>4</id></items>",
        "<items><name>D</name><id>2</id></items>",
        "</form:Form>",
    );

    let result = correct_edt_form_text(Path::new("Form.form"), input, None);

    assert_eq!(result, XmlFormCorrection::Modified(expected.to_owned()));

    let large_id_input = concat!(
        "<form:Form>",
        "<items><name>A</name><id>1000000000</id></items>",
        "<items><name>B</name><id>1000000000</id></items>",
        "</form:Form>",
    );
    let large_id_expected = concat!(
        "<form:Form>",
        "<items><name>A</name><id>1</id></items>",
        "<items><name>B</name><id>1000000000</id></items>",
        "</form:Form>",
    );

    let result = correct_edt_form_text(Path::new("Form.form"), large_id_input, None);

    assert_eq!(
        result,
        XmlFormCorrection::Modified(large_id_expected.to_owned())
    );

    let max_id_input = concat!(
        "<form:Form>",
        "<items><name>A</name><id>1</id></items>",
        "<items><name>B</name><id>18446744073709551615</id></items>",
        "<items><name>C</name><id>18446744073709551615</id></items>",
        "</form:Form>",
    );
    let max_id_expected = concat!(
        "<form:Form>",
        "<items><name>A</name><id>1</id></items>",
        "<items><name>B</name><id>2</id></items>",
        "<items><name>C</name><id>18446744073709551615</id></items>",
        "</form:Form>",
    );

    let result = correct_edt_form_text(Path::new("Form.form"), max_id_input, None);

    assert_eq!(
        result,
        XmlFormCorrection::Modified(max_id_expected.to_owned())
    );
}

#[test]
fn xml_forms_skips_direct_base_form_and_non_form_files() {
    let repo = temp_repo("skips");
    let base_path = PathBuf::from("src/Forms/Форма/BaseForm/Form.form");
    let other_form_path = PathBuf::from("src/Forms/Форма/Вложенная.form");
    write_file(repo.join(&base_path), base_form());
    write_file(repo.join(&other_form_path), base_form());

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let base_file = classify_repo_path(&roots, base_path.clone(), None).unwrap();
    let other_file = classify_repo_path(&roots, other_form_path.clone(), None).unwrap();
    let config = xml_forms_config();

    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![base_file, other_file],
            mode: PipelineMode::Hook,
        },
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

fn duplicate_ids_form() -> &'static str {
    concat!(
        "<form:Form>\n",
        "  <items>\n",
        "    <name>Первый</name>\n",
        "    <id>1</id>\n",
        "  </items>\n",
        "  <items>\n",
        "    <name>Второй</name>\n",
        "    <id>1</id>\n",
        "  </items>\n",
        "  <items>\n",
        "    <name>Третий</name>\n",
        "    <id>3</id>\n",
        "  </items>\n",
        "</form:Form>\n",
    )
}

fn corrected_duplicate_ids_form() -> &'static str {
    concat!(
        "<form:Form>\n",
        "  <items>\n",
        "    <name>Первый</name>\n",
        "    <id>2</id>\n",
        "  </items>\n",
        "  <items>\n",
        "    <name>Второй</name>\n",
        "    <id>1</id>\n",
        "  </items>\n",
        "  <items>\n",
        "    <name>Третий</name>\n",
        "    <id>3</id>\n",
        "  </items>\n",
        "</form:Form>\n",
    )
}

fn base_form() -> &'static str {
    concat!(
        "<form:Form>\n",
        "  <items>\n",
        "    <name>Общий</name>\n",
        "    <id>5</id>\n",
        "  </items>\n",
        "</form:Form>\n",
    )
}

fn base_form_with_duplicate_ids() -> &'static str {
    concat!(
        "<form:Form>\n",
        "  <items>\n",
        "    <name>Общий</name>\n",
        "    <id>9</id>\n",
        "  </items>\n",
        "  <items>\n",
        "    <name>Другой</name>\n",
        "    <id>9</id>\n",
        "  </items>\n",
        "</form:Form>\n",
    )
}

fn corrected_base_form_with_duplicate_ids() -> &'static str {
    concat!(
        "<form:Form>\n",
        "  <items>\n",
        "    <name>Общий</name>\n",
        "    <id>1</id>\n",
        "  </items>\n",
        "  <items>\n",
        "    <name>Другой</name>\n",
        "    <id>9</id>\n",
        "  </items>\n",
        "</form:Form>\n",
    )
}

fn extension_form_with_duplicate_base_id() -> &'static str {
    concat!(
        "<form:Form>\n",
        "  <items>\n",
        "    <name>Общий</name>\n",
        "    <id>1</id>\n",
        "  </items>\n",
        "  <items>\n",
        "    <name>Локальный</name>\n",
        "    <id>5</id>\n",
        "  </items>\n",
        "</form:Form>\n",
    )
}

fn extension_form_matching_base() -> &'static str {
    concat!(
        "<form:Form>\n",
        "  <items>\n",
        "    <name>Общий</name>\n",
        "    <id>2</id>\n",
        "  </items>\n",
        "  <items>\n",
        "    <name>Другой</name>\n",
        "    <id>9</id>\n",
        "  </items>\n",
        "</form:Form>\n",
    )
}

fn corrected_extension_form_matching_base() -> &'static str {
    concat!(
        "<form:Form>\n",
        "  <items>\n",
        "    <name>Общий</name>\n",
        "    <id>1</id>\n",
        "  </items>\n",
        "  <items>\n",
        "    <name>Другой</name>\n",
        "    <id>9</id>\n",
        "  </items>\n",
        "</form:Form>\n",
    )
}

fn corrected_extension_form_with_base_id() -> &'static str {
    concat!(
        "<form:Form>\n",
        "  <items>\n",
        "    <name>Общий</name>\n",
        "    <id>5</id>\n",
        "  </items>\n",
        "  <items>\n",
        "    <name>Локальный</name>\n",
        "    <id>1</id>\n",
        "  </items>\n",
        "</form:Form>\n",
    )
}

fn xml_forms_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["КорректировкаXMLФорм.os"]
            }
        }"#,
    )
    .unwrap()
}

fn temp_repo(test_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after UNIX_EPOCH")
        .as_nanos();
    let path = std::env::current_dir()
        .expect("current dir must be available")
        .join("target")
        .join("xml-forms-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary XML forms test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory must be created");
    }
    fs::write(path, content).expect("test fixture must be written");
}
