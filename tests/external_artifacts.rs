use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::external_artifacts::{
    EXTERNAL_ARTIFACTS_RULE, ExternalArtifactBoundary, ExternalArtifactSettings,
    discover_platform_executable_in_paths, evaluate_external_artifact_boundary,
};
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioRegistry, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{
    SourceFileKind, classify_path, classify_repo_path, resolve_source_roots,
};

#[test]
fn external_artifacts_report_missing_platform_dependency_without_mutating_file() {
    let repo = temp_repo("missing_platform");
    let repo_path = PathBuf::from("src/External/Отчет.epf");
    write_file(repo.join(&repo_path), "external artifact bytes");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = external_artifacts_config(
        r#""ИспользоватьНастройкиПоУмолчанию": false,
           "ВерсияПлатформы": "99.99.99.99""#,
    );

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

    assert_eq!(classify_path(&repo_path), SourceFileKind::ExternalArtifact);
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        "external artifact bytes"
    );
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, EXTERNAL_ARTIFACTS_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert!(
        report.results[0]
            .message
            .contains("1C platform executable is required")
    );
    assert!(report.results[0].message.contains("99.99.99.99"));
    assert!(report.modified_paths().is_empty());
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn external_artifacts_skip_non_external_files() {
    let repo = temp_repo("skip_non_external");
    let repo_path = PathBuf::from("src/CommonModules/Модуль/Module.bsl");
    write_file(repo.join(&repo_path), "Процедура Тест()\nКонецПроцедуры\n");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();
    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &external_artifacts_config(""),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, EXTERNAL_ARTIFACTS_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
    assert_eq!(
        report.results[0].message,
        "scenario handles only external report, processing, and extension artifacts"
    );
    assert_eq!(report.hook_exit_code(), 0);
}

#[test]
fn external_artifacts_invalid_settings_do_not_block_non_external_files() {
    let repo = temp_repo("invalid_settings_non_external");
    let repo_path = PathBuf::from("src/CommonModules/Модуль/Module.bsl");
    write_file(repo.join(&repo_path), "Процедура Тест()\nКонецПроцедуры\n");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();
    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &external_artifacts_config(r#""ВерсияПлатформы": 8320"#),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
    assert_eq!(report.hook_exit_code(), 0);
}

#[test]
fn external_artifacts_settings_validate_shapes() {
    let invalid_defaults = serde_json::json!({
        "ИспользоватьНастройкиПоУмолчанию": "false",
        "ВерсияПлатформы": "8.3.20.1996"
    });
    let invalid_version = serde_json::json!({
        "ИспользоватьНастройкиПоУмолчанию": false,
        "ВерсияПлатформы": 8320
    });
    let explicit_version = serde_json::json!({
        "ИспользоватьНастройкиПоУмолчанию": false,
        "ВерсияПлатформы": " 8.3.20.1996 "
    });
    let default_settings = serde_json::json!({
        "ИспользоватьНастройкиПоУмолчанию": true,
        "ВерсияПлатформы": "8.3.20.1996"
    });

    assert!(matches!(
        ExternalArtifactSettings::from_settings(Some(&invalid_defaults)),
        Err(message) if message.contains("ИспользоватьНастройкиПоУмолчанию must be a boolean")
    ));
    assert!(matches!(
        ExternalArtifactSettings::from_settings(Some(&invalid_version)),
        Err(message) if message.contains("ВерсияПлатформы must be a string")
    ));
    assert_eq!(
        ExternalArtifactSettings::from_settings(Some(&explicit_version)).unwrap(),
        ExternalArtifactSettings {
            platform_version: Some("8.3.20.1996".to_owned())
        }
    );
    assert_eq!(
        ExternalArtifactSettings::from_settings(Some(&default_settings)).unwrap(),
        ExternalArtifactSettings {
            platform_version: None
        }
    );
}

#[test]
fn external_artifacts_boundary_reports_discovered_runtime_without_executing_it() {
    let settings = ExternalArtifactSettings {
        platform_version: None,
    };
    let result = evaluate_external_artifact_boundary(
        Path::new("src/External/Расширение.cfe"),
        SourceFileKind::ExternalArtifact,
        &settings,
        Some(Path::new("/opt/1cv8/8.3.20.1996/1cv8")),
    );

    assert!(matches!(
        result,
        ExternalArtifactBoundary::Failed(message)
            if message.contains("/LoadCfg followed by /DumpConfigToFiles -Extension")
                && message.contains("was not run")
    ));
}

#[test]
fn external_artifacts_platform_discovery_honors_required_version_path_fragment() {
    let repo = temp_repo("platform_discovery");
    let bin_dir = repo.join("8.3.20.1996/bin");
    let executable = bin_dir.join("1cv8");
    write_file(&executable, "#!/bin/sh\n");
    make_executable(&executable);

    assert_eq!(
        discover_platform_executable_in_paths([bin_dir.clone()], Some("8.3.20.1996")),
        Some(executable)
    );
    assert_eq!(
        discover_platform_executable_in_paths([bin_dir], Some("8.3.21.0000")),
        None
    );
}

fn external_artifacts_config(settings: &str) -> prec_bsl::config::ResolvedConfig {
    let settings = if settings.is_empty() {
        "{}".to_owned()
    } else {
        format!("{{{settings}}}")
    };
    parse_config_str(&format!(
        r#"{{
            "Precommt4onecСценарии": {{
                "ГлобальныеСценарии": ["{EXTERNAL_ARTIFACTS_RULE}.os"],
                "НастройкиСценариев": {{
                    "{EXTERNAL_ARTIFACTS_RULE}": {settings}
                }}
            }}
        }}"#
    ))
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
        .join("external-artifact-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary external-artifact test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;

    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}
