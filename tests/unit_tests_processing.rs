use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioRegistry, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};
use prec_bsl::unit_tests_processing::UNIT_TESTS_PROCESSING_RULE;

#[test]
fn unit_tests_processing_inserts_loader_region_and_is_idempotent() {
    let repo = temp_repo("inserts_loader");
    let repo_path = PathBuf::from("src/tests/ОбщиеМодули/ТестовыйМодуль/Модуль.bsl");
    let input = read_fixture_text(
        "tests/fixtures/golden/ОбработкаЮнитТестов/кириллический_модуль/input.bsl",
    );
    let expected = read_fixture_text(
        "tests/fixtures/golden/ОбработкаЮнитТестов/кириллический_модуль/expected.bsl",
    );
    write_file(repo.join(&repo_path), &input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = unit_tests_processing_config();

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

    let output = fs::read_to_string(repo.join(&repo_path)).unwrap();
    assert_eq!(output, expected);
    assert!(output.starts_with("#Область ТестыAPI\n"));
    assert!(output.contains("ИсполняемыеСценарии.Добавить(\"ПервыйТест\");"));
    assert!(output.contains("ИсполняемыеСценарии.Добавить(\"ВторойТест\");"));
    assert!(!output.contains("ИсполняемыеСценарии.Добавить(\"Служебный\");"));
    assert!(
        output.find("#Область ТестыAPI").unwrap() < output.rfind("#Область Тесты").unwrap(),
        "loader region must be inserted before the test region"
    );
    assert_eq!(first_report.results.len(), 1);
    assert_eq!(first_report.results[0].rule_id, UNIT_TESTS_PROCESSING_RULE);
    assert_eq!(
        first_report.results[0].status,
        ScenarioResultStatus::Modified
    );
    assert_eq!(
        first_report.results[0].message,
        "updated unit test loader method"
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
    assert_eq!(fs::read_to_string(repo.join(&repo_path)).unwrap(), expected);
    assert_eq!(second_report.hook_exit_code(), 0);
}

#[test]
fn unit_tests_processing_replaces_existing_loader_region() {
    let repo = temp_repo("replaces_loader");
    let repo_path = PathBuf::from("src/tests/Модуль.bsl");
    let input = concat!(
        "#Область ТестыAPI\n",
        "Функция ИсполняемыеСценарии(ДополнительныеПараметры = Неопределено) Экспорт\n",
        "\tИсполняемыеСценарии = Новый Массив;\n",
        "\tИсполняемыеСценарии.Добавить(\"СтарыйТест\");\n",
        "\tВозврат ИсполняемыеСценарии;\n",
        "КонецФункции\n",
        "#КонецОбласти\n",
        "\n",
        "// @unit-test: actual\n",
        "Процедура НовыйТест() Экспорт\n",
        "КонецПроцедуры\n",
    );
    write_file(repo.join(&repo_path), input);

    let report = run_for_file(&repo, repo_path.clone());
    let output = fs::read_to_string(repo.join(&repo_path)).unwrap();
    let second_report = run_for_file(&repo, repo_path.clone());

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Modified);
    assert_eq!(output.matches("#Область ТестыAPI").count(), 1);
    assert!(!output.contains("СтарыйТест"));
    assert!(output.contains("ИсполняемыеСценарии.Добавить(\"НовыйТест\");"));
    assert!(second_report.results.is_empty());
    assert_eq!(fs::read_to_string(repo.join(&repo_path)).unwrap(), output);
}

#[test]
fn unit_tests_processing_replaces_existing_loader_region_with_nested_regions() {
    let repo = temp_repo("replaces_nested_loader");
    let repo_path = PathBuf::from("src/tests/Модуль.bsl");
    let input = concat!(
        "#Область ТестыAPI\n",
        "#Область Внутренняя\n",
        "Процедура Служебный() Экспорт\n",
        "КонецПроцедуры\n",
        "#КонецОбласти\n",
        "Функция ИсполняемыеСценарии(ДополнительныеПараметры = Неопределено) Экспорт\n",
        "\tИсполняемыеСценарии = Новый Массив;\n",
        "\tИсполняемыеСценарии.Добавить(\"СтарыйТест\");\n",
        "\tВозврат ИсполняемыеСценарии;\n",
        "КонецФункции\n",
        "#КонецОбласти\n",
        "\n",
        "// @unit-test: actual\n",
        "Процедура НовыйТест() Экспорт\n",
        "КонецПроцедуры\n",
    );
    write_file(repo.join(&repo_path), input);

    let report = run_for_file(&repo, repo_path.clone());
    let output = fs::read_to_string(repo.join(&repo_path)).unwrap();
    let second_report = run_for_file(&repo, repo_path.clone());

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Modified);
    assert_eq!(output.matches("#Область ТестыAPI").count(), 1);
    assert!(!output.contains("#Область Внутренняя"));
    assert!(!output.contains("СтарыйТест"));
    assert!(output.contains("ИсполняемыеСценарии.Добавить(\"НовыйТест\");"));
    assert!(second_report.results.is_empty());
    assert_eq!(fs::read_to_string(repo.join(&repo_path)).unwrap(), output);
}

