use std::fs;

use crate::scenario_pipeline::{ScenarioExecutionContext, ScenarioResult, ScenarioRun};
use crate::source_files::SourceFileKind;

pub const TRAILING_WHITESPACE_RULE: &str = "УдалениеЛишнихКонцевыхПробелов";
pub const EXTRA_BLANK_LINES_RULE: &str = "УдалениеЛишнихПустыхСтрок";
pub const KEYWORD_SPACING_RULE: &str = "ДобавлениеПробеловПередКлючевымиСловами";
pub const CANONICAL_SPELLING_RULE: &str = "ИсправлениеНеКаноническогоНаписания";

pub fn trailing_whitespace(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    run_bsl_text_fixer(
        context,
        remove_trailing_spaces_and_tabs,
        "removed trailing spaces or tabs",
    )
}

pub fn extra_blank_lines(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    run_bsl_text_fixer(
        context,
        remove_extra_blank_lines,
        "removed excessive blank lines",
    )
}

pub fn keyword_spacing(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    run_bsl_text_fixer(
        context,
        add_spaces_before_keywords,
        "added spaces before keywords",
    )
}

pub fn canonical_spelling(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    run_bsl_text_fixer(
        context,
        fix_non_canonical_spelling,
        "fixed non-canonical keyword spelling",
    )
}

fn run_bsl_text_fixer(
    context: &ScenarioExecutionContext<'_>,
    fix: fn(&str) -> String,
    modified_message: &str,
) -> ScenarioRun {
    if context.file.kind != SourceFileKind::BslModule {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            "scenario handles only BSL modules",
        ));
    }

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

    let output = fix(&input);
    if output == input {
        return ScenarioRun::clean();
    }

    if let Err(error) = fs::write(&path, output) {
        return ScenarioRun::single(ScenarioResult::hard_failure(
            context.rule_id,
            context.file.repo_path.clone(),
            format!("failed to write file: {error}"),
        ));
    }

    ScenarioRun::single(ScenarioResult::modified(
        context.rule_id,
        context.file.repo_path.clone(),
        modified_message,
    ))
}

pub fn remove_trailing_spaces_and_tabs(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for line in input.split_inclusive('\n') {
        let (body, ending) = split_line_ending(line);
        output.push_str(body.trim_end_matches([' ', '\t']));
        output.push_str(ending);
    }

    output
}

pub fn remove_extra_blank_lines(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut blank_run = Vec::new();

    for line in input.split_inclusive('\n') {
        let (body, ending) = split_line_ending(line);
        if is_blank_line(body, ending) {
            blank_run.push((body, ending));
            continue;
        }

        flush_blank_run(&mut output, &blank_run);
        blank_run.clear();
        output.push_str(body);
        output.push_str(ending);
    }

    flush_blank_run(&mut output, &blank_run);
    output
}

pub fn add_spaces_before_keywords(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut state = KeywordSpacingState::default();
    for line in input.split_inclusive('\n') {
        let (body, ending) = split_line_ending(line);
        output.push_str(&add_spaces_before_keywords_in_line(body, &mut state));
        output.push_str(ending);
    }

    output
}

pub fn fix_non_canonical_spelling(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut state = CanonicalSpellingState::default();

    for line in input.split_inclusive('\n') {
        let (body, ending) = split_line_ending(line);
        output.push_str(&fix_non_canonical_spelling_in_line(body, &mut state));
        output.push_str(ending);
    }

    output
}

#[derive(Debug, Default)]
struct KeywordSpacingState {
    in_string: bool,
    skip_escaped_quote: bool,
}

fn add_spaces_before_keywords_in_line(line: &str, state: &mut KeywordSpacingState) -> String {
    let Some(export_start) = find_export_without_space_after_closing_paren(line, state) else {
        return line.to_owned();
    };

    let mut output = String::with_capacity(line.len() + 1);
    output.push_str(&line[..export_start]);
    output.push(' ');
    output.push_str(&line[export_start..]);
    output
}

fn find_export_without_space_after_closing_paren(
    line: &str,
    state: &mut KeywordSpacingState,
) -> Option<usize> {
    let mut previous_code_char = None;

    for (index, char) in line.char_indices() {
        if char == '"' {
            if state.skip_escaped_quote {
                state.skip_escaped_quote = false;
                continue;
            }
            if state.in_string && line[index + char.len_utf8()..].starts_with('"') {
                state.skip_escaped_quote = true;
                continue;
            }
            state.in_string = !state.in_string;
            previous_code_char = Some(char);
            continue;
        }

        if state.in_string {
            continue;
        }

        if char == '/' && line[index + char.len_utf8()..].starts_with('/') {
            return None;
        }

        if starts_with_export_keyword(&line[index..]) && previous_code_char == Some(')') {
            return Some(index);
        }

        previous_code_char = Some(char);
    }

    None
}

