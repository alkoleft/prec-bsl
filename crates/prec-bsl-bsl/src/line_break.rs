//! Сценарий `ПроверкаПереносовСтрок` (bsl-ls `IncorrectLineBreak` parity):
//! диагностика висячих `;` / `)` / `);` / `,` в начале строки и бинарных
//! операторов в конце строки.
//!
//! Ядро модуля — stateful-классификатор строки [`classify_line`]: состояние
//! `in_string` переносится между строками (учёт `""`-экранирования), query-регион
//! открывается запросным ключевым словом внутри `"`-строки и закрывается
//! закрывающей `"`. Детектор [`find_line_break_violations`] матчит FIRST/END
//! паттерны на кодовой части каждой строки. Прецедент переносимой логики —
//! `CommentSpacingState` в `text_fixers.rs`.

use std::fs;

use crate::text_fixers::is_identifier_char;
use prec_bsl_pipeline::{
    ScenarioDefinition, ScenarioExecutionContext, ScenarioResult, ScenarioRun, SourceSpan,
};
use prec_bsl_source::SourceFileKind;
use regex::{Regex, RegexBuilder};
use serde_json::Value;

pub const LINE_BREAK_RULE: &str = "ПроверкаПереносовСтрок";

pub const LINE_BREAK_SCENARIO: ScenarioDefinition = ScenarioDefinition::compatibility(
    LINE_BREAK_RULE,
    "ПроверкаПереносовСтрок.os",
    line_break_handler,
);

/// Паттерн начала строки из bsl-ls `listOfIncorrectFirstSymbol` (опечатка
/// `(:?` исправлена на `(?:`; кейс-инсенситивность задаётся при компиляции).
const DEFAULT_FIRST_SYMBOL_PATTERN: &str = r"\)|;|,\s*\S+|\);";
/// Паттерн конца строки из bsl-ls `listOfIncorrectLastSymbol`.
const DEFAULT_LAST_SYMBOL_PATTERN: &str = r"ИЛИ|И|OR|AND|\+|-|/|%|\*";

/// Настройки `НастройкиСценариев.ПроверкаПереносовСтрок.*` (ASCII-ключи bsl-ls).
///
/// `check_first`/`check_last` гейтят соответствующие проверки; паттерны —
/// скомпилированные обрамления bsl-ls-паттернов `listOfIncorrectFirstSymbol`
/// (`^\s*(...)`) и `listOfIncorrectLastSymbol` (`\s+(...)\s*(?://.*)?$`).
/// Компиляция из JSON-значений — [`settings_from_value`].
#[derive(Debug, Clone)]
pub struct LineBreakSettings {
    check_first: bool,
    first_pattern: Regex,
    check_last: bool,
    last_pattern: Regex,
}

impl LineBreakSettings {
    /// Настройки по умолчанию: обе проверки включены, паттерны bsl-ls.
    pub fn default_settings() -> Self {
        Self {
            check_first: true,
            first_pattern: compile_first_pattern(DEFAULT_FIRST_SYMBOL_PATTERN)
                .expect("default first-symbol pattern must compile"),
            check_last: true,
            last_pattern: compile_last_pattern(DEFAULT_LAST_SYMBOL_PATTERN)
                .expect("default last-symbol pattern must compile"),
        }
    }

    /// Кастомные флаги/паттерны для тестов детектора (в проде — только
    /// [`settings_from_value`]).
    #[cfg(test)]
    fn custom(
        check_first: bool,
        first_pattern: &str,
        check_last: bool,
        last_pattern: &str,
    ) -> Self {
        Self {
            check_first,
            first_pattern: compile_first_pattern(first_pattern)
                .expect("test first-symbol pattern must compile"),
            check_last,
            last_pattern: compile_last_pattern(last_pattern)
                .expect("test last-symbol pattern must compile"),
        }
    }
}

/// `^\s*(<pattern>)` — висячий токен в начале строки (кейс-инсенситив).
fn compile_first_pattern(pattern: &str) -> Result<Regex, regex::Error> {
    RegexBuilder::new(&format!(r"^\s*({pattern})"))
        .case_insensitive(true)
        .build()
}

