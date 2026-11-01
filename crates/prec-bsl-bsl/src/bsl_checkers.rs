use std::collections::BTreeSet;
use std::fs;

use tree_sitter::Node;

use crate::bsl_parser::{BslByteSpan, BslParser};
use prec_bsl_pipeline::{
    ScenarioDefinition, ScenarioExecutionContext, ScenarioResult, ScenarioRun, SourceSpan,
};
use prec_bsl_source::SourceFileKind;

pub const FORBID_GOTO_RULE: &str = "ЗапретИспользованияПерейти";
pub const DUPLICATE_METHODS_RULE: &str = "ПроверкаДублейПроцедурИФункций";
pub const PREPROCESSOR_RULE: &str = "ПроверкаКорректностиИнструкцийПрепроцессора";
pub const REGIONS_RULE: &str = "ПроверкаКорректностиОбластей";

pub const FORBID_GOTO_SCENARIO: ScenarioDefinition = ScenarioDefinition::required_v1(
    FORBID_GOTO_RULE,
    "ЗапретИспользованияПерейти.os",
    forbid_goto,
);
pub const DUPLICATE_METHODS_SCENARIO: ScenarioDefinition = ScenarioDefinition::required_v1(
    DUPLICATE_METHODS_RULE,
    "ПроверкаДублейПроцедурИФункций.os",
    duplicate_methods,
);
pub const PREPROCESSOR_SCENARIO: ScenarioDefinition = ScenarioDefinition::required_v1(
    PREPROCESSOR_RULE,
    "ПроверкаКорректностиИнструкцийПрепроцессора.os",
    preprocessor_instructions,
);
pub const REGIONS_SCENARIO: ScenarioDefinition =
    ScenarioDefinition::required_v1(REGIONS_RULE, "ПроверкаКорректностиОбластей.os", regions);

const GOTO_STATEMENT_KIND: &str = "goto_statement";
const GOTO_KEYWORD_KIND: &str = "GOTO_KEYWORD";
const PROCEDURE_DEFINITION_KIND: &str = "procedure_definition";
const FUNCTION_DEFINITION_KIND: &str = "function_definition";
const PREPROCESSOR_KIND: &str = "preprocessor";
const PREPROC_KIND: &str = "preproc";
const ANNOTATION_KIND: &str = "annotation";
const PREPROC_REGION_KEYWORD_KIND: &str = "PREPROC_REGION_KEYWORD";
const PREPROC_ENDREGION_KEYWORD_KIND: &str = "PREPROC_ENDREGION_KEYWORD";

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