fn starts_with_export_keyword(input: &str) -> bool {
    let Some(candidate) = input.get(.."Экспорт".len()) else {
        return false;
    };

    candidate.to_lowercase() == "экспорт"
        && input["Экспорт".len()..]
            .chars()
            .next()
            .is_none_or(|char| !is_identifier_char(char))
}

fn is_identifier_char(char: char) -> bool {
    char == '_' || char.is_alphanumeric()
}

#[derive(Debug, Default)]
struct CanonicalSpellingState {
    in_string: bool,
    skip_escaped_quote: bool,
}

fn fix_non_canonical_spelling_in_line(line: &str, state: &mut CanonicalSpellingState) -> String {
    let mut output = String::with_capacity(line.len());
    let mut index = 0;

    while index < line.len() {
        let char = line[index..].chars().next().unwrap();

        if char == '"' {
            output.push(char);
            if state.skip_escaped_quote {
                state.skip_escaped_quote = false;
            } else if state.in_string && line[index + char.len_utf8()..].starts_with('"') {
                state.skip_escaped_quote = true;
            } else {
                state.in_string = !state.in_string;
            }
            index += char.len_utf8();
            continue;
        }

        if state.in_string {
            output.push(char);
            index += char.len_utf8();
            continue;
        }

        if char == '/' && line[index + char.len_utf8()..].starts_with('/') {
            output.push_str(&line[index..]);
            break;
        }

        if is_canonical_token_start(char) {
            let start = index;
            index += char.len_utf8();
            while index < line.len() {
                let next_char = line[index..].chars().next().unwrap();
                if !is_canonical_token_continue(next_char) {
                    break;
                }
                index += next_char.len_utf8();
            }

            let token = &line[start..index];
            output.push_str(canonical_keyword(token).unwrap_or(token));
            continue;
        }

        output.push(char);
        index += char.len_utf8();
    }

    output
}

fn is_canonical_token_start(char: char) -> bool {
    char == '#' || char == '&' || is_identifier_char(char)
}

fn is_canonical_token_continue(char: char) -> bool {
    is_identifier_char(char)
}

fn canonical_keyword(token: &str) -> Option<&'static str> {
    if is_reference_accepted_spelling(token) {
        return None;
    }

    let normalized = token.to_lowercase();
    let canonical = match normalized.as_str() {
        "если" => "Если",
        "#если" => "#Если",
        "тогда" => "Тогда",
        "#тогда" => "#Тогда",
        "иначе" => "Иначе",
        "#иначе" => "#Иначе",
        "иначеесли" => "ИначеЕсли",
        "#иначеесли" => "#ИначеЕсли",
        "конецесли" => "КонецЕсли",
        "#конецесли" => "#КонецЕсли",
        "#область" => "#Область",
        "#конецобласти" => "#КонецОбласти",
        "клиент" => "Клиент",
        "наклиенте" => "НаКлиенте",
        "насервере" => "НаСервере",
        "толстыйклиентобычноеприложение" => {
            "ТолстыйКлиентОбычноеПриложение"
        }
        "толстыйклиентуправляемоеприложение" => {
            "ТолстыйКлиентУправляемоеПриложение"
        }
        "сервер" => "Сервер",
        "внешнеесоединение" => "ВнешнееСоединение",
        "тонкийклиент" => "ТонкийКлиент",
        "вебклиент" => "ВебКлиент",
        "&наклиенте" => "&НаКлиенте",
        "&насервере" => "&НаСервере",
        "&насерверебезконтекста" => "&НаСервереБезКонтекста",
        "&наклиентенасерверебезконтекста" => {
            "&НаКлиентеНаСервереБезКонтекста"
        }
        "&наклиентенасервере" => "&НаКлиентеНаСервере",
        "для" => "Для",
        "каждого" => "Каждого",
        "цикл" => "Цикл",
        "конеццикла" => "КонецЦикла",
        "выполнить" => "Выполнить",
        "по" => "По",
        "прервать" => "Прервать",
        "продолжить" => "Продолжить",
        "из" => "Из",
        "новый" => "Новый",
        "перейти" => "Перейти",
        "перем" => "Перем",
        "пока" => "Пока",
        "попытка" => "Попытка",
        "исключение" => "Исключение",
        "конецпопытки" => "КонецПопытки",
        "вызватьисключение" => "ВызватьИсключение",
        "процедура" => "Процедура",
        "конецпроцедуры" => "КонецПроцедуры",
        "функция" => "Функция",
        "конецфункции" => "КонецФункции",
        "возврат" => "Возврат",
        "добавитьобработчик" => "ДобавитьОбработчик",
        "удалитьобработчик" => "УдалитьОбработчик",
        "и" => "И",
        "или" => "ИЛИ",
        "не" => "НЕ",
        "истина" => "Истина",
        "ложь" => "Ложь",
        "знач" => "Знач",
        "неопределено" => "Неопределено",
        "null" => "NULL",
        _ => return None,
    };

    if token == canonical {
        None
    } else {
        Some(canonical)
    }
}