/// `\s+(<pattern>)\s*(?://.*)?$` — бинарный оператор в конце строки (span —
/// захватывающая группа вокруг оператора).
fn compile_last_pattern(pattern: &str) -> Result<Regex, regex::Error> {
    RegexBuilder::new(&format!(r"\s+({pattern})\s*(?://.*)?$"))
        .case_insensitive(true)
        .build()
}

/// Находка детектора: `line` — 1-based номер строки, `byte_start`/`byte_end` —
/// абсолютные смещения span в исходном тексте (для FIRST — весь матч, для END —
/// оператор из захватывающей группы).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineBreakViolation {
    pub line: usize,
    pub byte_start: usize,
    pub byte_end: usize,
}

/// Переносимое между строками состояние сканера: незакрытая `"`-строка и
/// query-регион.
#[derive(Debug, Default)]
struct LineScanState {
    in_string: bool,
    in_query: bool,
    /// Кавычка по текущему индексу — вторая в паре `""` (экранированная).
    skip_escaped_quote: bool,
}

/// Результат классификации одной строки (без окончания строки).
#[derive(Debug, Default)]
struct LineClassification {
    /// Диапазоны (byte_start, byte_end) частей строки внутри `"`-литералов
    /// (включая кавычки) — FIRST/END матчи внутри них игнорируются.
    string_ranges: Vec<(usize, usize)>,
    /// Смещение `//`-комментария вне строк — матчи от него и далее игнорируются.
    comment_start: Option<usize>,
    /// Строка находится внутри query-региона — FIRST/END проверки отключены.
    in_query: bool,
}

impl LineClassification {
    /// Есть ли код (вне строк и комментария) в диапазоне `[start, end)`?
    fn has_code_in(&self, start: usize, end: usize) -> bool {
        if self
            .comment_start
            .is_some_and(|comment_start| comment_start <= start)
        {
            return false;
        }
        !self
            .string_ranges
            .iter()
            .any(|&(string_start, string_end)| start < string_end && end > string_start)
    }
}

/// Классифицировать строку кода (без окончания строки), обновив состояние.
///
/// Скан посимвольный, state переносится между строками:
/// - вход в строку на `"`, выход на закрывающей `"`; `""` — экранированная
///   кавычка, не вход и не выход (lookahead-парнинг пар по образцу
///   `CommentSpacingState` в `text_fixers.rs`);
/// - строка, начавшаяся как in_string-продолжение, остаётся строкой до
///   закрывающей `"` (`|`-продолжения попадают в string_ranges);
/// - `//` вне строки фиксируется как `comment_start` и завершает разбор;
/// - запросный литерал (слово сразу после открывающей `"` — ВЫБРАТЬ/ВЫБОРКА/
///   SELECT…) открывает query-регион на строке открытия; регион длится до
///   закрывающей `"` — на этой же строке (однострочный запрос) или на одной
///   из следующих (state переносится).
fn classify_line(body: &str, state: &mut LineScanState) -> LineClassification {
    let mut classification = LineClassification {
        in_query: state.in_query,
        ..LineClassification::default()
    };

    // Незакрытая строка с предыдущей строки: продолжение от начала строки.
    let mut string_start: Option<usize> = if state.in_string { Some(0) } else { None };
    // Запросный литерал, открывшийся на этой строке и ещё не закрытый.
    let mut query_start: Option<usize> = None;
    state.in_string = false;

    let mut index = 0;

    while index < body.len() {
        let char = body[index..].chars().next().unwrap();

        if state.in_query {
            // Внутри query-региона всё до закрывающей кавычки — строковый
            // контент; `""` — экранированная кавычка, не закрытие региона.
            if char == '"' {
                if state.skip_escaped_quote {
                    state.skip_escaped_quote = false;
                } else if body[index + char.len_utf8()..].starts_with('"') {
                    state.skip_escaped_quote = true;
                } else {
                    classification
                        .string_ranges
                        .push((query_start.unwrap_or(0), index + 1));
                    state.in_query = false;
                    query_start = None;
                }
            }
            index += char.len_utf8();
            continue;
        }

        if char == '"' {
            if state.skip_escaped_quote {
                // Вторая кавычка пары `""` — экранированная, не выход.
                state.skip_escaped_quote = false;
            } else {
                match string_start {
                    Some(start) => {
                        if body[index + char.len_utf8()..].starts_with('"') {
                            // Первая кавычка пары `""` внутри литерала.
                            state.skip_escaped_quote = true;
                        } else {
                            classification.string_ranges.push((start, index + 1));
                            string_start = None;
                        }
                    }
                    None => {
                        if is_query_keyword(&body[index + char.len_utf8()..]) {
                            // Запросный литерал: регион открывается здесь и
                            // закрывается закрывающей `"` (возможно, на одной
                            // из следующих строк).
                            state.in_query = true;
                            query_start = Some(index);
                        } else {
                            string_start = Some(index);
                        }
                    }
                }
            }
            index += char.len_utf8();
            continue;
        }

        if string_start.is_some() {
            // Внутри строки: `|`-продолжения и прочее остаются строкой.
            index += char.len_utf8();
            continue;
        }

        if char == '/' && body[index + char.len_utf8()..].starts_with('/') {
            classification.comment_start = Some(index);
            break;
        }

        index += char.len_utf8();
    }

    // Незакрытый на этой строке литерал: остаток строки — строка, состояние
    // in_string переносится на следующую строку. Для запросного литерала
    // in_string не нужен: регион переносится через state.in_query.
    if let Some(start) = string_start {
        classification.string_ranges.push((start, body.len()));
        state.in_string = true;
    }
    // Незакрытый на этой строке запросный литерал: остаток строки-открытия —
    // строковый контент (строки региона детектор пропускает целиком).
    if let Some(start) = query_start {
        classification.string_ranges.push((start, body.len()));
    }

    classification
}

