use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde::Serialize;

use prec_bsl_pipeline::{
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

/// Renders simple `+`/`-` line diffs for every dry-run result carrying
/// `proposed_content`. The original file content is read from `repo_root`.
///
/// Returns an empty string when there is nothing to show.
pub fn render_dry_run_diffs(report: &PipelineReport, repo_root: &Path) -> String {
    let mut output = String::new();
    for result in sorted_results(report.results.iter()) {
        let Some(proposed) = result.proposed_content.as_deref() else {
            continue;
        };
        if !result.status.is_modification() {
            continue;
        }

        let absolute_path = repo_root.join(&result.path);
        let original = std::fs::read_to_string(&absolute_path).unwrap_or_default();
        writeln!(output, "{}", display_path(&result.path))
            .expect("writing to a String must not fail");
        for line in render_line_diff(&original, proposed) {
            writeln!(output, "{line}").expect("writing to a String must not fail");
        }
        writeln!(output).expect("writing to a String must not fail");
    }
    output
}

/// Line diff used for dry-run previews: every removed line gets `-`, every
/// added line gets `+`. Common lines are omitted. Unified hunks are
/// deliberately out of scope.
///
/// Implemented with Hirschberg's linear-space LCS: the classic
/// dynamic-programming table would need `O(n*m)` memory (a 10 000-line file
/// pair would allocate hundreds of megabytes), while this implementation
/// keeps only a few one-dimensional score rows (`O(n+m)` memory) at the cost
/// of an extra pass per recursion level. The emitted diff is an LCS diff of
/// the same quality as the quadratic version.
fn render_line_diff(original: &str, proposed: &str) -> Vec<String> {
    let original_lines: Vec<&str> = original.lines().collect();
    let proposed_lines: Vec<&str> = proposed.lines().collect();

    let mut diff_lines = Vec::new();
    collect_lcs_diff(&original_lines, &proposed_lines, &mut diff_lines);
    diff_lines
}

/// Hirschberg divide-and-conquer LCS diff over line slices: emits `-line` for
/// every removed original line and `+line` for every added proposed line,
/// skipping common lines.
fn collect_lcs_diff(original: &[&str], proposed: &[&str], output: &mut Vec<String>) {
    if original.is_empty() {
        output.extend(proposed.iter().map(|line| format!("+{line}")));
        return;
    }
    if proposed.is_empty() {
        output.extend(original.iter().map(|line| format!("-{line}")));
        return;
    }

    // Base case: one side is a single line. The LCS is 1 when the line occurs
    // on the other side, 0 otherwise, so the diff is a straightforward
    // add/remove of the unmatched lines.
    if original.len() == 1 {
        let original_line = original[0];
        match proposed.iter().position(|line| *line == original_line) {
            Some(match_index) => {
                output.extend(
                    proposed[..match_index]
                        .iter()
                        .map(|line| format!("+{line}")),
                );
                output.extend(
                    proposed[match_index + 1..]
                        .iter()
                        .map(|line| format!("+{line}")),
                );
            }
            None => {
                output.push(format!("-{original_line}"));
                output.extend(proposed.iter().map(|line| format!("+{line}")));
            }
        }
        return;
    }
    if proposed.len() == 1 {
        let proposed_line = proposed[0];
        match original.iter().position(|line| *line == proposed_line) {
            Some(match_index) => {
                output.extend(
                    original[..match_index]
                        .iter()
                        .map(|line| format!("-{line}")),
                );
                output.extend(
                    original[match_index + 1..]
                        .iter()
                        .map(|line| format!("-{line}")),
                );
            }
            None => {
                output.extend(original.iter().map(|line| format!("-{line}")));
                output.push(format!("+{proposed_line}"));
            }
        }
        return;
    }

    // Split the original lines in half and pick the proposed split index that
    // maximizes LCS(left half, proposed prefix) + LCS(right half, proposed
    // suffix), then recurse into both halves.
    let mid = original.len() / 2;
    let left_scores = lcs_scores(&original[..mid], proposed);
    let right_scores = lcs_suffix_scores(&original[mid..], proposed);

    let mut best_split = 0;
    let mut best_total = 0;
    for split in 0..=proposed.len() {
        let total = left_scores[split] + right_scores[proposed.len() - split];
        if total > best_total {
            best_total = total;
            best_split = split;
        }
    }

    collect_lcs_diff(&original[..mid], &proposed[..best_split], output);
    collect_lcs_diff(&original[mid..], &proposed[best_split..], output);
}

/// LCS score row of `original` against every prefix of `proposed`:
/// `scores[j]` is the LCS length of `original` and `proposed[..j]`.
/// Two rolling rows, `O(len(proposed))` memory.
fn lcs_scores(original: &[&str], proposed: &[&str]) -> Vec<usize> {
    let mut previous = vec![0usize; proposed.len() + 1];
    let mut current = vec![0usize; proposed.len() + 1];
    for original_line in original {
        for (j, proposed_line) in proposed.iter().enumerate() {
            current[j + 1] = if original_line == proposed_line {
                previous[j] + 1
            } else {
                previous[j + 1].max(current[j])
            };
        }
        std::mem::swap(&mut previous, &mut current);
        current.fill(0);
    }
    previous
}

/// LCS score row of `original` against every suffix of `proposed`:
/// `scores[j]` is the LCS length of `original` and the last `j` lines of
/// `proposed`. Computed by running the same rolling-row DP on reversed
/// slices; the split search then pairs `left[split]` with
/// `right[len - split]` under one indexing scheme.
fn lcs_suffix_scores(original: &[&str], proposed: &[&str]) -> Vec<usize> {
    let mut reversed_original: Vec<&str> = original.to_vec();
    reversed_original.reverse();
    let mut reversed_proposed: Vec<&str> = proposed.to_vec();
    reversed_proposed.reverse();
    lcs_scores(&reversed_original, &reversed_proposed)
}

pub fn render_text_report(report: &PipelineReport) -> String {
    let mut output = String::new();

    writeln!(
        output,
        "prec-bsl {}: обработано файлов: {}",
        text_mode_label(report.mode),
        report.processed_files.len()
    )
    .expect("writing to a String must not fail");

    let modified_paths = report.modified_paths();
    if !modified_paths.is_empty() {
        writeln!(output).expect("writing to a String must not fail");
        writeln!(output, "Измененные файлы:").expect("writing to a String must not fail");
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
        writeln!(output, "Блокирующие ошибки:").expect("writing to a String must not fail");
        for result in hard_failures {
            writeln!(
                output,
                "  [{}] {}: {}",
                result.rule_id,
                display_path(&result.path),
                blocking_summary_message(result)
            )
            .expect("writing to a String must not fail");
        }
    }

    let sorted_results = text_detail_results(&report.results);
    let grouped = grouped_results(sorted_results);
    if !grouped.is_empty() {
        writeln!(output).expect("writing to a String must not fail");
        writeln!(output, "Сообщения:").expect("writing to a String must not fail");
        for (rule_id, by_path) in grouped {
            writeln!(output, "  {rule_id}").expect("writing to a String must not fail");
            for (path, results) in by_path {
                writeln!(output, "    {}", display_path(path))
                    .expect("writing to a String must not fail");
                for result in results {
                    writeln!(
                        output,
                        "      {}: {}{}",
                        text_severity_label(result.status),
                        text_message(&result.message),
                        span_suffix(result.source_span)
                    )
                    .expect("writing to a String must not fail");
                }
            }
        }
    }

    writeln!(output).expect("writing to a String must not fail");
    writeln!(output, "Код выхода: {}", report.exit_code())
        .expect("writing to a String must not fail");

    output
}

fn text_detail_results(results: &[ScenarioResult]) -> Vec<&ScenarioResult> {
    let actionable = sorted_results(results.iter().filter(|result| {
        matches!(
            result.status,
            ScenarioResultStatus::Modified | ScenarioResultStatus::Warning
        )
    }));
    if !actionable.is_empty() {
        return actionable;
    }

    if results.iter().any(|result| result.status.is_blocking()) {
        return Vec::new();
    }

    sorted_results(results.iter())
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
            proposed_content: result.proposed_content.clone(),
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

fn blocking_summary_message(result: &ScenarioResult) -> String {
    match result.status {
        ScenarioResultStatus::Unsupported => {
            format!("не поддерживается: {}", text_message(&result.message))
        }
        _ => text_message(&result.message),
    }
}

fn text_message(message: &str) -> String {
    if let Some(value) = message.strip_prefix("form element id must be a positive integer: ") {
        return format!(
            "идентификатор элемента формы должен быть положительным целым числом: {value}"
        );
    }
    if let Some(value) = message.strip_prefix("unreferenced metadata object directory for ") {
        return format!("каталог объекта метаданных не указан в составе конфигурации: {value}");
    }
    if let Some(value) = message.strip_prefix("unreferenced metadata object file for ") {
        return format!("файл объекта метаданных не указан в составе конфигурации: {value}");
    }
    if let Some(value) = message.strip_prefix("missing files for ") {
        return format!("отсутствуют файлы для {value}");
    }
    if let Some(value) = message.strip_prefix("missing metadata directory for ") {
        return format!("отсутствует каталог метаданных для объектов: {value}");
    }
    if let Some(value) = message.strip_prefix("missing source root: ") {
        return format!("не найден корень исходников: {value}");
    }
    if let Some(value) = message.strip_prefix("matched dictionary word '") {
        if let Some((word, line)) = value.split_once("' at line ") {
            return format!("найдено слово из словаря '{word}' в строке {line}");
        }
    }
    if let Some(value) = message.strip_prefix("String length ")
        && let Some((length, rest)) = value.split_once(" exceeded maximum ")
        && let Some((limit, line)) = rest.split_once(" at line ")
    {
        return format!(
            "длина строки {length} превышает максимально допустимую {limit} (строка {line})"
        );
    }
    if let Some(value) = message.strip_prefix("max line length setting ")
        && let Some(setting) = value.strip_suffix(" must be a positive integer")
    {
        return format!("настройка {setting} должна быть целым положительным числом");
    }
    if let Some(value) = message.strip_prefix("incorrect line break at line ") {
        return format!("неправильный перенос выражения (строка {value})");
    }
    if let Some(value) = message.strip_prefix("missing semicolon at line ") {
        return format!("отсутствует точка с запятой (строка {value})");
    }
    if let Some(value) = message.strip_prefix("failed to read file: ") {
        return format!("не удалось прочитать файл: {value}");
    }
    if let Some(value) = message.strip_prefix("failed to write file: ") {
        return format!("не удалось записать файл: {value}");
    }
    if let Some(value) = message.strip_prefix("copyright file is not configured or found: ") {
        return format!("файл копирайта не настроен или не найден: {value}");
    }
    if let Some(value) = message.strip_prefix("copyright path must be repository-relative: ") {
        return format!("путь к файлу копирайта должен быть относительным к репозиторию: {value}");
    }
    if let Some(value) = message.strip_prefix("base form element match is ambiguous: ") {
        return format!("неоднозначное соответствие элемента базовой формы: {value}");
    }
    if let Some(value) = message.strip_prefix("configuration metadata case differs for ") {
        return format!("регистр имени метаданных конфигурации отличается для {value}");
    }
    if let Some(value) = message.strip_prefix("failed to decode XML form text: ") {
        return format!("не удалось декодировать текст XML-формы: {value}");
    }
    if let Some(value) = message.strip_prefix("failed to parse XML form: ") {
        return format!("не удалось разобрать XML-форму: {value}");
    }

    match message {
        "added spaces in comments" => "добавлены пробелы в комментариях".to_owned(),
        "added spaces after commas" => "добавлены пробелы после запятых".to_owned(),
        "added spaces around operators" => "добавлены пробелы вокруг операторов".to_owned(),
        "removed trailing commas" => "удалены лишние запятые в конце".to_owned(),
        "BSL syntax errors prevent unit test loader update" => {
            "ошибки синтаксиса BSL мешают обновить загрузчик юнит-тестов".to_owned()
        }
        "base form files are processed through the owning Form.form" => {
            "файлы базовой формы обрабатываются через основной Form.form".to_owned()
        }
        "configuration composition sorting is disabled by scenario settings" => {
            "сортировка состава конфигурации отключена настройками сценария".to_owned()
        }
        "corrected duplicate XML form element ids" => {
            "исправлены повторяющиеся или неположительные идентификаторы элементов XML-формы"
                .to_owned()
        }
        "disabled form customization permission" => {
            "отключено разрешение на настройку формы".to_owned()
        }
        "disabled full-text search metadata settings" => {
            "отключены настройки полнотекстового поиска в метаданных".to_owned()
        }
        "dynamic local .os execution is not supported in v1" => {
            "динамическое выполнение локальных .os-сценариев не поддерживается в v1".to_owned()
        }
        "file contained tabs" => "файл содержит табы".to_owned(),
        "fixed incorrect line breaks" => "исправлены неправильные переносы строк".to_owned(),
        "form element name and id must be plain XML text" => {
            "имя и идентификатор элемента формы должны быть обычным XML-текстом".to_owned()
        }
        "full-text search metadata setting is already disabled" => {
            "настройка полнотекстового поиска в метаданных уже отключена".to_owned()
        }
        "goto statement is forbidden" => "оператор Перейти запрещен".to_owned(),
        "inserted or updated copyright header" => {
            "вставлен или обновлен заголовок копирайта".to_owned()
        }
        "inserted missing semicolons" => "добавлены пропущенные точки с запятой".to_owned(),
        "matched dictionary word" => "найдено слово из словаря".to_owned(),
        "metadata file is excluded from full-text search disabling" => {
            "файл метаданных исключен из отключения полнотекстового поиска".to_owned()
        }
        "metadata sync requires Configuration.mdo or Configuration.xml" => {
            "синхронизация метаданных требует Configuration.mdo или Configuration.xml".to_owned()
        }
        "module contains configured copyright skip tag" => {
            "модуль содержит настроенный тег пропуска копирайта".to_owned()
        }
        "removed duplicate metadata entries" => {
            "удалены дублирующиеся записи метаданных".to_owned()
        }
        "removed excessive blank lines" => "удалены лишние пустые строки".to_owned(),
        "removed trailing spaces or tabs" => {
            "удалены лишние пробелы или табы в конце строк".to_owned()
        }
        "removed trailing whitespace" => "удалены лишние пробелы или табы в конце строк".to_owned(),
        "scenario does not handle deleted files" => {
            "сценарий не обрабатывает удаленные файлы".to_owned()
        }
        "scenario handles only BSL modules" => "сценарий обрабатывает только модули BSL".to_owned(),
        "scenario handles only EDT Form.form files" => {
            "сценарий обрабатывает только EDT-файлы Form.form".to_owned()
        }
        "scenario handles only XML/EDT metadata files" => {
            "сценарий обрабатывает только XML/EDT-файлы метаданных".to_owned()
        }
        "sorted metadata composition" => "отсортирован состав метаданных".to_owned(),
        "subsystem composition sorting is disabled by scenario settings" => {
            "сортировка состава подсистемы отключена настройками сценария".to_owned()
        }
        _ => message.to_owned(),
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn mode_label(mode: PipelineMode) -> &'static str {
    match mode {
        PipelineMode::Hook => "hook",
        PipelineMode::ExecRules => "exec-rules",
        PipelineMode::Check => "check",
        PipelineMode::Fix => "fix",
    }
}

fn text_mode_label(mode: PipelineMode) -> &'static str {
    match mode {
        PipelineMode::Hook => "хук",
        PipelineMode::ExecRules => "exec-rules",
        PipelineMode::Check => "check",
        PipelineMode::Fix => "fix",
    }
}

fn text_severity_label(status: ScenarioResultStatus) -> &'static str {
    match status {
        ScenarioResultStatus::Modified => "изменено",
        ScenarioResultStatus::Warning => "предупреждение",
        ScenarioResultStatus::HardFailure => "ошибка",
        ScenarioResultStatus::Skipped => "пропущено",
        ScenarioResultStatus::Unsupported => "не поддерживается",
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
    #[serde(skip_serializing_if = "Option::is_none")]
    proposed_content: Option<String>,
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
mod tests;
