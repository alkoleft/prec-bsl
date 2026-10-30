use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};

mod support;

use support::fixtures::{
    FixtureDiagnostic, FixtureRun, FixtureSeverity, GoldenFixture, assert_checker_fixture,
    assert_fixer_fixture,
};

const LINE_BREAK_RULE: &str = "ПроверкаПереносовСтрок";

/// Модуль с тремя нарушениями IncorrectLineBreak: висячий `;` (строка 3),
/// висячий `);` (строка 6, FIRST-матч `\)` по лево-альтернативе bsl-ls) и
/// бинарный оператор `И` в конце строки (строка 7).
const VIOLATION_MODULE: &str = concat!(
    "Процедура Тест()\n",
    "\tВызов(Арг)\n",
    "\t;\n",
    "\tВызов(\n",
    "\t\tАргумент\n",
    "\t);\n",
    "\tЕсли А = Б И\n",
    "\t\tБ = В Тогда\n",
    "\t\tСообщить(\"готово\");\n",
    "\tКонецЕсли;\n",
    "КонецПроцедуры\n",
);

/// Результат `fix_line_breaks` на [`VIOLATION_MODULE`]: `;` приклеен к
/// вызову, `);` — к аргументу, `И` перенесён в начало следующей строки.
const EXPECTED_FIXED_MODULE: &str = concat!(
    "Процедура Тест()\n",
    "\tВызов(Арг);\n",
    "\tВызов(\n",
    "\t\tАргумент);\n",
    "\tЕсли А = Б\n",
    "\t\tИ Б = В Тогда\n",
    "\t\tСообщить(\"готово\");\n",
    "\tКонецЕсли;\n",
    "КонецПроцедуры\n",
);

#[test]
fn line_break_check_reports_violations_without_modifying_file() {
    let repo = temp_repo("check_violations");
    let repo_path = PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl");
    write_file(repo.join(&repo_path), VIOLATION_MODULE);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПроверкаПереносовСтрок.os"]
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
            mode: PipelineMode::Check,
        },
    );

    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        VIOLATION_MODULE,
        "check mode must not modify the file"
    );

    // DryRun-контракт: 3 per-violation warnings + один modified_dry_run
    // (фикс меняет файл) с proposed_content = результат фикстера.
    assert_eq!(report.results.len(), 4);
    let expected = [(3, "\t;"), (6, "\t)"), (7, "И")];
    for (result, (line, span_text)) in report.results.iter().take(3).zip(expected) {
        assert_eq!(result.rule_id, LINE_BREAK_RULE);
        assert_eq!(result.status, ScenarioResultStatus::Warning);
        assert_eq!(
            result.message,
            format!("incorrect line break at line {line}")
        );

        let span = result
            .source_span
            .as_ref()
            .expect("warning must carry a source span");
        assert_eq!(
            &VIOLATION_MODULE[span.start_byte..span.end_byte],
            span_text,
            "span of line {line} must point at the offending token"
        );
    }

    let dry_run = &report.results[3];
    assert_eq!(dry_run.rule_id, LINE_BREAK_RULE);
    assert_eq!(dry_run.status, ScenarioResultStatus::Modified);
    assert_eq!(dry_run.message, "fixed incorrect line breaks");
    assert_eq!(
        dry_run.proposed_content.as_deref(),
        Some(EXPECTED_FIXED_MODULE),
        "dry-run must propose the fixer output"
    );

    assert_eq!(
        report.modified_paths(),
        vec![repo_path.clone()],
        "modified_dry_run must report the would-be modification"
    );
    assert_eq!(report.exit_code(), 1);
}

#[test]
fn line_break_check_fails_on_invalid_settings() {
    let repo = temp_repo("invalid_settings");
    let repo_path = PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl");
    write_file(repo.join(&repo_path), VIOLATION_MODULE);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();
    let config = parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПроверкаПереносовСтрок.os"],
                "НастройкиСценариев": {
                    "ПроверкаПереносовСтрок": {
                        "checkFirstSymbol": "yes"
                    }
                }
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
            mode: PipelineMode::Check,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, LINE_BREAK_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
    assert_eq!(
        report.results[0].message,
        "line break setting checkFirstSymbol must be a boolean"
    );
    assert_eq!(report.exit_code(), 1);
}

