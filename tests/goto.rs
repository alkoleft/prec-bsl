use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::bsl_checkers::FORBID_GOTO_RULE;
use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};

#[test]
fn goto_checker_reports_russian_and_english_goto_with_source_spans() {
    let repo = temp_repo("reports_goto");
    let repo_path = PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl");
    let input = concat!(
        "Процедура Тест()\n",
        "    Перейти ~Метка;\n",
        "    goto ~Other;\n",
        "~Метка:\n",
        "~Other:\n",
        "КонецПроцедуры\n",
    );
    write_file(repo.join(&repo_path), input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = goto_config();

    let report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(fs::read_to_string(repo.join(&repo_path)).unwrap(), input);
    assert_eq!(report.results.len(), 2);
    assert!(report.results.iter().all(|result| {
        result.rule_id == FORBID_GOTO_RULE
            && result.status == ScenarioResultStatus::HardFailure
            && result.message == "goto statement is forbidden"
            && result.source_span.is_some()
    }));
    assert_eq!(
        span_text(input, report.results[0].source_span.unwrap()),
        "Перейти"
    );
    assert_eq!(
        span_text(input, report.results[1].source_span.unwrap()),
        "goto"
    );
    assert!(report.modified_paths().is_empty());
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn goto_checker_ignores_comments_and_string_literals() {
    let repo = temp_repo("ignores_text");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!(
        "Процедура Тест()\n",
        "    // Перейти ~Комментарий;\n",
        "    Сообщить(\"goto ~Строка\");\n",
        "КонецПроцедуры\n",
    );
    write_file(repo.join(&repo_path), input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();

    let report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &goto_config(),
            files: vec![file],
            mode: PipelineMode::ExecRules,
        },
    );

    assert!(report.results.is_empty());
    assert_eq!(report.exec_rules_exit_code(), 0);
}

#[test]
fn goto_checker_skips_non_bsl_files() {
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
            config: &goto_config(),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, FORBID_GOTO_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
    assert_eq!(
        report.results[0].message,
        "scenario handles only BSL modules"
    );
    assert_eq!(report.hook_exit_code(), 0);
}

fn goto_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ЗапретИспользованияПерейти.os"]
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
        .join("goto-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary goto test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
