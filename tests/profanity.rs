use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioRegistry, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};
use prec_bsl::text_checkers::PROFANITY_RULE;

#[test]
fn profanity_reports_configured_dictionary_matches_without_modifying_file() {
    let repo = temp_repo("configured_dictionary");
    let repo_path = PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl");
    let input = read_fixture_text(
        "tests/fixtures/golden/ПроверкаНецензурныхСлов/кириллический_модуль/input.bsl",
    );
    write_file(repo.join(&repo_path), &input);
    write_file(repo.join("НецензурныеСлова.txt"), "плохоеСлово\n");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = profanity_config(r#""ФайлСНецензурнымиСловами": "НецензурныеСлова.txt""#);

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

    assert_eq!(fs::read_to_string(repo.join(&repo_path)).unwrap(), input);
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, PROFANITY_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Warning);
    assert_eq!(
        report.results[0].message,
        "matched dictionary word 'плохоеСлово' at line 2"
    );
    assert!(report.modified_paths().is_empty());
    assert_eq!(report.hook_exit_code(), 0);
}

#[test]
fn profanity_uses_default_dictionary_when_setting_is_absent() {
    let repo = temp_repo("default_dictionary");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    write_file(repo.join(&repo_path), "Сообщить(\"ПЛОХОЕСЛОВО\");\n");
    write_file(repo.join("НецензурныеСлова.txt"), "плохоеСлово\n");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();
    let config = parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПроверкаНецензурныхСлов.os"]
            }
        }"#,
    )
    .unwrap();

    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::ExecRules,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Warning);
    assert_eq!(
        report.results[0].message,
        "matched dictionary word 'плохоеСлово' at line 1"
    );
}

#[test]
fn profanity_skips_when_default_dictionary_is_absent() {
    let repo = temp_repo("missing_default_dictionary");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    write_file(repo.join(&repo_path), "Сообщить(\"плохоеСлово\");\n");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();
    let config = parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПроверкаНецензурныхСлов.os"]
            }
        }"#,
    )
    .unwrap();

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

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
    assert_eq!(
        report.results[0].message,
        "profanity dictionary is not configured or found: НецензурныеСлова.txt"
    );
    assert_eq!(report.hook_exit_code(), 0);
}

#[test]
fn profanity_fails_when_configured_dictionary_is_missing() {
    let repo = temp_repo("missing_configured_dictionary");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    write_file(repo.join(&repo_path), "Сообщить(\"плохоеСлово\");\n");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();
    let config = profanity_config(r#""ФайлСНецензурнымиСловами": "missing.txt""#);

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

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert!(
        report.results[0]
            .message
            .contains("failed to read configured profanity dictionary missing.txt")
    );
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn profanity_fails_when_dictionary_setting_is_not_a_string() {
    let repo = temp_repo("invalid_dictionary_setting");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    write_file(repo.join(&repo_path), "Сообщить(\"плохоеСлово\");\n");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();
    let config = profanity_config(r#""ФайлСНецензурнымиСловами": 1"#);

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

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].message,
        "dictionary setting ФайлСНецензурнымиСловами must be a string"
    );
}

#[test]
fn profanity_fails_when_dictionary_setting_is_empty() {
    let repo = temp_repo("empty_dictionary_setting");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    write_file(repo.join(&repo_path), "Сообщить(\"плохоеСлово\");\n");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();
    let config = profanity_config(r#""ФайлСНецензурнымиСловами": " ""#);

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

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].message,
        "dictionary setting ФайлСНецензурнымиСловами must not be empty"
    );
}

fn profanity_config(setting: &str) -> prec_bsl::config::ResolvedConfig {
    parse_config_str(&format!(
        r#"{{
            "Precommt4onecСценарии": {{
                "ГлобальныеСценарии": ["ПроверкаНецензурныхСлов.os"],
                "НастройкиСценариев": {{
                    "ПроверкаНецензурныхСлов": {{{setting}}}
                }}
            }}
        }}"#,
    ))
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
        .join("profanity-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary profanity test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
