#[path = "support/rat.rs"]
mod rat_support;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use prec_bsl::bsl_parser::BslParser;
use prec_bsl::scenarios::{ScenarioSupport, find_reference_scenario, normalize_scenario_id};
use prec_bsl::source_files::{
    SourceFileKind, collect_source_files as collect_prec_bsl_source_files, parse_source_dir_list,
    resolve_source_roots,
};
use rat_support::{
    RAT_PARSER_ROOTS, RAT_SOURCE_ROOTS, TempRatCopy, collect_bsl_files,
    collect_source_files as collect_raw_source_files, copy_required_source_roots, git_status_short,
    rat_repo,
};
use serde_json::Value;

const RAT_TEXT_FIXER_RULES: &str = concat!(
    "УдалениеЛишнихКонцевыхПробелов,",
    "УдалениеЛишнихПустыхСтрок,",
    "ДобавлениеПробеловПередКлючевымиСловами,",
    "ИсправлениеНеКаноническогоНаписания,",
    "ВставкаКопирайтов"
);
const RAT_SOURCE_DIRS: &str = "fixtures/configuration,exts/rat,tests";
const RAT_ACCEPTANCE_COPYRIGHT: &str =
    "//© prec-bsl RAT acceptance\n//\n// Temporary fixture copyright\n//©\n";
const RAT_COPYRIGHT_PROBE_FILE: &str = "exts/rat/src/CommonModules/РатJSON/Module.bsl";
const RAT_XML_EDT_FIXER_RULES: &str =
    "ОтключениеПолнотекстовогоПоиска,ОтключениеРазрешенияИзменятьФорму";
const RAT_XML_FORM_RULE: &str = "КорректировкаXMLФорм";
const RAT_FULL_TEXT_PROBE_FILE: &str =
    "fixtures/configuration/src/InformationRegisters/Ф_ХранилищеЗначений/Ф_ХранилищеЗначений.mdo";
const RAT_FORM_CHANGE_PROBE_FILE: &str =
    "tests/src/DataProcessors/Мок_Ванесса/Forms/Форма/Form.form";
const RAT_XML_FORM_FAILURE_DIR: &str = "exts/rat/src/DataProcessors/РатШагиVA/Forms/Таймер";
const RAT_XML_FORM_FAILURE_FILE: &str =
    "exts/rat/src/DataProcessors/РатШагиVA/Forms/Таймер/Form.form";

#[test]
fn rat_source_roots_copy_to_tempdir_without_mutating_checkout() {
    let Some(repo) = rat_repo() else {
        eprintln!("skipping RAT acceptance: /home/alko/develop/open-source/rat is not available");
        return;
    };

    let before_status = git_status_short(repo).expect("RAT git status must be readable");
    let tempdir = TempRatCopy::new().expect("temporary RAT copy directory must be created");

    let copied_roots = copy_required_source_roots(repo, tempdir.path())
        .expect("required RAT source roots must copy into a temporary directory");

    assert_eq!(copied_roots.len(), RAT_SOURCE_ROOTS.len());
    for root in &copied_roots {
        assert!(
            root.is_dir(),
            "copied RAT source root must exist: {}",
            root.display()
        );
    }

    let copied_files = copied_roots
        .iter()
        .map(|root| collect_raw_source_files(root))
        .collect::<Result<Vec<_>, _>>()
        .expect("copied RAT source files must be discoverable")
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    assert!(
        copied_files.iter().any(|path| has_extension(path, "bsl")),
        "temporary RAT copy must include BSL modules"
    );
    assert!(
        copied_files.iter().any(|path| has_extension(path, "mdo")),
        "temporary RAT copy must include EDT metadata"
    );
    assert!(
        copied_files
            .iter()
            .any(|path| path.to_string_lossy().chars().any(|char| !char.is_ascii())),
        "temporary RAT copy must preserve Cyrillic paths"
    );

    fs::write(
        tempdir
            .path()
            .join("fixtures/configuration/prec-bsl-temp-marker.txt"),
        "mutating acceptance checks write only to the temp copy\n",
    )
    .expect("temporary RAT copy must be writable");

    assert!(
        !repo
            .join("fixtures/configuration/prec-bsl-temp-marker.txt")
            .exists(),
        "acceptance test must not write marker files into the real RAT checkout"
    );

    let after_status = git_status_short(repo).expect("RAT git status must be readable after copy");
    assert_eq!(
        after_status, before_status,
        "copy-only RAT acceptance must not change real checkout status"
    );
}