#[test]
fn line_break_check_skips_non_bsl_files() {
    let repo = temp_repo("non_bsl_file");
    // EDT-файл метаданных внутри source root: не BslModule → skipped.
    let repo_path = PathBuf::from("src/CommonModules/Тестовый/Тестовый.mdo");
    write_file(
        repo.join(&repo_path),
        "<mdclass:CommonModule xmlns:mdclass=\"http://g5.1c.ru/v8/dt/metadata\"/>",
    );

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();
    let config = parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПроверкаПереносовСтрок.os"]
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
            mode: PipelineMode::Check,
        },
    );

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].rule_id, LINE_BREAK_RULE);
    assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
    assert_eq!(
        report.results[0].message,
        "scenario handles only BSL modules"
    );
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn line_break_hook_applies_fix_and_is_idempotent() {
    let repo = temp_repo("hook_apply");
    let repo_path = PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl");
    write_file(repo.join(&repo_path), VIOLATION_MODULE);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = line_break_config();

    let first = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file.clone()],
            mode: PipelineMode::Hook,
        },
    );

    // Apply: единственный результат — modified; warnings не эмитятся.
    assert_eq!(first.results.len(), 1);
    assert_eq!(first.results[0].rule_id, LINE_BREAK_RULE);
    assert_eq!(first.results[0].status, ScenarioResultStatus::Modified);
    assert_eq!(first.results[0].message, "fixed incorrect line breaks");
    assert_eq!(first.results[0].modified_paths, vec![repo_path.clone()]);
    assert_eq!(first.modified_paths(), vec![repo_path.clone()]);
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        EXPECTED_FIXED_MODULE,
        "hook must apply the fixer"
    );
    assert_eq!(first.hook_exit_code(), 1);

    // Повторный прогон на фиксированном файле: clean, exit 0.
    let second = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert!(second.results.is_empty(), "second run must be clean");
    assert_eq!(second.hook_exit_code(), 0);
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        EXPECTED_FIXED_MODULE
    );
}

#[test]
fn line_break_fix_cli_writes_and_is_idempotent() {
    let repo = temp_git_repo("fix_cli");
    let repo_path = PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl");
    write_file(repo.join(&repo_path), VIOLATION_MODULE);
    write_file(
        repo.join("v8config.json"),
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПроверкаПереносовСтрок.os"]
            }
        }"#,
    );
    run_git(&repo, ["add", "."]);
    run_git(&repo, ["commit", "-m", "baseline"]);

    let first = run_prec_bsl(
        &repo,
        [
            "fix",
            repo.to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            LINE_BREAK_RULE,
            "--mode",
            "all",
        ],
    );

    assert_eq!(
        first.status.code(),
        Some(1),
        "fix with modifications must exit 1: {}",
        output_text(&first)
    );
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        EXPECTED_FIXED_MODULE,
        "fix must apply the fixer"
    );

    let second = run_prec_bsl(
        &repo,
        [
            "fix",
            repo.to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            LINE_BREAK_RULE,
            "--mode",
            "all",
        ],
    );

    assert_eq!(
        second.status.code(),
        Some(0),
        "idempotent second run must exit 0: {}",
        output_text(&second)
    );
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        EXPECTED_FIXED_MODULE
    );
}

