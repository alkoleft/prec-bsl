//! Integration and golden-fixture tests for the `ПроверкаТочекЗапятой`
//! (semicolon presence) extension scenario: enabled-by-default with
//! opt-out, hook-mode modification and restage-visible modified paths,
//! check-mode per-statement warnings, refusal branches (lone-`\r`,
//! unsafe parse errors) leaving the file untouched, golden fixtures
//! for the fixer contract (EOF form, trailing comment insertion point,
//! CRLF preservation), and fix-order convergence with
//! `ОдинОператорНаСтроку` (both permutations of the two fixers reach
//! the same one-statement-per-line, `;`-terminated text).

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
const SEMICOLON_PRESENCE_RULE: &str = "ПроверкаТочекЗапятой";

/// Hook mode: the EOF form (the issue's main case) — `Возврат "Булево"`
/// without `;` is modified by appending one `;` at the stop token; the
/// single result reports the insertion and the modified path (restage
/// queue); hook exit code 1 (unreviewed modification blocks).
#[test]
fn semicolon_presence_hook_mode_appends_missing_semicolons() {
    let repo = temp_repo("hook_mode");
    let repo_path = PathBuf::from(REPO_PATH);
    let input = "Функция Ф()\n\tВозврат \"Булево\"\nКонецФункции";
    write_file(repo.join(&repo_path), input);

    let report = run_semicolon_pipeline(&repo, &repo_path, semicolon_config(), PipelineMode::Hook);

    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        "Функция Ф()\n\tВозврат \"Булево\";\nКонецФункции",
        "hook must insert the missing `;` at the stop token"
    );
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, SEMICOLON_PRESENCE_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Modified);
    assert_eq!(report.results[0].message, "inserted missing semicolons");
    assert_eq!(report.modified_paths(), vec![repo_path.clone()]);
    assert_eq!(report.hook_exit_code(), 1);

    // Second run over the fixed file: no results, exit 0 (idempotence).
    let second = run_semicolon_pipeline(&repo, &repo_path, semicolon_config(), PipelineMode::Hook);
    assert!(second.results.is_empty(), "{:?}", second.results);
    assert_eq!(second.hook_exit_code(), 0);
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        "Функция Ф()\n\tВозврат \"Булево\";\nКонецФункции"
    );
}

/// Check mode (DryRun): one Warning per flagged statement carrying the
/// stop-token source span, no `modified_dry_run` (the warnings already
/// give check exit 1 — the LineLength precedent, SKEP-109); the file on
/// disk stays byte-for-byte unchanged.
#[test]
fn semicolon_presence_check_mode_reports_per_statement_warnings() {
    let repo = temp_repo("check_mode");
    let repo_path = PathBuf::from(REPO_PATH);
    let input = "Если Истина Тогда\n\tА = 1;\nКонецЕсли // c\nБ = 2\n";
    write_file(repo.join(&repo_path), input);

    let report = run_semicolon_pipeline(&repo, &repo_path, semicolon_config(), PipelineMode::Check);

    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        input,
        "check mode must not modify the file"
    );

    // Two flagged statements: `КонецЕсли // c` (stop token `КонецЕсли`,
    // line 3) and `Б = 2` (stop token `2`, line 4). No dry-run
    // modification result accompanies them.
    let warnings: Vec<&prec_bsl::scenario_pipeline::ScenarioResult> = report
        .results
        .iter()
        .filter(|result| result.status == ScenarioResultStatus::Warning)
        .collect();
    assert_eq!(warnings.len(), 2, "{:?}", report.results);
    assert_eq!(
        report.results.len(),
        2,
        "check mode must not emit a modified_dry_run result"
    );

    let expected = [
        (3, "КонецЕсли", "missing semicolon at line 3"),
        (4, "2", "missing semicolon at line 4"),
    ];
    for (warning, (line, span_text, message)) in warnings.iter().zip(expected) {
        assert_eq!(warning.rule_id, SEMICOLON_PRESENCE_RULE);
        assert_eq!(warning.message, message);
        let span = warning
            .source_span
            .as_ref()
            .unwrap_or_else(|| panic!("warning for line {line} must carry a source span"));
        assert_eq!(
            &input[span.start_byte..span.end_byte],
            span_text,
            "span of line {line} must point at the stop token"
        );
    }

    // No would-be modification is reported through modified_paths in
    // check mode (no modified_dry_run by contract).
    assert!(report.modified_paths().is_empty());
    assert_eq!(report.exit_code(), 1);
}

