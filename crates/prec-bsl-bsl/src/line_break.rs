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

use regex::{Regex, RegexBuilder};

pub const LINE_BREAK_RULE: &str = "ПроверкаПереносовСтрок";

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
/// Компиляция из JSON-значений — в Task 2 (`settings_from_value`).
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

    /// Кастомные флаги/паттерны для тестов детектора (JSON-парсинг — Task 2).
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
///   кавычка, не выход (прецедент `CommentSpacingState` в `text_fixers.rs`);
/// - строка, начавшаяся как in_string-продолжение, остаётся строкой до
///   закрывающей `"` (`|`-продолжения попадают в string_ranges);
/// - `//` вне строки фиксируется как `comment_start` и завершает разбор;
/// - запросное ключевое слово (ВЫБРАТЬ/ВЫБОРКА/SELECT…) внутри `"`-строки
///   открывает query-регион до закрывающей `"` (state переносится).
fn classify_line(body: &str, state: &mut LineScanState) -> LineClassification {
    let mut classification = LineClassification {
        in_query: state.in_query,
        ..LineClassification::default()
    };

    // Незакрытая строка с предыдущей строки: продолжение от начала строки.
    let mut string_start: Option<usize> = if state.in_string { Some(0) } else { None };
    state.in_string = false;

    let mut index = 0;

    while index < body.len() {
        let char = body[index..].chars().next().unwrap();

        if state.in_query {
            // Внутри query-региона строка целиком пропускается до закрывающей
            // кавычки; всё до неё — строковый контент.
            if char == '"' && !is_escaped_quote(body, index) {
                classification.string_ranges.push((0, index + 1));
                state.in_query = false;
            }
            index += char.len_utf8();
            continue;
        }

        if char == '"' {
            match string_start {
                Some(start) => {
                    // Закрывающая кавычка текущего литерала. `""`-пара перед
                    // выходом уже учтена is_escaped_quote.
                    if is_escaped_quote(body, index) {
                        index += char.len_utf8();
                        continue;
                    }
                    classification.string_ranges.push((start, index + 1));
                    if is_query_keyword(&body[start + 1..index]) {
                        state.in_query = true;
                    }
                    string_start = None;
                }
                None => string_start = Some(index),
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
    // in_string переносится на следующую строку.
    if let Some(start) = string_start {
        classification.string_ranges.push((start, body.len()));
        state.in_string = true;
    } else if state.in_query {
        // Незакрытый query-регион: остаток строки тоже строковый контент.
        classification.string_ranges.push((0, body.len()));
    }

    classification
}

/// Является ли кавычка по смещению `quote_index` закрывающей (не частью `""`)?
///
/// Кавычка закрывающая, когда идущий подряд ряд кавычек, заканчивающийся ею,
/// имеет нечётную длину: пары `""` полностью экранированы, последняя — выход.
fn is_escaped_quote(body: &str, quote_index: usize) -> bool {
    let quote_len = '"'.len_utf8();

    // Ряд кавычек перед текущей (без неё).
    let mut run_start = quote_index;
    while run_start > 0 && body[..run_start].ends_with('"') {
        run_start -= quote_len;
    }
    let quotes_before = (quote_index - run_start) / quote_len;

    // Нечётное число кавычек перед текущей значит, что она закрывает литерал:
    // каждая пара `""` открывает и тут же закрывает экранирование.
    quotes_before % 2 == 1
}

/// Текст литерала между кавычками начинается с запросного ключевого слова?
fn is_query_keyword(literal_text: &str) -> bool {
    const QUERY_KEYWORDS: &[&str] = &["ВЫБРАТЬ", "ВЫБОРКА", "ВЫБОРКИ", "SELECT"];

    let text = literal_text.trim_start();
    if text.is_empty() {
        return false;
    }

    let keyword_end = text
        .find(|char: char| !is_identifier_char(char))
        .unwrap_or(text.len());
    let keyword = &text[..keyword_end];

    QUERY_KEYWORDS
        .iter()
        .any(|candidate| keyword.eq_ignore_ascii_case(candidate) || keyword == *candidate)
}

fn is_identifier_char(char: char) -> bool {
    char == '_' || char.is_alphanumeric()
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

    // Матч должен начинаться не позже конца ведущих пробелов: токен висит в
    // начале строки (иначе `;` в середине кода ложно флагается).
    let Some(matched) = settings.first_pattern.find(body) else {
        return;
    };
    if matched.start() > leading_whitespace_end(body) {
        return;
    }
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

fn leading_whitespace_end(body: &str) -> usize {
    body.len() - body.trim_start().len()
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
}