/// Файл из только-skip-нарушений: `check` видит их (warnings, exit 1), `fix`
/// молча пропускает (exit 0, файл не изменён) — обе стороны контракта.
#[test]
fn line_break_skip_only_module_check_vs_fix() {
    let repo = temp_git_repo("skip_only");
    // `\t;` после `Часть();` — защита `;;`: нарушение остаётся для ручной
    // правки, фиксер его не трогает.
    let skip_module = "Процедура Тест()\n\tЧасть();\n\t;\nКонецПроцедуры\n";
    let repo_path = PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl");
    write_file(repo.join(&repo_path), skip_module);
    write_file(
        repo.join("v8config.json"),
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПроверкаПереносовСтрок.os"]
            }
        }"#,
    );
    run_git(&repo, ["add", "."]);
    run_git(&repo, ["commit", "-m", "baseline"]);

    let check = run_prec_bsl(
        &repo,
        [
            "check",
            repo.to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            LINE_BREAK_RULE,
            "--mode",
            "all",
        ],
    );

    assert_eq!(
        check.status.code(),
        Some(1),
        "check must report skip-only violations: {}",
        output_text(&check)
    );
    let stdout = String::from_utf8_lossy(&check.stdout);
    assert!(
        stdout.contains("неправильный перенос выражения (строка 3)"),
        "check text output must carry the RU-localized warning:\n{stdout}"
    );
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        skip_module,
        "check must not modify the file"
    );

    let fix = run_prec_bsl(
        &repo,
        [
            "fix",
            repo.to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            LINE_BREAK_RULE,
            "--mode",
            "all",
        ],
    );

    assert_eq!(
        fix.status.code(),
        Some(0),
        "fix on a skip-only module must exit 0 silently: {}",
        output_text(&fix)
    );
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        skip_module,
        "fix must not touch the skip-only module"
    );
}

/// Текстовый вывод check-режима содержит RU-локализацию предупреждений
/// (`неправильный перенос выражения (строка N)`) — закрытие Low-находки
/// ревью Task 2.
#[test]
fn line_break_check_text_output_is_localized() {
    let repo = temp_git_repo("check_localized");
    let repo_path = PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl");
    write_file(repo.join(&repo_path), VIOLATION_MODULE);
    write_file(
        repo.join("v8config.json"),
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПроверкаПереносовСтрок.os"]
            }
        }"#,
    );
    run_git(&repo, ["add", "."]);
    run_git(&repo, ["commit", "-m", "baseline"]);

    let output = run_prec_bsl(
        &repo,
        [
            "check",
            repo.to_str().unwrap(),
            "--source-dir",
            "src",
            "--rules",
            LINE_BREAK_RULE,
            "--mode",
            "all",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in [3, 6, 7] {
        assert!(
            stdout.contains(&format!("неправильный перенос выражения (строка {line})")),
            "check output must localize warning for line {line}:\n{stdout}"
        );
    }
    assert!(
        stdout.contains("исправлены неправильные переносы строк"),
        "check output must localize the dry-run modification message:\n{stdout}"
    );
    // Dry-run diff-превью подхватывает proposed_content автоматически.
    assert!(stdout.contains("Предлагаемые изменения (не записаны):"));
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        VIOLATION_MODULE,
        "check must not modify the file"
    );
}

