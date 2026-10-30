use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use prec_bsl_pipeline::{
    ScenarioDefinition, ScenarioExecutionContext, ScenarioResult, ScenarioRun,
};
use prec_bsl_source::SourceFileKind;

pub const PROFANITY_RULE: &str = "ПроверкаНецензурныхСлов";
pub const PROFANITY_SCENARIO: ScenarioDefinition =
    ScenarioDefinition::required_v1(PROFANITY_RULE, "ПроверкаНецензурныхСлов.os", profanity);

const DICTIONARY_SETTING: &str = "ФайлСНецензурнымиСловами";
const DEFAULT_DICTIONARY: &str = "НецензурныеСлова.txt";

pub fn profanity(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    if context.file.kind != SourceFileKind::BslModule {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            "scenario handles only BSL modules",
        ));
    }

    let dictionary = match load_dictionary(context) {
        DictionaryLoad::Loaded(dictionary) => dictionary,
        DictionaryLoad::Skipped(message) => {
            return ScenarioRun::single(ScenarioResult::skipped(
                context.rule_id,
                context.file.repo_path.clone(),
                message,
            ));
        }
        DictionaryLoad::Failed(message) => {
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

    let results = find_profanity_matches(&input, &dictionary)
        .into_iter()
        .map(|matched| {
            ScenarioResult::warning(
                context.rule_id,
                context.file.repo_path.clone(),
                format!(
                    "matched dictionary word '{}' at line {}",
                    matched.word, matched.line
                ),
            )
        })
        .collect();

    ScenarioRun {
        results,
        post_processing_paths: Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DictionaryLoad {
    Loaded(Vec<String>),
    Skipped(String),
    Failed(String),
}

fn load_dictionary(context: &ScenarioExecutionContext<'_>) -> DictionaryLoad {
    let dictionary_path = match dictionary_path(context.settings) {
        Ok(Some(path)) => path,
        Ok(None) => {
            let path = PathBuf::from(DEFAULT_DICTIONARY);
            return load_default_dictionary(context.repo_root, &path);
        }
        Err(message) => return DictionaryLoad::Failed(message),
    };

    if !is_repository_relative_path(&dictionary_path) {
        return DictionaryLoad::Failed(format!(
            "dictionary path must be repository-relative: {}",
            dictionary_path.display()
        ));
    }

    load_configured_dictionary(context.repo_root, &dictionary_path)
}

fn dictionary_path(settings: Option<&Value>) -> Result<Option<PathBuf>, String> {
    let Some(settings) = settings else {
        return Ok(None);
    };
    let Some(value) = settings.get(DICTIONARY_SETTING) else {
        return Ok(None);
    };
    match value {
        Value::String(path) if path.trim().is_empty() => Err(format!(
            "dictionary setting {DICTIONARY_SETTING} must not be empty"
        )),
        Value::String(path) => Ok(Some(PathBuf::from(path.trim()))),
        _ => Err(format!(
            "dictionary setting {DICTIONARY_SETTING} must be a string"
        )),
    }
}

fn load_default_dictionary(repo_root: &Path, path: &Path) -> DictionaryLoad {
    if !repo_root.join(path).is_file() {
        return DictionaryLoad::Skipped(format!(
            "profanity dictionary is not configured or found: {}",
            path.display()
        ));
    }

    match fs::read_to_string(repo_root.join(path)) {
        Ok(content) => DictionaryLoad::Loaded(parse_dictionary(&content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            DictionaryLoad::Skipped(format!(
                "profanity dictionary is not configured or found: {}",
                path.display()
            ))
        }
        Err(error) => DictionaryLoad::Failed(format!(
            "failed to read profanity dictionary {}: {error}",
            path.display()
        )),
    }
}

fn load_configured_dictionary(repo_root: &Path, path: &Path) -> DictionaryLoad {
    match fs::read_to_string(repo_root.join(path)) {
        Ok(content) => DictionaryLoad::Loaded(parse_dictionary(&content)),
        Err(error) => DictionaryLoad::Failed(format!(
            "failed to read configured profanity dictionary {}: {error}",
            path.display()
        )),
    }
}

fn parse_dictionary(input: &str) -> Vec<String> {
    input
        .lines()
        .map(str::trim)
        .filter(|word| !word.is_empty() && !word.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfanityMatch {
    pub line: usize,
    pub word: String,
}

pub fn find_profanity_matches(input: &str, dictionary: &[String]) -> Vec<ProfanityMatch> {
    input
        .lines()
        .enumerate()
        .flat_map(|(line_index, line)| {
            let line_lowercase = line.to_lowercase();
            dictionary
                .iter()
                .filter(move |word| line_lowercase.contains(&word.to_lowercase()))
                .map(move |word| ProfanityMatch {
                    line: line_index + 1,
                    word: word.clone(),
                })
        })
        .collect()
}

fn is_repository_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                Component::Normal(_) | Component::CurDir | Component::ParentDir
            )
        })
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
}

pub const LINE_LENGTH_RULE: &str = "ПроверкаДлиныСтрок";
pub const LINE_LENGTH_SCENARIO: ScenarioDefinition =
    ScenarioDefinition::compatibility(LINE_LENGTH_RULE, "ПроверкаДлиныСтрок.os", line_length);

const MAX_LINE_LENGTH_SETTING: &str = "МаксимальнаяДлинаСтроки";
pub const DEFAULT_MAX_LINE_LENGTH: usize = 150;

pub fn line_length(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    if context.file.kind != SourceFileKind::BslModule {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            "scenario handles only BSL modules",
        ));
    }

    let limit = match max_line_length(context.settings) {
        Ok(limit) => limit,
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

    let results = find_long_lines(&input, limit)
        .into_iter()
        .map(|matched| {
            ScenarioResult::warning(
                context.rule_id,
                context.file.repo_path.clone(),
                format!(
                    "String length {} exceeded maximum {} at line {}",
                    matched.length, limit, matched.line
                ),
            )
        })
        .collect();

    ScenarioRun {
        results,
        post_processing_paths: Vec::new(),
    }
}

fn max_line_length(settings: Option<&Value>) -> Result<usize, String> {
    let Some(settings) = settings else {
        return Ok(DEFAULT_MAX_LINE_LENGTH);
    };
    let Some(value) = settings.get(MAX_LINE_LENGTH_SETTING) else {
        return Ok(DEFAULT_MAX_LINE_LENGTH);
    };
    match value {
        Value::Number(number) => number
            .as_u64()
            .and_then(|limit| usize::try_from(limit).ok())
            .filter(|limit| *limit > 0)
            .ok_or_else(invalid_setting_message),
        _ => Err(invalid_setting_message()),
    }
}

fn invalid_setting_message() -> String {
    format!("max line length setting {MAX_LINE_LENGTH_SETTING} must be a positive integer")
}

/// One over-limit source line: 1-based line number and its length in Unicode
/// scalar values (trailing whitespace and line-1 BOM excluded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LongLineMatch {
    pub line: usize,
    pub length: usize,
}