#[test]
fn rat_live_v8config_parses_and_reports_repository_local_scenarios() {
    let Some(repo) = rat_repo() else {
        eprintln!("skipping RAT acceptance: /home/alko/develop/open-source/rat is not available");
        return;
    };

    let config_path = repo.join("v8config.json");
    let config = fs::read_to_string(&config_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", config_path.display()));
    let config: Value = serde_json::from_str(&config)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", config_path.display()));

    assert_eq!(
        config["GLOBAL"]["ФорматEDT"],
        Value::Bool(true),
        "RAT live config must preserve the EDT-format compatibility fixture"
    );
    assert_eq!(
        config["GLOBAL"]["ВерсияПлатформы"],
        Value::String("8.3.20.1996".to_owned())
    );

    let scenario_config = config
        .get("Precommt4onecСценарии")
        .expect("historic Precommt4onecСценарии key must parse");
    let enabled = scenario_strings(scenario_config, "ГлобальныеСценарии");
    let disabled = scenario_strings(scenario_config, "ОтключенныеСценарии");

    assert!(
        enabled
            .iter()
            .any(|scenario| *scenario == "УдалениеЛишнихКонцевыхПробелов.os"),
        "RAT live config must exercise reference global scenarios"
    );
    assert!(
        disabled
            .iter()
            .any(|scenario| *scenario == "РазборОбычныхФормНаИсходники"),
        "disabled unsupported built-in scenario must remain parse-compatible"
    );

    let diagnostics = enabled
        .iter()
        .filter_map(|scenario| unsupported_enabled_scenario_diagnostic(scenario))
        .collect::<Vec<_>>();

    assert_eq!(
        diagnostics,
        vec![
            "unsupported repository-local scenario in v1: СортировкаДереваМетаданных; dynamic local .os execution is not supported in v1".to_owned(),
            "unsupported repository-local scenario in v1: СортировкаСоставаПодсистем; dynamic local .os execution is not supported in v1".to_owned(),
        ]
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.contains("РазборОбычныхФормНаИсходники")),
        "disabled unsupported scenarios must not be reported as enabled failures"
    );
}

#[test]
fn rat_parser_coverage_counts_and_reports_bsl_parse_errors() {
    let Some(repo) = rat_repo() else {
        eprintln!(
            "skipping RAT parser coverage: /home/alko/develop/open-source/rat is not available"
        );
        return;
    };

    let mut parser = BslParser::new().expect("RAT parser coverage must initialize BSL parser");
    let mut bsl_files = Vec::new();
    for root in RAT_PARSER_ROOTS {
        let root_path = repo.join(root);
        let mut files = collect_bsl_files(&root_path)
            .unwrap_or_else(|error| panic!("failed to collect BSL files from {root}: {error}"));
        bsl_files.append(&mut files);
    }
    bsl_files.sort();

    assert!(
        !bsl_files.is_empty(),
        "RAT parser coverage must cover at least one .bsl file"
    );

    let mut files_with_errors = Vec::new();
    let mut total_error_nodes = 0usize;

    for path in &bsl_files {
        let source = fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let parsed = parser
            .parse(&source)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()));
        let error_nodes = parsed.error_nodes();
        if !error_nodes.is_empty() {
            total_error_nodes += error_nodes.len();
            files_with_errors.push((
                path.strip_prefix(repo)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/"),
                error_nodes.len(),
            ));
        }
    }

    let report = render_rat_parser_coverage_report(
        bsl_files.len(),
        files_with_errors.as_slice(),
        total_error_nodes,
    );
    let report_path = std::env::current_dir()
        .expect("current directory must be readable")
        .join("target")
        .join("rat-acceptance")
        .join("rat-parser-coverage.txt");
    fs::create_dir_all(report_path.parent().expect("report path must have parent"))
        .expect("RAT parser coverage report directory must be created");
    fs::write(&report_path, report).unwrap_or_else(|error| {
        panic!(
            "failed to write RAT parser coverage report {}: {error}",
            report_path.display()
        )
    });
}