/// Check mode after the fix: zero findings, exit 0 — the AC-1 closing
/// scenario (repeat `check` is clean).
#[test]
fn semicolon_presence_check_after_fix_reports_zero_findings() {
    let repo = temp_repo("check_after_fix");
    let repo_path = PathBuf::from(REPO_PATH);
    write_file(
        repo.join(&repo_path),
        "Функция Ф()\n\tВозврат \"Булево\"\nКонецФункции",
    );

    run_semicolon_pipeline(&repo, &repo_path, semicolon_config(), PipelineMode::Hook);

    let report = run_semicolon_pipeline(&repo, &repo_path, semicolon_config(), PipelineMode::Check);

    assert!(report.results.is_empty(), "{:?}", report.results);
    assert_eq!(report.exit_code(), 0);
}

/// Refusal branch (unsafe parse errors, Apply): a file whose tree holds
/// an unsafe ERROR anywhere is refused by the whole-file fix policy — a
/// single Warning, the file stays byte-for-byte unchanged, and the hook
/// is not blocked (AC-7: the refusal is observable, never silent).
#[test]
fn semicolon_presence_unsafe_parse_errors_refuse_fix_in_hook_mode() {
    let repo = temp_repo("unsafe_error_hook");
    let repo_path = PathBuf::from(REPO_PATH);
    // `Сообщить(1 2)` — an ERROR node inside the call arguments (not
    // a safe form): the whole file is refused by the fix policy.
    let input = "Сообщить(1 2)\nА = 1";
    write_file(repo.join(&repo_path), input);

    let report = run_semicolon_pipeline(&repo, &repo_path, semicolon_config(), PipelineMode::Hook);

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, SEMICOLON_PRESENCE_RULE);
    assert_eq!(
        report.results[0].status,
        ScenarioResultStatus::Warning,
        "unsafe parse errors must be visible as a warning, not silence"
    );
    assert!(
        report.results[0].message.contains("parse"),
        "unexpected message: {}",
        report.results[0].message
    );
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        input,
        "unsafe file must stay unchanged"
    );
    assert_eq!(report.hook_exit_code(), 0, "warnings never block the hook");
}

/// Check mode on the same unsafe file: the per-statement parse-error
/// policy (mirrors bsl-ls `Trees.treeContainsErrors`) skips the errored
/// `Сообщить(1 2)` statement but still flags the clean `А = 1` — check is
/// a diagnostics command and never silently hides clean violations. The
/// fix refusal does not apply here (check writes nothing).
#[test]
fn semicolon_presence_check_mode_on_unsafe_file_flags_clean_statements_only() {
    let repo = temp_repo("unsafe_error_check");
    let repo_path = PathBuf::from(REPO_PATH);
    let input = "Сообщить(1 2)\nА = 1";
    write_file(repo.join(&repo_path), input);

    let report = run_semicolon_pipeline(&repo, &repo_path, semicolon_config(), PipelineMode::Check);

    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        input,
        "check mode must not modify the file"
    );
    assert_eq!(report.results.len(), 1, "{:?}", report.results);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Warning);
    assert_eq!(report.results[0].message, "missing semicolon at line 2");
    let span = report.results[0].source_span.as_ref().unwrap();
    assert_eq!(&input[span.start_byte..span.end_byte], "1");
    assert_eq!(report.exit_code(), 1);
}

/// Refusal branch (lone `\r`): out-of-contract line endings produce a
/// single Warning in both modes and the file stays unchanged.
#[test]
fn semicolon_presence_lone_cr_file_yields_warning_and_leaves_file_unchanged() {
    for mode in [PipelineMode::Hook, PipelineMode::Check] {
        let repo = temp_repo("lone_cr");
        let repo_path = PathBuf::from(REPO_PATH);
        let input = "А = 1\rБ = 2\r";
        write_file(repo.join(&repo_path), input);

        let report = run_semicolon_pipeline(&repo, &repo_path, semicolon_config(), mode);

        assert_eq!(report.results.len(), 1, "{mode:?}");
        assert_eq!(
            report.results[0].rule_id, SEMICOLON_PRESENCE_RULE,
            "{mode:?}"
        );
        assert_eq!(
            report.results[0].status,
            ScenarioResultStatus::Warning,
            "{mode:?}: lone-CR files must be refused visibly"
        );
        assert!(
            report.results[0].message.contains("CR"),
            "{mode:?}: unexpected message: {}",
            report.results[0].message
        );
        assert_eq!(
            fs::read_to_string(repo.join(&repo_path)).unwrap(),
            input,
            "{mode:?}: lone-CR file must stay unchanged"
        );
        assert_eq!(report.hook_exit_code(), 0, "warnings never block the hook");
    }
}

