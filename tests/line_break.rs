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
fn line_break_check_fails_on_non_object_settings() {
    let repo = temp_repo("non_object_settings");
    let repo_path = PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl");
    write_file(repo.join(&repo_path), VIOLATION_MODULE);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path, None).unwrap();
    let config = parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПроверкаПереносовСтрок.os"],
                "НастройкиСценариев": {
                    "ПроверкаПереносовСтрок": ["checkFirstSymbol"]
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
        "line break settings must be a settings object"
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

/// Task 4: JSON-контракт `check --format json`: per-violation warnings с
/// `severity`/`rule_id`/`message`/`source_span` плюс `modified_dry_run` с
/// `proposed_content`; файл байт-в-байт не изменён; exit 1; stdout — один
/// JSON-документ без text diff trailer.
#[test]
fn line_break_check_json_output_contract() {
    let repo = temp_git_repo("check_json");
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
            "--format",
            "json",
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(1),
        "check with findings must exit 1: {}",
        output_text(&output)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|error| panic!("stdout must parse as JSON: {error}\n{stdout}"));
    assert_eq!(json["mode"], "check");
    assert_eq!(json["exit_code"], 1);

    let results = json["results"].as_array().unwrap();
    assert_eq!(results.len(), 4, "3 warnings + 1 dry-run modification");

    // DryRun-контракт: модификация-превью с содержимым фикстера.
    let modified = results
        .iter()
        .find(|result| result["severity"] == "modified")
        .expect("dry-run JSON must include the would-be modification");
    assert_eq!(modified["rule_id"], LINE_BREAK_RULE);
    assert_eq!(modified["message"], "fixed incorrect line breaks");
    assert_eq!(modified["proposed_content"], EXPECTED_FIXED_MODULE);
    assert_eq!(modified["modified"], true);

    // Per-violation warnings: span указывает на нарушающий токен в исходнике.
    let warnings: Vec<&serde_json::Value> = results
        .iter()
        .filter(|result| result["severity"] == "warning")
        .collect();
    let expected = [(3, "\t;"), (6, "\t)"), (7, "И")];
    assert_eq!(warnings.len(), expected.len(), "warnings:\n{warnings:#?}");
    for (warning, (line, span_text)) in warnings.iter().zip(expected) {
        assert_eq!(warning["rule_id"], LINE_BREAK_RULE);
        assert_eq!(
            warning["message"],
            format!("incorrect line break at line {line}")
        );
        let span = warning["source_span"]
            .as_object()
            .unwrap_or_else(|| panic!("warning for line {line} must carry a source span"));
        let start = span["start_byte"].as_u64().unwrap() as usize;
        let end = span["end_byte"].as_u64().unwrap() as usize;
        assert_eq!(
            &VIOLATION_MODULE[start..end],
            span_text,
            "span of line {line} must point at the offending token"
        );
    }

    assert!(
        !stdout.contains("Предлагаемые"),
        "JSON output must not be followed by the text diff trailer:\n{stdout}"
    );
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        VIOLATION_MODULE,
        "check must not modify the file"
    );
}

/// Task 4: `fix` затем `check` на полностью фиксируемом модуле — check
/// выходит 0 и не находит ни одного нарушения (пустой JSON `results`).
#[test]
fn line_break_check_after_fix_reports_zero_findings() {
    let repo = temp_git_repo("check_after_fix");
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
        Some(1),
        "fix with modifications must exit 1: {}",
        output_text(&fix)
    );
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        EXPECTED_FIXED_MODULE,
        "fix must apply the fixer"
    );

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
            "--format",
            "json",
        ],
    );

    assert_eq!(
        check.status.code(),
        Some(0),
        "check after fix must exit 0: {}",
        output_text(&check)
    );
    let stdout = String::from_utf8_lossy(&check.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|error| panic!("stdout must parse as JSON: {error}\n{stdout}"));
    assert_eq!(json["mode"], "check");
    assert_eq!(json["exit_code"], 0);
    assert!(
        json["results"].as_array().unwrap().is_empty(),
        "fixed module must yield zero findings:\n{stdout}"
    );
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        EXPECTED_FIXED_MODULE,
        "check must not modify the fixed file"
    );
}

