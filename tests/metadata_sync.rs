use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::git_index::StagedStatus;
use prec_bsl::metadata_sync::{METADATA_SYNC_RULE, MetadataSyncCheck, check_metadata_sync_text};
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioRegistry, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{SourceFileKind, classify_repo_path, resolve_source_roots};

#[test]
fn metadata_sync_accepts_clean_edt_configuration() {
    let repo = temp_repo("clean_edt");
    let config_path = PathBuf::from("src/Configuration/Configuration.mdo");
    write_file(
        repo.join(&config_path),
        edt_configuration(&["CommonModule.Модуль"]),
    );
    write_file(
        repo.join("src/CommonModules/Модуль/Модуль.mdo"),
        "<mdclass:CommonModule/>",
    );

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, config_path, None).unwrap();
    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &metadata_sync_config(),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert!(report.results.is_empty());
    assert!(report.modified_paths().is_empty());
    assert_eq!(report.hook_exit_code(), 0);
}

#[test]
fn metadata_sync_reports_missing_edt_object_file() {
    let repo = temp_repo("missing_edt_object");
    let config_path = PathBuf::from("src/Configuration/Configuration.mdo");
    write_file(
        repo.join(&config_path),
        edt_configuration(&["CommonModule.Модуль"]),
    );
    fs::create_dir_all(repo.join("src/CommonModules")).unwrap();

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, config_path, None).unwrap();
    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &metadata_sync_config(),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, METADATA_SYNC_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].path,
        PathBuf::from("src/CommonModules/Модуль")
    );
    assert!(report.results[0].message.contains("missing files"));
    assert!(report.modified_paths().is_empty());
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn metadata_sync_reports_stale_edt_object_from_staged_object_file() {
    let repo = temp_repo("stale_staged_edt_object");
    let config_path = PathBuf::from("src/Configuration/Configuration.mdo");
    let object_path = PathBuf::from("src/CommonModules/Лишний/Лишний.mdo");
    write_file(repo.join(&config_path), edt_configuration(&[]));
    write_file(repo.join(&object_path), "<mdclass:CommonModule/>");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, object_path.clone(), Some(StagedStatus::Added)).unwrap();
    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &metadata_sync_config(),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert!(report.results.iter().any(|result| {
        result.path == PathBuf::from("src/CommonModules/Лишний")
            && result
                .message
                .contains("unreferenced metadata object directory")
    }));
    assert!(report.results.iter().any(|result| {
        result.path == object_path && result.message.contains("unreferenced metadata object file")
    }));
    assert!(report.modified_paths().is_empty());
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn metadata_sync_reports_deleted_referenced_object_from_staged_deleted_file() {
    let repo = temp_repo("deleted_referenced_object");
    let config_path = PathBuf::from("src/Configuration/Configuration.mdo");
    let object_path = PathBuf::from("src/CommonModules/Модуль/Модуль.mdo");
    write_file(
        repo.join(&config_path),
        edt_configuration(&["CommonModule.Модуль"]),
    );
    fs::create_dir_all(repo.join("src/CommonModules")).unwrap();

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, object_path, Some(StagedStatus::Deleted)).unwrap();
    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &metadata_sync_config(),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(
        report.results[0].path,
        PathBuf::from("src/CommonModules/Модуль")
    );
    assert!(report.results[0].message.contains("missing files"));
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn metadata_sync_reports_case_only_edt_directory_mismatch() {
    let repo = temp_repo("case_mismatch");
    let config_path = PathBuf::from("src/Configuration/Configuration.mdo");
    write_file(
        repo.join(&config_path),
        edt_configuration(&["CommonModule.Модуль"]),
    );
    write_file(
        repo.join("src/CommonModules/модуль/модуль.mdo"),
        "<mdclass:CommonModule/>",
    );

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, config_path, None).unwrap();
    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &metadata_sync_config(),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert!(report.results.iter().any(|result| {
        result.path == PathBuf::from("src/CommonModules/модуль")
            && result.message.contains("differs by case")
    }));
}

#[test]
fn metadata_sync_accepts_clean_designer_configuration() {
    let repo = temp_repo("clean_designer");
    let config_path = PathBuf::from("src/Configuration.xml");
    write_file(repo.join(&config_path), designer_configuration());
    write_file(
        repo.join("src/CommonModules/Модуль.xml"),
        "<MetaDataObject/>",
    );

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, config_path, None).unwrap();
    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &metadata_sync_config(),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert!(report.results.is_empty());
    assert_eq!(report.hook_exit_code(), 0);
}