pub fn find_long_lines(input: &str, limit: usize) -> Vec<LongLineMatch> {
    let mut matches = Vec::new();
    let mut in_string_literal = false;

    for (line_index, raw_line) in logical_lines(input).enumerate() {
        let line = if line_index == 0 {
            raw_line.strip_prefix('\u{feff}').unwrap_or(raw_line)
        } else {
            raw_line
        };
        let stripped = line.trim_start();

        let counted = if in_string_literal && stripped.starts_with('|') {
            // `|`-continuation of a multiline string literal: skipped entirely
            // unless code follows the closing quote on this line.
            match string_tail_remainder(stripped) {
                None => false,
                Some(remainder) => {
                    in_string_literal = ends_in_string_literal(remainder);
                    has_code(remainder)
                }
            }
        } else {
            in_string_literal = ends_in_string_literal(stripped);
            true
        };

        if counted {
            let length = line.trim_end().chars().count();
            if length > limit {
                matches.push(LongLineMatch {
                    line: line_index + 1,
                    length,
                });
            }
        }
    }

    matches
}

/// Source lines split on `\r\n`, `\n`, and a lone `\r`, mirroring the
/// `bsl-language-server` reader (Java `readLine` semantics). A lone `\r`
/// terminates a line in legacy Designer dumps with CR-only endings, where
/// `str::lines()` would treat the whole file as one line.
fn logical_lines(input: &str) -> impl Iterator<Item = &str> {
    let mut rest = input;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let Some(index) = rest.find(['\n', '\r']) else {
            let line = rest;
            rest = "";
            return Some(line);
        };
        let line = &rest[..index];
        let after = &rest[index..];
        let terminator = if after.starts_with("\r\n") { 2 } else { 1 };
        rest = &rest[index + terminator..];
        Some(line)
    })
}

/// Remainder of a `|`-continuation line after its closing quote (`""` escapes
/// honored). `None` means the literal stays open past this line.
fn string_tail_remainder(stripped: &str) -> Option<&str> {
    let mut rest = &stripped[1..];
    loop {
        let quote_index = rest.find('"')?;
        if rest[quote_index + 1..].starts_with('"') {
            rest = &rest[quote_index + 2..];
        } else {
            return Some(&rest[quote_index + 1..]);
        }
    }
}

/// Whether the code after a closing quote is anything but `;` and whitespace.
/// A `;` directly following a string part is excluded, mirroring bsl-ls.
fn has_code(remainder: &str) -> bool {
    remainder.chars().any(|ch| ch != ';' && !ch.is_whitespace())
}

