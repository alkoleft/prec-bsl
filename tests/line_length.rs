//! Integration and CLI-contract tests for the `ПроверкаДлиныСтрок` (line
//! length) check-only scenario: pipeline behavior (warnings, settings,
//! defaults, invalid settings, non-BSL skips) and the `prec-bsl check`
//! binary contracts (exit codes, file immutability, JSON output, CRLF).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};
use prec_bsl::text_checkers::LINE_LENGTH_RULE;

/// A syntactically plausible single-line BSL statement of exactly `length`
/// characters (statement keywords included), so tests can reason about the
/// reported length directly.
fn bsl_line_of_length(length: usize) -> String {
    const PREFIX: &str = "Сообщить(\"";
    const SUFFIX: &str = "\");";
    let padding = length - PREFIX.chars().count() - SUFFIX.chars().count();
    format!("{PREFIX}{}{SUFFIX}", "А".repeat(padding))
}

#[test]
fn line_length_flags_long_lines_with_message() {
    let repo = temp_repo("long_lines_message");
    let repo_path = PathBuf::from("src/ОбщиеМодули/МодульСДлиннойСтрокой/Модуль.bsl");
    let input = format!("{}\n", bsl_line_of_length(160));
    write_file(repo.join(&repo_path), &input);

    let report = run_line_length_pipeline(&repo, &repo_path, line_length_config(None));

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, LINE_LENGTH_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Warning);
    assert_eq!(report.results[0].path, repo_path);
    assert_eq!(
        report.results[0].message,
        "String length 160 exceeded maximum 150 at line 1"
    );
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        input,
        "check-only scenario must not modify the file"
    );
    assert!(report.modified_paths().is_empty());
    assert_eq!(
        report.hook_exit_code(),
        0,
        "warnings must not block the hook"
    );
}

