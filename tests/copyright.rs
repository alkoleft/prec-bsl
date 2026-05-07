use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};
use prec_bsl::text_fixers::{COPYRIGHT_RULE, CopyrightFix, insert_or_update_copyright};

#[test]
fn copyright_matches_golden_fixture_and_is_idempotent() {
    let repo = temp_repo("golden");
    let repo_path = PathBuf::from("src/ОбщиеМодули/КлиентскийМодуль/Модуль.bsl");
    let input =
        read_fixture_text("tests/fixtures/golden/ВставкаКопирайтов/кириллический_модуль/input.bsl");
    let expected = read_fixture_text(
        "tests/fixtures/golden/ВставкаКопирайтов/кириллический_модуль/expected.bsl",
    );
    write_file(repo.join(&repo_path), &input);
    write_file(repo.join("COPYRIGHT"), copyright_text());

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = copyright_config(r#""ПутьКФайлуКопирайта": "COPYRIGHT""#);

    let first_report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file.clone()],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(fs::read_to_string(repo.join(&repo_path)).unwrap(), expected);
    assert_eq!(first_report.results.len(), 1);
    assert_eq!(first_report.results[0].rule_id, COPYRIGHT_RULE);
    assert_eq!(
        first_report.results[0].status,
        ScenarioResultStatus::Modified
    );
    assert_eq!(
        first_report.results[0].message,
        "inserted or updated copyright header"
    );
    assert_eq!(first_report.modified_paths(), vec![repo_path.clone()]);
    assert_eq!(
        first_report.hook_exit_code(),
        1,
        "hook mode must block after unreviewed fixer modifications"
    );

    let second_report = run_pipeline(
        &prec_bsl::reference_registry(),
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
fn copyright_replaces_existing_stale_header() {
    let input = concat!(
        "//© Old Company\n",
        "//\n",
        "// Old text\n",
        "//©\n",
        "\n",
        "Процедура Выполнить()\n",
        "КонецПроцедуры"
    );

    assert_eq!(
        insert_or_update_copyright(input, copyright_text(), &[]),
        CopyrightFix::Modified(
            concat!(
                "//© ООО \"Пример\", 2024-2026\n",
                "//\n",
                "// Текст лицензии\n",
                "//©\n",
                "\n",
                "Процедура Выполнить()\n",
                "КонецПроцедуры"
            )
            .to_owned()
        )
    );
}

#[test]
fn copyright_skips_modules_with_default_or_configured_tags() {
    let repo = temp_repo("excluded_tag");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = "// IMPORT generated\nПроцедура A()\nКонецПроцедуры\n";
    write_file(repo.join(&repo_path), input);
    write_file(repo.join("COPYRIGHT"), copyright_text());

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config =
        copyright_config(r#""ПутьКФайлуКопирайта": "COPYRIGHT", "ИсключаемыеТэги": ["// IMPORT"]"#);

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
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
    assert_eq!(
        report.results[0].message,
        "module contains configured copyright skip tag"
    );
    assert_eq!(report.hook_exit_code(), 0);
}

#[test]
fn copyright_fails_on_ambiguous_existing_header() {
    let result = insert_or_update_copyright("//© broken\nПроцедура A()\n", copyright_text(), &[]);

    assert_eq!(
        result,
        CopyrightFix::Failed("copyright block shape is ambiguous".to_owned())
    );

    let result = insert_or_update_copyright(
        concat!(
            "//© First\n",
            "//©\n",
            "//© Second\n",
            "//©\n",
            "Процедура A()\n"
        ),
        copyright_text(),
        &[],
    );

    assert_eq!(
        result,
        CopyrightFix::Failed("copyright block shape is ambiguous".to_owned())
    );

    let result = insert_or_update_copyright(
        concat!(
            "//© stale\n",
            "Процедура A()\n",
            "КонецПроцедуры\n",
            "//©\n",
            "Процедура B()\n",
            "КонецПроцедуры\n"
        ),
        copyright_text(),
        &[],
    );

    assert_eq!(
        result,
        CopyrightFix::Failed("copyright block shape is ambiguous".to_owned())
    );
}

#[test]
fn copyright_skips_when_default_file_is_absent() {
    let repo = temp_repo("default_missing");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    write_file(repo.join(&repo_path), "Процедура A()\nКонецПроцедуры\n");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();
    let config = parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ВставкаКопирайтов.os"]
            }
        }"#,
    )
    .unwrap();

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

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
    assert_eq!(
        report.results[0].message,
        "copyright file is not configured or found: COPYRIGHT"
    );
}

#[test]
fn copyright_fails_when_configured_path_is_invalid() {
    let repo = temp_repo("invalid_path");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    write_file(repo.join(&repo_path), "Процедура A()\nКонецПроцедуры\n");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();
    let config = copyright_config(r#""ПутьКФайлуКопирайта": "../COPYRIGHT""#);

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

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].message,
        "copyright path must be repository-relative: ../COPYRIGHT"
    );
}

#[test]
fn copyright_fails_when_configured_path_uses_backslash_escape_or_windows_absolute() {
    for setting in [
        r#""ПутьКФайлуКопирайта": "folder\\COPYRIGHT""#,
        r#""ПутьКФайлуКопирайта": "..\\COPYRIGHT""#,
        r#""ПутьКФайлуКопирайта": "C:\\tmp\\COPYRIGHT""#,
    ] {
        let repo = temp_repo("invalid_backslash_path");
        let repo_path = PathBuf::from("src/Модуль.bsl");
        write_file(repo.join(&repo_path), "Процедура A()\nКонецПроцедуры\n");

        let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
        let file = classify_repo_path(&roots, repo_path, None).unwrap();
        let config = copyright_config(setting);

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

        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
        assert!(
            report.results[0]
                .message
                .starts_with("copyright path must be repository-relative: ")
        );
    }
}

#[test]
fn copyright_fails_when_configured_file_is_missing() {
    let repo = temp_repo("configured_missing");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    write_file(repo.join(&repo_path), "Процедура A()\nКонецПроцедуры\n");

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();
    let config = copyright_config(r#""ПутьКФайлуКопирайта": "missing-COPYRIGHT""#);

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

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert!(
        report.results[0]
            .message
            .contains("failed to read configured copyright file missing-COPYRIGHT")
    );
}

fn copyright_config(setting: &str) -> prec_bsl::config::ResolvedConfig {
    parse_config_str(&format!(
        r#"{{
            "Precommt4onecСценарии": {{
                "ГлобальныеСценарии": ["ВставкаКопирайтов.os"],
                "НастройкиСценариев": {{
                    "ВставкаКопирайтов": {{{setting}}}
                }}
            }}
        }}"#
    ))
    .unwrap()
}

fn copyright_text() -> &'static str {
    "//© ООО \"Пример\", 2024-2026\n//\n// Текст лицензии\n//©\n"
}

fn read_fixture_text(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path)
        .unwrap()
        .replace("<SP>", " ")
        .replace("<TAB>", "\t")
}

fn temp_repo(test_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after UNIX_EPOCH")
        .as_nanos();
    let path = std::env::current_dir()
        .expect("current dir must be available")
        .join("target")
        .join("copyright-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary copyright test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