/// The scenario is enabled by built-in defaults (all supported =
/// RequiredV1 + Compatibility): a run without any config file fixes the
/// violations. `ОтключенныеСценарии` turns the scenario off without
/// restating the whole enabled list.
#[test]
fn semicolon_presence_is_enabled_by_default_and_can_be_disabled() {
    let repo = temp_repo("enabled_by_default");
    let repo_path = PathBuf::from(REPO_PATH);
    write_file(
        repo.join(&repo_path),
        "Функция Ф()\n\tВозврат \"Булево\"\nКонецФункции",
    );

    let report = run_semicolon_pipeline(
        &repo,
        &repo_path,
        prec_bsl::config::built_in_defaults(),
        PipelineMode::Hook,
    );

    let result = report
        .results
        .iter()
        .find(|result| result.rule_id == SEMICOLON_PRESENCE_RULE)
        .expect("scenario must run under built-in defaults");
    assert_eq!(result.status, ScenarioResultStatus::Modified);
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        "Функция Ф()\n\tВозврат \"Булево\";\nКонецФункции",
        "default run must fix the violations"
    );

    let disabled_repo = temp_repo("disabled_via_config");
    let disabled_path = PathBuf::from(REPO_PATH);
    write_file(
        disabled_repo.join(&disabled_path),
        "Функция Ф()\n\tВозврат \"Булево\"\nКонецФункции",
    );

    let disabled_config = parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ОтключенныеСценарии": ["ПроверкаТочекЗапятой"]
            }
        }"#,
    )
    .unwrap();

    let disabled_report = run_semicolon_pipeline(
        &disabled_repo,
        &disabled_path,
        disabled_config,
        PipelineMode::Hook,
    );

    assert!(
        !disabled_report
            .results
            .iter()
            .any(|result| result.rule_id == SEMICOLON_PRESENCE_RULE),
        "ОтключенныеСценарии must turn the scenario off: {:?}",
        disabled_report.results
    );
    assert_eq!(
        fs::read_to_string(disabled_repo.join(&disabled_path)).unwrap(),
        "Функция Ф()\n\tВозврат \"Булево\"\nКонецФункции",
        "disabled scenario must not touch the file"
    );
}

/// The scenario id also resolves with the `.os` suffix, like every other
/// reference scenario.
#[test]
fn semicolon_presence_config_with_os_suffix_works_the_same() {
    let repo = temp_repo("os_suffix");
    let repo_path = PathBuf::from(REPO_PATH);
    write_file(
        repo.join(&repo_path),
        "Функция Ф()\n\tВозврат \"Булево\"\nКонецФункции",
    );

    let config = parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПроверкаТочекЗапятой.os"]
            }
        }"#,
    )
    .unwrap();

    let report = run_semicolon_pipeline(&repo, &repo_path, config, PipelineMode::Hook);

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, SEMICOLON_PRESENCE_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Modified);
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        "Функция Ф()\n\tВозврат \"Булево\";\nКонецФункции"
    );
}

/// Non-BSL files are skipped exactly like the other BSL scenarios.
#[test]
fn semicolon_presence_skips_non_bsl_files() {
    let repo = temp_repo("non_bsl");
    let repo_path = PathBuf::from("src/Каталог/Файл.json");
    write_file(repo.join(&repo_path), "{}");

    let report = run_semicolon_pipeline(&repo, &repo_path, semicolon_config(), PipelineMode::Hook);

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
}

// --- Golden fixtures (full pipeline through `reference_registry`) ---

#[test]
fn semicolon_presence_golden_eof_return() {
    assert_golden("eof_return");
}

#[test]
fn semicolon_presence_golden_comment() {
    assert_golden("comment");
}

