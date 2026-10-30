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
    WritePolicy,
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
#[derive(Debug, Clone, Copy, Default)]
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

/// Известные фикстеру FIRST-токены: `);`, `;`, `)` (от длинного к короткому,
/// чтобы `\t);` приклеился целиком; `,` — check-only, кастомные паттерны —
/// check-only; план F6).
const FIXABLE_FIRST_TOKENS: &[&str] = &[");", ";", ")"];

/// Известные фикстеру END-операторы (дефолтный список bsl-ls; `ИЛИ` раньше
/// `И`, иначе хвост `ИЛИ` ложно матчится как `И`; словные операторы
/// сравниваются кейс-инсенситивно).
const FIXABLE_LAST_OPERATORS: &[&str] = &["ИЛИ", "И", "OR", "AND", "+", "-", "/", "%", "*"];

/// Исправить IncorrectLineBreak-нарушения механически безопасным способом.
///
/// Парный проход (prev, cur) по строкам с [`classify_line`]: состояние
/// переносится по **исходным** строкам, поэтому правки не ломают
/// классификацию следующих строк (ревью F2: детектор не переиспользуется —
/// его позиции протухают после правок).
///
/// Правила фикса (только известные токены — план F6: check следует паттернам
/// настроек, fix — только механикам известных токенов; bool-флаги
/// `checkFirstSymbol`/`checkLastSymbol` гейтят обе стороны):
/// - FIRST: известный токен (`;` / `)` / `);`) в начале кодовой части cur,
///   prev — код-строка **без `//`-комментария** (токен вклеился бы в
///   комментарий и `)` проглотился бы им — ревью F1) → снять токен с cur,
///   приклеить к телу prev; остаток cur (комментарий) остаётся строкой-
///   носителем, пустой остаток → строка удаляется, её окончание переходит на
///   объединённую строку. Skip: prev уже оканчивается тем же токеном (защита
///   `;;`); prev с комментарием (вклейка ломает файл, нарушение остаётся
///   check-only); `,`-FIRST и кастомные матчи фиксер не видит вовсе.
/// - END: известный оператор в конце кодовой части prev, cur — код-строка
///   (не пустая, не чистый комментарий, не строка/query-регион) → оператор
///   перенести в начало кода cur после отступа **в исходном написании**
///   (lowercase `и`/`and` — валидный BSL, канонический `И` в cur при
///   байт-точной отклейке давал не-идемпотентный рост — ревью F2); тело
///   prev rstrip (иначе `УдалениеЛишнихКонцевыхПробелов` найдёт хвост).
///
/// Окончания строк (`\r\n`/`\n`/`\r`, final-no-newline) сохраняются в
/// точности; `\r` не попадает в приклеенный токен.
pub fn fix_line_breaks(input: &str, settings: &LineBreakSettings) -> String {
    let lines = split_input_lines(input);
    if lines.is_empty() {
        return input.to_owned();
    }

    let mut state = LineScanState::default();
    // Каждая собранная строка хранит классификацию своего исходного тела:
    // END-проверка prev должна знать строки/комментарии prev, а FIRST — что
    // prev является код-строкой.
    let mut fixed: Vec<(String, LineClassification)> = Vec::with_capacity(lines.len());

    for line in &lines {
        let (body, ending) = split_line_ending(line);
        let classification = classify_line(body, &mut state);

        let Some((previous, previous_classification)) = fixed.last_mut() else {
            fixed.push((format!("{body}{ending}"), classification));
            continue;
        };

        match plan_fix(
            previous,
            previous_classification,
            body,
            &classification,
            settings,
        ) {
            LineFix::None => fixed.push((format!("{body}{ending}"), classification)),
            LineFix::GlueFirstToken { token, remainder } => {
                let previous_body = strip_line_ending(previous);
                if remainder.is_empty() {
                    // Строка-носитель удалена целиком: её окончание
                    // переходит на объединённую строку.
                    *previous = format!("{previous_body}{token}{ending}");
                } else {
                    // prev сохраняет своё окончание; строка-носитель
                    // (остаток-комментарий) — окончание cur.
                    let previous_ending = line_ending_of(previous);
                    *previous = format!("{previous_body}{token}{previous_ending}");
                    fixed.push((format!("{remainder}{ending}"), classification));
                }
            }
            LineFix::MoveOperatorForward { operator } => {
                // Оператор отклеивается от rstrip-тела prev по фактической
                // длине среза (план матчит оператор на rstrip-теле; от
                // нетронутого тела `strip_suffix` молча проваливается на
                // хвостовом пробеле и оператор остаётся на prev И попадает
                // в cur — рост `+ + Б`). Хвост rstrip — иначе
                // `УдалениеЛишнихКонцевыхПробелов` найдёт пробелы.
                let previous_body = strip_line_ending(previous);
                let previous_ending = line_ending_of(previous);
                let operator_start = previous_body
                    .trim_end_matches([' ', '\t'])
                    .len()
                    .saturating_sub(operator.len());
                let without_operator = &previous_body[..operator_start];
                *previous = format!(
                    "{}{}",
                    without_operator.trim_end_matches([' ', '\t']),
                    previous_ending
                );

                let indent_len = body.len() - body.trim_start_matches([' ', '\t']).len();
                fixed.push((
                    format!(
                        "{}{} {}{}",
                        &body[..indent_len],
                        operator,
                        &body[indent_len..],
                        ending
                    ),
                    classification,
                ));
            }
        }
    }

    fixed.into_iter().map(|(line, _)| line).collect()
}

