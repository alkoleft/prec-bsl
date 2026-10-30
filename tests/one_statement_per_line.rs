//! Integration and golden-fixture tests for the `ОдинОператорНаСтроку`
//! (one statement per line) extension scenario: hook-mode modification
//! and idempotence, check-mode dry-run contract, parse-error warning
//! path, opt-in enablement and `.os`-suffix lookup.

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};
use support::fixtures::{
    FixtureDiagnostic, FixtureRun, FixtureSeverity, GoldenFixture, assert_fixer_fixture,
};

const REPO_PATH: &str = "src/ОбщиеМодули/КлиентскийМодуль/Модуль.bsl";
const ONE_STATEMENT_PER_LINE_RULE: &str = "ОдинОператорНаСтроку";

/// Hook mode: a module with violations is modified, the file content is
/// the fixed text, and the single result enumerates the violating rows.
#[test]
fn one_statement_per_line_hook_mode_modifies_file_with_violating_rows() {
    let repo = temp_repo("hook_mode");
    let repo_path = PathBuf::from(REPO_PATH);
    let input = "А = 1; Б = 2;\nВ = 3; Г = 4;\n";
    write_file(repo.join(&repo_path), input);

    let report = run_one_statement_pipeline(
        &repo,
        &repo_path,
        one_statement_config(),
        PipelineMode::Hook,
    );

    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        "А = 1;\nБ = 2;\nВ = 3;\nГ = 4;\n"
    );
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, ONE_STATEMENT_PER_LINE_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Modified);
    assert_eq!(
        report.results[0].message,
        "перенесены операторы на отдельные строки: строки 1, 2"
    );
    assert_eq!(report.modified_paths(), vec![repo_path.clone()]);
    assert_eq!(report.hook_exit_code(), 1);
}

/// Second run over the fixed file: no results, exit code 0.
#[test]
fn one_statement_per_line_second_hook_run_is_clean() {
    let repo = temp_repo("second_run");
    let repo_path = PathBuf::from(REPO_PATH);
    write_file(repo.join(&repo_path), "А = 1;\nБ = 2;\n");

    let report = run_one_statement_pipeline(
        &repo,
        &repo_path,
        one_statement_config(),
        PipelineMode::Hook,
    );

    assert!(report.results.is_empty(), "{:?}", report.results);
    assert_eq!(report.hook_exit_code(), 0);
}

/// Check mode: one `modified_dry_run` result with the proposed fixed
/// content; the file on disk stays byte-for-byte unchanged.
#[test]
fn one_statement_per_line_check_mode_reports_dry_run_without_writing() {
    let repo = temp_repo("check_mode");
    let repo_path = PathBuf::from(REPO_PATH);
    let input = "Если X Тогда А = 1; Б = 2;\n";
    write_file(repo.join(&repo_path), input);

    let report = run_one_statement_pipeline(
        &repo,
        &repo_path,
        one_statement_config(),
        PipelineMode::Check,
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Modified);
    assert_eq!(
        report.results[0].proposed_content.as_deref(),
        Some("Если X Тогда\n\tА = 1;\n\tБ = 2;\n")
    );
    assert_eq!(
        report.results[0].message,
        "перенесены операторы на отдельные строки: строка 1"
    );
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        input,
        "check mode must not modify the file"
    );
    // The dry-run result keeps status Modified by contract: the would-be
    // change is reported through `proposed_content` without any write.
    assert_eq!(
        report.results[0].proposed_content.as_deref(),
        Some("Если X Тогда\n\tА = 1;\n\tБ = 2;\n")
    );
}

/// Parse errors intersecting a violation row: the scenario warns and
/// leaves the file untouched (no silent modification of unreliable input).
#[test]
fn one_statement_per_line_parse_error_yields_warning_and_leaves_file_unchanged() {
    let repo = temp_repo("parse_error");
    let repo_path = PathBuf::from(REPO_PATH);
    // `А=2;;` leaves a bare-`;` ERROR on the same row as the surviving
    // violation (unit-fixture case from Task 3): relevant parse error.
    let input = "А=1; А=2;; А=3;";
    write_file(repo.join(&repo_path), input);

    let report = run_one_statement_pipeline(
        &repo,
        &repo_path,
        one_statement_config(),
        PipelineMode::Hook,
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Warning);
    assert!(report.results[0].message.contains("parse"));
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        input,
        "parse-error input must not be modified"
    );
    assert_eq!(report.hook_exit_code(), 0, "warnings never block the hook");
}