fn is_reference_accepted_spelling(token: &str) -> bool {
    matches!(
        token,
        "Если"
            | "#Если"
            | "Тогда"
            | "#Тогда"
            | "Иначе"
            | "#Иначе"
            | "ИначеЕсли"
            | "#ИначеЕсли"
            | "КонецЕсли"
            | "#КонецЕсли"
            | "#Область"
            | "#КонецОбласти"
            | "Клиент"
            | "НаКлиенте"
            | "НаСервере"
            | "ТолстыйКлиентОбычноеПриложение"
            | "ТолстыйКлиентУправляемоеПриложение"
            | "Сервер"
            | "ВнешнееСоединение"
            | "ТонкийКлиент"
            | "ВебКлиент"
            | "&НаКлиенте"
            | "&НаСервере"
            | "&НаСервереБезКонтекста"
            | "&НаКлиентеНаСервереБезКонтекста"
            | "&НаКлиентеНаСервере"
            | "Для"
            | "Каждого"
            | "Цикл"
            | "КонецЦикла"
            | "Выполнить"
            | "По"
            | "Прервать"
            | "Продолжить"
            | "Из"
            | "Новый"
            | "Перейти"
            | "Перем"
            | "Пока"
            | "Попытка"
            | "Исключение"
            | "КонецПопытки"
            | "ВызватьИсключение"
            | "Процедура"
            | "КонецПроцедуры"
            | "Функция"
            | "КонецФункции"
            | "Возврат"
            | "ДобавитьОбработчик"
            | "УдалитьОбработчик"
            | "И"
            | "ИЛИ"
            | "Или"
            | "НЕ"
            | "Не"
            | "Истина"
            | "ИСТИНА"
            | "Ложь"
            | "ЛОЖЬ"
            | "Знач"
            | "ЗНАЧ"
            | "Неопределено"
            | "НЕОПРЕДЕЛЕНО"
            | "NULL"
            | "Null"
    )
}

fn flush_blank_run(output: &mut String, blank_run: &[(&str, &str)]) {
    if blank_run.len() >= 2 {
        output.push_str(blank_run[0].1);
    } else {
        for (body, ending) in blank_run {
            output.push_str(body);
            output.push_str(ending);
        }
    }
}