/// Какое изменение нужно текущей паре (prev, cur).
#[derive(Debug, PartialEq, Eq)]
enum LineFix<'a> {
    None,
    /// Снять известный FIRST-токен с cur, приклеить к prev; `remainder` —
    /// строка-носитель без токена (комментарий или пустая).
    GlueFirstToken {
        token: &'a str,
        remainder: String,
    },
    /// Перенести известный END-оператор из prev в начало кода cur.
    MoveOperatorForward {
        operator: String,
    },
}

/// Спланировать фикс пары (prev, cur): `previous` — собранная строка (с
/// окончанием) и классификация её исходного тела, `cur_body` — исходное тело
/// cur с его классификацией.
fn plan_fix<'a>(
    previous: &str,
    previous_classification: &LineClassification,
    cur_body: &'a str,
    cur_classification: &LineClassification,
    settings: &LineBreakSettings,
) -> LineFix<'a> {
    // Query-регион и строка-продолжение: cur не код — пару не трогаем (в том
    // числе END: оператор из prev не должен попадать в строку/запрос).
    if cur_classification.in_query || starts_in_string(cur_body, cur_classification) {
        return LineFix::None;
    }

    // FIRST: известный токен в начале кодовой части cur, prev — код-строка
    // без `//`-комментария (ревью F1: вклейка в комментарий-хвост ломает
    // файл — `Аргумент // комм` + `\t);` → `Аргумент // комм);`) и без
    // check-only нарушений (приклейка к `,`-строке даёт мусор вида
    // `Оповещение);` — план F6: `,`-кейсы остаются для ручной правки).
    if settings.check_first
        && is_code_line(strip_line_ending(previous), previous_classification)
        && previous_classification.comment_start.is_none()
        && !starts_with_check_only_token(strip_line_ending(previous))
    {
        let indent_len = cur_body.len() - cur_body.trim_start_matches([' ', '\t']).len();
        let after_indent = &cur_body[indent_len..];
        let token = FIXABLE_FIRST_TOKENS
            .iter()
            .copied()
            .find(|token| after_indent.starts_with(token));

        if let Some(token) = token {
            let token_end = indent_len + token.len();
            let rest = &cur_body[token_end..];
            // Токен — код (не строка/комментарий), а за ним только пробелы
            // или комментарий до конца строки (висячий токен, не `\t); хвост`).
            if cur_classification.has_code_in(indent_len, token_end)
                && (rest.trim().is_empty() || rest.trim_start().starts_with("//"))
            {
                let previous_body = strip_line_ending(previous);
                // Skip: prev уже оканчивается тем же токеном (защита `;;`).
                if !previous_body.trim_end().ends_with(token) {
                    return LineFix::GlueFirstToken {
                        token,
                        remainder: trim_line_tail(&cur_body[..indent_len], rest),
                    };
                }
            }
        }
    }

    // END: известный оператор в конце кодовой части prev, cur — код-строка.
    if settings.check_last && is_code_line(cur_body, cur_classification) {
        let previous_body = strip_line_ending(previous);
        let trimmed = previous_body.trim_end_matches([' ', '\t']);
        if let Some(operator) = trailing_known_operator(trimmed) {
            let operator_start = trimmed.len() - operator.len();
            // Оператор отделён пробелом от кода слева (дефолтный END-паттерн
            // bsl-ls требует `\s+` перед оператором) и является кодом на
            // prev (не строка, не комментарий — напр. `// хвост И` не трогаем).
            if operator_start > 0
                && trimmed[..operator_start].ends_with([' ', '\t'])
                && previous_classification.has_code_in(operator_start, trimmed.len())
            {
                return LineFix::MoveOperatorForward {
                    operator: operator.to_owned(),
                };
            }
        }
    }

    LineFix::None
}