#[test]
fn unit_tests_processing_does_not_generate_loader_without_annotated_exported_methods() {
    let repo = temp_repo("no_annotated_methods");
    let repo_path = PathBuf::from("src/tests/Модуль.bsl");
    let input = concat!(
        "// @unit-test: separated\n",
        "\n",
        "Процедура НеТест() Экспорт\n",
        "КонецПроцедуры\n",
        "\n",
        "Процедура ОбычныйМетод() Экспорт\n",
        "КонецПроцедуры\n",
    );
    write_file(repo.join(&repo_path), input);

    let report = run_for_file(&repo, repo_path.clone());

    assert!(report.results.is_empty());
    assert_eq!(fs::read_to_string(repo.join(&repo_path)).unwrap(), input);
    assert_eq!(report.exec_rules_exit_code(), 0);
}

#[test]
fn unit_tests_processing_reports_parse_errors_without_modifying_file() {
    let repo = temp_repo("parse_errors");
    let repo_path = PathBuf::from("src/tests/Модуль.bsl");
    let input = concat!(
        "// @unit-test: broken\n",
        "Процедура Сломанный() Экспорт\n",
        "    Если Истина Тогда\n",
    );
    write_file(repo.join(&repo_path), input);

    let report = run_for_file(&repo, repo_path.clone());

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, UNIT_TESTS_PROCESSING_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].message,
        "BSL syntax errors prevent unit test loader update"
    );
    assert_eq!(fs::read_to_string(repo.join(&repo_path)).unwrap(), input);
    assert_eq!(report.exec_rules_exit_code(), 1);
}

#[test]
fn unit_tests_processing_skips_non_test_paths_and_non_bsl_files() {
    let repo = temp_repo("skips");
    let bsl_path = PathBuf::from("src/ОбщиеМодули/Модуль.bsl");
    let mdo_path = PathBuf::from("src/tests/Object.mdo");
    write_file(
        repo.join(&bsl_path),
        "// @unit-test: smoke\nПроцедура Тест() Экспорт\nКонецПроцедуры\n",
    );
    write_file(repo.join(&mdo_path), "<mdclass:CommonModule/>\n");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let files = vec![
        classify_repo_path(&roots, bsl_path.clone(), None).unwrap(),
        classify_repo_path(&roots, mdo_path.clone(), None).unwrap(),
    ];

    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &unit_tests_processing_config(),
            files,
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(report.results.len(), 2);
    assert!(report.results.iter().all(|result| {
        result.rule_id == UNIT_TESTS_PROCESSING_RULE
            && result.status == ScenarioResultStatus::Skipped
    }));
    assert_eq!(
        report.results[0].message,
        "scenario handles only BSL modules inside tests directory"
    );
    assert_eq!(
        report.results[1].message,
        "scenario handles only BSL modules"
    );
    assert_eq!(report.hook_exit_code(), 0);
}

fn run_for_file(repo: &Path, repo_path: PathBuf) -> prec_bsl::scenario_pipeline::PipelineReport {
    let roots = resolve_source_roots(repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();
    run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: repo,
            source_roots: &roots,
            config: &unit_tests_processing_config(),
            files: vec![file],
            mode: PipelineMode::ExecRules,
        },
    )
}

fn unit_tests_processing_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ОбработкаЮнитТестов.os"]
            }
        }"#,
    )
    .unwrap()
}

fn read_fixture_text(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path).unwrap()
}

fn temp_repo(test_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after UNIX_EPOCH")
        .as_nanos();
    let path = std::env::current_dir()
        .expect("current dir must be available")
        .join("target")
        .join("unit-tests-processing-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary unit test processing repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
