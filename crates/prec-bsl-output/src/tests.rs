use std::path::PathBuf;

use serde_json::Value;

use super::*;

#[test]
fn output_text_groups_messages_and_separates_modified_files_and_failures() {
    let report = sample_report();

    let output = render_text_report(&report);

    assert_eq!(
        output,
        concat!(
            "prec-bsl hook: processed 2 file(s)\n",
            "\n",
            "Modified files:\n",
            "  src/Исправленный.bsl\n",
            "\n",
            "Hard failures:\n",
            "  [ПроверкаНецензурныхСлов] src/Плохой.bsl: matched dictionary word\n",
            "\n",
            "Messages:\n",
            "  ПроверкаНецензурныхСлов\n",
            "    src/Плохой.bsl\n",
            "      hard_failure: matched dictionary word [7..13]\n",
            "  УдалениеЛишнихКонцевыхПробелов\n",
            "    src/Исправленный.bsl\n",
            "      modified: removed trailing whitespace\n",
            "      warning: file contained tabs\n",
            "\n",
            "Exit code: 1\n"
        )
    );
}

#[test]
fn output_text_is_stable_for_shuffled_results() {
    let original = sample_report();
    let mut shuffled = original.clone();
    shuffled.results.reverse();

    assert_eq!(render_text_report(&original), render_text_report(&shuffled));
}

#[test]
fn output_text_renders_unsupported_as_blocking_failure() {
    let report = PipelineReport {
        mode: PipelineMode::ExecRules,
        processed_files: vec![PathBuf::from("src/Модуль.bsl")],
        results: vec![ScenarioResult::unsupported(
            "ЛокальныйСценарий",
            "src/Модуль.bsl",
            "dynamic local .os execution is not supported in v1",
        )],
    };

    let output = render_text_report(&report);

    assert!(output.contains("Hard failures:\n  [ЛокальныйСценарий] src/Модуль.bsl"));
    assert!(output.contains("unsupported: dynamic local .os execution is not supported in v1"));
    assert!(output.contains("Exit code: 1"));
}

#[test]
fn output_json_contains_stable_contract_fields_and_source_span() {
    let report = sample_report();

    let output = render_json_report(&report);
    let json = serde_json::from_str::<Value>(&output).unwrap();
    let results = json["results"].as_array().unwrap();

    assert_eq!(json["mode"], "hook");
    assert_eq!(json["exit_code"], 1);
    assert_eq!(json["processed_files"][0], "src/Исправленный.bsl");
    assert_eq!(json["modified_files"][0], "src/Исправленный.bsl");

    for result in results {
        for field in ["rule_id", "path", "severity", "modified", "message"] {
            assert!(
                result.get(field).is_some(),
                "missing required JSON result field {field}: {result}"
            );
        }
    }

    let profanity = results
        .iter()
        .find(|result| result["rule_id"] == "ПроверкаНецензурныхСлов")
        .unwrap();
    assert_eq!(profanity["path"], "src/Плохой.bsl");
    assert_eq!(profanity["severity"], "hard_failure");
    assert_eq!(profanity["modified"], false);
    assert_eq!(profanity["message"], "matched dictionary word");
    assert_eq!(profanity["source_span"]["start_byte"], 7);
    assert_eq!(profanity["source_span"]["end_byte"], 13);

    let trailing_whitespace = results
        .iter()
        .find(|result| result["severity"] == "modified")
        .unwrap();
    assert_eq!(
        trailing_whitespace["modified_paths"][0],
        "src/Исправленный.bsl"
    );
}

#[test]
fn render_report_dispatches_by_cli_format() {
    let report = PipelineReport {
        mode: PipelineMode::ExecRules,
        processed_files: Vec::new(),
        results: Vec::new(),
    };

    assert!(render_report(&report, OutputFormat::Text).contains("prec-bsl exec-rules"));
    assert!(render_report(&report, OutputFormat::Json).contains("\"mode\": \"exec-rules\""));
}

fn sample_report() -> PipelineReport {
    PipelineReport {
        mode: PipelineMode::Hook,
        processed_files: vec![
            PathBuf::from("src/Исправленный.bsl"),
            PathBuf::from("src/Плохой.bsl"),
        ],
        results: vec![
            ScenarioResult::warning(
                "УдалениеЛишнихКонцевыхПробелов",
                "src/Исправленный.bsl",
                "file contained tabs",
            ),
            ScenarioResult::hard_failure(
                "ПроверкаНецензурныхСлов",
                "src/Плохой.bsl",
                "matched dictionary word",
            )
            .with_source_span(SourceSpan::new(7, 13)),
            ScenarioResult::modified(
                "УдалениеЛишнихКонцевыхПробелов",
                "src/Исправленный.bsl",
                "removed trailing whitespace",
            ),
        ],
    }
}