/// Остаток строки-носителя FIRST-токена: ведущий отступ сохраняется, пробелы
/// между токеном и комментарием убираются; пустой остаток → пустая строка
/// (носитель удаляется).
fn trim_line_tail(indent: &str, rest: &str) -> String {
    let rest = rest.trim();
    if rest.is_empty() {
        String::new()
    } else {
        format!("{indent}{rest}")
    }
}

/// Тело cur начинается внутри `"`-литерала (продолжение с предыдущей строки)?
fn starts_in_string(cur_body: &str, classification: &LineClassification) -> bool {
    !cur_body.is_empty()
        && classification
            .string_ranges
            .iter()
            .any(|&(start, _)| start == 0)
}

/// Строка — код: не пустая, не чистый `//`-комментарий, содержит код вне
/// строк и комментария. Диапазон `has_code_in(0, len)` не подходит: строка
/// с любым литералом посередине дала бы false; проверяем по сегментам.
fn is_code_line(body: &str, classification: &LineClassification) -> bool {
    let trimmed = body.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return false;
    }

    let limit = classification.comment_start.unwrap_or(body.len());
    let mut cursor = 0;
    for &(range_start, range_end) in &classification.string_ranges {
        let segment_end = range_start.min(limit);
        if segment_end > cursor && !body[cursor..segment_end].trim().is_empty() {
            return true;
        }
        cursor = range_end.max(cursor);
    }
    cursor < limit && !body[cursor..limit].trim().is_empty()
}

/// Код-часть строки начинается с check-only FIRST-токена (`,`)? К такому
/// prev фиксер ничего не приклеивает: `,`-нарушение лечится только вручную.
fn starts_with_check_only_token(body: &str) -> bool {
    let after_indent = body.trim_start_matches([' ', '\t']);
    after_indent.starts_with(',')
}