pub fn regions(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
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

    let errors = match find_region_correctness_errors(&input) {
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
    branch_context: BranchContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorInstructionError {
    pub span: BslByteSpan,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionCorrectnessError {
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

    let contexts = preprocessor_branch_contexts(input);
    for definition in &mut definitions {
        definition.branch_context = contexts.context_at(definition.name_span.start_byte);
    }

    Ok(definitions
        .iter()
        .filter(|definition| can_coexist_with_same_name(definition, &definitions))
        .cloned()
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

pub fn find_region_correctness_errors(
    input: &str,
) -> Result<Vec<RegionCorrectnessError>, crate::bsl_parser::BslParserError> {
    let mut parser = BslParser::new()?;
    let parsed = parser.parse(input)?;
    let mut errors = Vec::new();
    collect_region_parse_errors(parsed.tree().root_node(), input, &mut errors);
    errors.extend(find_region_balance_errors(input));
    deduplicate_region_errors(&mut errors);
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
    ) && let Some(name) = node.child_by_field_name("name")
    {
        definitions.push(MethodDefinition {
            name: name
                .utf8_text(input.as_bytes())
                .expect("tree-sitter name span must point to valid UTF-8 source")
                .to_owned(),
            name_span: BslByteSpan::new(name.start_byte(), name.end_byte()),
            branch_context: BranchContext::default(),
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_method_definitions(child, input, definitions);
    }
}

fn can_coexist_with_same_name(
    definition: &MethodDefinition,
    definitions: &[MethodDefinition],
) -> bool {
    definitions.iter().any(|other| {
        other.name_span != definition.name_span
            && other.name.to_lowercase() == definition.name.to_lowercase()
            && !definition
                .branch_context
                .is_mutually_exclusive_with(&other.branch_context)
    })
}

/// Preprocessor keywords that lead an ERROR region for an unbalanced
/// `#Если`-family directive (tree-sitter-bsl 0.1.7 parses an unterminated
/// `#Если` block or a stray `#Иначе`/`#ИначеЕсли`/`#КонецЕсли` as ONE
/// ERROR region starting at the directive keyword). The lexical balance
/// check (`find_preprocessor_if_balance_errors`) owns these diagnostics —
/// it reports the exact directive span and the precise message — so the
/// tree walk must not add a second, vaguer error for the same directive.
const PREPROC_IF_FAMILY_KEYWORD_KINDS: &[&str] = &[
    "PREPROC_IF_KEYWORD",
    "PREPROC_ELSIF_KEYWORD",
    "PREPROC_ELSE_KEYWORD",
    "PREPROC_ENDIF_KEYWORD",
];

fn collect_preprocessor_instruction_errors(
    node: Node<'_>,
    input: &str,
    inside_preprocessor: bool,
    errors: &mut Vec<PreprocessorInstructionError>,
) {
    let current_inside_preprocessor = inside_preprocessor || node.kind() == PREPROCESSOR_KIND;
    if is_error_node(node)
        && !is_unbalanced_if_family_error_region(node)
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

/// An ERROR region whose first child is an `#Если`-family keyword: an
/// unbalanced or unterminated preprocessor conditional. The balance
/// check reports it exactly once with a directive-level span, so the
/// tree walk skips it (0.1.7 delta: these regions used to have other
/// shapes in 0.1.6 and did not collide with the balance diagnostics).
fn is_unbalanced_if_family_error_region(node: Node<'_>) -> bool {
    if !node.is_error() {
        return false;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .next()
        .is_some_and(|first| PREPROC_IF_FAMILY_KEYWORD_KINDS.contains(&first.kind()))
}

fn collect_region_parse_errors(
    node: Node<'_>,
    input: &str,
    errors: &mut Vec<RegionCorrectnessError>,
) {
    if is_error_node(node)
        && !is_unbalanced_region_error_region(node)
        && is_region_related_error(node, input)
    {
        errors.push(RegionCorrectnessError {
            span: BslByteSpan::new(node.start_byte(), node.end_byte()),
            message: region_error_message(node),
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.has_error() || child.is_error() || child.is_missing() {
            collect_region_parse_errors(child, input, errors);
        }
    }
}

/// An ERROR region whose first child is a `#Область`/`#КонецОбласти`
/// keyword: an unbalanced or malformed region directive. The lexical
/// balance check (`find_region_balance_errors`) owns these diagnostics —
/// directive-level span, precise message — so the tree walk skips it
/// (tree-sitter-bsl 0.1.7 parses such directives as ONE ERROR region
/// where 0.1.6 produced other shapes that did not collide).
fn is_unbalanced_region_error_region(node: Node<'_>) -> bool {
    if !node.is_error() {
        return false;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .next()
        .is_some_and(|first| is_region_keyword_kind(first.kind()))
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

fn is_region_related_error(node: Node<'_>, input: &str) -> bool {
    is_region_keyword_kind(node.kind())
        || node_subtree_contains_region_keyword(node)
        || node_span_starts_with_region_marker(node, input)
}

fn is_region_keyword_kind(kind: &str) -> bool {
    matches!(
        kind,
        PREPROC_REGION_KEYWORD_KIND | PREPROC_ENDREGION_KEYWORD_KIND
    )
}

fn node_subtree_contains_preprocessor(node: Node<'_>) -> bool {
    if is_preprocessor_kind(node.kind()) {
        return true;
    }

    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(node_subtree_contains_preprocessor)
}

fn node_subtree_contains_region_keyword(node: Node<'_>) -> bool {
    if is_region_keyword_kind(node.kind()) {
        return true;
    }

    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(node_subtree_contains_region_keyword)
}

fn node_span_starts_with_preprocessor_marker(node: Node<'_>, input: &str) -> bool {
    input
        .get(node.start_byte()..node.end_byte())
        .map(str::trim_start)
        .is_some_and(|text| text.starts_with('#') || text.starts_with('&'))
}

fn node_span_starts_with_region_marker(node: Node<'_>, input: &str) -> bool {
    input
        .get(node.start_byte()..node.end_byte())
        .map(str::trim_start)
        .is_some_and(|text| {
            let normalized = text.to_lowercase();
            normalized.starts_with("#область")
                || normalized.starts_with("#конецобласти")
                || normalized.starts_with("#region")
                || normalized.starts_with("#endregion")
        })
}

fn preprocessor_error_message(node: Node<'_>) -> String {
    if node.is_missing() {
        format!("invalid preprocessor instruction: missing {}", node.kind())
    } else {
        "invalid preprocessor instruction".to_owned()
    }
}

fn region_error_message(node: Node<'_>) -> String {
    if node.is_missing() {
        format!("invalid region directive: missing {}", node.kind())
    } else {
        "invalid region directive".to_owned()
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct BranchContext {
    choices: Vec<BranchChoice>,
}

impl BranchContext {
    fn is_mutually_exclusive_with(&self, other: &Self) -> bool {
        self.choices.iter().any(|choice| {
            other.choices.iter().any(|other_choice| {
                choice.block_id == other_choice.block_id
                    && choice.branch_id != other_choice.branch_id
            })
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BranchChoice {
    block_id: usize,
    branch_id: usize,
}

#[derive(Debug, Clone, Copy)]
struct BranchFrame {
    block_id: usize,
    branch_id: usize,
    else_seen: bool,
}

#[derive(Debug, Clone)]
struct BranchContextCheckpoint {
    start_byte: usize,
    context: BranchContext,
}

#[derive(Debug, Clone)]
struct BranchContexts {
    checkpoints: Vec<BranchContextCheckpoint>,
}

impl BranchContexts {
    fn context_at(&self, byte_offset: usize) -> BranchContext {
        self.checkpoints
            .iter()
            .take_while(|checkpoint| checkpoint.start_byte <= byte_offset)
            .last()
            .map(|checkpoint| checkpoint.context.clone())
            .unwrap_or_default()
    }
}

fn preprocessor_branch_contexts(input: &str) -> BranchContexts {
    let invalid_blocks = invalid_preprocessor_branch_blocks(input);
    let mut checkpoints = Vec::new();
    let mut stack = Vec::<BranchFrame>::new();
    let mut next_block_id = 0;

    let mut offset = 0;
    for line in input.split_inclusive('\n') {
        checkpoints.push(BranchContextCheckpoint {
            start_byte: offset,
            context: branch_context_from_stack(&stack, &invalid_blocks),
        });

        let line_without_ending = line.trim_end_matches(['\r', '\n']);
        if let Some(directive) = preprocessor_directive(line_without_ending, offset) {
            match directive.kind {
                PreprocessorDirectiveKind::If => {
                    stack.push(BranchFrame {
                        block_id: next_block_id,
                        branch_id: 0,
                        else_seen: false,
                    });
                    next_block_id += 1;
                }
                PreprocessorDirectiveKind::Elsif => {
                    if let Some(open_if) = stack.last_mut() {
                        open_if.branch_id += 1;
                    }
                }
                PreprocessorDirectiveKind::Else => {
                    if let Some(open_if) = stack.last_mut() {
                        open_if.branch_id += 1;
                        open_if.else_seen = true;
                    }
                }
                PreprocessorDirectiveKind::EndIf => {
                    stack.pop();
                }
            }
        }

        offset += line.len();
    }

    checkpoints.push(BranchContextCheckpoint {
        start_byte: offset,
        context: branch_context_from_stack(&stack, &invalid_blocks),
    });

    BranchContexts { checkpoints }
}

fn branch_context_from_stack(
    stack: &[BranchFrame],
    invalid_blocks: &BTreeSet<usize>,
) -> BranchContext {
    BranchContext {
        choices: stack
            .iter()
            .filter(|frame| !invalid_blocks.contains(&frame.block_id))
            .map(|frame| BranchChoice {
                block_id: frame.block_id,
                branch_id: frame.branch_id,
            })
            .collect(),
    }
}

fn invalid_preprocessor_branch_blocks(input: &str) -> BTreeSet<usize> {
    let mut invalid_blocks = BTreeSet::new();
    let mut stack = Vec::<BranchFrame>::new();
    let mut next_block_id = 0;

    let mut offset = 0;
    for line in input.split_inclusive('\n') {
        let line_without_ending = line.trim_end_matches(['\r', '\n']);
        if let Some(directive) = preprocessor_directive(line_without_ending, offset) {
            match directive.kind {
                PreprocessorDirectiveKind::If => {
                    stack.push(BranchFrame {
                        block_id: next_block_id,
                        branch_id: 0,
                        else_seen: false,
                    });
                    next_block_id += 1;
                }
                PreprocessorDirectiveKind::Elsif => match stack.last_mut() {
                    Some(open_if) if open_if.else_seen => {
                        invalid_blocks.insert(open_if.block_id);
                        open_if.branch_id += 1;
                    }
                    Some(open_if) => {
                        open_if.branch_id += 1;
                    }
                    None => {}
                },
                PreprocessorDirectiveKind::Else => match stack.last_mut() {
                    Some(open_if) if open_if.else_seen => {
                        invalid_blocks.insert(open_if.block_id);
                        open_if.branch_id += 1;
                    }
                    Some(open_if) => {
                        open_if.branch_id += 1;
                        open_if.else_seen = true;
                    }
                    None => {}
                },
                PreprocessorDirectiveKind::EndIf => {
                    stack.pop();
                }
            }
        }
        offset += line.len();
    }

    invalid_blocks.extend(stack.into_iter().map(|frame| frame.block_id));
    invalid_blocks
}

#[derive(Debug, Clone, Copy)]
struct RegionDirective {
    span: BslByteSpan,
}

fn find_region_balance_errors(input: &str) -> Vec<RegionCorrectnessError> {
    let mut errors = Vec::new();
    let mut stack = Vec::<RegionDirective>::new();

    let mut offset = 0;
    for line in input.split_inclusive('\n') {
        let line_without_ending = line.trim_end_matches(['\r', '\n']);
        if let Some(directive) = region_directive(line_without_ending, offset) {
            match directive.kind {
                RegionDirectiveKind::Region => {
                    if !directive.has_name {
                        errors.push(RegionCorrectnessError {
                            span: directive.span,
                            message: "invalid region directive: missing identifier".to_owned(),
                        });
                    }
                    stack.push(RegionDirective {
                        span: directive.span,
                    });
                }
                RegionDirectiveKind::EndRegion => {
                    if stack.pop().is_none() {
                        errors.push(invalid_region_directive(directive.span));
                    }
                }
            }
        }
        offset += line.len();
    }

    errors.extend(stack.into_iter().map(|directive| RegionCorrectnessError {
        span: directive.span,
        message: "invalid region directive: missing #КонецОбласти".to_owned(),
    }));
    errors
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegionDirectiveKind {
    Region,
    EndRegion,
}

#[derive(Debug, Clone, Copy)]
struct ParsedRegionDirective {
    kind: RegionDirectiveKind,
    span: BslByteSpan,
    has_name: bool,
}

fn region_directive(line: &str, line_offset: usize) -> Option<ParsedRegionDirective> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") {
        return None;
    }

    let start_byte = line_offset + line.len() - trimmed.len();
    let (token, rest) = trimmed
        .split_once(char::is_whitespace)
        .map_or((trimmed, ""), |(token, rest)| (token, rest));
    let normalized = token.to_lowercase();
    let kind = match normalized.as_str() {
        "#область" | "#region" => RegionDirectiveKind::Region,
        "#конецобласти" | "#endregion" => RegionDirectiveKind::EndRegion,
        _ => return None,
    };

    Some(ParsedRegionDirective {
        kind,
        span: BslByteSpan::new(start_byte, start_byte + token.len()),
        has_name: !rest.trim().is_empty(),
    })
}

fn invalid_region_directive(span: BslByteSpan) -> RegionCorrectnessError {
    RegionCorrectnessError {
        span,
        message: "invalid region directive".to_owned(),
    }
}

fn deduplicate_region_errors(errors: &mut Vec<RegionCorrectnessError>) {
    let mut seen = std::collections::BTreeSet::new();
    errors.retain(|error| {
        seen.insert((
            error.span.start_byte,
            error.span.end_byte,
            error.message.clone(),
        ))
    });
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

    /// tree-sitter-bsl 0.1.7 parses an unterminated `#Если` block as ONE
    /// ERROR region led by PREPROC_IF_KEYWORD (0.1.6 left it in other
    /// shapes). The lexical balance check already reports it precisely
    /// (`missing #КонецЕсли` at the `#Если` span), so the tree walk must
    /// not add a second, vaguer error for the same directive.
    #[test]
    fn preprocessor_instruction_errors_report_an_unterminated_if_block_once() {
        let input = "#Если Сервер Тогда\n    Сообщить(\"server\");\n";

        let errors = find_preprocessor_instruction_errors(input).unwrap();

        assert_eq!(errors.len(), 1);
        assert_eq!(span_text(input, errors[0].span), "#Если");
        assert_eq!(
            errors[0].message,
            "invalid preprocessor instruction: missing #КонецЕсли"
        );
    }

    /// Same 0.1.7 delta for unbalanced branch directives: `#Иначе`/
    /// `#ИначеЕсли`/`#КонецЕсли` in an invalid position become small
    /// ERROR regions led by their keyword. The balance check owns these
    /// diagnostics; the tree walk must stay silent about them.
    #[test]
    fn preprocessor_instruction_errors_report_unbalanced_branch_directives_once() {
        let cases: &[(&str, &str)] = &[
            // duplicate #Иначе inside a valid block
            ("#Если Сервер Тогда\n#Иначе\n#Иначе\n#КонецЕсли\n", "#Иначе"),
            // #ИначеЕсли after #Иначе
            (
                "#Если Сервер Тогда\n#Иначе\n#ИначеЕсли Клиент Тогда\n#КонецЕсли\n",
                "#ИначеЕсли",
            ),
            // unmatched #КонецЕсли
            ("#КонецЕсли\n", "#КонецЕсли"),
            // unmatched #Иначе (no open block)
            ("#Иначе\nСообщить(\"other\");\n", "#Иначе"),
        ];

        for (input, expected_span) in cases {
            let errors = find_preprocessor_instruction_errors(input).unwrap();
            assert_eq!(errors.len(), 1, "input: {input}");
            assert_eq!(span_text(input, errors[0].span), *expected_span);
            assert_eq!(
                errors[0].message, "invalid preprocessor instruction",
                "input: {input}"
            );
        }
    }

    /// Same 0.1.7 delta for regions: an unterminated `#Область` or a
    /// stray `#КонецОбласти` parses as ONE ERROR region led by the
    /// directive keyword (0.1.6 left them in other shapes). The lexical
    /// balance check owns these diagnostics with directive-level spans,
    /// so the tree walk must not add a second error for the same
    /// directive.
    #[test]
    fn region_correctness_errors_report_unbalanced_region_directives_once() {
        let cases: &[(&str, &str, &str)] = &[
            (
                "#Область Служебный\nПроцедура Тест()\nКонецПроцедуры\n",
                "#Область",
                "invalid region directive: missing #КонецОбласти",
            ),
            (
                "#область Служебный\nПроцедура Тест()\nКонецПроцедуры\n",
                "#область",
                "invalid region directive: missing #КонецОбласти",
            ),
            (
                "#Область\n#КонецОбласти\n",
                "#Область",
                "invalid region directive: missing identifier",
            ),
            (
                "#КонецОбласти\nПроцедура Тест()\nКонецПроцедуры\n",
                "#КонецОбласти",
                "invalid region directive",
            ),
        ];

        for (input, expected_span, expected_message) in cases {
            let errors = find_region_correctness_errors(input).unwrap();
            assert_eq!(errors.len(), 1, "input: {input}");
            assert_eq!(span_text(input, errors[0].span), *expected_span);
            assert_eq!(errors[0].message, *expected_message, "input: {input}");
        }
    }

    fn span_text(input: &str, span: BslByteSpan) -> &str {
        &input[span.start_byte..span.end_byte]
    }
}
