use std::fs;
use std::path::{Component, Path};

use tree_sitter::Node;

use crate::bsl_parser::{BslByteSpan, BslParser};
use prec_bsl_pipeline::{
    ScenarioDefinition, ScenarioExecutionContext, ScenarioResult, ScenarioRun,
};
use prec_bsl_source::{SourceFile, SourceFileKind};

pub const UNIT_TESTS_PROCESSING_RULE: &str = "ОбработкаЮнитТестов";
pub const UNIT_TESTS_PROCESSING_SCENARIO: ScenarioDefinition = ScenarioDefinition::required_v1(
    UNIT_TESTS_PROCESSING_RULE,
    "ОбработкаЮнитТестов.os",
    unit_tests_processing,
);

const PROCEDURE_DEFINITION_KIND: &str = "procedure_definition";
const FUNCTION_DEFINITION_KIND: &str = "function_definition";

pub fn unit_tests_processing(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    if context.file.kind != SourceFileKind::BslModule {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            "scenario handles only BSL modules",
        ));
    }
    if !is_test_path(context.file) {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            "scenario handles only BSL modules inside tests directory",
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

    let output = match update_unit_test_loader(&input) {
        Ok(Some(output)) => output,
        Ok(None) => return ScenarioRun::clean(),
        Err(error) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                error.to_string(),
            ));
        }
    };

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
        "updated unit test loader method",
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitTestMethod {
    pub name: String,
    pub name_span: BslByteSpan,
}

pub fn update_unit_test_loader(input: &str) -> Result<Option<String>, UnitTestProcessingError> {
    let discovery = discover_unit_test_processing(input)?;
    if discovery.has_parse_errors {
        return Err(UnitTestProcessingError::ParseErrors);
    }
    let methods = discovery.test_methods;
    if methods.is_empty() {
        return Ok(None);
    }

    let loader_region = unit_tests_api_region(
        &methods
            .iter()
            .map(|method| method.name.as_str())
            .collect::<Vec<_>>(),
        line_ending(input),
    );
    let output = replace_or_insert_loader_region(input, &loader_region, discovery.loader_method);
    if output == input {
        Ok(None)
    } else {
        Ok(Some(output))
    }
}

#[derive(Debug)]
pub enum UnitTestProcessingError {
    Parser(crate::bsl_parser::BslParserError),
    ParseErrors,
}

impl std::fmt::Display for UnitTestProcessingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parser(source) => write!(formatter, "{source}"),
            Self::ParseErrors => {
                write!(
                    formatter,
                    "BSL syntax errors prevent unit test loader update"
                )
            }
        }
    }
}

impl std::error::Error for UnitTestProcessingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parser(source) => Some(source),
            Self::ParseErrors => None,
        }
    }
}

impl From<crate::bsl_parser::BslParserError> for UnitTestProcessingError {
    fn from(source: crate::bsl_parser::BslParserError) -> Self {
        Self::Parser(source)
    }
}

pub fn find_unit_test_methods(
    input: &str,
) -> Result<Vec<UnitTestMethod>, crate::bsl_parser::BslParserError> {
    Ok(discover_unit_test_processing(input)?.test_methods)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UnitTestProcessingDiscovery {
    test_methods: Vec<UnitTestMethod>,
    loader_method: Option<BslByteSpan>,
    has_parse_errors: bool,
}

fn discover_unit_test_processing(
    input: &str,
) -> Result<UnitTestProcessingDiscovery, crate::bsl_parser::BslParserError> {
    let mut parser = BslParser::new()?;
    let parsed = parser.parse(input)?;
    let mut discovery = UnitTestProcessingDiscovery {
        test_methods: Vec::new(),
        loader_method: None,
        has_parse_errors: parsed.has_errors(),
    };
    collect_unit_test_processing_discovery(parsed.tree().root_node(), input, &mut discovery);
    Ok(discovery)
}

fn is_test_path(file: &SourceFile) -> bool {
    path_contains_tests_component(&file.repo_path)
        || path_contains_tests_component(&file.source_relative_path)
}

fn path_contains_tests_component(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(value) => value
            .to_str()
            .is_some_and(|text| text.eq_ignore_ascii_case("tests")),
        _ => false,
    })
}

