use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};

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

    assert_eq!(report.results.len(), 3);
    let expected = [(3, "\t;"), (6, "\t)"), (7, "И")];
    for (result, (line, span_text)) in report.results.iter().zip(expected) {
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

    assert!(report.modified_paths().is_empty());
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