#[test]
fn line_length_uses_setting_from_config() {
    let repo = temp_repo("setting_from_config");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    // 130 chars: over the configured 120 limit, under the default 150.
    let input = format!("{}\n", bsl_line_of_length(130));
    write_file(repo.join(&repo_path), &input);

    let report = run_line_length_pipeline(
        &repo,
        &repo_path,
        line_length_config(Some(r#""МаксимальнаяДлинаСтроки": 120"#)),
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Warning);
    assert_eq!(
        report.results[0].message,
        "String length 130 exceeded maximum 120 at line 1"
    );
    assert!(
        report.results[0].message.contains("maximum 120"),
        "the configured limit must be reported: {}",
        report.results[0].message
    );
}

#[test]
fn line_length_defaults_to_150_when_no_settings() {
    let repo = temp_repo("default_limit");
    let over_path = PathBuf::from("src/СлишкомДлинная.bsl");
    let exact_path = PathBuf::from("src/РовноЛимит.bsl");
    write_file(
        repo.join(&over_path),
        &format!("{}\n", bsl_line_of_length(151)),
    );
    write_file(
        repo.join(&exact_path),
        &format!("{}\n", bsl_line_of_length(150)),
    );

    // No НастройкиСценариев at all: the scenario-level default is 150.
    let over_report = run_line_length_pipeline(&repo, &over_path, line_length_config(None));
    assert_eq!(over_report.results.len(), 1);
    assert_eq!(
        over_report.results[0].message,
        "String length 151 exceeded maximum 150 at line 1"
    );

    // The НастройкиСценариев block present, but without the limit setting.
    let exact_report = run_line_length_pipeline(
        &repo,
        &exact_path,
        line_length_config(Some(r#""ДругаяНастройка": true"#)),
    );
    assert!(
        exact_report.results.is_empty(),
        "exactly 150 chars must not be flagged: {:?}",
        exact_report.results
    );
}

#[test]
fn line_length_fails_on_invalid_setting() {
    let repo = temp_repo("invalid_setting_string");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    write_file(repo.join(&repo_path), "Сообщить(\"текст\");\n");

    let report = run_line_length_pipeline(
        &repo,
        &repo_path,
        line_length_config(Some(r#""МаксимальнаяДлинаСтроки": "120""#)),
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].message,
        "max line length setting МаксимальнаяДлинаСтроки must be a positive integer"
    );
    assert_eq!(report.hook_exit_code(), 1);
}

#[test]
fn line_length_float_setting_fails() {
    let repo = temp_repo("invalid_setting_float");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    write_file(repo.join(&repo_path), "Сообщить(\"текст\");\n");

    let report = run_line_length_pipeline(
        &repo,
        &repo_path,
        line_length_config(Some(r#""МаксимальнаяДлинаСтроки": 150.0"#)),
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].message,
        "max line length setting МаксимальнаяДлинаСтроки must be a positive integer"
    );
}

#[test]
fn line_length_zero_and_negative_settings_fail() {
    for setting in ["0", "-5"] {
        let repo = temp_repo(&format!("invalid_setting_{setting}"));
        let repo_path = PathBuf::from("src/Модуль.bsl");
        write_file(repo.join(&repo_path), "Сообщить(\"текст\");\n");

        let report = run_line_length_pipeline(
            &repo,
            &repo_path,
            line_length_config(Some(&format!(r#""МаксимальнаяДлинаСтроки": {setting}"#))),
        );

        assert_eq!(
            report.results.len(),
            1,
            "setting {setting}: exactly one hard failure expected"
        );
        assert_eq!(
            report.results[0].status,
            ScenarioResultStatus::HardFailure,
            "setting {setting}"
        );
        assert_eq!(
            report.results[0].message,
            "max line length setting МаксимальнаяДлинаСтроки must be a positive integer",
            "setting {setting}"
        );
    }
}

#[test]
fn line_length_not_enabled_by_default() {
    let repo = temp_repo("not_enabled_by_default");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    let input = format!("{}\n", bsl_line_of_length(160));
    write_file(repo.join(&repo_path), &input);

    // A config without the scenario in ГлобальныеСценарии (the shape used
    // when only other scenarios are enabled) must not run it.
    let config = parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
            }
        }"#,
    )
    .unwrap();

    let report = run_line_length_pipeline(&repo, &repo_path, config);

    assert!(
        report
            .results
            .iter()
            .all(|result| result.rule_id != LINE_LENGTH_RULE),
        "scenario must be opt-in only: {:?}",
        report.results
    );
    assert!(
        !report
            .results
            .iter()
            .any(|result| result.status == ScenarioResultStatus::Warning),
        "no line-length warnings may appear without the scenario enabled: {:?}",
        report.results
    );
}

#[test]
fn line_length_skips_non_bsl_files() {
    let repo = temp_repo("non_bsl_skip");
    let xml_path = PathBuf::from("src/Конфигурация.xml");
    write_file(repo.join(&xml_path), "<Конфигурация/>\n");

    let report = run_line_length_pipeline(&repo, &xml_path, line_length_config(None));

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, LINE_LENGTH_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
    assert_eq!(
        report.results[0].message,
        "scenario handles only BSL modules"
    );
    assert_eq!(report.results[0].path, xml_path);
}

#[test]
fn line_length_multiple_violations_in_one_file() {
    let repo = temp_repo("multiple_violations");
    let repo_path = PathBuf::from("src/Модуль.bsl");
    // Violations on lines 3, 21 and 100 with distinct lengths 151/160/180.
    let mut lines = Vec::new();
    for index in 1..=100 {
        let line = match index {
            3 => bsl_line_of_length(151),
            21 => bsl_line_of_length(160),
            100 => bsl_line_of_length(180),
            _ => "Сообщить(\"короткая строка\");".to_owned(),
        };
        lines.push(line);
    }
    let input = format!("{}\n", lines.join("\n"));
    write_file(repo.join(&repo_path), &input);

    let report = run_line_length_pipeline(&repo, &repo_path, line_length_config(None));

    assert_eq!(
        report.results.len(),
        3,
        "exactly three warnings expected: {:?}",
        report.results
    );
    assert!(
        report
            .results
            .iter()
            .all(|result| result.status == ScenarioResultStatus::Warning)
    );

    // The pipeline emits one result per over-limit line in file order
    // (lines 3, 21, 100). The report itself has no extra sorting at this
    // layer: this pins the actual contract observed today.
    let messages: Vec<&str> = report.results.iter().map(|r| r.message.as_str()).collect();
    assert_eq!(
        messages,
        vec![
            "String length 151 exceeded maximum 150 at line 3",
            "String length 160 exceeded maximum 150 at line 21",
            "String length 180 exceeded maximum 150 at line 100",
        ]
    );

    // CLI exit contract: any warning is a violation for check.
    assert_eq!(report.check_exit_code(), 1);
}

#[test]
fn line_length_check_exit_code_is_1_on_violations() {
    let repo = TempGitRepo::new("exit_one");
    let module = PathBuf::from("src/Модуль.bsl");
    let content = format!("{}\n", bsl_line_of_length(160));
    write_file(repo.path().join(&module), &content);
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    let output = run_prec_bsl(
        repo.path(),
        [
            "check",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            LINE_LENGTH_RULE,
            "--mode",
            "all",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("длина строки 160 превышает максимально допустимую 150 (строка 1)"),
        "check text output must report the localized warning:\n{stdout}"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        content,
        "check must not modify the file"
    );
}

#[test]
fn line_length_check_exit_code_is_0_when_clean() {
    let repo = TempGitRepo::new("exit_zero");
    let module = PathBuf::from("src/Модуль.bsl");
    let content = format!("{}\n", bsl_line_of_length(150));
    write_file(repo.path().join(&module), &content);
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    let output = run_prec_bsl(
        repo.path(),
        [
            "check",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            LINE_LENGTH_RULE,
            "--mode",
            "all",
        ],
    );

    assert_eq!(output.status.code(), Some(0), "{}", output_text(&output));
    assert_eq!(
        fs::read_to_string(repo.path().join(&module)).unwrap(),
        content,
        "check must not modify the file"
    );
}

#[test]
fn line_length_json_output_contains_rule_and_message() {
    let repo = TempGitRepo::new("json_output");
    let module = PathBuf::from("src/Модуль.bsl");
    write_file(
        repo.path().join(&module),
        &format!("{}\n", bsl_line_of_length(160)),
    );
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    let output = run_prec_bsl(
        repo.path(),
        [
            "check",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            LINE_LENGTH_RULE,
            "--mode",
            "all",
            "--format",
            "json",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The whole stdout must be a single valid JSON document.
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|error| panic!("stdout must parse as JSON: {error}\n{stdout}"));

    let results = json["results"].as_array().unwrap();
    let warning = results
        .iter()
        .find(|result| result["severity"] == "warning")
        .expect("JSON report must include the line-length warning");
    assert_eq!(warning["rule_id"], LINE_LENGTH_RULE);
    assert_eq!(warning["severity"], "warning");
    assert_eq!(
        warning["message"],
        "String length 160 exceeded maximum 150 at line 1"
    );
    assert_eq!(warning["path"], "src/Модуль.bsl");
}

#[test]
fn line_length_crlf_check_does_not_modify_file() {
    let repo = TempGitRepo::new("crlf_untouched");
    let module = PathBuf::from("src/CRLFМодуль.bsl");
    // CRLF file with a violating line (161 chars before `\r\n`): check must
    // report it and leave the bytes exactly as they were.
    let content = format!("{}\r\n", bsl_line_of_length(161));
    write_file(repo.path().join(&module), &content);
    run_git(repo.path(), ["add", "."]);
    run_git(repo.path(), ["commit", "-m", "baseline"]);

    let before = fs::read(repo.path().join(&module)).unwrap();

    let output = run_prec_bsl(
        repo.path(),
        [
            "check",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            LINE_LENGTH_RULE,
            "--mode",
            "all",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("длина строки 161 превышает максимально допустимую 150 (строка 1)"),
        "the CRLF violating line must be reported with `\\r` excluded from the length:\n{stdout}"
    );
    let after = fs::read(repo.path().join(&module)).unwrap();
    assert_eq!(
        before, after,
        "check must leave the CRLF file byte-identical"
    );
}

/// Runs the line-length scenario over one repo file through the real
/// reference registry and pipeline.
fn run_line_length_pipeline(
    repo: &Path,
    repo_path: &Path,
    config: prec_bsl::config::ResolvedConfig,
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
            mode: PipelineMode::Check,
        },
    )
}

/// Config with the line-length scenario enabled and an optional raw JSON
/// settings body for `НастройкиСценариев.ПроверкаДлиныСтрок`.
fn line_length_config(settings: Option<&str>) -> prec_bsl::config::ResolvedConfig {
    let settings_block = match settings {
        Some(settings) => format!(
            r#",
                "НастройкиСценариев": {{
                    "ПроверкаДлиныСтрок": {{{settings}}}
                }}"#
        ),
        None => String::new(),
    };
    parse_config_str(&format!(
        r#"{{
            "Precommt4onecСценарии": {{
                "ГлобальныеСценарии": ["ПроверкаДлиныСтрок"]{settings_block}
            }}
        }}"#,
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
        .join("line-length-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary line length test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// Minimal throwaway Git repository for the CLI binary contracts. Copied
/// from `tests/check_fix_modes.rs` (integration-test harness duplication is
/// deliberate: each integration test file stays self-contained).
struct TempGitRepo {
    path: PathBuf,
}

impl TempGitRepo {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after UNIX_EPOCH")
            .as_nanos();
        let path = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("line-length-tests")
            .join(format!("{name}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        run_git(&path, ["init"]);
        run_git(&path, ["config", "user.email", "prec-bsl@example.invalid"]);
        run_git(&path, ["config", "user.name", "prec-bsl tests"]);
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempGitRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_prec_bsl<const N: usize>(cwd: impl AsRef<Path>, args: [&str; N]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_prec-bsl"))
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap()
}

fn run_git<const N: usize>(repo: &Path, args: [&str; N]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("-c")
        .arg("core.quotePath=false")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git command failed in {}: {}\nstdout: {}\nstderr: {}",
        repo.display(),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn output_text(output: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}
