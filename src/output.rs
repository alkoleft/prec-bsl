use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde::Serialize;

use crate::scenario_pipeline::{
    PipelineMode, PipelineReport, ScenarioResult, ScenarioResultStatus, SourceSpan,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Text
    }
}

pub fn render_report(report: &PipelineReport, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => render_text_report(report),
        OutputFormat::Json => render_json_report(report),
    }
}

pub fn render_text_report(report: &PipelineReport) -> String {
    let mut output = String::new();

    writeln!(
        output,
        "prec-bsl {}: processed {} file(s)",
        mode_label(report.mode),
        report.processed_files.len()
    )
    .expect("writing to a String must not fail");

    let modified_paths = report.modified_paths();
    if !modified_paths.is_empty() {
        writeln!(output).expect("writing to a String must not fail");
        writeln!(output, "Modified files:").expect("writing to a String must not fail");
        for path in modified_paths {
            writeln!(output, "  {}", display_path(&path))
                .expect("writing to a String must not fail");
        }
    }

    let hard_failures = sorted_results(
        report
            .results
            .iter()
            .filter(|result| result.status.is_blocking()),
    );
    if !hard_failures.is_empty() {
        writeln!(output).expect("writing to a String must not fail");
        writeln!(output, "Hard failures:").expect("writing to a String must not fail");
        for result in hard_failures {
            writeln!(
                output,
                "  [{}] {}: {}",
                result.rule_id,
                display_path(&result.path),
                result.message
            )
            .expect("writing to a String must not fail");
        }
    }

    let sorted_results = sorted_results(report.results.iter());
    let grouped = grouped_results(sorted_results);
    if !grouped.is_empty() {
        writeln!(output).expect("writing to a String must not fail");
        writeln!(output, "Messages:").expect("writing to a String must not fail");
        for (rule_id, by_path) in grouped {
            writeln!(output, "  {rule_id}").expect("writing to a String must not fail");
            for (path, results) in by_path {
                writeln!(output, "    {}", display_path(path))
                    .expect("writing to a String must not fail");
                for result in results {
                    writeln!(
                        output,
                        "      {}: {}{}",
                        severity_label(result.status),
                        result.message,
                        span_suffix(result.source_span)
                    )
                    .expect("writing to a String must not fail");
                }
            }
        }
    }

    writeln!(output).expect("writing to a String must not fail");
    writeln!(output, "Exit code: {}", report.exit_code())
        .expect("writing to a String must not fail");

    output
}

pub fn render_json_report(report: &PipelineReport) -> String {
    let payload = JsonReport {
        mode: mode_label(report.mode),
        exit_code: report.exit_code(),
        processed_files: report
            .processed_files
            .iter()
            .map(|path| display_path(path))
            .collect(),
        modified_files: report
            .modified_paths()
            .iter()
            .map(|path| display_path(path))
            .collect(),
        results: sorted_json_results(&report.results),
    };

    serde_json::to_string_pretty(&payload).expect("output payload must be serializable")
}

fn grouped_results<'a>(
    results: Vec<&'a ScenarioResult>,
) -> BTreeMap<&'a str, BTreeMap<&'a Path, Vec<&'a ScenarioResult>>> {
    let mut grouped = BTreeMap::new();
    for result in results {
        grouped
            .entry(result.rule_id.as_str())
            .or_insert_with(BTreeMap::new)
            .entry(result.path.as_path())
            .or_insert_with(Vec::new)
            .push(result);
    }
    grouped
}

fn sorted_json_results(results: &[ScenarioResult]) -> Vec<JsonResult> {
    let sorted = sorted_results(results.iter());

    sorted
        .into_iter()
        .map(|result| JsonResult {
            rule_id: result.rule_id.clone(),
            path: display_path(&result.path),
            severity: severity_label(result.status),
            modified: result.status.is_modification() || !result.modified_paths.is_empty(),
            message: result.message.clone(),
            source_span: result.source_span.map(JsonSourceSpan::from),
            modified_paths: result
                .modified_paths
                .iter()
                .map(|path| display_path(path))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        })
        .collect()
}

fn sorted_results<'a>(
    results: impl IntoIterator<Item = &'a ScenarioResult>,
) -> Vec<&'a ScenarioResult> {
    let mut sorted = results.into_iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| {
        (
            left.rule_id.as_str(),
            left.path.as_path(),
            severity_label(left.status),
            left.message.as_str(),
            left.source_span
                .map(|span| (span.start_byte, span.end_byte)),
        )
            .cmp(&(
                right.rule_id.as_str(),
                right.path.as_path(),
                severity_label(right.status),
                right.message.as_str(),
                right
                    .source_span
                    .map(|span| (span.start_byte, span.end_byte)),
            ))
    });

    sorted
}

fn span_suffix(source_span: Option<SourceSpan>) -> String {
    source_span
        .map(|span| format!(" [{}..{}]", span.start_byte, span.end_byte))
        .unwrap_or_default()
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn mode_label(mode: PipelineMode) -> &'static str {
    match mode {
        PipelineMode::Hook => "hook",
        PipelineMode::ExecRules => "exec-rules",
    }
}

fn severity_label(status: ScenarioResultStatus) -> &'static str {
    match status {
        ScenarioResultStatus::Modified => "modified",
        ScenarioResultStatus::Warning => "warning",
        ScenarioResultStatus::HardFailure => "hard_failure",
        ScenarioResultStatus::Skipped => "skipped",
        ScenarioResultStatus::Unsupported => "unsupported",
    }
}

#[derive(Debug, Serialize)]
struct JsonReport {
    mode: &'static str,
    exit_code: i32,
    processed_files: Vec<String>,
    modified_files: Vec<String>,
    results: Vec<JsonResult>,
}

#[derive(Debug, Serialize)]
struct JsonResult {
    rule_id: String,
    path: String,
    severity: &'static str,
    modified: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_span: Option<JsonSourceSpan>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    modified_paths: Vec<String>,
}

#[derive(Debug, Serialize)]
struct JsonSourceSpan {
    start_byte: usize,
    end_byte: usize,
}

impl From<SourceSpan> for JsonSourceSpan {
    fn from(source_span: SourceSpan) -> Self {
        Self {
            start_byte: source_span.start_byte,
            end_byte: source_span.end_byte,
        }
    }
}

#[cfg(test)]
mod tests {
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
}