#[test]
fn metadata_sync_ignores_nested_designer_child_objects_outside_configuration_composition() {
    let repo = temp_repo("nested_designer_child_objects");
    let config_path = PathBuf::from("src/Configuration.xml");
    write_file(
        repo.join(&config_path),
        concat!(
            "<MetaDataObject>\n",
            "  <Configuration>\n",
            "    <ChildObjects>\n",
            "      <CommonModule>Модуль</CommonModule>\n",
            "    </ChildObjects>\n",
            "    <SomeNestedNode>\n",
            "      <ChildObjects>\n",
            "        <Catalog>НеСостав</Catalog>\n",
            "      </ChildObjects>\n",
            "    </SomeNestedNode>\n",
            "  </Configuration>\n",
            "</MetaDataObject>\n",
        ),
    );
    write_file(
        repo.join("src/CommonModules/Модуль.xml"),
        "<MetaDataObject/>",
    );

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, config_path, None).unwrap();
    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &metadata_sync_config(),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert!(report.results.is_empty());
    assert_eq!(report.hook_exit_code(), 0);
}

#[test]
fn metadata_sync_accepts_edt_extension_configuration_without_languages_section() {
    let repo = temp_repo("edt_extension_without_languages");
    let config_path = PathBuf::from("src/Configuration/Configuration.mdo");
    write_file(
        repo.join(&config_path),
        concat!(
            "<mdclass:Configuration>\n",
            "  <objectBelonging>Adopted</objectBelonging>\n",
            "  <commonModules>CommonModule.Модуль</commonModules>\n",
            "</mdclass:Configuration>\n",
        ),
    );
    write_file(
        repo.join("src/CommonModules/Модуль/Модуль.mdo"),
        "<mdclass:CommonModule/>",
    );

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, config_path, None).unwrap();
    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &metadata_sync_config(),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert!(report.results.is_empty());
    assert_eq!(report.hook_exit_code(), 0);
}

#[test]
fn metadata_sync_skips_non_configuration_metadata_during_full_tree_runs() {
    let repo = temp_repo("skip_full_tree_object");
    let config_path = PathBuf::from("src/Configuration/Configuration.mdo");
    let object_path = PathBuf::from("src/CommonModules/Лишний/Лишний.mdo");
    write_file(repo.join(&config_path), edt_configuration(&[]));
    write_file(repo.join(&object_path), "<mdclass:CommonModule/>");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, object_path, None).unwrap();
    let report = run_pipeline(
        &ScenarioRegistry::reference(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &metadata_sync_config(),
            files: vec![file],
            mode: PipelineMode::ExecRules,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
}

#[test]
fn metadata_sync_reports_malformed_configuration_xml() {
    let result = check_metadata_sync_text(
        Path::new("."),
        &test_source_root("src"),
        Path::new("src/Configuration/Configuration.mdo"),
        SourceFileKind::ConfigurationMetadata,
        "<mdclass:Configuration><languages></mdclass:Configuration>",
    );

    assert!(
        matches!(result, MetadataSyncCheck::Failed(issues) if issues[0].message.contains("failed to parse XML/EDT file"))
    );
}

fn metadata_sync_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(&format!(
        r#"{{
            "Precommt4onecСценарии": {{
                "ГлобальныеСценарии": ["{METADATA_SYNC_RULE}"]
            }}
        }}"#
    ))
    .unwrap()
}

fn edt_configuration(objects: &[&str]) -> String {
    let object_lines = objects
        .iter()
        .map(|object| {
            let tag = object
                .split_once('.')
                .map(|(metadata_type, _name)| match metadata_type {
                    "Catalog" => "catalogs",
                    "CommonModule" => "commonModules",
                    _ => "commonModules",
                })
                .unwrap_or("commonModules");
            format!("  <{tag}>{object}</{tag}>\n")
        })
        .collect::<String>();
    format!(
        "<mdclass:Configuration>\n  <languages>\n    <name>Русский</name>\n  </languages>\n{object_lines}</mdclass:Configuration>\n"
    )
}

fn designer_configuration() -> &'static str {
    concat!(
        "<MetaDataObject>\n",
        "  <Configuration>\n",
        "    <ChildObjects>\n",
        "      <CommonModule>Модуль</CommonModule>\n",
        "    </ChildObjects>\n",
        "  </Configuration>\n",
        "</MetaDataObject>\n",
    )
}

fn test_source_root(path: &str) -> prec_bsl::source_files::SourceRoot {
    prec_bsl::source_files::SourceRoot {
        configured_path: PathBuf::from(path),
        absolute_path: PathBuf::from(path),
        repo_relative_path: PathBuf::from(path),
    }
}

fn temp_repo(test_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after UNIX_EPOCH")
        .as_nanos();
    let path = std::env::current_dir()
        .expect("current dir must be available")
        .join("target")
        .join("metadata-sync-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary metadata-sync test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: impl AsRef<str>) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content.as_ref()).unwrap();
}
