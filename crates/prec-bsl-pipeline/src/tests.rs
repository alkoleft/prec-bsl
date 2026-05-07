use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use prec_bsl_config::parse_config_str;
use prec_bsl_git::StagedStatus;
use prec_bsl_source::{classify_repo_path, resolve_source_roots};

const TRAILING_WHITESPACE: &str = "УдалениеЛишнихКонцевыхПробелов";
const EXTRA_BLANK_LINES: &str = "УдалениеЛишнихПустыхСтрок";

#[test]
fn scenario_pipeline_keeps_configured_order_with_normalized_ids() {
    let repo = temp_repo("configured_order");
    write_file(repo.join("src/Модуль.bsl"), "");
    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, "src/Модуль.bsl", None).unwrap();
    let config = parse_config_str(
        r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": [
                        "УдалениеЛишнихКонцевыхПробелов.os",
                        "ПроверкаНецензурныхСлов",
                        "УдалениеЛишнихПустыхСтрок.os"
                    ],
                    "ОтключенныеСценарии": ["ПроверкаНецензурныхСлов.os"]
                }
            }"#,
    )
    .unwrap();

    let registry = ScenarioRegistry::reference()
        .with_handler(TRAILING_WHITESPACE, hard_failure)
        .with_handler(EXTRA_BLANK_LINES, hard_failure);

    let report = run_pipeline(
        &registry,
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::ExecRules,
        },
    );

    let executed_rules = report
        .results
        .iter()
        .map(|result| result.rule_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(executed_rules, vec![TRAILING_WHITESPACE, EXTRA_BLANK_LINES]);
}

#[test]
fn scenario_pipeline_uses_project_specific_scenario_order() {
    let repo = temp_repo("project_order");
    write_file(repo.join("src/Модуль.bsl"), "");
    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, "src/Модуль.bsl", None).unwrap();
    let config = parse_config_str(
        r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"],
                    "Проекты": {
                        "src": {
                            "ГлобальныеСценарии": ["УдалениеЛишнихПустыхСтрок.os"]
                        }
                    }
                }
            }"#,
    )
    .unwrap();

    let registry = ScenarioRegistry::reference().with_handler(EXTRA_BLANK_LINES, hard_failure);

    let report = run_pipeline(
        &registry,
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::ExecRules,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, EXTRA_BLANK_LINES);
}

#[test]
fn scenario_pipeline_appends_post_processing_files_to_queue_once() {
    let repo = temp_repo("post_processing_queue");
    write_file(repo.join("src/input.bsl"), "");
    write_file(repo.join("src/generated.bsl"), "");
    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, "src/input.bsl", None).unwrap();
    let config = parse_config_str(
        r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                }
            }"#,
    )
    .unwrap();
    let registry =
        ScenarioRegistry::reference().with_handler(TRAILING_WHITESPACE, append_generated_once);

    let report = run_pipeline(
        &registry,
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::ExecRules,
        },
    );

    assert_eq!(
        report.processed_files,
        vec![
            PathBuf::from("src/input.bsl"),
            PathBuf::from("src/generated.bsl")
        ]
    );
}

