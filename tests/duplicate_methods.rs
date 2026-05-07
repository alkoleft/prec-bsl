use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::bsl_checkers::DUPLICATE_METHODS_RULE;
use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};

#[test]
fn duplicate_methods_reports_all_duplicate_procedure_and_function_definitions_with_spans() {
    let repo = temp_repo("reports_duplicates");
    let repo_path = PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl");
    let input = concat!(
        "Процедура Повтор()\n",
        "КонецПроцедуры\n",
        "\n",
        "Функция Уникальная()\n",
        "    Возврат 1;\n",
        "КонецФункции\n",
        "\n",
        "Функция повтор()\n",
        "    Возврат 2;\n",
        "КонецФункции\n",
    );
    write_file(repo.join(&repo_path), input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();

    let report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &duplicate_methods_config(),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(fs::read_to_string(repo.join(&repo_path)).unwrap(), input);
    assert_eq!(report.results.len(), 2);
    assert!(report.results.iter().all(|result| {
        result.rule_id == DUPLICATE_METHODS_RULE
            && result.path == repo_path
            && result.status == ScenarioResultStatus::HardFailure
            && result
                .message
                .starts_with("duplicate procedure or function definition:")
            && result.source_span.is_some()
    }));
    assert_eq!(
        span_text(input, report.results[0].source_span.unwrap()),
        "Повтор"
    );
    assert_eq!(
        span_text(input, report.results[1].source_span.unwrap()),
        "повтор"
    );
    assert!(report.modified_paths().is_empty());
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn duplicate_methods_accepts_unique_procedure_and_function_names() {
    let repo = temp_repo("accepts_unique");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!(
        "Процедура Подготовить()\n",
        "КонецПроцедуры\n",
        "\n",
        "Функция Рассчитать()\n",
        "    Возврат 1;\n",
        "КонецФункции\n",
    );
    write_file(repo.join(&repo_path), input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();

    let report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &duplicate_methods_config(),
            files: vec![file],
            mode: PipelineMode::ExecRules,
        },
    );

    assert!(report.results.is_empty());
    assert_eq!(report.exec_rules_exit_code(), 0);
}

#[test]
fn duplicate_methods_skips_non_bsl_files() {
    let repo = temp_repo("skips_non_bsl");
    let repo_path = PathBuf::from("src/Object.mdo");
    write_file(repo.join(&repo_path), "<mdclass:CommonModule/>\n");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();

    let report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &duplicate_methods_config(),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, DUPLICATE_METHODS_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
    assert_eq!(
        report.results[0].message,
        "scenario handles only BSL modules"
    );
    assert_eq!(report.hook_exit_code(), 0);
}

fn duplicate_methods_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПроверкаДублейПроцедурИФункций.os"]
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
        .join("duplicate-methods-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary duplicate methods test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