/// The scenario is opt-in: without `ГлобальныеСценарии` listing it, no
/// results are produced for this rule.
#[test]
fn one_statement_per_line_is_not_enabled_by_default() {
    let repo = temp_repo("not_enabled");
    let repo_path = PathBuf::from(REPO_PATH);
    write_file(repo.join(&repo_path), "А = 1; Б = 2;\n");

    let config = parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов"]
            }
        }"#,
    )
    .unwrap();

    let report = run_one_statement_pipeline(&repo, &repo_path, config, PipelineMode::Hook);

    assert!(report.results.is_empty(), "{:?}", report.results);
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        "А = 1; Б = 2;\n",
        "scenario must not run when not enabled"
    );
}

/// The scenario id also resolves with the `.os` suffix, like every other
/// reference scenario.
#[test]
fn one_statement_per_line_config_with_os_suffix_works_the_same() {
    let repo = temp_repo("os_suffix");
    let repo_path = PathBuf::from(REPO_PATH);
    write_file(repo.join(&repo_path), "А = 1; Б = 2;\n");

    let config = parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ОдинОператорНаСтроку.os"]
            }
        }"#,
    )
    .unwrap();

    let report = run_one_statement_pipeline(&repo, &repo_path, config, PipelineMode::Hook);

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, ONE_STATEMENT_PER_LINE_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Modified);
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        "А = 1;\nБ = 2;\n"
    );
}

/// Non-BSL files are skipped exactly like the other BSL scenarios.
#[test]
fn one_statement_per_line_skips_non_bsl_files() {
    let repo = temp_repo("non_bsl");
    let repo_path = PathBuf::from("src/Каталог/Файл.json");
    write_file(repo.join(&repo_path), "{}");

    let report = run_one_statement_pipeline(
        &repo,
        &repo_path,
        one_statement_config(),
        PipelineMode::Hook,
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
}

// --- Golden fixtures (full pipeline through `reference_registry`) ---

#[test]
fn one_statement_per_line_golden_chain() {
    assert_golden("chain");
}

#[test]
fn one_statement_per_line_golden_if_then() {
    assert_golden("if-then");
}

#[test]
fn one_statement_per_line_golden_mixed() {
    assert_golden("mixed");
}

#[test]
fn one_statement_per_line_golden_crlf() {
    assert_golden("crlf");
}

#[test]
fn one_statement_per_line_golden_spaces() {
    assert_golden("spaces");
}

#[test]
fn one_statement_per_line_golden_multiline() {
    assert_golden("multiline");
}

#[test]
fn one_statement_per_line_golden_strings_comments() {
    assert_golden("strings-comments");
}

fn assert_golden(case: &str) {
    let fixture = GoldenFixture::new(
        format!("tests/fixtures/golden/ОдинОператорНаСтроку/{case}"),
        PathBuf::from(REPO_PATH),
    );

    assert_fixer_fixture(&fixture, |input, path| {
        let repo = temp_repo(&format!("golden_{case}"));
        write_file(repo.join(path), input);

        let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
        let file = classify_repo_path(&roots, path.to_path_buf(), None).unwrap();
        let report = run_pipeline(
            &prec_bsl::reference_registry(),
            PipelineRequest {
                repo_root: &repo,
                source_roots: &roots,
                config: &one_statement_config(),
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
                assert_eq!(result.rule_id, ONE_STATEMENT_PER_LINE_RULE);
                FixtureDiagnostic::new(
                    FixtureSeverity::Modified,
                    result.rule_id.clone(),
                    path,
                    result.message.clone(),
                )
            })
            .collect();

        assert_eq!(
            report.hook_exit_code(),
            if diagnostics.is_empty() { 0 } else { 1 }
        );
        assert_eq!(
            report.modified_paths(),
            if diagnostics.is_empty() {
                Vec::new()
            } else {
                vec![path.to_path_buf()]
            }
        );

        FixtureRun::new(output, diagnostics)
    });
}

fn run_one_statement_pipeline(
    repo: &Path,
    repo_path: &Path,
    config: prec_bsl::config::ResolvedConfig,
    mode: PipelineMode,
) -> prec_bsl::scenario_pipeline::PipelineReport {
    let roots = resolve_source_roots(repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.to_path_buf(), None)
        .unwrap_or_else(|| panic!("file must classify under src: {}", repo_path.display()));

    run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode,
        },
    )
}

fn one_statement_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ОдинОператорНаСтроку"]
            }
        }"#,
    )
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
        .join("one-statement-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary one-statement test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