/// Модуль для межсценарного инварианта: 4 полностью фиксируемых нарушения —
/// висячий `;` (строка 3), висячий `);` (строка 7), END-оператор `И` с
/// хвостовым пробелом (строка 8 — rstrip-кейс: после снятия оператора prev
/// не должен оставить хвост для `УдалениеЛишнихКонцевыхПробелов`), END-оператор
/// `+` (строка 10). Без check-only `,`-кейсов и prev-с-комментарием skip-кейсов.
/// Код вокруг подобран под взаимодействие с другими фиксерами: вызовы с
/// запятыми (приклейка `;` не создаёт запятую без пробела — ПробелыПослеЗапятых),
/// канонические написания ключевых слов.
const CROSS_SCENARIO_MODULE: &str = concat!(
    "Процедура ВычислитьИтог(Арг1, Арг2)\n",
    "\tВызов(Арг1, Арг2)\n",
    "\t;\n",
    "\tВызов(\n",
    "\t\tАрг1,\n",
    "\t\tАрг2\n",
    "\t);\n",
    "\tЕсли А = Б И \n",
    "\t\tБ = В Тогда\n",
    "\t\tСумма = Арг1 +\n",
    "\t\t\tАрг2;\n",
    "\t\tСообщить(\"готово\");\n",
    "\tКонецЕсли;\n",
    "КонецПроцедуры\n",
);

/// Ожидаемый вывод `fix_line_breaks` на [`CROSS_SCENARIO_MODULE`]: токены
/// приклеены к prev-строкам, операторы перенесены в начало следующих строк,
/// prev после снятия оператора rstrip (нет хвостовых пробелов).
const CROSS_SCENARIO_FIXED_MODULE: &str = concat!(
    "Процедура ВычислитьИтог(Арг1, Арг2)\n",
    "\tВызов(Арг1, Арг2);\n",
    "\tВызов(\n",
    "\t\tАрг1,\n",
    "\t\tАрг2);\n",
    "\tЕсли А = Б\n",
    "\t\tИ Б = В Тогда\n",
    "\t\tСумма = Арг1\n",
    "\t\t\t+ Арг2;\n",
    "\t\tСообщить(\"готово\");\n",
    "\tКонецЕсли;\n",
    "КонецПроцедуры\n",
);

/// Task 5: межсценарный инвариант — фикс `ПроверкаПереносовСтрок` не создаёт
/// новых нарушений других правил. Полный прогон всех RequiredV1-сценариев +
/// `ПроверкаПереносовСтрок` + `ПробелыПослеЗапятых`; line_break стоит первым
/// в списке, поэтому каждый следующий сценарий (включая
/// `УдалениеЛишнихКонцевыхПробелов` и `ПробелыПослеЗапятых`) исполняется уже
/// на выводе фикса. Повторный полный прогон на фиксированном файле: 0
/// Modified/Warning/HardFailure от других сценариев (Skipped не считается),
/// line_break чист, exit 0.
#[test]
fn line_break_cross_scenario_invariant_after_fix() {
    let repo = temp_repo("cross_scenario");
    let repo_path = PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl");
    write_file(repo.join(&repo_path), CROSS_SCENARIO_MODULE);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = parse_config_str(CROSS_SCENARIO_CONFIG).unwrap();

    // Первый полный Apply-прогон: line_break первым — остальные сценарии
    // видят уже зафиксированный файл.
    let first = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file.clone()],
            mode: PipelineMode::Fix,
        },
    );

    assert_no_cross_scenario_findings(&first, "first full run");
    let line_break_results: Vec<&prec_bsl::scenario_pipeline::ScenarioResult> = first
        .results
        .iter()
        .filter(|result| result.rule_id == LINE_BREAK_RULE)
        .collect();
    assert_eq!(
        line_break_results.len(),
        1,
        "first run must apply exactly one line-break modification"
    );
    assert_eq!(line_break_results[0].status, ScenarioResultStatus::Modified);
    assert_eq!(
        first.modified_paths(),
        vec![repo_path.clone()],
        "the line-break fixer must be the only modifier"
    );
    assert_eq!(first.exit_code(), 1);
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        CROSS_SCENARIO_FIXED_MODULE,
        "first run must apply the expected fix"
    );

    // Повторный полный прогон на фиксированном файле: идемпотентность всего
    // набора — 0 Modified/Warning/HardFailure от других сценариев, line_break
    // чист (Apply не эмитит ничего на чистом файле), exit 0.
    let second = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file.clone()],
            mode: PipelineMode::Fix,
        },
    );

    assert_no_cross_scenario_findings(&second, "second full run");
    assert!(
        second
            .results
            .iter()
            .all(|result| result.rule_id != LINE_BREAK_RULE),
        "second run must leave no line-break results:\n{:?}",
        second.results
    );
    assert_eq!(second.exit_code(), 0);
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        CROSS_SCENARIO_FIXED_MODULE,
        "second run must not change the fixed file"
    );

    // Детекторное доказательство чистоты: Check-прогон line_break на выводе
    // фикса не находит ни одного нарушения (Apply-clean не различает
    // «чисто» и «skip-only», check различает).
    let check = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &line_break_config(),
            files: vec![file],
            mode: PipelineMode::Check,
        },
    );

    assert!(
        check.results.is_empty(),
        "detector must find no violations on the fixer output:\n{:?}",
        check.results
    );
    assert_eq!(check.exit_code(), 0);
    assert_eq!(
        fs::read_to_string(repo.join(&repo_path)).unwrap(),
        CROSS_SCENARIO_FIXED_MODULE,
        "check must not modify the fixed file"
    );
}