#[test]
fn rat_text_idempotence_runs_text_fixers_on_temp_copy_only() {
    let Some(repo) = rat_repo() else {
        eprintln!(
            "skipping RAT text idempotence acceptance: /home/alko/develop/open-source/rat is not available"
        );
        return;
    };

    let before_status = git_status_short(repo).expect("RAT git status must be readable");
    let tempdir = TempRatCopy::new().expect("temporary RAT copy directory must be created");
    copy_required_source_roots(repo, tempdir.path())
        .expect("required RAT source roots must copy into a temporary directory");
    fs::write(tempdir.path().join("COPYRIGHT"), RAT_ACCEPTANCE_COPYRIGHT)
        .expect("temporary RAT copy must include deterministic copyright text");

    let first_run = run_prec_bsl([
        "exec-rules",
        tempdir
            .path()
            .to_str()
            .expect("temp RAT path must be UTF-8"),
        "--source-dir",
        RAT_SOURCE_DIRS,
        "--rules",
        RAT_TEXT_FIXER_RULES,
    ]);
    assert!(
        first_run.status.success(),
        "first RAT text fixer run must succeed:\n{}",
        output_text(&first_run)
    );
    let first_stdout = String::from_utf8_lossy(&first_run.stdout);
    assert!(first_stdout.contains("prec-bsl exec-rules: processed "));
    assert!(
        first_stdout.contains("Modified files:"),
        "first RAT text fixer run must report modified files:\n{}",
        first_stdout
    );
    assert!(
        first_stdout.contains("ВставкаКопирайтов"),
        "first RAT text fixer run must include the copyright fixer:\n{}",
        first_stdout
    );
    let copyright_probe = fs::read_to_string(tempdir.path().join(RAT_COPYRIGHT_PROBE_FILE))
        .expect("RAT copyright probe file must remain readable in the temp copy");
    assert!(
        copyright_probe.starts_with(RAT_ACCEPTANCE_COPYRIGHT),
        "RAT text fixer run must apply deterministic copyright text to a BSL file"
    );

    let second_run = run_prec_bsl([
        "exec-rules",
        tempdir
            .path()
            .to_str()
            .expect("temp RAT path must be UTF-8"),
        "--source-dir",
        RAT_SOURCE_DIRS,
        "--rules",
        RAT_TEXT_FIXER_RULES,
    ]);
    assert!(
        second_run.status.success(),
        "second RAT text fixer run must succeed:\n{}",
        output_text(&second_run)
    );
    let second_stdout = String::from_utf8_lossy(&second_run.stdout);
    assert!(second_stdout.contains("prec-bsl exec-rules: processed "));
    assert!(
        !second_stdout.contains("Modified files:"),
        "second RAT text fixer run must be clean for the same scenario set:\n{}",
        second_stdout
    );

    let after_status = git_status_short(repo).expect("RAT git status must be readable after run");
    assert_eq!(
        after_status, before_status,
        "RAT text idempotence acceptance must not change the real checkout status"
    );
}