/// Текст после открывающей кавычки начинается с запросного ключевого слова?
///
/// Сравнение кириллически кейс-инсенситивно: `"выбрать"` — тоже запрос
/// (END/FIRST-паттерны компилируются case-insensitive).
fn is_query_keyword(literal_text: &str) -> bool {
    // Ключевые слова в нижнем регистре для сравнения через to_lowercase().
    const QUERY_KEYWORDS: &[&str] = &["выбрать", "выборка", "выборки", "select"];

    let text = literal_text.trim_start();
    if text.is_empty() {
        return false;
    }

    let keyword_end = text
        .find(|char: char| !is_identifier_char(char))
        .unwrap_or(text.len());
    let keyword = text[..keyword_end].to_lowercase();

    QUERY_KEYWORDS.iter().any(|candidate| keyword == *candidate)
}

/// Найти IncorrectLineBreak-нарушения: FIRST (висячий `;`/`)`/`);`/`,` в начале
/// строки) и END (бинарный оператор в конце строки перед переносом).
///
/// Для каждой строки (разбор `\r\n`/`\n`/`\r` — прецедент `split_line_ending`)
/// выполняется `classify_line`; матч FIRST (`^\s*(?:...)`, span = весь матч) и
/// END (`\s+(...)\s*(?://.*)?$`, span = оператор) проверяется на кодовой части
/// — вне `string_ranges` и до `comment_start`; строки внутри query-региона
/// пропускаются целиком. `line` — 1-based, offsets — абсолютные в `input`.
pub fn find_line_break_violations(
    input: &str,
    settings: &LineBreakSettings,
) -> Vec<LineBreakViolation> {
    let mut violations = Vec::new();
    let mut state = LineScanState::default();

    let mut line_start = 0;
    let mut line_number = 0;
    for raw_line in split_input_lines(input) {
        line_number += 1;
        let (body, _ending) = split_line_ending(raw_line);
        let classification = classify_line(body, &mut state);

        if !classification.in_query {
            detect_first_violation(
                body,
                &classification,
                settings,
                line_start,
                line_number,
                &mut violations,
            );
            detect_last_violation(
                body,
                &classification,
                settings,
                line_start,
                line_number,
                &mut violations,
            );
        }

        line_start += raw_line.len();
    }

    violations
}

fn detect_first_violation(
    body: &str,
    classification: &LineClassification,
    settings: &LineBreakSettings,
    line_start: usize,
    line_number: usize,
    violations: &mut Vec<LineBreakViolation>,
) {
    if !settings.check_first {
        return;
    }

    let Some(matched) = settings.first_pattern.find(body) else {
        return;
    };
    if !classification.has_code_in(matched.start(), matched.end()) {
        return;
    }

    violations.push(LineBreakViolation {
        line: line_number,
        byte_start: line_start + matched.start(),
        byte_end: line_start + matched.end(),
    });
}

