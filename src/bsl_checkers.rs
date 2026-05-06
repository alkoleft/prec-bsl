use std::collections::BTreeMap;
use std::fs;

use tree_sitter::Node;

use crate::bsl_parser::{BslByteSpan, BslParser};
use crate::scenario_pipeline::{ScenarioExecutionContext, ScenarioResult, ScenarioRun, SourceSpan};
use crate::source_files::SourceFileKind;

pub const FORBID_GOTO_RULE: &str = "ЗапретИспользованияПерейти";
pub const DUPLICATE_METHODS_RULE: &str = "ПроверкаДублейПроцедурИФункций";
pub const PREPROCESSOR_RULE: &str = "ПроверкаКорректностиИнструкцийПрепроцессора";

const GOTO_STATEMENT_KIND: &str = "goto_statement";
const GOTO_KEYWORD_KIND: &str = "GOTO_KEYWORD";
const PROCEDURE_DEFINITION_KIND: &str = "procedure_definition";
const FUNCTION_DEFINITION_KIND: &str = "function_definition";
const PREPROCESSOR_KIND: &str = "preprocessor";
const PREPROC_KIND: &str = "preproc";
const ANNOTATION_KIND: &str = "annotation";

pub fn forbid_goto(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
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

    let usages = match find_goto_usages(&input) {
        Ok(usages) => usages,
        Err(error) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                error.to_string(),
            ));
        }
    };

    let results = usages
        .into_iter()
        .map(|usage| {
            ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                "goto statement is forbidden",
            )
            .with_source_span(SourceSpan::new(usage.span.start_byte, usage.span.end_byte))
        })
        .collect();

    ScenarioRun {
        results,
        post_processing_paths: Vec::new(),
    }
}

pub fn duplicate_methods(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
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

    let definitions = match find_duplicate_method_definitions(&input) {
        Ok(definitions) => definitions,
        Err(error) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                error.to_string(),
            ));
        }
    };

    let results = definitions
        .into_iter()
        .map(|definition| {
            ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                format!(
                    "duplicate procedure or function definition: {}",
                    definition.name
                ),
            )
            .with_source_span(SourceSpan::new(
                definition.name_span.start_byte,
                definition.name_span.end_byte,
            ))
        })
        .collect();

    ScenarioRun {
        results,
        post_processing_paths: Vec::new(),
    }
}