#[test]
fn rat_xml_edt_acceptance_runs_on_temp_copy_only() {
    let Some(repo) = rat_repo() else {
        eprintln!(
            "skipping RAT XML/EDT acceptance: /home/alko/develop/open-source/rat is not available"
        );
        return;
    };

    let before_status = git_status_short(repo).expect("RAT git status must be readable");
    let tempdir = TempRatCopy::new().expect("temporary RAT copy directory must be created");
    copy_required_source_roots(repo, tempdir.path())
        .expect("required RAT source roots must copy into a temporary directory");

    let source_dirs = parse_source_dir_list(RAT_SOURCE_DIRS);
    let source_roots = resolve_source_roots(tempdir.path(), &source_dirs);
    assert!(
        source_roots.diagnostics.is_empty(),
        "copied RAT source roots must resolve without diagnostics: {:?}",
        source_roots.diagnostics
    );
    let source_files = collect_prec_bsl_source_files(&source_roots.roots)
        .expect("copied RAT XML/EDT source files must be discoverable through production code");
    assert!(
        source_files.iter().any(|file| {
            file.kind == SourceFileKind::ConfigurationMetadata
                && file.repo_path
                    == Path::new("fixtures/configuration/src/Configuration/Configuration.mdo")
        }),
        "RAT XML/EDT acceptance must discover Configuration.mdo"
    );
    assert!(
        source_files.iter().any(|file| {
            file.kind == SourceFileKind::EdtMetadata
                && file
                    .repo_path
                    .to_string_lossy()
                    .ends_with("/РатJSON/РатJSON.mdo")
        }),
        "RAT XML/EDT acceptance must discover object .mdo files"
    );
    assert!(
        source_files.iter().any(|file| {
            file.kind == SourceFileKind::EdtForm
                && file.repo_path == Path::new(RAT_FORM_CHANGE_PROBE_FILE)
        }),
        "RAT XML/EDT acceptance must discover Form.form files"
    );

    replace_once_in_temp_file(
        tempdir.path(),
        RAT_FULL_TEXT_PROBE_FILE,
        "<fullTextSearch>DontUse</fullTextSearch>",
        "<fullTextSearch>Use</fullTextSearch>",
    );
    replace_once_in_temp_file(
        tempdir.path(),
        RAT_FORM_CHANGE_PROBE_FILE,
        "<allowFormCustomize>false</allowFormCustomize>",
        "<allowFormCustomize>true</allowFormCustomize>",
    );

    let xml_source_dirs = format!(
        "{},{}",
        Path::new(RAT_FULL_TEXT_PROBE_FILE)
            .parent()
            .expect("full-text probe must have parent")
            .display(),
        Path::new(RAT_FORM_CHANGE_PROBE_FILE)
            .parent()
            .expect("form-change probe must have parent")
            .display()
    );
    let first_run = run_prec_bsl([
        "exec-rules",
        tempdir
            .path()
            .to_str()
            .expect("temp RAT path must be UTF-8"),
        "--source-dir",
        xml_source_dirs.as_str(),
        "--rules",
        RAT_XML_EDT_FIXER_RULES,
    ]);
    assert!(
        first_run.status.success(),
        "first RAT XML/EDT fixer run must succeed:\n{}",
        output_text(&first_run)
    );
    let first_stdout = String::from_utf8_lossy(&first_run.stdout);
    assert!(first_stdout.contains("prec-bsl exec-rules: processed "));
    assert!(
        first_stdout.contains("Modified files:"),
        "first RAT XML/EDT fixer run must report modified files:\n{}",
        first_stdout
    );
    assert!(
        first_stdout.contains(RAT_FULL_TEXT_PROBE_FILE),
        "first RAT XML/EDT fixer run must report full-text search probe:\n{}",
        first_stdout
    );
    assert!(
        first_stdout.contains(RAT_FORM_CHANGE_PROBE_FILE),
        "first RAT XML/EDT fixer run must report form-change probe:\n{}",
        first_stdout
    );
    assert!(
        fs::read_to_string(tempdir.path().join(RAT_FULL_TEXT_PROBE_FILE))
            .expect("full-text probe must remain readable")
            .contains("<fullTextSearch>DontUse</fullTextSearch>"),
        "full-text search fixer must disable the seeded RAT metadata property"
    );
    assert!(
        fs::read_to_string(tempdir.path().join(RAT_FORM_CHANGE_PROBE_FILE))
            .expect("form-change probe must remain readable")
            .contains("<allowFormCustomize>false</allowFormCustomize>"),
        "form-change fixer must disable the seeded RAT form property"
    );

    let second_run = run_prec_bsl([
        "exec-rules",
        tempdir
            .path()
            .to_str()
            .expect("temp RAT path must be UTF-8"),
        "--source-dir",
        xml_source_dirs.as_str(),
        "--rules",
        RAT_XML_EDT_FIXER_RULES,
    ]);
    assert!(
        second_run.status.success(),
        "second RAT XML/EDT fixer run must succeed:\n{}",
        output_text(&second_run)
    );
    let second_stdout = String::from_utf8_lossy(&second_run.stdout);
    assert!(second_stdout.contains("prec-bsl exec-rules: processed "));
    assert!(
        !second_stdout.contains("Modified files:"),
        "second RAT XML/EDT fixer run must be clean for the same scenario set:\n{}",
        second_stdout
    );

    let form_before_failure = fs::read_to_string(tempdir.path().join(RAT_XML_FORM_FAILURE_FILE))
        .expect("XML form failure probe must be readable before the diagnostic run");
    let form_correction_run = run_prec_bsl([
        "exec-rules",
        tempdir
            .path()
            .to_str()
            .expect("temp RAT path must be UTF-8"),
        "--source-dir",
        RAT_XML_FORM_FAILURE_DIR,
        "--rules",
        RAT_XML_FORM_RULE,
    ]);
    assert!(
        !form_correction_run.status.success(),
        "RAT XML form correction diagnostic run must fail closed:\n{}",
        output_text(&form_correction_run)
    );
    let form_correction_stdout = String::from_utf8_lossy(&form_correction_run.stdout);
    assert!(
        form_correction_stdout.contains("prec-bsl exec-rules: processed 2 file(s)"),
        "RAT XML form correction diagnostic run must process the copied form directory:\n{}",
        form_correction_stdout
    );
    assert!(
        form_correction_stdout.contains(RAT_XML_FORM_RULE)
            && form_correction_stdout.contains(RAT_XML_FORM_FAILURE_FILE)
            && form_correction_stdout.contains("form element id must be a positive integer: -1"),
        "RAT XML form correction diagnostic must include rule, file, and failure reason:\n{}",
        form_correction_stdout
    );
    let form_after_failure = fs::read_to_string(tempdir.path().join(RAT_XML_FORM_FAILURE_FILE))
        .expect("XML form failure probe must be readable after the diagnostic run");
    assert_eq!(
        form_after_failure, form_before_failure,
        "failed XML form correction run must not partially rewrite the RAT temp copy"
    );

    let after_status = git_status_short(repo).expect("RAT git status must be readable after run");
    assert_eq!(
        after_status, before_status,
        "RAT XML/EDT acceptance must not change the real checkout status"
    );
}