/// Известный END-оператор в конце тела: возвращает **фактический срез хвоста
/// как в файле** (байт-диапазон). Словные операторы (`И`/`ИЛИ`/`OR`/`AND`)
/// сравниваются кейс-инсенситивно (через `to_lowercase`, кириллица включена)
/// — lowercase `и`/`or` валиден в BSL и фиксируется; отклейка и вставка идут
/// по фактическому срезу (исходное написание сохраняется — ревью F2:
/// байт-точный `strip_suffix("И")` на lowercase `и` не отклеивал ничего, а
/// канонический `И` в cur давал не-идемпотентный рост `И И Б = В`).
/// Знаковые операторы сравниваются точно; разрез — только по границе символа.
fn trailing_known_operator(body: &str) -> Option<&str> {
    for operator in FIXABLE_LAST_OPERATORS {
        let split = body.len().saturating_sub(operator.len());
        if !body.is_char_boundary(split) {
            continue;
        }
        let tail = &body[split..];
        let matches = if operator.chars().all(|char| char.is_alphabetic()) {
            tail.to_lowercase() == operator.to_lowercase()
        } else {
            tail == *operator
        };
        if matches {
            return Some(tail);
        }
    }
    None
}

/// Тело строки без её окончания (`fixed_lines` хранят строки с окончаниями).
fn strip_line_ending(line: &str) -> &str {
    split_line_ending(line).0
}

