use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::bsl_checkers::REGIONS_RULE;
use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioRegistry, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};

#[test]
fn regions_checker_accepts_valid_nested_regions() {
    let repo = temp_repo("accepts_valid_nested");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!(
        "#Область Служебный\n",
        "#Область Вложенный\n",
        "Процедура Тест()\n",
        "КонецПроцедуры\n",
        "#КонецОбласти\n",
        "#КонецОбласти\n",
    );
    write_file(repo.join(&repo_path), input);

    let report = run_regions(&repo, repo_path, PipelineMode::ExecRules);

    assert!(report.results.is_empty());
    assert_eq!(report.exec_rules_exit_code(), 0);
}

#[test]
fn regions_checker_reports_missing_end_region_with_span() {
    let repo = temp_repo("missing_end");
    let repo_path = PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl");
    let input = concat!(
        "#Область Служебный\n",
        "Процедура Тест()\n",
        "КонецПроцедуры\n",
    );
    write_file(repo.join(&repo_path), input);

    let report = run_regions(&repo, repo_path.clone(), PipelineMode::Hook);

    assert_eq!(fs::read_to_string(repo.join(&repo_path)).unwrap(), input);
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, REGIONS_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].message,
        "invalid region directive: missing #КонецОбласти"
    );
    assert_eq!(
        span_text(input, report.results[0].source_span.unwrap()),
        "#Область"
    );
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn regions_checker_reports_unmatched_end_region() {
    let repo = temp_repo("unmatched_end");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!("#КонецОбласти\n", "Процедура Тест()\n", "КонецПроцедуры\n",);
    write_file(repo.join(&repo_path), input);

    let report = run_regions(&repo, repo_path, PipelineMode::Hook);

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, REGIONS_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(report.results[0].message, "invalid region directive");
    assert_eq!(
        span_text(input, report.results[0].source_span.unwrap()),
        "#КонецОбласти"
    );
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn regions_checker_balances_case_insensitive_region_directives() {
    let repo = temp_repo("case_insensitive");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!(
        "#область Служебный\n",
        "Процедура Тест()\n",
        "КонецПроцедуры\n",
    );
    write_file(repo.join(&repo_path), input);

    let report = run_regions(&repo, repo_path, PipelineMode::Hook);

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, REGIONS_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].message,
        "invalid region directive: missing #КонецОбласти"
    );
    assert_eq!(
        span_text(input, report.results[0].source_span.unwrap()),
        "#область"
    );
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn regions_checker_balances_english_region_directives() {
    let repo = temp_repo("english_region");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!(
        "#Region Service\n",
        "Процедура Тест()\n",
        "КонецПроцедуры\n",
        "#EndRegion\n",
    );
    write_file(repo.join(&repo_path), input);

    let report = run_regions(&repo, repo_path, PipelineMode::ExecRules);

    assert!(report.results.is_empty());
    assert_eq!(report.exec_rules_exit_code(), 0);
}

#[test]
fn regions_checker_reports_missing_region_name() {
    let repo = temp_repo("missing_name");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!("#Область\n", "#КонецОбласти\n");
    write_file(repo.join(&repo_path), input);

    let report = run_regions(&repo, repo_path, PipelineMode::Hook);

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, REGIONS_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].message,
        "invalid region directive: missing identifier"
    );
    assert_eq!(
        span_text(input, report.results[0].source_span.unwrap()),
        "#Область"
    );
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn regions_checker_ignores_ordinary_bsl_parse_errors() {
    let repo = temp_repo("ordinary_bsl_error");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!(
        "#Область Служебный\n",
        "Процедура Тест()\n",
        "    Если Тогда\n",
        "КонецПроцедуры\n",
        "#КонецОбласти\n",
    );
    write_file(repo.join(&repo_path), input);

    let report = run_regions(&repo, repo_path, PipelineMode::ExecRules);

    assert!(report.results.is_empty());
    assert_eq!(report.exec_rules_exit_code(), 0);
}

#[test]
fn regions_checker_ignores_comments_and_string_literals() {
    let repo = temp_repo("ignores_text");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = concat!(
        "Процедура Тест()\n",
        "    // #Область Комментарий\n",
        "    Сообщить(\"#КонецОбласти\");\n",
        "КонецПроцедуры\n",
    );
    write_file(repo.join(&repo_path), input);

    let report = run_regions(&repo, repo_path, PipelineMode::ExecRules);

    assert!(report.results.is_empty());
    assert_eq!(report.exec_rules_exit_code(), 0);
}

#[test]
fn regions_checker_skips_non_bsl_files() {
    let repo = temp_repo("skips_non_bsl");
    let repo_path = PathBuf::from("src/Object.mdo");
    write_file(repo.join(&repo_path), "<mdclass:CommonModule/>\n");

    let report = run_regions(&repo, repo_path, PipelineMode::Hook);

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, REGIONS_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
    assert_eq!(
        report.results[0].message,
        "scenario handles only BSL modules"
    );
    assert_eq!(report.hook_exit_code(), 0);
}

fn run_regions(
    repo: &Path,
    repo_path: PathBuf,
    mode: PipelineMode,
) -> prec_bsl::scenario_pipeline::PipelineReport {
    let roots = resolve_source_roots(repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();

    run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: repo,
            source_roots: &roots,
            config: &regions_config(),
            files: vec![file],
            mode,
        },
    )
}

fn regions_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПроверкаКорректностиОбластей.os"]
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
        .join("regions-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary regions test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory must be created");
    }
    fs::write(path, content).expect("test fixture must be written");
}