fn replace_once_in_temp_file(repo: &Path, repo_path: &str, from: &str, to: &str) {
    let path = repo.join(repo_path);
    let input = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let count = input.matches(from).count();
    assert!(
        count > 0,
        "RAT temp-copy probe {} must contain at least one replacement target",
        repo_path
    );
    let output = input.replacen(from, to, 1);
    fs::write(&path, output)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
}

fn scenario_strings<'a>(config: &'a Value, key: &str) -> Vec<&'a str> {
    config
        .get(key)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{key} must be an array"))
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("{key} must contain only strings"))
        })
        .collect()
}

fn unsupported_enabled_scenario_diagnostic(scenario: &str) -> Option<String> {
    let normalized = normalize_scenario_id(scenario);
    match find_reference_scenario(scenario).map(|definition| definition.support) {
        Some(ScenarioSupport::RequiredV1) => None,
        Some(ScenarioSupport::Unsupported) => {
            Some(format!("unsupported built-in scenario in v1: {normalized}"))
        }
        None => Some(format!(
            "unsupported repository-local scenario in v1: {normalized}; dynamic local .os execution is not supported in v1"
        )),
    }
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value == extension)
}

fn render_rat_parser_coverage_report(
    total_files: usize,
    files_with_errors: &[(String, usize)],
    total_error_nodes: usize,
) -> String {
    let mut report = String::new();
    report.push_str("RAT parser coverage\n");
    report.push_str("===================\n");
    report.push_str(&format!("parser_roots: {}\n", RAT_PARSER_ROOTS.join(", ")));
    report.push_str(&format!("bsl_files: {total_files}\n"));
    report.push_str(&format!(
        "files_with_parse_errors: {}\n",
        files_with_errors.len()
    ));
    report.push_str(&format!("parse_error_nodes: {total_error_nodes}\n"));
    report.push_str("parse_error_files:\n");

    if files_with_errors.is_empty() {
        report.push_str("- none\n");
    } else {
        for (path, error_count) in files_with_errors {
            report.push_str(&format!("- {path}: {error_count}\n"));
        }
    }

    report
}

fn run_prec_bsl<const N: usize>(args: [&str; N]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_prec-bsl"))
        .args(args)
        .output()
        .expect("prec-bsl binary must run")
}

fn output_text(output: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}