fn is_blank_line(body: &str, ending: &str) -> bool {
    !ending.is_empty() && body.trim_matches([' ', '\t']).is_empty()
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
    fn trailing_whitespace_removal_preserves_lf_crlf_and_final_no_newline() {
        let input = "Процедура A()   \n\tСообщить();\t\r\nКонецПроцедуры  ";
        let output = remove_trailing_spaces_and_tabs(input);

        assert_eq!(output, "Процедура A()\n\tСообщить();\r\nКонецПроцедуры");
    }

    #[test]
    fn extra_blank_line_removal_preserves_single_blank_lines_and_line_endings() {
        let input = "Процедура A()\r\n\r\n\r\n\tСообщить();\r\n \t\r\nКонецПроцедуры()";
        let output = remove_extra_blank_lines(input);

        assert_eq!(
            output,
            "Процедура A()\r\n\r\n\tСообщить();\r\n \t\r\nКонецПроцедуры()"
        );
    }

    #[test]
    fn keyword_spacing_adds_space_before_export_and_preserves_case() {
        let input = "Процедура A()Экспорт\r\nФункция B()эКсПоРт // comment\r\n";
        let output = add_spaces_before_keywords(input);

        assert_eq!(
            output,
            "Процедура A() Экспорт\r\nФункция B() эКсПоРт // comment\r\n"
        );
    }

    #[test]
    fn keyword_spacing_ignores_strings_comments_and_existing_spaces() {
        let input = concat!(
            "Процедура A() Экспорт\n",
            "// Процедура B()Экспорт\n",
            "Строка = \"A()Экспорт\";\n",
            "Строка = \"A \"\" B()Экспорт\"\" C\";"
        );
        let output = add_spaces_before_keywords(input);

        assert_eq!(output, input);
    }

    #[test]
    fn keyword_spacing_ignores_multiline_strings() {
        let input = concat!(
            "Строка = \"первая\n",
            "|Процедура B()Экспорт\n",
            "|\";\n",
            "Процедура A()Экспорт"
        );
        let output = add_spaces_before_keywords(input);

        assert_eq!(
            output,
            concat!(
                "Строка = \"первая\n",
                "|Процедура B()Экспорт\n",
                "|\";\n",
                "Процедура A() Экспорт"
            )
        );
    }

    #[test]
    fn canonical_spelling_normalizes_russian_and_english_keywords() {
        let input = concat!(
            "&насерверебезконтекста\n",
            "процедура A() экспорт\n",
            "если Значение = null или Значение = неопределено тогда\n",
            "\tвозврат истина;\n",
            "конецесли;\n",
            "конецпроцедуры"
        );
        let output = fix_non_canonical_spelling(input);

        assert_eq!(
            output,
            concat!(
                "&НаСервереБезКонтекста\n",
                "Процедура A() экспорт\n",
                "Если Значение = NULL ИЛИ Значение = Неопределено Тогда\n",
                "\tВозврат Истина;\n",
                "КонецЕсли;\n",
                "КонецПроцедуры"
            )
        );
    }

    #[test]
    fn canonical_spelling_ignores_strings_comments_and_identifier_parts() {
        let input = concat!(
            "// если тогда null\n",
            "Сообщить(\"если null\");\n",
            "ИмяЕсли = \"A\";\n",
            "Текст = \"первая\n",
            "|возврат null\n",
            "|\";\n",
            "если Истина Тогда"
        );
        let output = fix_non_canonical_spelling(input);

        assert_eq!(
            output,
            concat!(
                "// если тогда null\n",
                "Сообщить(\"если null\");\n",
                "ИмяЕсли = \"A\";\n",
                "Текст = \"первая\n",
                "|возврат null\n",
                "|\";\n",
                "Если Истина Тогда"
            )
        );
    }

    #[test]
    fn canonical_spelling_preserves_reference_accepted_aliases() {
        let input = "Если Флаг Или Не Значение Тогда Возврат ИСТИНА = Null;";
        let output = fix_non_canonical_spelling(input);

        assert_eq!(output, input);
    }

    #[test]
    fn canonical_spelling_covers_reference_keyword_scope() {
        let input = concat!(
            "#если #тогда #иначе #иначеесли #конецесли #область #конецобласти\n",
            "&наклиенте &насервере &насерверебезконтекста ",
            "&наклиентенасерверебезконтекста &наклиентенасервере\n",
            "клиент наклиенте насервере толстыйклиентобычноеприложение ",
            "толстыйклиентуправляемоеприложение сервер внешнеесоединение ",
            "тонкийклиент вебклиент\n",
            "если тогда иначе иначеесли конецесли для каждого цикл конеццикла ",
            "выполнить по прервать продолжить из новый перейти перем пока попытка ",
            "исключение конецпопытки вызватьисключение\n",
            "процедура конецпроцедуры функция конецфункции возврат ",
            "добавитьобработчик удалитьобработчик и или не истина ложь знач ",
            "неопределено null"
        );
        let output = fix_non_canonical_spelling(input);

        assert_eq!(
            output,
            concat!(
                "#Если #Тогда #Иначе #ИначеЕсли #КонецЕсли #Область #КонецОбласти\n",
                "&НаКлиенте &НаСервере &НаСервереБезКонтекста ",
                "&НаКлиентеНаСервереБезКонтекста &НаКлиентеНаСервере\n",
                "Клиент НаКлиенте НаСервере ТолстыйКлиентОбычноеПриложение ",
                "ТолстыйКлиентУправляемоеПриложение Сервер ВнешнееСоединение ",
                "ТонкийКлиент ВебКлиент\n",
                "Если Тогда Иначе ИначеЕсли КонецЕсли Для Каждого Цикл КонецЦикла ",
                "Выполнить По Прервать Продолжить Из Новый Перейти Перем Пока Попытка ",
                "Исключение КонецПопытки ВызватьИсключение\n",
                "Процедура КонецПроцедуры Функция КонецФункции Возврат ",
                "ДобавитьОбработчик УдалитьОбработчик И ИЛИ НЕ Истина Ложь Знач ",
                "Неопределено NULL"
            )
        );
    }
}
