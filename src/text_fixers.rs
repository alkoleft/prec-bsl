use std::fs;

use crate::scenario_pipeline::{ScenarioExecutionContext, ScenarioResult, ScenarioRun};
use crate::source_files::SourceFileKind;

pub const TRAILING_WHITESPACE_RULE: &str = "УдалениеЛишнихКонцевыхПробелов";
pub const EXTRA_BLANK_LINES_RULE: &str = "УдалениеЛишнихПустыхСтрок";

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
}