/// Whether the line ends inside an unclosed string literal. Quotes after a
/// `//` comment start are ignored: comments never open a literal.
fn ends_in_string_literal(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut in_literal = false;
    while index < bytes.len() {
        match bytes[index] {
            b'"' if in_literal => {
                if bytes.get(index + 1) == Some(&b'"') {
                    index += 2;
                } else {
                    in_literal = false;
                    index += 1;
                }
            }
            b'"' => {
                in_literal = true;
                index += 1;
            }
            b'/' if !in_literal && bytes.get(index + 1) == Some(&b'/') => {
                return in_literal;
            }
            _ => index += 1,
        }
    }
    in_literal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_long_lines_flags_line_over_limit() {
        let line = "А".repeat(151);
        assert_eq!(
            find_long_lines(&line, 150),
            vec![LongLineMatch {
                line: 1,
                length: 151
            }]
        );
    }

    #[test]
    fn find_long_lines_exact_limit_not_flagged() {
        let line = "А".repeat(150);
        assert_eq!(find_long_lines(&line, 150), Vec::new());
    }

    #[test]
    fn find_long_lines_counts_chars_not_bytes() {
        // 151 кириллический символ = 302 UTF-8 байта, но 151 символ: длина считается
        // в символах, не в байтах.
        let line = "Ы".repeat(151);
        assert_eq!(
            find_long_lines(&line, 150),
            vec![LongLineMatch {
                line: 1,
                length: 151
            }]
        );
    }

    #[test]
    fn find_long_lines_flags_long_multiline_literal_opening_line() {
        // Открывающая строка литерала — обычная строка и флагается, если длинная;
        // её `|`-продолжение не флагается.
        let opening = format!("Текст = \"{}", "А".repeat(142));
        let continuation = format!("|{}", "Б".repeat(160));
        let input = format!("{opening}\n{continuation}");
        assert_eq!(
            find_long_lines(&input, 150),
            vec![LongLineMatch {
                line: 1,
                length: 151
            }]
        );
    }

    #[test]
    fn find_long_lines_skips_multiline_string_continuations() {
        // Незакрытая `"` на строке 1; строки 2–3 начинаются с `|` и длиннее лимита —
        // не флагаются; строка 4 обычная длинная — флагается.
        let opening = format!("Текст = \"{}", "А".repeat(100));
        let continuation = format!("|{}", "Б".repeat(160));
        let normal_long = "В".repeat(151);
        let input = format!("{opening}\n{continuation}\n{continuation}\n{normal_long}");
        assert_eq!(
            find_long_lines(&input, 150),
            vec![LongLineMatch {
                line: 4,
                length: 151
            }]
        );
    }

    #[test]
    fn find_long_lines_counts_code_after_closing_quote_on_tail_line() {
        // STRINGTAIL: код после закрывающей `"` на строке `|`-продолжения — остаток
        // строки считается; вся строка флагается, если длинная.
        let tail = format!("|хвост\"; ОстатокКода = 1; {}", "А".repeat(151));
        let input = format!("Текст = \"\n{tail}");
        assert_eq!(
            find_long_lines(&input, 150),
            vec![LongLineMatch {
                line: 2,
                length: 177
            }]
        );

        // `;` сразу после `"`-хвоста без кода — строка не флагается (bsl-ls исключает
        // SEMICOLON сразу после STRINGPART/STRINGTAIL).
        let tail_semicolon_only = format!("|{}\";", "Б".repeat(160));
        let input = format!("Текст = \"\n{tail_semicolon_only}");
        assert_eq!(find_long_lines(&input, 150), Vec::new());
    }

    #[test]
    fn find_long_lines_ignores_trailing_whitespace() {
        let line = format!("{}{}", "А".repeat(150), " ".repeat(10));
        assert_eq!(find_long_lines(&line, 150), Vec::new());
    }

    #[test]
    fn find_long_lines_escapes_quotes_in_literal() {
        // `""`-экранирование не ломает state-tracker: литерал остаётся открытым,
        // `|`-продолжения после него распознаются как продолжения (не флагаются),
        // а обычная строка после закрытия — флагается.
        let opening = format!("Текст = \"начало \"\"{}", "А".repeat(132));
        let continuation = format!("|{}", "Б".repeat(160));
        let normal_long = "В".repeat(151);
        let input = format!("{opening}\n{continuation}\n{normal_long}");
        assert_eq!(
            find_long_lines(&input, 150),
            vec![LongLineMatch {
                line: 3,
                length: 151
            }]
        );
    }

    #[test]
    fn find_long_lines_strips_bom() {
        // BOM не входит в длину первой строки.
        let with_bom_exact = format!("\u{feff}{}", "А".repeat(150));
        assert_eq!(find_long_lines(&with_bom_exact, 150), Vec::new());

        let with_bom_over = format!("\u{feff}{}", "А".repeat(151));
        assert_eq!(
            find_long_lines(&with_bom_over, 150),
            vec![LongLineMatch {
                line: 1,
                length: 151
            }]
        );
    }

    #[test]
    fn find_long_lines_crlf_ignored() {
        // `\r` при CRLF не входит в длину.
        let line = "А".repeat(151);
        let input = format!("{line}\r\n");
        assert_eq!(
            find_long_lines(&input, 150),
            vec![LongLineMatch {
                line: 1,
                length: 151
            }]
        );
    }

    #[test]
    fn find_long_lines_lone_cr_terminates_line() {
        // Designer-выгрузки со старыми CR-only окончаниями строк: одиночный `\r`
        // завершает строку (семантика bsl-ls/Java readLine), а не склеивает файл
        // в одну мегастроку. Реальный случай из parity-корпуса yaxunit
        // (РасходТовара/ObjectModule.bsl: 327 CR-only строк, ни одной > 150).
        let short_a = "СтрокаОдна = 1;";
        let short_b = "СтрокаДве = 2;";
        let long = "Б".repeat(151);
        let input = format!("{short_a}\r{short_b}\r{long}\r");
        assert_eq!(
            find_long_lines(&input, 150),
            vec![LongLineMatch {
                line: 3,
                length: 151
            }]
        );

        // CR-only файл без длинных строк — чист, и строка ровно в лимит не
        // флагается.
        let exact = "В".repeat(150);
        assert_eq!(find_long_lines(&format!("{exact}\r"), 150), Vec::new());
    }

    #[test]
    fn find_long_lines_flags_long_comment() {
        // Комментарий длиннее лимита флагается как обычная строка.
        let comment = format!("// {}", "А".repeat(151));
        assert_eq!(
            find_long_lines(&comment, 150),
            vec![LongLineMatch {
                line: 1,
                length: 154
            }]
        );
    }

    #[test]
    fn find_long_lines_pipe_in_comment_is_normal_line() {
        // `|` в комментарии вне литерала — обычная строка (флагается, если длинная).
        let comment_pipe = format!("// | {}", "А".repeat(150));
        assert_eq!(
            find_long_lines(&comment_pipe, 150),
            vec![LongLineMatch {
                line: 1,
                length: 155
            }]
        );
    }

    #[test]
    fn find_long_lines_empty_and_literals_only_input() {
        // Пустой ввод → нет совпадений.
        assert_eq!(find_long_lines("", 150), Vec::new());

        // Файл только из литералов: открывающая строка короче лимита, продолжения
        // длиннее — не флагаются.
        let literal_only = format!("Текст = \"{}\n|{}", "А".repeat(100), "Б".repeat(160));
        assert_eq!(find_long_lines(&literal_only, 150), Vec::new());
    }

    #[test]
    fn find_long_lines_quote_in_comment_keeps_literal_state_closed() {
        // Кавычка в комментарии не открывает литерал: последующие строки без `|`
        // флагаются как обычные.
        let comment = "// комментарий с \"незакрытой кавычкой";
        let normal_long = "В".repeat(151);
        let input = format!("{comment}\n{normal_long}");
        assert_eq!(
            find_long_lines(&input, 150),
            vec![LongLineMatch {
                line: 2,
                length: 151
            }]
        );
    }

    #[test]
    fn find_long_lines_tab_counts_as_one_char() {
        // Табуляция считается одним символом (не 4/8): 148 табов между символами
        // дают ровно 150 символов — не флагается.
        let tabs_mid = format!("А{}Б", "\t".repeat(148));
        assert_eq!(find_long_lines(&tabs_mid, 150), Vec::new());

        // 149 табов между символами → 151 символ — флагается.
        let tabs_mid_over = format!("А{}Б", "\t".repeat(149));
        assert_eq!(
            find_long_lines(&tabs_mid_over, 150),
            vec![LongLineMatch {
                line: 1,
                length: 151
            }]
        );
    }

    #[test]
    fn profanity_matches_dictionary_words_case_insensitively() {
        let dictionary = vec!["плохоеСлово".to_owned(), "BAD".to_owned()];

        assert_eq!(
            find_profanity_matches("строка\nТут плохоеслово\nbad token", &dictionary),
            vec![
                ProfanityMatch {
                    line: 2,
                    word: "плохоеСлово".to_owned()
                },
                ProfanityMatch {
                    line: 3,
                    word: "BAD".to_owned()
                }
            ]
        );
    }

    #[test]
    fn dictionary_ignores_empty_lines_and_comments() {
        assert_eq!(
            parse_dictionary("  \n# comment\n слово \r\n"),
            vec!["слово".to_owned()]
        );
    }
}
