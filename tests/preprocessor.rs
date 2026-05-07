use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::bsl_checkers::PREPROCESSOR_RULE;
use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};

#[test]
fn preprocessor_checker_accepts_valid_directive_blocks() {
    let repo = temp_repo("accepts_valid");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!(
        "#Если Сервер Тогда\n",
        "    Сообщить(\"server\");\n",
        "#ИначеЕсли Клиент Тогда\n",
        "    Сообщить(\"client\");\n",
        "#Иначе\n",
        "    Сообщить(\"other\");\n",
        "#КонецЕсли\n",
    );
    write_file(repo.join(&repo_path), input);

    let report = run_preprocessor(&repo, repo_path, PipelineMode::ExecRules);

    assert!(report.results.is_empty());
    assert_eq!(report.exec_rules_exit_code(), 0);
}

#[test]
fn preprocessor_checker_reports_incomplete_if_directive_with_span() {
    let repo = temp_repo("incomplete_if");
    let repo_path = PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl");
    let input = concat!("#Если Сервер Тогда\n", "    Сообщить(\"server\");\n",);
    write_file(repo.join(&repo_path), input);

    let report = run_preprocessor(&repo, repo_path, PipelineMode::Hook);

    assert_eq!(
        fs::read_to_string(repo.join("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl")).unwrap(),
        input
    );
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, PREPROCESSOR_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].message,
        "invalid preprocessor instruction: missing #КонецЕсли"
    );
    assert!(report.results[0].source_span.is_some());
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn preprocessor_checker_balances_case_insensitive_if_directives() {
    let repo = temp_repo("case_insensitive_if");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!("#если Сервер Тогда\n", "Сообщить(\"server\");\n",);
    write_file(repo.join(&repo_path), input);

    let report = run_preprocessor(&repo, repo_path, PipelineMode::Hook);

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, PREPROCESSOR_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].message,
        "invalid preprocessor instruction: missing #КонецЕсли"
    );
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn preprocessor_checker_reports_missing_directive_expression() {
    let repo = temp_repo("missing_expression");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!("#Если Тогда\n", "#КонецЕсли\n");
    write_file(repo.join(&repo_path), input);

    let report = run_preprocessor(&repo, repo_path, PipelineMode::Hook);

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, PREPROCESSOR_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].message,
        "invalid preprocessor instruction: missing identifier"
    );
    assert!(report.results[0].source_span.is_some());
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn preprocessor_checker_reports_unmatched_else_directive() {
    let repo = temp_repo("unmatched_else");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!("#Иначе\n", "Сообщить(\"other\");\n");
    write_file(repo.join(&repo_path), input);

    let report = run_preprocessor(&repo, repo_path, PipelineMode::Hook);

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, PREPROCESSOR_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].message,
        "invalid preprocessor instruction"
    );
    assert_eq!(
        span_text(input, report.results[0].source_span.unwrap()),
        "#Иначе"
    );
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn preprocessor_checker_reports_invalid_if_branch_ordering() {
    let cases = [
        (
            "duplicate_else",
            concat!(
                "#Если Сервер Тогда\n",
                "#Иначе\n",
                "#Иначе\n",
                "#КонецЕсли\n",
            ),
            "#Иначе",
        ),
        (
            "elsif_after_else",
            concat!(
                "#Если Сервер Тогда\n",
                "#Иначе\n",
                "#ИначеЕсли Клиент Тогда\n",
                "#КонецЕсли\n",
            ),
            "#ИначеЕсли",
        ),
        ("unmatched_endif", "#КонецЕсли\n", "#КонецЕсли"),
    ];

    for (case_name, input, expected_span_text) in cases {
        let repo = temp_repo(case_name);
        let repo_path = PathBuf::from("src/Модуль.bsl");
        write_file(repo.join(&repo_path), input);

        let report = run_preprocessor(&repo, repo_path, PipelineMode::Hook);

        assert_eq!(report.results.len(), 1, "{case_name}");
        assert_eq!(report.results[0].rule_id, PREPROCESSOR_RULE);
        assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
        assert_eq!(
            span_text(input, report.results[0].source_span.unwrap()),
            expected_span_text
        );
        assert_eq!(report.hook_exit_code(), 1);
    }
}

#[test]
fn preprocessor_checker_reports_malformed_annotation() {
    let repo = temp_repo("malformed_annotation");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!("&Перед\n", "Процедура Тест()\n", "КонецПроцедуры\n",);
    write_file(repo.join(&repo_path), input);

    let report = run_preprocessor(&repo, repo_path, PipelineMode::Hook);

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, PREPROCESSOR_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].message,
        "invalid preprocessor instruction"
    );
    assert!(report.results[0].source_span.is_some());
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn preprocessor_checker_ignores_ordinary_bsl_parse_errors() {
    let repo = temp_repo("ordinary_bsl_error");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!("Процедура Тест()\n", "    Если Тогда\n", "КонецПроцедуры\n",);
    write_file(repo.join(&repo_path), input);

    let report = run_preprocessor(&repo, repo_path, PipelineMode::ExecRules);

    assert!(report.results.is_empty());
    assert_eq!(report.exec_rules_exit_code(), 0);
}

#[test]
fn preprocessor_checker_ignores_comments_and_string_literals() {
    let repo = temp_repo("ignores_text");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!(
        "Процедура Тест()\n",
        "    // #Если Сервер Тогда\n",
        "    Сообщить(\"#КонецЕсли\");\n",
        "КонецПроцедуры\n",
    );
    write_file(repo.join(&repo_path), input);

    let report = run_preprocessor(&repo, repo_path, PipelineMode::ExecRules);

    assert!(report.results.is_empty());
    assert_eq!(report.exec_rules_exit_code(), 0);
}

#[test]
fn preprocessor_checker_skips_non_bsl_files() {
    let repo = temp_repo("skips_non_bsl");
    let repo_path = PathBuf::from("src/Object.mdo");
    write_file(repo.join(&repo_path), "<mdclass:CommonModule/>\n");

    let report = run_preprocessor(&repo, repo_path, PipelineMode::Hook);

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, PREPROCESSOR_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
    assert_eq!(
        report.results[0].message,
        "scenario handles only BSL modules"
    );
    assert_eq!(report.hook_exit_code(), 0);
}

fn run_preprocessor(
    repo: &Path,
    repo_path: PathBuf,
    mode: PipelineMode,
) -> prec_bsl::scenario_pipeline::PipelineReport {
    let roots = resolve_source_roots(repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();

    run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: repo,
            source_roots: &roots,
            config: &preprocessor_config(),
            files: vec![file],
            mode,
        },
    )
}

fn preprocessor_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПроверкаКорректностиИнструкцийПрепроцессора.os"]
            }
        }"#,
    )
    .unwrap()
}

fn span_text(input: &str, span: prec_bsl::scenario_pipeline::SourceSpan) -> &str {
    &input[span.start_byte..span.end_byte]
}

fn temp_repo(test_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after UNIX_EPOCH")
        .as_nanos();
    let path = std::env::current_dir()
        .expect("current dir must be available")
        .join("target")
        .join("preprocessor-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary preprocessor test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