#[test]
fn semicolon_presence_golden_crlf() {
    assert_golden("crlf");
}

fn assert_golden(case: &str) {
    let fixture = GoldenFixture::new(
        format!("tests/fixtures/golden/ПроверкаТочекЗапятой/{case}"),
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
                config: &semicolon_config(),
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
                assert_eq!(result.rule_id, SEMICOLON_PRESENCE_RULE);
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

// --- Fix-order convergence with `ОдинОператорНаСтроку` (plan Task 5) ---

/// AC-8 (coexistence): the semicolon fixer and the OSPL fixer do not
/// conflict. An input holding BOTH violation kinds — a one-line
/// statement chain `А = 0 Б = 1` (OSPL moves each statement to its own
/// line) with every statement missing its `;` (the semicolon fixer
/// terminates them) — must converge to the same text under both fix
/// orders: `fix_missing_semicolons(fix_one_statement_per_line(x))`
/// equals `fix_one_statement_per_line(fix_missing_semicolons(x))`. A
/// repeated run of each combination changes nothing (convergence, not
/// mere commutativity), and the converged text has every statement on
/// its own line AND terminated with `;`.
#[test]
fn semicolon_before_ospl_and_ospl_before_semicolon_converge() {
    // Both violation kinds at once: the chain `А = 0 Б = 1` is one OSPL
    // violation (two statements share the row) and two semicolon
    // violations (neither statement is terminated); the EOF `Возврат`
    // adds the issue's main case to the mix.
    let input = "Функция Ф()\n\tА = 0 Б = 1\n\tВозврат \"Булево\"\nКонецФункции";

    // Sanity: the input really holds both violation kinds.
    assert!(
        !prec_bsl::one_statement_per_line::find_violations(input)
            .unwrap()
            .is_empty(),
        "input must hold an OSPL violation"
    );
    assert!(
        !prec_bsl::semicolon_presence::find_missing_semicolons(input)
            .unwrap()
            .is_empty(),
        "input must hold a missing-semicolon violation"
    );

    let ospl_first = prec_bsl::semicolon_presence::fix_missing_semicolons(
        &prec_bsl::one_statement_per_line::fix_one_statement_per_line(input).unwrap(),
    )
    .unwrap();
    let semicolon_first = prec_bsl::one_statement_per_line::fix_one_statement_per_line(
        &prec_bsl::semicolon_presence::fix_missing_semicolons(input).unwrap(),
    )
    .unwrap();

    assert_eq!(
        ospl_first, semicolon_first,
        "fix order must not matter: OSPL-first {ospl_first:?} vs semicolon-first {semicolon_first:?}"
    );

    // The converged text: every statement on its own line AND
    // terminated with `;` (both detectors report the file as clean).
    assert_eq!(
        ospl_first,
        "Функция Ф()\n\tА = 0;\n\tБ = 1;\n\tВозврат \"Булево\";\nКонецФункции"
    );
    assert!(
        prec_bsl::one_statement_per_line::find_violations(&ospl_first)
            .unwrap()
            .is_empty(),
        "converged text must have one statement per line"
    );
    assert!(
        prec_bsl::semicolon_presence::find_missing_semicolons(&ospl_first)
            .unwrap()
            .is_empty(),
        "converged text must have every statement terminated with `;`"
    );

    // Convergence, not just commutativity: re-running each combination
    // over the converged text changes nothing in either order.
    let rerun_ospl_first = prec_bsl::semicolon_presence::fix_missing_semicolons(
        &prec_bsl::one_statement_per_line::fix_one_statement_per_line(&ospl_first).unwrap(),
    )
    .unwrap();
    let rerun_semicolon_first = prec_bsl::one_statement_per_line::fix_one_statement_per_line(
        &prec_bsl::semicolon_presence::fix_missing_semicolons(&ospl_first).unwrap(),
    )
    .unwrap();
    assert_eq!(
        rerun_ospl_first, ospl_first,
        "OSPL-first combo must be a fixed point"
    );
    assert_eq!(
        rerun_semicolon_first, ospl_first,
        "semicolon-first combo must be a fixed point"
    );
}

fn run_semicolon_pipeline(
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

fn semicolon_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПроверкаТочекЗапятой"]
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
        .join("semicolon-presence-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary semicolon test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