fn collect_unit_test_processing_discovery(
    node: Node<'_>,
    input: &str,
    discovery: &mut UnitTestProcessingDiscovery,
) {
    if matches!(
        node.kind(),
        PROCEDURE_DEFINITION_KIND | FUNCTION_DEFINITION_KIND
    ) && node.child_by_field_name("export").is_some()
        && has_unit_test_marker_on_previous_line(input, node.start_byte())
        && let Some(name) = node.child_by_field_name("name")
    {
        discovery.test_methods.push(UnitTestMethod {
            name: name
                .utf8_text(input.as_bytes())
                .expect("tree-sitter name span must point to valid UTF-8 source")
                .to_owned(),
            name_span: BslByteSpan::new(name.start_byte(), name.end_byte()),
        });
    }

    if matches!(
        node.kind(),
        FUNCTION_DEFINITION_KIND | PROCEDURE_DEFINITION_KIND
    ) && node.child_by_field_name("export").is_some()
        && let Some(name) = node.child_by_field_name("name")
        && name
            .utf8_text(input.as_bytes())
            .expect("tree-sitter name span must point to valid UTF-8 source")
            .to_lowercase()
            == "исполняемыесценарии"
    {
        discovery.loader_method = Some(BslByteSpan::new(node.start_byte(), node.end_byte()));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_unit_test_processing_discovery(child, input, discovery);
    }
}

fn has_unit_test_marker_on_previous_line(input: &str, method_start: usize) -> bool {
    let before_method = &input[..method_start];
    let previous_text = before_method.strip_suffix('\n').unwrap_or(before_method);
    let previous_text = previous_text.strip_suffix('\r').unwrap_or(previous_text);
    let previous_line_end = previous_text.len();
    let previous_line_start = before_method[..previous_line_end]
        .rfind('\n')
        .map_or(0, |position| position + 1);
    let previous_line = before_method[previous_line_start..previous_line_end]
        .trim_end_matches([' ', '\t', '\r'])
        .trim_start();

    previous_line.starts_with("//") && previous_line.to_lowercase().contains("@unit-test:")
}

fn line_ending(input: &str) -> &'static str {
    if input.contains("\r\n") { "\r\n" } else { "\n" }
}

fn unit_tests_api_region(method_names: &[&str], eol: &str) -> String {
    let mut lines = vec![
        "#Область ТестыAPI".to_owned(),
        String::new(),
        "// ИсполняемыеСценарии".to_owned(),
        "// \tСервисный метод для получения списка тестовых методов".to_owned(),
        "// Параметры:".to_owned(),
        "// \tДополнительныеПараметры - Структура - Дополнительные параметры, используемые при формировании списка тестов".to_owned(),
        "// Возвращаемое значение:".to_owned(),
        "// \tМассив - Имена методов тестов".to_owned(),
        "Функция ИсполняемыеСценарии(ДополнительныеПараметры = Неопределено) Экспорт".to_owned(),
        String::new(),
        "\tИсполняемыеСценарии = Новый Массив;".to_owned(),
    ];

    lines.extend(
        method_names
            .iter()
            .map(|name| format!("\tИсполняемыеСценарии.Добавить(\"{name}\");")),
    );
    lines.extend([
        String::new(),
        "\tВозврат ИсполняемыеСценарии;".to_owned(),
        String::new(),
        "КонецФункции".to_owned(),
        String::new(),
        "#КонецОбласти".to_owned(),
    ]);

    lines.join(eol)
}

fn replace_or_insert_loader_region(
    input: &str,
    loader_region: &str,
    loader_method: Option<BslByteSpan>,
) -> String {
    if let Some(loader_method) = loader_method
        && let Some((start, end)) = find_region_bounds_around_span(input, "тестыapi", loader_method)
    {
        return format!("{}{}{}", &input[..start], loader_region, &input[end..]);
    }

    if let Some((start, end)) = find_region_bounds(input, "тестыapi") {
        return format!("{}{}{}", &input[..start], loader_region, &input[end..]);
    }

    if let Some(position) = find_region_start(input, "тесты") {
        return format!(
            "{}{}{}{}{}",
            &input[..position],
            loader_region,
            line_ending(input),
            line_ending(input),
            &input[position..]
        );
    }

    format!(
        "{}{}{}{}",
        loader_region,
        line_ending(input),
        line_ending(input),
        input
    )
}