fn detect_last_violation(
    body: &str,
    classification: &LineClassification,
    settings: &LineBreakSettings,
    line_start: usize,
    line_number: usize,
    violations: &mut Vec<LineBreakViolation>,
) {
    if !settings.check_last {
        return;
    }

    let Some(captures) = settings.last_pattern.captures(body) else {
        return;
    };
    let Some(operator) = captures.get(1) else {
        return;
    };
    // Оператор должен быть в коде (regex `\s+(...)$` уже гарантирует позицию
    // в конце строки; проверка кодовой части отсекает строки/комментарии).
    if !classification.has_code_in(operator.start(), operator.end()) {
        return;
    }

    violations.push(LineBreakViolation {
        line: line_number,
        byte_start: line_start + operator.start(),
        byte_end: line_start + operator.end(),
    });
}

/// Разбор входа на строки с окончаниями: `\r\n`/`\n`/`\r` (прецедент
/// `split_line_ending` из `text_fixers.rs`, расширенный на многострочный вход).
fn split_input_lines(input: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;

    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\n' => {
                lines.push(&input[start..=index]);
                index += 1;
                start = index;
            }
            b'\r' => {
                let ending_len = if bytes.get(index + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                };
                lines.push(&input[start..index + ending_len]);
                index += ending_len;
                start = index;
            }
            _ => index += 1,
        }
    }

    if start < bytes.len() {
        lines.push(&input[start..]);
    }

    lines
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(body) = line.strip_suffix("\r\n") {
        (body, "\r\n")
    } else if let Some(body) = line.strip_suffix('\n') {
        (body, "\n")
    } else if let Some(body) = line.strip_suffix('\r') {
        (body, "\r")
    } else {
        (line, "")
    }
}

/// Ключи настроек `НастройкиСценариев.ПроверкаПереносовСтрок.*`
/// (ASCII-имена bsl-ls `IncorrectLineBreakDiagnostic`).
const SETTING_CHECK_FIRST: &str = "checkFirstSymbol";
const SETTING_CHECK_LAST: &str = "checkLastSymbol";
const SETTING_FIRST_PATTERN: &str = "listOfIncorrectFirstSymbol";
const SETTING_LAST_PATTERN: &str = "listOfIncorrectLastSymbol";

/// Разобрать настройки сценария из JSON-значения `НастройкиСценариев`.
///
/// `None` и пустой объект дают дефолты bsl-ls (обе проверки включены,
/// паттерны по умолчанию). Значения валидируются на границе (прецедент
/// `dictionary_path` в `text_checkers.rs`): не-bool флаг, не-string паттерн,
/// пустой или некомпилируемый regex — `Err` с именем настройки; handler
/// превращает его в per-file `hard_failure`.
fn settings_from_value(settings: Option<&Value>) -> Result<LineBreakSettings, String> {
    let Some(settings) = settings else {
        return Ok(LineBreakSettings::default_settings());
    };

    let check_first = bool_setting(settings, SETTING_CHECK_FIRST)?;
    let check_last = bool_setting(settings, SETTING_CHECK_LAST)?;
    let first_pattern = pattern_setting(settings, SETTING_FIRST_PATTERN, compile_first_pattern)?;
    let last_pattern = pattern_setting(settings, SETTING_LAST_PATTERN, compile_last_pattern)?;

    Ok(LineBreakSettings {
        check_first,
        first_pattern,
        check_last,
        last_pattern,
    })
}

fn bool_setting(settings: &Value, key: &str) -> Result<bool, String> {
    match settings.get(key) {
        None | Some(Value::Null) => Ok(true),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(format!("line break setting {key} must be a boolean")),
    }
}

fn pattern_setting(
    settings: &Value,
    key: &str,
    compile: fn(&str) -> Result<Regex, regex::Error>,
) -> Result<Regex, String> {
    let pattern = match settings.get(key) {
        None | Some(Value::Null) => return Ok(default_pattern(key, compile)),
        Some(Value::String(pattern)) => pattern.trim(),
        Some(_) => return Err(format!("line break setting {key} must be a string")),
    };

    if pattern.is_empty() {
        return Err(format!("line break setting {key} must not be empty"));
    }

    compile(pattern)
        .map_err(|_| format!("line break setting {key} must be a valid regular expression"))
}