fn line_break_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПроверкаПереносовСтрок.os"]
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
        .join("line-break-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary line-break test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// Git-репозиторий для CLI-спавн-тестов (прецедент `check_fix_modes.rs`).
fn temp_git_repo(test_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after UNIX_EPOCH")
        .as_nanos();
    let path = std::env::current_dir()
        .expect("current dir must be available")
        .join("target")
        .join("line-break-tests")
        .join(format!(
            "cli_{}_{}_{}",
            std::process::id(),
            test_name,
            nonce
        ));
    fs::create_dir_all(&path).expect("temporary git repo must be created");
    run_git(&path, ["init"]);
    run_git(&path, ["config", "user.email", "prec-bsl@example.invalid"]);
    run_git(&path, ["config", "user.name", "prec-bsl tests"]);
    path
}

fn run_git<const N: usize>(repo: &Path, args: [&str; N]) {
    let output = std::process::Command::new("git")
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

fn run_prec_bsl<const N: usize>(cwd: impl AsRef<Path>, args: [&str; N]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_prec-bsl"))
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap()
}

fn output_text(output: &std::process::Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Golden-фикстуры фикстера (`assert_fixer_fixture`: golden output +
/// идемпотентность второго прогона).
#[test]
fn line_break_golden_simple() {
    let fixture = GoldenFixture::new(
        "tests/fixtures/golden/ПроверкаПереносовСтрок/simple",
        PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl"),
    );
    assert_fixer_fixture(&fixture, |input, path| {
        line_break_fixer_pipeline_run("golden_simple", input, path)
    });
}

#[test]
fn line_break_golden_strings() {
    let fixture = GoldenFixture::new(
        "tests/fixtures/golden/ПроверкаПереносовСтрок/strings",
        PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl"),
    );
    assert_fixer_fixture(&fixture, |input, path| {
        line_break_fixer_pipeline_run("golden_strings", input, path)
    });
}

#[test]
fn line_break_golden_query_region() {
    let fixture = GoldenFixture::new(
        "tests/fixtures/golden/ПроверкаПереносовСтрок/query_region",
        PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl"),
    );
    assert_fixer_fixture(&fixture, |input, path| {
        line_break_fixer_pipeline_run("golden_query_region", input, path)
    });
}

#[test]
fn line_break_golden_line_endings() {
    let fixture = GoldenFixture::new(
        "tests/fixtures/golden/ПроверкаПереносовСтрок/line_endings",
        PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl"),
    );
    assert_fixer_fixture(&fixture, |input, path| {
        line_break_fixer_pipeline_run("golden_line_endings", input, path)
    });
}

#[test]
fn line_break_golden_comments() {
    let fixture = GoldenFixture::new(
        "tests/fixtures/golden/ПроверкаПереносовСтрок/comments",
        PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl"),
    );
    assert_fixer_fixture(&fixture, |input, path| {
        line_break_fixer_pipeline_run("golden_comments", input, path)
    });
}

/// Skip-кейсы (`,`-серия, `;;`, оператор перед комментарием) — check-контракт:
/// диагностики без изменения input. Второй прогон Apply не обязан быть
/// diagnostics-empty на skip-кейсах (ревью F3/S1), поэтому не
/// `assert_fixer_fixture`.
#[test]
fn line_break_golden_skip_cases() {
    let fixture = GoldenFixture::new(
        "tests/fixtures/golden/ПроверкаПереносовСтрок/skip_cases",
        PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl"),
    );
    assert_checker_fixture(&fixture, |input, path| {
        line_break_checker_pipeline_run("golden_skip_cases", input, path)
    });
}

/// Прогон pipeline в Apply-режиме (как hook) для golden-фикстур фикстера.
fn line_break_fixer_pipeline_run(test_name: &str, input: &str, path: &Path) -> FixtureRun {
    let repo = temp_repo(test_name);
    write_file(repo.join(path), input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, path.to_path_buf(), None).unwrap();
    let config = line_break_config();

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
            assert_eq!(result.rule_id, LINE_BREAK_RULE);
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
        if diagnostics.is_empty() { 0 } else { 1 },
        "hook exit code must block on unreviewed fixer modifications"
    );
    assert_eq!(
        report.modified_paths(),
        if diagnostics.is_empty() {
            Vec::new()
        } else {
            vec![path.to_path_buf()]
        },
        "modified paths must report every changed file"
    );

    FixtureRun::new(output, diagnostics)
}

/// Прогон pipeline в DryRun-режиме (check) для golden-фикстур чекера: только
/// warning-диагностики, input не меняется.
fn line_break_checker_pipeline_run(test_name: &str, input: &str, path: &Path) -> FixtureRun {
    let repo = temp_repo(test_name);
    write_file(repo.join(path), input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, path.to_path_buf(), None).unwrap();
    let config = line_break_config();

    let report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::Check,
        },
    );

    let diagnostics: Vec<FixtureDiagnostic> = report
        .results
        .iter()
        .filter(|result| result.status == ScenarioResultStatus::Warning)
        .map(|result| {
            assert_eq!(result.rule_id, LINE_BREAK_RULE);
            FixtureDiagnostic::new(
                FixtureSeverity::Warning,
                result.rule_id.clone(),
                path,
                result.message.clone(),
            )
        })
        .collect();

    // Skip-only вход: фиксер ничего не меняет — modified_dry_run не эмитится.
    assert!(
        !report
            .results
            .iter()
            .any(|result| result.status.is_modification()),
        "skip-only fixtures must not produce dry-run modifications"
    );

    FixtureRun::new(input.to_owned(), diagnostics)
}
