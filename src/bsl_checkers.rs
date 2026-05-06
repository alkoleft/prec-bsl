use std::collections::BTreeMap;
use std::fs;

use tree_sitter::Node;

use crate::bsl_parser::{BslByteSpan, BslParser};
use crate::scenario_pipeline::{ScenarioExecutionContext, ScenarioResult, ScenarioRun, SourceSpan};
use crate::source_files::SourceFileKind;

pub const FORBID_GOTO_RULE: &str = "ЗапретИспользованияПерейти";
pub const DUPLICATE_METHODS_RULE: &str = "ПроверкаДублейПроцедурИФункций";

const GOTO_STATEMENT_KIND: &str = "goto_statement";
const GOTO_KEYWORD_KIND: &str = "GOTO_KEYWORD";
const PROCEDURE_DEFINITION_KIND: &str = "procedure_definition";
const FUNCTION_DEFINITION_KIND: &str = "function_definition";

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GotoUsage {
    pub span: BslByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodDefinition {
    pub name: String,
    pub name_span: BslByteSpan,
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