pub fn preprocessor_instructions(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
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

    let errors = match find_preprocessor_instruction_errors(&input) {
        Ok(errors) => errors,
        Err(error) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                error.to_string(),
            ));
        }
    };

    let results = errors
        .into_iter()
        .map(|error| {
            ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                error.message,
            )
            .with_source_span(SourceSpan::new(error.span.start_byte, error.span.end_byte))
        })
        .collect();

    ScenarioRun {
        results,
        post_processing_paths: Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GotoUsage {
    pub span: BslByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodDefinition {
    pub name: String,
    pub name_span: BslByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorInstructionError {
    pub span: BslByteSpan,
    pub message: String,
}

pub fn find_goto_usages(input: &str) -> Result<Vec<GotoUsage>, crate::bsl_parser::BslParserError> {
    let mut parser = BslParser::new()?;
    let parsed = parser.parse(input)?;
    let mut usages = Vec::new();
    collect_goto_usages(parsed.tree().root_node(), &mut usages);
    Ok(usages)
}

pub fn find_duplicate_method_definitions(
    input: &str,
) -> Result<Vec<MethodDefinition>, crate::bsl_parser::BslParserError> {
    let mut parser = BslParser::new()?;
    let parsed = parser.parse(input)?;
    let mut definitions = Vec::new();
    collect_method_definitions(parsed.tree().root_node(), input, &mut definitions);

    let mut counts = BTreeMap::<String, usize>::new();
    for definition in &definitions {
        *counts.entry(definition.name.to_lowercase()).or_default() += 1;
    }

    Ok(definitions
        .into_iter()
        .filter(|definition| counts[&definition.name.to_lowercase()] > 1)
        .collect())
}

pub fn find_preprocessor_instruction_errors(
    input: &str,
) -> Result<Vec<PreprocessorInstructionError>, crate::bsl_parser::BslParserError> {
    let mut parser = BslParser::new()?;
    let parsed = parser.parse(input)?;
    let mut errors = Vec::new();
    collect_preprocessor_instruction_errors(parsed.tree().root_node(), input, false, &mut errors);
    errors.extend(find_preprocessor_if_balance_errors(input));
    Ok(errors)
}

fn collect_goto_usages(node: Node<'_>, usages: &mut Vec<GotoUsage>) {
    if node.kind() == GOTO_STATEMENT_KIND {
        usages.push(GotoUsage {
            span: goto_keyword_span(node),
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_goto_usages(child, usages);
    }
}

fn collect_method_definitions(
    node: Node<'_>,
    input: &str,
    definitions: &mut Vec<MethodDefinition>,
) {
    if matches!(
        node.kind(),
        PROCEDURE_DEFINITION_KIND | FUNCTION_DEFINITION_KIND
    ) {
        if let Some(name) = node.child_by_field_name("name") {
            definitions.push(MethodDefinition {
                name: name
                    .utf8_text(input.as_bytes())
                    .expect("tree-sitter name span must point to valid UTF-8 source")
                    .to_owned(),
                name_span: BslByteSpan::new(name.start_byte(), name.end_byte()),
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_method_definitions(child, input, definitions);
    }
}

fn collect_preprocessor_instruction_errors(
    node: Node<'_>,
    input: &str,
    inside_preprocessor: bool,
    errors: &mut Vec<PreprocessorInstructionError>,
) {
    let current_inside_preprocessor = inside_preprocessor || node.kind() == PREPROCESSOR_KIND;
    if is_error_node(node)
        && is_preprocessor_related_error(node, input, current_inside_preprocessor)
    {
        errors.push(PreprocessorInstructionError {
            span: BslByteSpan::new(node.start_byte(), node.end_byte()),
            message: preprocessor_error_message(node),
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.has_error() || child.is_error() || child.is_missing() {
            collect_preprocessor_instruction_errors(
                child,
                input,
                current_inside_preprocessor,
                errors,
            );
        }
    }
}

fn is_error_node(node: Node<'_>) -> bool {
    node.is_error() || node.is_missing()
}

fn is_preprocessor_related_error(node: Node<'_>, input: &str, inside_preprocessor: bool) -> bool {
    inside_preprocessor
        || is_preprocessor_kind(node.kind())
        || node_subtree_contains_preprocessor(node)
        || node_span_starts_with_preprocessor_marker(node, input)
}

fn is_preprocessor_kind(kind: &str) -> bool {
    kind == PREPROCESSOR_KIND
        || kind == PREPROC_KIND
        || kind == ANNOTATION_KIND
        || kind.starts_with("PREPROC_")
}

fn node_subtree_contains_preprocessor(node: Node<'_>) -> bool {
    if is_preprocessor_kind(node.kind()) {
        return true;
    }

    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(node_subtree_contains_preprocessor)
}

fn node_span_starts_with_preprocessor_marker(node: Node<'_>, input: &str) -> bool {
    input
        .get(node.start_byte()..node.end_byte())
        .map(str::trim_start)
        .is_some_and(|text| text.starts_with('#') || text.starts_with('&'))
}

fn preprocessor_error_message(node: Node<'_>) -> String {
    if node.is_missing() {
        format!("invalid preprocessor instruction: missing {}", node.kind())
    } else {
        "invalid preprocessor instruction".to_owned()
    }
}

#[derive(Debug, Clone, Copy)]
struct IfDirective {
    span: BslByteSpan,
    else_seen: bool,
}

fn find_preprocessor_if_balance_errors(input: &str) -> Vec<PreprocessorInstructionError> {
    let mut errors = Vec::new();
    let mut stack = Vec::<IfDirective>::new();

    let mut offset = 0;
    for line in input.split_inclusive('\n') {
        let line_without_ending = line.trim_end_matches(['\r', '\n']);
        if let Some(directive) = preprocessor_directive(line_without_ending, offset) {
            match directive.kind {
                PreprocessorDirectiveKind::If => stack.push(IfDirective {
                    span: directive.span,
                    else_seen: false,
                }),
                PreprocessorDirectiveKind::Elsif => match stack.last() {
                    Some(open_if) if open_if.else_seen => {
                        errors.push(invalid_preprocessor_instruction(directive.span));
                    }
                    Some(_) => {}
                    None => errors.push(invalid_preprocessor_instruction(directive.span)),
                },
                PreprocessorDirectiveKind::Else => match stack.last_mut() {
                    Some(open_if) if open_if.else_seen => {
                        errors.push(invalid_preprocessor_instruction(directive.span));
                    }
                    Some(open_if) => open_if.else_seen = true,
                    None => errors.push(invalid_preprocessor_instruction(directive.span)),
                },
                PreprocessorDirectiveKind::EndIf => {
                    if stack.pop().is_none() {
                        errors.push(invalid_preprocessor_instruction(directive.span));
                    }
                }
            }
        }
        offset += line.len();
    }

    errors.extend(
        stack
            .into_iter()
            .map(|directive| PreprocessorInstructionError {
                span: directive.span,
                message: "invalid preprocessor instruction: missing #КонецЕсли".to_owned(),
            }),
    );
    errors
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreprocessorDirectiveKind {
    If,
    Elsif,
    Else,
    EndIf,
}

#[derive(Debug, Clone, Copy)]
struct PreprocessorDirective {
    kind: PreprocessorDirectiveKind,
    span: BslByteSpan,
}

fn preprocessor_directive(line: &str, line_offset: usize) -> Option<PreprocessorDirective> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") {
        return None;
    }

    let start_byte = line_offset + line.len() - trimmed.len();
    let token = trimmed
        .split_once(char::is_whitespace)
        .map_or(trimmed, |(token, _rest)| token);
    let normalized = token.to_lowercase();
    let kind = match normalized.as_str() {
        "#если" | "#if" => PreprocessorDirectiveKind::If,
        "#иначеесли" | "#elsif" => PreprocessorDirectiveKind::Elsif,
        "#иначе" | "#else" => PreprocessorDirectiveKind::Else,
        "#конецесли" | "#endif" => PreprocessorDirectiveKind::EndIf,
        _ => return None,
    };

    Some(PreprocessorDirective {
        kind,
        span: BslByteSpan::new(start_byte, start_byte + token.len()),
    })
}

fn invalid_preprocessor_instruction(span: BslByteSpan) -> PreprocessorInstructionError {
    PreprocessorInstructionError {
        span,
        message: "invalid preprocessor instruction".to_owned(),
    }
}

fn goto_keyword_span(statement: Node<'_>) -> BslByteSpan {
    let mut cursor = statement.walk();
    statement
        .children(&mut cursor)
        .find(|child| child.kind() == GOTO_KEYWORD_KIND)
        .map(|keyword| BslByteSpan::new(keyword.start_byte(), keyword.end_byte()))
        .unwrap_or_else(|| BslByteSpan::new(statement.start_byte(), statement.end_byte()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goto_usages_include_russian_and_english_keyword_spans() {
        let input = concat!(
            "Процедура Тест()\n",
            "    Перейти ~Метка;\n",
            "    goto ~Other;\n",
            "~Метка:\n",
            "~Other:\n",
            "КонецПроцедуры\n",
        );

        let usages = find_goto_usages(input).unwrap();

        assert_eq!(usages.len(), 2);
        assert_eq!(span_text(input, usages[0].span), "Перейти");
        assert_eq!(span_text(input, usages[1].span), "goto");
    }

    #[test]
    fn goto_usages_ignore_comments_and_string_literals() {
        let input = concat!(
            "Процедура Тест()\n",
            "    // Перейти ~Комментарий;\n",
            "    Сообщить(\"goto ~Строка\");\n",
            "КонецПроцедуры\n",
        );

        let usages = find_goto_usages(input).unwrap();

        assert!(usages.is_empty());
    }

    #[test]
    fn duplicate_method_definitions_include_all_repeated_names_with_name_spans() {
        let input = concat!(
            "Процедура Повтор()\n",
            "КонецПроцедуры\n",
            "\n",
            "Функция Уникальная()\n",
            "КонецФункции\n",
            "\n",
            "Функция повтор()\n",
            "КонецФункции\n",
        );

        let duplicates = find_duplicate_method_definitions(input).unwrap();

        assert_eq!(duplicates.len(), 2);
        assert_eq!(duplicates[0].name, "Повтор");
        assert_eq!(span_text(input, duplicates[0].name_span), "Повтор");
        assert_eq!(duplicates[1].name, "повтор");
        assert_eq!(span_text(input, duplicates[1].name_span), "повтор");
    }

    fn span_text(input: &str, span: BslByteSpan) -> &str {
        &input[span.start_byte..span.end_byte]
    }
}