/// Инвариант межсценарной чистоты: в отчёте нет результатов со статусом
/// Modified/Warning/HardFailure от сценариев, отличных от line_break
/// (Skipped не считается — XML/платформенные сценарии пропускают BSL-модуль).
fn assert_no_cross_scenario_findings(
    report: &prec_bsl::scenario_pipeline::PipelineReport,
    phase: &str,
) {
    let offenders: Vec<String> = report
        .results
        .iter()
        .filter(|result| {
            result.rule_id != LINE_BREAK_RULE
                && matches!(
                    result.status,
                    ScenarioResultStatus::Modified
                        | ScenarioResultStatus::Warning
                        | ScenarioResultStatus::HardFailure
                )
        })
        .map(|result| {
            format!(
                "[{}] {}: {}",
                match result.status {
                    ScenarioResultStatus::Modified => "modified",
                    ScenarioResultStatus::Warning => "warning",
                    ScenarioResultStatus::HardFailure => "hard_failure",
                    _ => "other",
                },
                result.rule_id,
                result.message
            )
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "{phase}: другие сценарии не должны находить нарушений \
         на фиксированном line_break файле:\n{}",
        offenders.join("\n")
    );
}

/// Конфиг полного прогона: `ПроверкаПереносовСтрок` первой (остальные
/// сценарии исполняются на выводе фикса), затем все RequiredV1-сценарии
/// (список `EXPECTED_REQUIRED_V1` / reference-v8config.json) и
/// `ПробелыПослеЗапятых` для проверки приклейки `;` к строкам с запятыми.
const CROSS_SCENARIO_CONFIG: &str = r#"{
    "Precommt4onecСценарии": {
        "ГлобальныеСценарии": [
            "ПроверкаПереносовСтрок.os",
            "ВставкаКопирайтов.os",
            "ДобавлениеПробеловПередКлючевымиСловами.os",
            "ЗапретИспользованияПерейти.os",
            "ИсправлениеНеКаноническогоНаписания.os",
            "КорректировкаXMLФорм.os",
            "ОтключениеПолнотекстовогоПоиска.os",
            "ОтключениеРазрешенияИзменятьФорму.os",
            "ПроверкаДублейПроцедурИФункций.os",
            "ПроверкаКорректностиИнструкцийПрепроцессора.os",
            "ПроверкаКорректностиОбластей.os",
            "ПроверкаНецензурныхСлов.os",
            "РазборОтчетовОбработокРасширений.os",
            "СинхронизацияОбъектовМетаданныхИФайлов.os",
            "СортировкаСостава.os",
            "УдалениеДублейМетаданных.os",
            "УдалениеЛишнихКонцевыхПробелов.os",
            "УдалениеЛишнихПустыхСтрок.os",
            "ПробелыПослеЗапятых.os"
        ]
    }
}"#;

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