fn find_region_bounds_around_span(
    input: &str,
    region_name: &str,
    span: BslByteSpan,
) -> Option<(usize, usize)> {
    let mut candidate = None;
    let mut offset = 0;
    for line in input.split_inclusive('\n') {
        if is_region_start_line(line, region_name) {
            candidate = Some(offset + line.len() - line.trim_start().len());
        }
        if offset > span.start_byte {
            break;
        }
        offset += line.len();
    }

    let start = candidate?;
    let bounds = find_region_bounds_from(input, start)?;
    (bounds.0 <= span.start_byte && bounds.1 >= span.end_byte).then_some(bounds)
}

fn find_region_bounds(input: &str, region_name: &str) -> Option<(usize, usize)> {
    let start = find_region_start(input, region_name)?;
    find_region_bounds_from(input, start)
}

fn find_region_bounds_from(input: &str, start: usize) -> Option<(usize, usize)> {
    let mut offset = start;
    let mut depth = 0usize;
    for line in input[start..].split_inclusive('\n') {
        let line_start = offset;
        let trimmed = line.trim();
        offset += line.len();
        if is_any_region_start_line(line) {
            depth += 1;
            continue;
        }
        if is_region_end_line(trimmed) {
            depth = depth.saturating_sub(1);
            let line_body_len = line.trim_end_matches(['\r', '\n']).len();
            if depth == 0 {
                return Some((start, line_start + line_body_len));
            }
        }
    }

    None
}

fn find_region_start(input: &str, region_name: &str) -> Option<usize> {
    let mut offset = 0;
    for line in input.split_inclusive('\n') {
        if is_region_start_line(line, region_name) {
            return Some(offset + line.len() - line.trim_start().len());
        }
        offset += line.len();
    }
    None
}

fn is_region_start_line(line: &str, expected_name: &str) -> bool {
    region_start_line_name(line).is_some_and(|name| name.to_lowercase() == expected_name)
}

fn is_any_region_start_line(line: &str) -> bool {
    region_start_line_name(line).is_some()
}

fn region_start_line_name(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let (token, rest) = trimmed
        .split_once(char::is_whitespace)
        .map_or((trimmed, ""), |(token, rest)| (token, rest));
    if !matches!(token.to_lowercase().as_str(), "#область" | "#region") {
        return None;
    }

    rest.split_whitespace().next()
}

fn is_region_end_line(trimmed_line: &str) -> bool {
    matches!(
        trimmed_line.to_lowercase().as_str(),
        "#конецобласти" | "#endregion"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_exported_unit_test_procedures_and_functions_with_spans() {
        let input = concat!(
            "// @unit-test: smoke\n",
            "Процедура ПервыйТест() Экспорт\n",
            "КонецПроцедуры\n",
            "\n",
            "// @unit-test: function\n",
            "Функция ВторойТест() Экспорт\n",
            "    Возврат Истина;\n",
            "КонецФункции\n",
            "\n",
            "// @unit-test: not exported\n",
            "Процедура Служебный()\n",
            "КонецПроцедуры\n",
        );

        let methods = find_unit_test_methods(input).unwrap();

        assert_eq!(
            methods
                .iter()
                .map(|method| method.name.as_str())
                .collect::<Vec<_>>(),
            vec!["ПервыйТест", "ВторойТест"]
        );
        assert_eq!(span_text(input, methods[0].name_span), "ПервыйТест");
        assert_eq!(span_text(input, methods[1].name_span), "ВторойТест");
    }

    #[test]
    fn updates_loader_region_idempotently() {
        let input = concat!(
            "#Область Тесты\n",
            "// @unit-test: smoke\n",
            "Процедура Проверка() Экспорт\n",
            "КонецПроцедуры\n",
            "#КонецОбласти\n",
        );

        let output = update_unit_test_loader(input).unwrap().unwrap();
        let second = update_unit_test_loader(&output).unwrap();

        assert!(output.contains("#Область ТестыAPI"));
        assert!(output.contains("ИсполняемыеСценарии.Добавить(\"Проверка\");"));
        assert_eq!(second, None);
    }

    fn span_text(input: &str, span: BslByteSpan) -> &str {
        &input[span.start_byte..span.end_byte]
    }
}