#[test]
fn scenario_pipeline_distinguishes_result_statuses_and_hook_exit() {
    let repo = temp_repo("statuses_and_exit");
    write_file(repo.join("src/Модуль.bsl"), "");
    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, "src/Модуль.bsl", None).unwrap();
    let config = parse_config_str(
        r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                }
            }"#,
    )
    .unwrap();
    let registry = ScenarioRegistry::reference().with_handler(TRAILING_WHITESPACE, all_statuses);

    let report = run_pipeline(
        &registry,
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    let statuses = report
        .results
        .iter()
        .map(|result| result.status)
        .collect::<Vec<_>>();

    assert_eq!(
        statuses,
        vec![
            ScenarioResultStatus::Modified,
            ScenarioResultStatus::Warning,
            ScenarioResultStatus::HardFailure,
            ScenarioResultStatus::Skipped,
        ]
    );
    assert_eq!(report.critical_results().len(), 1);
    assert_eq!(
        report.modified_paths(),
        vec![PathBuf::from("src/Модуль.bsl")]
    );
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn scenario_pipeline_accumulates_critical_errors_after_traversal() {
    let repo = temp_repo("critical_after_traversal");
    write_file(repo.join("src/one.bsl"), "");
    write_file(repo.join("src/two.bsl"), "");
    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let files = ["src/one.bsl", "src/two.bsl"]
        .into_iter()
        .map(|path| classify_repo_path(&roots, path, None).unwrap())
        .collect::<Vec<_>>();
    let config = parse_config_str(
        r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                }
            }"#,
    )
    .unwrap();
    let registry = ScenarioRegistry::reference().with_handler(TRAILING_WHITESPACE, hard_failure);

    let report = run_pipeline(
        &registry,
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files,
            mode: PipelineMode::ExecRules,
        },
    );

    assert_eq!(
        report.processed_files,
        vec![PathBuf::from("src/one.bsl"), PathBuf::from("src/two.bsl")]
    );
    assert_eq!(report.critical_results().len(), 2);
    assert_eq!(report.exec_rules_exit_code(), 1);
}

#[test]
fn scenario_pipeline_reports_unregistered_enabled_scenario_as_unsupported() {
    let repo = temp_repo("unregistered");
    write_file(repo.join("src/Модуль.bsl"), "");
    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, "src/Модуль.bsl", None).unwrap();
    let config = parse_config_str(
        r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                }
            }"#,
    )
    .unwrap();

    let report = run_pipeline(
        &ScenarioRegistry::empty(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::ExecRules,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Unsupported);
    assert!(
        report.results[0]
            .message
            .contains("scenario is not registered")
    );
}

#[test]
fn scenario_pipeline_skips_deleted_files_without_deleted_file_capability() {
    let repo = temp_repo("deleted_file_skip");
    fs::create_dir_all(repo.join("src")).unwrap();
    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file =
        classify_repo_path(&roots, "src/Удаленный.bsl", Some(StagedStatus::Deleted)).unwrap();
    let config = parse_config_str(
        r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                }
            }"#,
    )
    .unwrap();
    let registry = ScenarioRegistry::reference().with_handler(TRAILING_WHITESPACE, hard_failure);

    let report = run_pipeline(
        &registry,
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
    assert_eq!(
        report.results[0].message,
        "scenario does not handle deleted files"
    );
}

#[test]
fn scenario_pipeline_passes_deleted_files_to_explicit_deleted_file_handlers() {
    let repo = temp_repo("deleted_file_handler");
    fs::create_dir_all(repo.join("src")).unwrap();
    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file =
        classify_repo_path(&roots, "src/Удаленный.bsl", Some(StagedStatus::Deleted)).unwrap();
    let config = parse_config_str(
        r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                }
            }"#,
    )
    .unwrap();
    let registry =
        ScenarioRegistry::reference().with_deleted_file_handler(TRAILING_WHITESPACE, hard_failure);

    let report = run_pipeline(
        &registry,
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(report.hook_exit_code(), 1);
}

fn append_generated_once(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    if context.file.repo_path == Path::new("src/input.bsl") {
        ScenarioRun::clean().with_post_processing_path("src/generated.bsl")
    } else {
        ScenarioRun::clean()
    }
}

fn all_statuses(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    ScenarioRun {
        results: vec![
            ScenarioResult::modified(context.rule_id, context.file.repo_path.clone(), "modified"),
            ScenarioResult::warning(context.rule_id, context.file.repo_path.clone(), "warning"),
            ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                "hard failure",
            ),
            ScenarioResult::skipped(context.rule_id, context.file.repo_path.clone(), "skip"),
        ],
        post_processing_paths: Vec::new(),
    }
}

fn hard_failure(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    ScenarioRun::single(ScenarioResult::hard_failure(
        context.rule_id,
        context.file.repo_path.clone(),
        "hard failure",
    ))
}

fn temp_repo(test_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after UNIX_EPOCH")
        .as_nanos();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join("scenario-pipeline-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary scenario-pipeline test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
