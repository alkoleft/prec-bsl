use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

mod support;

use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};
use prec_bsl::text_fixers::{OPERATOR_SPACING_RULE, fix_operator_spacing};
use support::fixtures::{
    FixtureDiagnostic, FixtureRun, FixtureSeverity, GoldenFixture, assert_fixer_fixture,
};

#[test]
fn operator_spacing_matches_golden_fixture_and_is_idempotent() {
    let fixture = GoldenFixture::new(
        "tests/fixtures/golden/ПробелыВокругОператоров/кириллический_модуль",
        PathBuf::from("src/ОбщиеМодули/КлиентскийМодуль/Модуль.bsl"),
    );

    assert_fixer_fixture(&fixture, |input, path| {
        operator_spacing_pipeline_run("cyrillic", input, path)
    });
}

fn operator_spacing_pipeline_run(test_name: &str, input: &str, path: &Path) -> FixtureRun {
    let repo = temp_repo(test_name);
    write_file(repo.join(path), input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, path.to_path_buf(), None).unwrap();
    let config = operator_spacing_config();

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

    let output = fs::read_to_string(repo.join(path)).unwrap();
    let diagnostics: Vec<FixtureDiagnostic> = report
        .results
        .iter()
        .map(|result| {
            assert_eq!(result.status, ScenarioResultStatus::Modified);
            assert_eq!(result.rule_id, OPERATOR_SPACING_RULE);
            FixtureDiagnostic::new(
                FixtureSeverity::Modified,
                result.rule_id.clone(),
                path,
                result.message.clone(),
            )
        })
        .collect();

    // AGENTS.md modeling rule: hook mode exits 0 only when no blocking
    // diagnostics and no unreviewed modifications remain.
    let expected_exit_code = if diagnostics.is_empty() { 0 } else { 1 };
    assert_eq!(
        report.hook_exit_code(),
        expected_exit_code,
        "hook exit code must block on unreviewed fixer modifications"
    );
    assert_eq!(
        report.modified_paths(),
        if diagnostics.is_empty() {
            Vec::new()
        } else {
            vec![path.to_path_buf()]
        },
        "modified paths must report every silently changed file"
    );

    FixtureRun::new(output, diagnostics)
}

#[test]
fn operator_spacing_pipeline_blocks_and_second_run_is_clean() {
    // Прямая (не golden) проверка hook-контракта на фикстуре: первый прогон
    // блокирует с Modified, повторный на уже исправленном тексте чист.
    let repo = temp_repo("hook_contract");
    let repo_path = PathBuf::from("src/ОбщиеМодули/КлиентскийМодуль/Модуль.bsl");
    let input = read_fixture_text(
        "tests/fixtures/golden/ПробелыВокругОператоров/кириллический_модуль/input.bsl",
    );
    let expected = read_fixture_text(
        "tests/fixtures/golden/ПробелыВокругОператоров/кириллический_модуль/expected.bsl",
    );
    write_file(repo.join(&repo_path), &input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = operator_spacing_config();

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
    assert_eq!(first_report.results[0].rule_id, OPERATOR_SPACING_RULE);
    assert_eq!(
        first_report.results[0].status,
        ScenarioResultStatus::Modified
    );
    assert_eq!(
        first_report.results[0].message,
        "added spaces around operators"
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
fn operator_spacing_skips_non_bsl_files() {
    let repo = temp_repo("skip_non_bsl");
    let repo_path = PathBuf::from("src/ОбщиеМодули/Модуль/Модуль.xml");
    let input = "<root>60*60*24</root>";
    write_file(repo.join(&repo_path), input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = operator_spacing_config();

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
    assert_eq!(report.results[0].rule_id, OPERATOR_SPACING_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
    assert!(report.modified_paths().is_empty());
    assert_eq!(report.hook_exit_code(), 0);
}

#[test]
fn operator_spacing_line_ending_matrix_is_idempotent() {
    for (name, line_ending) in [("lf", "\n"), ("crlf", "\r\n"), ("cr", "\r")] {
        let input = format!("А=Б{line_ending}В<=Г{line_ending}Метод(-З){line_ending}");
        let expected = format!("А = Б{line_ending}В <= Г{line_ending}Метод(-З){line_ending}");

        let first = fix_operator_spacing(&input);
        let second = fix_operator_spacing(&first);

        assert_eq!(first, expected, "{name} rewrite must preserve line endings");
        assert_eq!(second, expected, "{name} second rewrite must be clean");
    }
}

#[test]
fn operator_spacing_is_not_enabled_without_explicit_scenario_list() {
    let repo = temp_repo("not_enabled");
    let repo_path = PathBuf::from("src/ОбщиеМодули/КлиентскийМодуль/Модуль.bsl");
    let input = "Секунды=60*60*24;\n";
    write_file(repo.join(&repo_path), input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
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

    assert!(report.results.is_empty());
    assert_eq!(fs::read_to_string(repo.join(&repo_path)).unwrap(), input);
    assert_eq!(report.hook_exit_code(), 0);
}

fn operator_spacing_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПробелыВокругОператоров.os"]
            }
        }"#,
    )
    .unwrap()
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
        .join("operator-spacing-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary operator spacing test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