fn line_ending_of(line: &str) -> &str {
    split_line_ending(line).1
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
/// (паритет bsl-ls) по настройкам `НастройкиСценариев` и механически
/// безопасный фикс известного подмножества токенов.
///
/// Контракт режимов:
/// - **DryRun (check)**: per-violation `warning` со span плюс, если фикс
///   меняет файл, один `modified_dry_run` с `proposed_content`
///   (diff-превью `render_dry_run_diffs` подхватывает автоматически);
/// - **Apply (fix/hook/exec-rules)**: если фикс меняет файл — только
///   `modified` (единственный результат, warnings не эмитятся — прецедент
///   всех фикстеров); если не меняет — `ScenarioRun::clean()`
///   (идемпотентность: повторный прогон → 0 результатов, exit 0).
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

    let violations = find_line_break_violations(&input, &settings);

    if context.write_policy == WritePolicy::Apply {
        // Apply: только modified — невылеченные skip-нарушения видны через
        // `check`, не ломают exit-0 идемпотентность фикса.
        let output = fix_line_breaks(&input, &settings);
        if output == input {
            return ScenarioRun::clean();
        }

        return crate::text_fixers::apply_text_modification(
            context,
            &path,
            output,
            "fixed incorrect line breaks",
        );
    }

    // DryRun: per-violation warnings + опциональный modified_dry_run.
    let mut results = violations
        .into_iter()
        .map(|violation| {
            ScenarioResult::warning(
                context.rule_id,
                context.file.repo_path.clone(),
                format!("incorrect line break at line {}", violation.line),
            )
            .with_source_span(SourceSpan::new(violation.byte_start, violation.byte_end))
        })
        .collect::<Vec<_>>();

    let output = fix_line_breaks(&input, &settings);
    if output != input {
        results.push(ScenarioResult::modified_dry_run(
            context.rule_id,
            context.file.repo_path.clone(),
            "fixed incorrect line breaks",
            output,
        ));
    }

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

    // Task 3: фиксер `fix_line_breaks`.

    #[test]
    fn fix_glues_semicolon_to_previous_line() {
        let input = "\tВызов(Арг)\n\t;";
        let output = fix_line_breaks(input, &LineBreakSettings::default_settings());

        assert_eq!(output, "\tВызов(Арг);");
    }

    #[test]
    fn fix_glues_semicolon_keeps_comment() {
        let input = "\tВызов(Арг)\n\t; // комментарий";
        let output = fix_line_breaks(input, &LineBreakSettings::default_settings());

        assert_eq!(output, "\tВызов(Арг);\n\t// комментарий");
    }

    #[test]
    fn fix_glues_close_paren() {
        let input = "\tВызов(\n\t\tАргумент\n\t);";
        let output = fix_line_breaks(input, &LineBreakSettings::default_settings());

        assert_eq!(output, "\tВызов(\n\t\tАргумент);");
    }

    #[test]
    fn fix_skips_semicolon_after_semicolon() {
        let input = "Часть();\n\t;";
        let output = fix_line_breaks(input, &LineBreakSettings::default_settings());

        assert_eq!(
            output, input,
            "gluing to a prev already ending with ; must be skipped"
        );
    }

    #[test]
    fn fix_skips_comma_first() {
        for input in [
            "Вызов(\n\t, Оповещение\n);",
            "Вызов(\n\t,,,,, Оповещение\n);",
        ] {
            let output = fix_line_breaks(input, &LineBreakSettings::default_settings());
            assert_eq!(
                output, input,
                "comma-first violations are check-only: {input:?}"
            );
        }
    }

    #[test]
    fn fix_moves_operator_to_next_line_start() {
        let input = "\tЕсли А = Б И\n\t\tБ = В Тогда";
        let output = fix_line_breaks(input, &LineBreakSettings::default_settings());

        assert_eq!(output, "\tЕсли А = Б\n\t\tИ Б = В Тогда");
    }

    #[test]
    fn fix_skips_operator_before_comment_line() {
        let input = "\tС = А +\n\t\t// комментарий";
        let output = fix_line_breaks(input, &LineBreakSettings::default_settings());

        assert_eq!(
            output, input,
            "operator before a pure comment line must be skipped"
        );
    }

    #[test]
    fn fix_skips_operator_before_empty_line() {
        let input = "\tС = А +\n\n\tБ;";
        let output = fix_line_breaks(input, &LineBreakSettings::default_settings());

        assert_eq!(
            output, input,
            "operator before an empty line must be skipped"
        );
    }

    #[test]
    fn fix_ignores_custom_pattern_matches() {
        // Кастомный FIRST-паттерн: его матчи — check-only, фиксер не трогает.
        let settings = LineBreakSettings::custom(true, r"@", true, DEFAULT_LAST_SYMBOL_PATTERN);
        let custom_input = "Проц()\n\t@токен";
        assert_eq!(
            fix_line_breaks(custom_input, &settings),
            custom_input,
            "custom pattern matches are check-only"
        );

        // Известные токены фиксируются и на кастомных паттернах: check следует
        // настройкам, fix — только механикам известных токенов.
        assert_eq!(
            fix_line_breaks("Проц()\n\t;", &settings),
            "Проц();",
            "known tokens stay fixable under custom patterns"
        );
    }

    #[test]
    fn fix_preserves_line_endings() {
        let crlf = "\tВызов(Арг)\r\n\t;\r\n";
        assert_eq!(
            fix_line_breaks(crlf, &LineBreakSettings::default_settings()),
            "\tВызов(Арг);\r\n"
        );

        // Окончание удалённой строки-носителя переносится на prev: mixed-концы
        // сохраняются построчно, `\r` не попадает в приклеенный токен.
        let mixed = "\tВызов(Арг)\r\n\t;\n";
        assert_eq!(
            fix_line_breaks(mixed, &LineBreakSettings::default_settings()),
            "\tВызов(Арг);\n"
        );
    }

    #[test]
    fn fix_preserves_final_no_newline() {
        let input = "\tВызов(Арг)\n\t;";
        let output = fix_line_breaks(input, &LineBreakSettings::default_settings());

        assert_eq!(output, "\tВызов(Арг);");

        let clean = "Процедура А()\nКонецПроцедуры";
        assert_eq!(
            fix_line_breaks(clean, &LineBreakSettings::default_settings()),
            clean
        );
    }

    #[test]
    fn fix_skips_gluing_into_comment_line() {
        // Ревью F1: prev-строка с `//`-комментарием в конце — приклейка
        // FIRST-токена вклеивает его в комментарий (`Аргумент // комм` +
        // `\t);` → `Аргумент // комм);` — `)` проглатывается комментарием,
        // файл ломается). Нарушение остаётся check-only.
        let call_in_comment =
            "Процедура А()\n\tВызов(\n\t\tАргумент // комм\n\t);\nКонецПроцедуры\n";
        assert_eq!(
            fix_line_breaks(call_in_comment, &LineBreakSettings::default_settings()),
            call_in_comment,
            "gluing into a prev line ending with a comment must be skipped"
        );

        // Лёгкий вариант: `\t;` после `Вызов(Арг) // комм` — приклейка дала
        // бы `Вызов(Арг) // комм;` (искажение комментария).
        let semicolon_in_comment = "Процедура А()\n\tВызов(Арг) // комм\n\t;\nКонецПроцедуры\n";
        assert_eq!(
            fix_line_breaks(semicolon_in_comment, &LineBreakSettings::default_settings()),
            semicolon_in_comment,
            "gluing a semicolon after a trailing comment must be skipped"
        );

        // `;;`-защита тоже не должна обходиться через комментарий:
        // `Строка1(); // комм` + `\t;` → не должно стать `... // комм;`.
        let double_semicolon_comment = "Процедура А()\n\tСтрока1(); // комм\n\t;\nКонецПроцедуры\n";
        assert_eq!(
            fix_line_breaks(
                double_semicolon_comment,
                &LineBreakSettings::default_settings()
            ),
            double_semicolon_comment,
            "semicolon gluing into a comment line must stay skipped"
        );
    }

    #[test]
    fn fix_moves_lowercase_operator() {
        // Ревью F2: словные END-операторы матчатся кейс-инсенситивно, но
        // отклейка была байт-точной — lowercase `и` не отклеивалось, а в cur
        // вставлялся канонический `И`: каждый прогон добавлял ещё один `И`.
        let input = "\tЕсли А = Б и\n\t\tБ = В Тогда";
        let once = fix_line_breaks(input, &LineBreakSettings::default_settings());

        assert_eq!(once, "\tЕсли А = Б\n\t\tи Б = В Тогда");

        let twice = fix_line_breaks(&once, &LineBreakSettings::default_settings());
        assert_eq!(
            twice, once,
            "fixer must be idempotent on a lowercase operator"
        );
    }

    #[test]
    fn fix_moves_mixed_case_operator() {
        let input = "\tЕсли А = Б Или\n\t\tБ = В Тогда";
        let once = fix_line_breaks(input, &LineBreakSettings::default_settings());

        assert_eq!(once, "\tЕсли А = Б\n\t\tИли Б = В Тогда");

        let twice = fix_line_breaks(&once, &LineBreakSettings::default_settings());
        assert_eq!(
            twice, once,
            "fixer must be idempotent on a mixed-case operator"
        );
    }

    #[test]
    fn fix_moves_lowercase_latin_operator() {
        let input = "If A = B and\n\t\tB = C Then";
        let once = fix_line_breaks(input, &LineBreakSettings::default_settings());

        assert_eq!(once, "If A = B\n\t\tand B = C Then");

        let twice = fix_line_breaks(&once, &LineBreakSettings::default_settings());
        assert_eq!(
            twice, once,
            "fixer must be idempotent on a lowercase latin operator"
        );
    }

    #[test]
    fn fix_moves_operator_with_trailing_whitespace() {
        // Хвостовой пробел после оператора: план матчит оператор на
        // rstrip-теле, отклейка должна идти от того же базиса — иначе
        // `strip_suffix` на нетронутом теле проваливается и оператор
        // остаётся на prev и дублируется в cur.
        let cases = [
            (
                "\tЕсли А = Б И \n\t\tБ = В Тогда",
                "\tЕсли А = Б\n\t\tИ Б = В Тогда",
            ),
            (
                "\tЕсли А = Б И\t\n\t\tБ = В Тогда",
                "\tЕсли А = Б\n\t\tИ Б = В Тогда",
            ),
            ("\tС = А + \n\t\tБ;", "\tС = А\n\t\t+ Б;"),
        ];
        for (input, expected) in cases {
            let once = fix_line_breaks(input, &LineBreakSettings::default_settings());
            assert_eq!(once, expected, "trailing-whitespace case {input:?}");

            let twice = fix_line_breaks(&once, &LineBreakSettings::default_settings());
            assert_eq!(twice, once, "fixer must be idempotent on {input:?}");
        }
    }

    #[test]
    fn fix_idempotent() {
        let defaults = LineBreakSettings::default_settings();
        let custom = LineBreakSettings::custom(true, r"@", true, DEFAULT_LAST_SYMBOL_PATTERN);
        let cases: &[(&str, &LineBreakSettings)] = &[
            ("\tВызов(Арг)\n\t;", &defaults),
            ("\tВызов(Арг)\n\t; // комментарий", &defaults),
            ("\tВызов(\n\t\tАргумент\n\t);", &defaults),
            ("Часть();\n\t;", &defaults),
            ("Вызов(\n\t, Оповещение\n);", &defaults),
            ("Вызов(\n\t,,,,, Оповещение\n);", &defaults),
            ("\tЕсли А = Б И\n\t\tБ = В Тогда", &defaults),
            ("\tС = А +\n\t\t// комментарий", &defaults),
            ("\tС = А +\n\n\tБ;", &defaults),
            ("Проц()\n\t@токен", &custom),
            ("\tВызов(Арг)\r\n\t;\r\n", &defaults),
            ("\tВызов(Арг)\r\n\t;\n", &defaults),
            ("Проц()\n\t;", &custom),
            // Ревью F1/F2: комментарий-prev и не-канонический регистр
            // словного оператора — обе новых проверки держат идемпотентность.
            ("\tЕсли А = Б и\n\t\tБ = В Тогда", &defaults),
            ("\tЕсли А = Б Или\n\t\tБ = В Тогда", &defaults),
            ("If A = B and\n\t\tB = C Then", &defaults),
            ("If A = B or\n\t\tB = C Then", &defaults),
            (
                "Процедура А()\n\tВызов(\n\t\tАргумент // комм\n\t);\nКонецПроцедуры\n",
                &defaults,
            ),
            (
                "Процедура А()\n\tВызов(Арг) // комм\n\t;\nКонецПроцедуры\n",
                &defaults,
            ),
            (
                "Процедура А()\n\tСтрока1(); // комм\n\t;\nКонецПроцедуры\n",
                &defaults,
            ),
        ];

        for (input, settings) in cases {
            let once = fix_line_breaks(input, settings);
            let twice = fix_line_breaks(&once, settings);
            assert_eq!(twice, once, "fixer must be idempotent on {input:?}");
        }
    }

    #[test]
    fn fix_noop_clean() {
        let input = concat!(
            "Процедура Тест()\n",
            "\tВызов(Арг);\n",
            "\tЕсли А = Б\n",
            "\t\tИ Б = В Тогда\n",
            "\t\tСообщить(\"готово\");\n",
            "\tКонецЕсли;\n",
            "КонецПроцедуры\n",
        );
        let output = fix_line_breaks(input, &LineBreakSettings::default_settings());

        assert_eq!(output, input);
    }

    #[test]
    fn fix_respects_disabled_checks() {
        let first_disabled = LineBreakSettings::custom(
            false,
            DEFAULT_FIRST_SYMBOL_PATTERN,
            true,
            DEFAULT_LAST_SYMBOL_PATTERN,
        );
        let first_input = "\tВызов(Арг)\n\t;";
        assert_eq!(
            fix_line_breaks(first_input, &first_disabled),
            first_input,
            "checkFirstSymbol=false must gate first-symbol fixes"
        );

        let last_disabled = LineBreakSettings::custom(
            true,
            DEFAULT_FIRST_SYMBOL_PATTERN,
            false,
            DEFAULT_LAST_SYMBOL_PATTERN,
        );
        let last_input = "\tЕсли А = Б И\n\t\tБ = В Тогда";
        assert_eq!(
            fix_line_breaks(last_input, &last_disabled),
            last_input,
            "checkLastSymbol=false must gate last-symbol fixes"
        );
    }
}