fn default_pattern(key: &str, compile: fn(&str) -> Result<Regex, regex::Error>) -> Regex {
    let default = if key == SETTING_FIRST_PATTERN {
        DEFAULT_FIRST_SYMBOL_PATTERN
    } else {
        DEFAULT_LAST_SYMBOL_PATTERN
    };

    compile(default).expect("default line-break pattern must compile")
}

/// Handler сценария `ПроверкаПереносовСтрок`: диагностика IncorrectLineBreak
/// (паритет bsl-ls) по настройкам `НастройкиСценариев`.
///
/// Task 2: и DryRun (check), и Apply (hook/exec-rules/fix) выдают
/// per-violation warnings со span. Фиксер и `modified_dry_run`-превью
/// (DryRun) / `modified` (Apply) подключает Task 3.
pub fn line_break_handler(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    if context.file.kind != SourceFileKind::BslModule {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            "scenario handles only BSL modules",
        ));
    }

    let settings = match settings_from_value(context.settings) {
        Ok(settings) => settings,
        Err(message) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                message,
            ));
        }
    };

    let path = context.repo_root.join(&context.file.repo_path);
    let input = match fs::read_to_string(&path) {
        Ok(input) => input,
        Err(error) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                format!("failed to read file: {error}"),
            ));
        }
    };

    let results = find_line_break_violations(&input, &settings)
        .into_iter()
        .map(|violation| {
            ScenarioResult::warning(
                context.rule_id,
                context.file.repo_path.clone(),
                format!("incorrect line break at line {}", violation.line),
            )
            .with_source_span(SourceSpan::new(violation.byte_start, violation.byte_end))
        })
        .collect();

    ScenarioRun {
        results,
        post_processing_paths: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_hanging_semicolon() {
        let input = "\tВызов(Арг)\n\t;";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 2);
        assert_eq!(
            &input[violations[0].byte_start..violations[0].byte_end],
            "\t;"
        );
    }

    #[test]
    fn detects_hanging_close_paren() {
        let input = "\tВызов(\n\t\tАргумент\n\t);";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 3);
        // Левая альтернатива bsl-ls `\)` выигрывает у `\);`.
        assert_eq!(
            &input[violations[0].byte_start..violations[0].byte_end],
            "\t)"
        );
    }

    #[test]
    fn detects_hanging_comma_run() {
        let input = "Вызов(\n\t,,,,, Оповещение\n);";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        let comma_run = violations
            .iter()
            .find(|violation| violation.line == 2)
            .expect("comma-run line must be flagged");
        assert_eq!(&input[comma_run.byte_start..comma_run.byte_end], "\t,,,,,");
    }

    #[test]
    fn detects_trailing_and() {
        let input = "Если А = Б И\n\tБ = В Тогда";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 1);
        assert_eq!(
            &input[violations[0].byte_start..violations[0].byte_end],
            "И"
        );
    }

    #[test]
    fn detects_trailing_or_en() {
        let input = "If A = B OR\n\tB = C Then";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 1);
        assert_eq!(
            &input[violations[0].byte_start..violations[0].byte_end],
            "OR"
        );
    }

    #[test]
    fn detects_trailing_plus() {
        let input = "С = А +\n\tБ;";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 1);
        assert_eq!(
            &input[violations[0].byte_start..violations[0].byte_end],
            "+"
        );
    }

    #[test]
    fn no_violation_comment() {
        let input = "// Строка - Неопределено -";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert!(violations.is_empty());
    }

    #[test]
    fn no_violation_string_literal() {
        let input = "С = \"Строка ИЛИ\";";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert!(violations.is_empty());
    }

    #[test]
    fn detects_trailing_operator_after_escaped_quotes() {
        let input = "С = \"a\"\"b\" И";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 1);
        assert_eq!(
            &input[violations[0].byte_start..violations[0].byte_end],
            "И"
        );
    }

    #[test]
    fn no_violation_operator_inside_string_escaped() {
        let input = "С = \"a\"\" и\"";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert!(violations.is_empty());
    }

    #[test]
    fn no_violation_multiline_string_continuation() {
        let input = "С = \"Первая строка\n\tпродолжение";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert!(violations.is_empty());
    }

    #[test]
    fn no_violation_query_region() {
        let input = "Запрос.Текст = \"ВЫБРАТЬ\n\t*\n\t|ИЗ\n\t|Справочник.А КАК А\";";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert!(violations.is_empty());

        // После закрытия query-региона детектор снова работает.
        let resumed = format!("{input}\n\t;");
        let resumed_violations =
            find_line_break_violations(&resumed, &LineBreakSettings::default_settings());

        assert_eq!(resumed_violations.len(), 1);
        assert_eq!(resumed_violations[0].line, 5);
    }

    #[test]
    fn detects_after_empty_string_literal() {
        // `""` — пустой литерал (открытие и сразу закрытие), а не
        // экранированная пара: состояние строки не переносится на следующие
        // строки.
        let input = "А = \"\";\nБ = В И";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 2);
        assert_eq!(
            &input[violations[0].byte_start..violations[0].byte_end],
            "И"
        );
    }

    #[test]
    fn detects_after_literal_ending_with_escaped_quote() {
        // `"a"""` — литерал `a"`: пара `""` (экранирование) и закрывающая `"`.
        let input = "А = \"a\"\"\";\nБ = В И";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 2);
    }

    #[test]
    fn detects_after_single_line_query_literal() {
        // Однострочный запросный литерал закрывает query-регион своей
        // закрывающей кавычкой: код следующих строк сканируется.
        let input = "Запрос.Текст = \"ВЫБРАТЬ А\";\n\t;";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 2);
        assert_eq!(
            &input[violations[0].byte_start..violations[0].byte_end],
            "\t;"
        );
    }

    #[test]
    fn no_panic_multiline_literal_with_cyrillic_continuation() {
        // Кириллическое продолжение незакрытого литерала и вложенные кавычки:
        // разбор не паникует на мультибайтных границах и восстанавливает
        // состояние к концу литерала (строка 3 сканируется как код).
        let input = "Текст = \"Первая строка\nВторая \"строка\" хвост\";\nБ = В И";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 3);
        assert_eq!(
            &input[violations[0].byte_start..violations[0].byte_end],
            "И"
        );
    }

    #[test]
    fn no_violation_operator_in_line_comment() {
        let input = "\t// С = А И";
        let violations = find_line_break_violations(input, &LineBreakSettings::default_settings());

        assert!(violations.is_empty());
    }

    #[test]
    fn check_first_disabled() {
        let settings = LineBreakSettings::custom(
            false,
            DEFAULT_FIRST_SYMBOL_PATTERN,
            true,
            DEFAULT_LAST_SYMBOL_PATTERN,
        );
        let input = "\tВызов(Арг)\n\t;";
        let violations = find_line_break_violations(input, &settings);

        assert!(violations.is_empty());
    }

    #[test]
    fn check_last_disabled() {
        let settings = LineBreakSettings::custom(
            true,
            DEFAULT_FIRST_SYMBOL_PATTERN,
            false,
            DEFAULT_LAST_SYMBOL_PATTERN,
        );
        let input = "Если А = Б И\n\tБ = В Тогда";
        let violations = find_line_break_violations(input, &settings);

        assert!(violations.is_empty());
    }

    #[test]
    fn custom_first_pattern() {
        // `listOfIncorrectFirstSymbol` без ветки `,`: `,`-кейс не флагается.
        let settings = LineBreakSettings::custom(true, r"\)|;", true, DEFAULT_LAST_SYMBOL_PATTERN);
        let input = "Вызов(\n\t, Оповещение\n\t;";
        let violations = find_line_break_violations(input, &settings);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 3);

        // На дефолтных настройках та же `,`-строка флагается.
        let default_violations =
            find_line_break_violations(input, &LineBreakSettings::default_settings());
        assert_eq!(default_violations.len(), 2);
    }

    fn settings_value(json: &str) -> Value {
        serde_json::from_str(json).expect("test settings JSON must parse")
    }

    #[test]
    fn settings_missing_all_defaults() {
        // None и пустой объект → дефолты: обе проверки включены, паттерны bsl-ls.
        for settings in [
            settings_from_value(None),
            settings_from_value(Some(&Value::Object(serde_json::Map::new()))),
        ] {
            let settings = settings.expect("missing settings must resolve to defaults");

            assert!(settings.check_first);
            assert!(settings.check_last);
            assert_eq!(
                settings.first_pattern.as_str(),
                compile_first_pattern(DEFAULT_FIRST_SYMBOL_PATTERN)
                    .expect("default pattern must compile")
                    .as_str()
            );
            assert_eq!(
                settings.last_pattern.as_str(),
                compile_last_pattern(DEFAULT_LAST_SYMBOL_PATTERN)
                    .expect("default pattern must compile")
                    .as_str()
            );
        }

        // Дефолты эквивалентны `default_settings()` на репрезентативном входе.
        let input = "\tВызов(Арг)\n\t;\nЕсли А = Б И\n\tБ = В Тогда";
        let from_json = settings_from_value(None).expect("None settings must resolve to defaults");
        assert_eq!(
            find_line_break_violations(input, &from_json),
            find_line_break_violations(input, &LineBreakSettings::default_settings())
        );
    }

    #[test]
    fn settings_invalid_bool_type() {
        let settings = settings_value(r#"{"checkFirstSymbol": "yes"}"#);

        assert_eq!(
            settings_from_value(Some(&settings)).expect_err("non-bool flag must be rejected"),
            "line break setting checkFirstSymbol must be a boolean"
        );
    }

    #[test]
    fn settings_invalid_regex() {
        let settings = settings_value(r#"{"listOfIncorrectFirstSymbol": "[unclosed"}"#);

        assert_eq!(
            settings_from_value(Some(&settings))
                .expect_err("non-compilable regex must be rejected"),
            "line break setting listOfIncorrectFirstSymbol must be a valid regular expression"
        );

        let last_settings = settings_value(r#"{"listOfIncorrectLastSymbol": "[unclosed"}"#);
        assert_eq!(
            settings_from_value(Some(&last_settings))
                .expect_err("non-compilable last-symbol regex must be rejected"),
            "line break setting listOfIncorrectLastSymbol must be a valid regular expression"
        );
    }

    #[test]
    fn settings_empty_regex() {
        let settings = settings_value(r#"{"listOfIncorrectFirstSymbol": ""}"#);

        assert_eq!(
            settings_from_value(Some(&settings)).expect_err("empty regex must be rejected"),
            "line break setting listOfIncorrectFirstSymbol must not be empty"
        );

        let last_settings = settings_value(r#"{"listOfIncorrectLastSymbol": "  "}"#);
        assert_eq!(
            settings_from_value(Some(&last_settings))
                .expect_err("whitespace-only regex must be rejected"),
            "line break setting listOfIncorrectLastSymbol must not be empty"
        );
    }

    #[test]
    fn settings_non_string_list() {
        let settings = settings_value(r#"{"listOfIncorrectFirstSymbol": 42}"#);

        assert_eq!(
            settings_from_value(Some(&settings)).expect_err("non-string pattern must be rejected"),
            "line break setting listOfIncorrectFirstSymbol must be a string"
        );

        let last_settings = settings_value(r#"{"listOfIncorrectLastSymbol": true}"#);
        assert_eq!(
            settings_from_value(Some(&last_settings))
                .expect_err("non-string last pattern must be rejected"),
            "line break setting listOfIncorrectLastSymbol must be a string"
        );
    }

    #[test]
    fn settings_custom_patterns_roundtrip() {
        // Кастомные паттерны (без `,`-ветки FIRST) компилируются и работают
        // в детекторе: `,`-строка не флагается, `;`-строка флагается.
        let settings = settings_value(
            r#"{"listOfIncorrectFirstSymbol": "\\)|;", "listOfIncorrectLastSymbol": "И"}"#,
        );
        let settings =
            settings_from_value(Some(&settings)).expect("custom patterns must compile and resolve");

        let input = "Вызов(\n\t, Оповещение\n\t;";
        let violations = find_line_break_violations(input, &settings);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 3);
        assert_eq!(
            &input[violations[0].byte_start..violations[0].byte_end],
            "\t;"
        );

        // Кастомный END-паттерн (только `И`) флагает `И` в конце строки…
        let end_input = "Если А = Б И\n\tБ = В Тогда";
        assert_eq!(find_line_break_violations(end_input, &settings).len(), 1);

        // …но не `ИЛИ` (альтернативы `ИЛИ` в кастомном паттерне нет).
        let or_input = "Если А = Б ИЛИ\n\tБ = В Тогда";
        assert!(find_line_break_violations(or_input, &settings).is_empty());
    }
}
