//! Detection of statements missing a terminating `;`
//! (`ПроверкаТочекЗапятой`, check core).
//!
//! Tree-sitter transliteration of the bsl-ls `SemicolonPresenceDiagnostic`
//! `visitStatement` contract over the tree-sitter-bsl 0.1.7 tree shapes
//! (probe facts in `.superpowers/gh-issue/10/treedumps.md`):
//!
//! - the semicolon set is the OSPL `STATEMENT_KINDS` matrix minus
//!   `var_statement` (procedure-level `Перем` parses as `subVar` — not a
//!   statement — in the bsl-ls ANTLR model, so bsl-ls never flags it;
//!   spec `parser-strategy.md` "Semicolon presence contract") and minus
//!   `preprocessor` (the region node itself is not a statement; its
//!   statement children ARE visited — probe (f)/(t3));
//! - termination predicate: scan the statement node's children in reverse
//!   order, skipping `line_comment` extras; terminated iff the first
//!   non-comment child encountered is the `;` token. For
//!   `while_statement` (the grammar has no `optional(';')` there) the
//!   terminator is the nearest next sibling, skipping comment siblings,
//!   counted in either form: a clean `";"` node or an ERROR node whose
//!   text is exactly `;` (defensive 0.1.6 fallback — not reproducible on
//!   0.1.7 probes, kept from the `one_statement_per_line.rs` precedent);
//! - diagnostic span and line: the **stop token** (last significant
//!   non-comment child), byte span + 1-based line of that token — not the
//!   statement start, so multi-line statements report the line where the
//!   expression ends (bsl-ls `getStop()` range parity);
//! - check-mode parse-error policy: per statement — a statement whose
//!   subtree contains a tree-sitter `ERROR` node is skipped individually
//!   (mirrors bsl-ls `Trees.treeContainsErrors(ctx)`); MISSING nodes are
//!   not blockers (ANTLR recovers missing tokens inline);
//! - fix-mode parse-error policy: whole-file (SKEP-104) — any unsafe
//!   ERROR outside the safe forms (`has_unsafe_parse_errors`, NOT the
//!   OSPL row-intersection semantics: insertion offsets come from the
//!   tree and must not be computed over unreliable regions). On refusal
//!   the input is returned unchanged and the refusal is visible to the
//!   handler through `has_unsafe_parse_errors` / `has_lone_cr` (the
//!   same holds for lone-`\r` files, out of contract);
//! - fix mechanics: insertion-only — one `;` at each
//!   `stop_token_span.end_byte` (before the trailing comment on the
//!   line, probe (h)/(i2)), computed over the original bytes, sorted,
//!   applied in a single copy pass. Idempotent, including the EOF form
//!   (probe (e): the inserted `;` re-parses as the statement's
//!   terminator child); a UTF-8 BOM survives byte-for-byte;
//! - handler mode contract (`semicolon_presence_handler`): lone-`\r`
//!   files are refused with a single Warning in BOTH modes (checked
//!   before the mode split — tree-sitter rows are LF-based); in Apply
//!   the unsafe-ERROR refusal is likewise a single Warning with the
//!   file untouched — the fixer itself cannot distinguish refusal from
//!   clean input, so the handler calls the checks first; DryRun emits
//!   one Warning per violation with the stop-token span and no
//!   `modified_dry_run`.

use std::fs;

use prec_bsl_pipeline::{
    ScenarioDefinition, ScenarioExecutionContext, ScenarioResult, ScenarioRun, SourceSpan,
    WritePolicy,
};
use prec_bsl_source::SourceFileKind;

use crate::bsl_parser::{BslByteSpan, BslParser, BslParserError, ParsedBsl, has_lone_cr};

/// Extension scenario id (`precommit4onec` name without `.os`).
pub const SEMICOLON_PRESENCE_RULE: &str = "ПроверкаТочекЗапятой";

/// Statement kinds whose terminating `;` is checked. The OSPL
/// `STATEMENT_KINDS` matrix (grammar `_statement`, grammar.js:303 in
/// tree-sitter-bsl 0.1.7) minus:
/// - `var_statement` — procedure-level `Перем` is `subVar` in the bsl-ls
///   ANTLR model, not a statement: bsl-ls never flags it, so parity
///   excludes it from this set (deliberate, see treedumps РАСХОЖДЕНИЕ-2;
///   module-level `Перем` is `var_definition` and is not a statement in
///   either model);
/// - `preprocessor` — the region node itself is excluded, but its
///   statement children are visited (probe (f)/(t3): region bodies hold
///   flaggable statement nodes as children of the `preprocessor` node).
const SEMICOLON_STATEMENT_KINDS: &[&str] = &[
    "execute_statement",
    "call_statement",
    "assignment_statement",
    "return_statement",
    "try_statement",
    "rise_error_statement",
    "if_statement",
    "while_statement",
    "for_statement",
    "for_each_statement",
    "continue_statement",
    "break_statement",
    "goto_statement",
    "label_statement",
    "add_handler_statement",
    "remove_handler_statement",
    "await_statement",
];

/// Containers that hold statements but are not statements themselves
/// (the OSPL `CONTAINER_KINDS` matrix — identical for these kinds, see
/// treedumps (j): `elseif_clause`/`else_clause` hold statement children
/// and are never flagged themselves).
const CONTAINER_KINDS: &[&str] = &[
    "source_file",
    "preprocessor",
    "procedure_definition",
    "function_definition",
    "if_statement",
    "elseif_clause",
    "else_clause",
    "while_statement",
    "for_statement",
    "for_each_statement",
    "try_statement",
];

/// One flagged statement: its expression is not terminated by `;`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemicolonViolation {
    /// Byte span of the stop token — the last significant non-comment
    /// child of the statement node. The handler builds the diagnostic
    /// `SourceSpan` from it; the fixer inserts `;` at its `end_byte`;
    /// parity assertions compare the whole range (bsl-ls `getStop()`).
    pub stop_token_span: BslByteSpan,
    /// 1-based line of the stop token. NOT the statement start line: a
    /// multi-line statement reports the line where its expression ends
    /// (bsl-ls diagnostic range parity).
    pub line: usize,
}

/// Find all statements missing a terminating `;` (bsl-ls parity).
pub fn find_missing_semicolons(input: &str) -> Result<Vec<SemicolonViolation>, BslParserError> {
    let mut parser = BslParser::new()?;
    let parsed = parser.parse(input)?;
    let mut violations = Vec::new();
    collect_violations(parsed.tree().root_node(), input, &mut violations);
    Ok(violations)
}

/// True when the file contains an unsafe parse ERROR — any ERROR node
/// outside the known safe forms — anywhere in the file. Fix-mode
/// parse-error policy: whole-file (SKEP-104). Insertion offsets come
/// from the tree and must not be computed over unreliable regions, so
/// ANY unsafe ERROR blocks modification of the file entirely — unlike
/// the `ОдинОператорНаСтроку` `has_relevant_parse_errors`
/// row-intersection semantics. Safe forms (the OSPL precedent,
/// `one_statement_per_line.rs` `is_safe_error_region`):
/// - MISSING nodes (ANTLR recovers missing tokens inline — probe (r9));
/// - a bare `;` ERROR and an ERROR-`;` right after a `while_statement`
///   (defensive 0.1.6 fallbacks — not reproducible on 0.1.7);
/// - a compound keyword-led ERROR region (`Если`/`Пока`/`Попытка`/`Для`
///   without an end keyword — probe (v5)/(v6), bsl-ls fixture t2);
/// - an ERROR region containing preprocessor keywords (preproc inside
///   an expression — bsl-ls fixture main case).
pub fn has_unsafe_parse_errors(input: &str) -> Result<bool, BslParserError> {
    let mut parser = BslParser::new()?;
    let parsed = parser.parse(input)?;
    if !parsed.has_errors() {
        return Ok(false);
    }
    Ok(has_unsafe_error_in_tree(&parsed, input))
}

/// Fix missing-semicolon violations: insertion-only. For every
/// violation exactly one `;` is inserted at `stop_token_span.end_byte`
/// (before the trailing comment on that line, if any — probe (h)/(i2)).
/// Edits are computed over the original bytes, sorted, and applied in a
/// single copy pass; nothing else in the file changes. The fix is
/// idempotent (`fix(fix(x)) == fix(x)`), including the EOF form (probe
/// (e): the inserted `;` re-parses as the statement's own terminator).
/// A UTF-8 BOM survives byte-for-byte.
///
/// Refusal contract (SKEP-103/104): on refusal the input is returned
/// unchanged (`Ok(input.to_owned())`) and the refusal itself is NOT
/// swallowed — the handler must call `has_lone_cr` and
/// `has_unsafe_parse_errors` separately to distinguish "clean" from
/// "refused" outcomes:
/// - lone-`\r` line endings (CR-only or mixed): out of contract;
/// - unsafe parse errors anywhere in the file: whole-file refusal.
pub fn fix_missing_semicolons(input: &str) -> Result<String, BslParserError> {
    // Lone-`\r` line endings are out of contract (`has_lone_cr`):
    // tree-sitter's row model is LF-based and the insertion offsets
    // come from the tree; never touch such files here.
    if has_lone_cr(input) {
        return Ok(input.to_owned());
    }
    if has_unsafe_parse_errors(input)? {
        return Ok(input.to_owned());
    }
    let violations = find_missing_semicolons(input)?;
    if violations.is_empty() {
        return Ok(input.to_owned());
    }
    // Sorted insertion points over the ORIGINAL bytes, one pass.
    let mut insertions: Vec<usize> = violations
        .iter()
        .map(|violation| violation.stop_token_span.end_byte)
        .collect();
    insertions.sort_unstable();
    let mut output = String::with_capacity(input.len() + insertions.len());
    let mut copied = 0usize;
    for at in insertions {
        output.push_str(&input[copied..at]);
        output.push(';');
        copied = at;
    }
    output.push_str(&input[copied..]);
    Ok(output)
}

// --- Handler (plan Task 4): scenario boundary over detector + fixer ---

/// Extension scenario id (`precommit4onec` name without `.os`).
pub const SEMICOLON_PRESENCE_SCENARIO: ScenarioDefinition = ScenarioDefinition::compatibility(
    SEMICOLON_PRESENCE_RULE,
    "ПроверкаТочекЗапятой.os",
    semicolon_presence_handler,
);

/// Handler сценария `ПроверкаТочекЗапятой`: two-mode check + fix (the
/// `line_break_handler` mode contract).
///
/// Контракт режимов:
/// - lone-`\r` файлы (CR-only/mixed) — out of contract в ОБЕИХ режимах:
///   единственный `warning` до развилки режимов, файл нетронут (строки
///   tree-sitter LF-based: check давал бы ложные номера строк — прецедент
///   OSPL-хендлера);
/// - **Apply (fix/hook/exec-rules)**: unsafe parse ERROR — единственный
///   `warning` (fix refused, файл нетронут; refusal наблюдаем, не
///   молчит); иначе fix — modified (`apply_text_modification`, modified
///   path репортится и рестажится) или clean (идемпотентность: повторный
///   прогон → 0 результатов, exit 0). Фиксер не различает отказ от
///   чистого входа сам, поэтому refusal-проверки вызываются здесь ДО
///   fix;
/// - **DryRun (check)**: per-violation `warning` со span stop-токена —
///   БЕЗ `modified_dry_run` (SKEP-109: warnings уже дают check exit 1;
///   прецедент LineLength; diff-превью здесь не нужен — вставка ровно
///   одного `;` на нарушение не требует визуального подтверждения).
///   Parse-error политика check — per-statement skip (внутри детектора):
///   unsafe-файлы дают меньше предупреждений, но не молчат о чистых
///   нарушениях; фикс-отказ в check-режиме не репортится отдельно
///   (check не пишет файл — нечего отказывать).
pub fn semicolon_presence_handler(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
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

    // Lone-`\r` line endings (CR-only or mixed) are out of contract in
    // BOTH modes: tree-sitter rows advance only on `\n`, so check
    // diagnostics would carry false line numbers and the fix would
    // compute offsets over the wrong rows. Warn and leave the file
    // untouched (checked before the mode split, so check mode produces
    // no false diagnostics either — the OSPL handler precedent).
    if has_lone_cr(&input) {
        return ScenarioRun::single(ScenarioResult::warning(
            context.rule_id,
            context.file.repo_path.clone(),
            "файл использует неканонические переводы строк (CR); сценарий ПроверкаТочекЗапятой пропущен",
        ));
    }

    if context.write_policy == WritePolicy::DryRun {
        // DryRun: per-violation warnings со stop-token span. Check-mode
        // parse-error политика — per-statement skip (внутри детектора),
        // поэтому unsafe-файлы здесь просто дают меньше предупреждений;
        // фикс-отказ в check-режиме не репортится отдельно (check не
        // пишет файл — нечего отказывать).
        let violations = match find_missing_semicolons(&input) {
            Ok(violations) => violations,
            Err(error) => {
                return ScenarioRun::single(ScenarioResult::hard_failure(
                    context.rule_id,
                    context.file.repo_path.clone(),
                    error.to_string(),
                ));
            }
        };

        return ScenarioRun {
            results: violations
                .into_iter()
                .map(|violation| {
                    ScenarioResult::warning(
                        context.rule_id,
                        context.file.repo_path.clone(),
                        format!("missing semicolon at line {}", violation.line),
                    )
                    .with_source_span(SourceSpan::new(
                        violation.stop_token_span.start_byte,
                        violation.stop_token_span.end_byte,
                    ))
                })
                .collect(),
            post_processing_paths: Vec::new(),
        };
    }

    // Apply: unsafe-ERROR refusal проверяется ДО fix — фиксер возвращает
    // Ok(input) и на отказе, и на чистом входе (refusal-контракт
    // SKEP-103/104), различить их может только handler.
    match has_unsafe_parse_errors(&input) {
        Ok(false) => {}
        Ok(true) => {
            return ScenarioRun::single(ScenarioResult::warning(
                context.rule_id,
                context.file.repo_path.clone(),
                "file has unsafe parse errors; semicolon fix is refused for the whole file",
            ));
        }
        Err(error) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                error.to_string(),
            ));
        }
    }

    let output = match fix_missing_semicolons(&input) {
        Ok(output) => output,
        Err(error) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                error.to_string(),
            ));
        }
    };
    if output == input {
        return ScenarioRun::clean();
    }

    crate::text_fixers::apply_text_modification(
        context,
        &path,
        output,
        "inserted missing semicolons",
    )
}

/// Whole-file unsafe-ERROR scan: any ERROR node that is not a safe form.
fn has_unsafe_error_in_tree(parsed: &ParsedBsl, source: &str) -> bool {
    let root = parsed.tree().root_node();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_error() && !is_safe_error_form(node, source) {
            return true;
        }
        // MISSING nodes are always safe (ANTLR inline recovery).
        if !node.is_missing() {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.has_error() || child.is_error() || child.is_missing() {
                    stack.push(child);
                }
            }
        }
    }
    false
}

/// Compound keywords that lead an ERROR region wrapping an unterminated
/// compound statement (`Если`/`Пока`/`Попытка`/`Для` — the OSPL
/// `COMPOUND_KEYWORD_KINDS` matrix, identical for these kinds).
const COMPOUND_KEYWORD_KINDS: &[&str] =
    &["IF_KEYWORD", "WHILE_KEYWORD", "TRY_KEYWORD", "FOR_KEYWORD"];

/// Safe ERROR forms for the whole-file fix policy (see
/// `has_unsafe_parse_errors`).
fn is_safe_error_form(node: tree_sitter::Node<'_>, source: &str) -> bool {
    if node.is_missing() {
        return true;
    }
    // Bare `;` ERROR / ERROR-`;` after a while (defensive 0.1.6
    // fallbacks, not reproducible on 0.1.7 — treedumps РАСХОЖДЕНИЕ-1).
    if node
        .utf8_text(source.as_bytes())
        .is_ok_and(|text| text == ";")
    {
        return true;
    }
    // Compound keyword-led ERROR region (unterminated compound
    // recovered inline by ANTLR).
    if is_compound_error_region(node) {
        return true;
    }
    // Preprocessor-containing ERROR region (e.g. `#Если` inside a call).
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind().starts_with("PREPROC_") {
            return true;
        }
    }
    false
}

fn is_compound_error_region(node: tree_sitter::Node<'_>) -> bool {
    let mut cursor = node.walk();
    let mut children = node.children(&mut cursor);
    children
        .next()
        .is_some_and(|first| COMPOUND_KEYWORD_KINDS.contains(&first.kind()))
}

/// Visit statement nodes in document order, descending into containers
/// (including `preprocessor` region bodies) and ERROR regions that still
/// hold valid statement nodes inside.
fn collect_violations(
    node: tree_sitter::Node<'_>,
    source: &str,
    violations: &mut Vec<SemicolonViolation>,
) {
    if is_semicolon_statement_kind(node.kind()) {
        if node.has_error() {
            // Check-mode parse-error policy: per statement. A statement
            // whose subtree contains an ERROR node is skipped
            // individually (MISSING nodes set `has_error()` too, but the
            // probe facts keep them non-blocking — see
            // `statement_contains_error`).
            if statement_contains_error(node) {
                // still descend: inner statements are evaluated on their own
            } else if let Some(violation) = unterminated_violation(node, source) {
                violations.push(violation);
            }
        } else if let Some(violation) = unterminated_violation(node, source) {
            violations.push(violation);
        }
    } else if !node.is_error() && !is_container_kind(node.kind()) {
        // line_comment, expressions, definition brackets etc. — no
        // statements inside. ERROR regions fall through: their inner
        // statements stay visible (probe (v5)/(v6)).
        return;
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_violations(cursor.node(), source, violations);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// True when the statement subtree contains a tree-sitter ERROR node
/// (NOT a MISSING node — ANTLR recovers missing tokens inline, and the
/// probe facts (r9)/(v2) keep MISSING-only statements flaggable).
fn statement_contains_error(node: tree_sitter::Node<'_>) -> bool {
    if node.is_error() {
        return true;
    }
    if node.is_missing() {
        return false;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if statement_contains_error(child) {
            return true;
        }
    }
    false
}

/// The termination predicate. Terminated iff the first non-comment child
/// encountered in the reverse scan is the `;` token; for
/// `while_statement` the terminator is the nearest next sibling (the
/// grammar has no `optional(';')` there), skipping comment siblings.
/// Returns the stop-token violation for an unterminated statement.
fn unterminated_violation(node: tree_sitter::Node<'_>, source: &str) -> Option<SemicolonViolation> {
    let is_while = node.kind() == "while_statement";
    let mut cursor = node.walk();
    let children: Vec<tree_sitter::Node<'_>> = node.children(&mut cursor).collect();
    // Reverse scan over significant (non-comment, non-phantom) children.
    let mut last_significant: Option<tree_sitter::Node<'_>> = None;
    for child in children.iter().rev() {
        if child.kind() == "line_comment" || child.is_missing() {
            continue;
        }
        if last_significant.is_none() {
            // first non-comment child from the end: the terminator slot
            if is_terminator_semicolon(*child, source) && !is_while {
                return None;
            }
        }
        last_significant = Some(*child);
        if is_terminator_semicolon(*child, source) && !is_while {
            // a `;` that is not in the terminator slot (mid-statement,
            // probe (r4)) does not terminate the statement
            continue;
        }
        break;
    }
    let last = last_significant?;
    if is_while {
        // while terminator: nearest next sibling after the node
        let mut sibling = node.next_sibling();
        while let Some(next) = sibling {
            if next.kind() == "line_comment" {
                sibling = next.next_sibling();
                continue;
            }
            if is_terminator_semicolon(next, source) {
                return None;
            }
            break;
        }
    }
    Some(SemicolonViolation {
        stop_token_span: BslByteSpan::new(last.start_byte(), last.end_byte()),
        line: last.end_position().row + 1,
    })
}

/// A `;` token in either form: a clean `";"` node or an ERROR node whose
/// text is exactly `;` (defensive 0.1.6 fallback, not reproducible on
/// 0.1.7 probes — see treedumps РАСХОЖДЕНИЕ-1).
fn is_terminator_semicolon(node: tree_sitter::Node<'_>, source: &str) -> bool {
    if !node.is_error() {
        return node.kind() == ";";
    }
    node.utf8_text(source.as_bytes())
        .is_ok_and(|text| text == ";")
}

fn is_semicolon_statement_kind(kind: &str) -> bool {
    SEMICOLON_STATEMENT_KINDS.contains(&kind)
}

fn is_container_kind(kind: &str) -> bool {
    CONTAINER_KINDS.contains(&kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Byte span of `needle` in `input` (first occurrence).
    fn span_of(input: &str, needle: &str) -> BslByteSpan {
        let start = input.find(needle).unwrap_or_else(|| {
            panic!("needle {needle:?} not found in input");
        });
        BslByteSpan::new(start, start + needle.len())
    }

    fn violation(input: &str, needle: &str, line: usize) -> SemicolonViolation {
        SemicolonViolation {
            stop_token_span: span_of(input, needle),
            line,
        }
    }

    // --- EOF form (issue main case, probe (d)/(e)) ---

    #[test]
    fn eof_return_without_semicolon_is_flagged() {
        let input = "\tВозврат \"Булево\"";
        let violations = find_missing_semicolons(input).unwrap();

        assert_eq!(violations, vec![violation(input, "\"Булево\"", 1)]);
    }

    #[test]
    fn eof_return_fixed_output_is_not_flagged() {
        // Probe (e): the fix-inserted EOF `;` parses as a CHILD of the
        // return_statement (unnamed `;` token, last child) — the
        // predicate sees it as the terminator, so the fix is idempotent.
        let input = "\tВозврат \"Булево\";";
        assert!(find_missing_semicolons(input).unwrap().is_empty());
    }

    // --- while sibling-terminator forms (probe (c1)/(c2)) ---

    #[test]
    fn while_with_semicolon_sibling_node_is_not_flagged() {
        // 0.1.7: while_statement has no optional(';'), so its terminator
        // appears as a SIBLING `;` node. Clean-node form (the only form
        // reproducible on 0.1.7 probes).
        let input = "Пока Истина Цикл\n    А = 1;\nКонецЦикла;\n";
        let violations = find_missing_semicolons(input).unwrap();

        assert!(violations.is_empty());
    }

    /// Documenting test (treedumps РАСХОЖДЕНИЕ-1): the ERROR-`;` branch
    /// of the while predicate is a defensive 0.1.6 fallback — no input
    /// producing an ERROR node with text `;` after `КонецЦикла` was
    /// found on 0.1.7. The test pins the DECISION (keep the branch) and
    /// asserts the reproducible clean-sibling form is recognized.
    #[test]
    fn while_error_semi_branch_is_defensive_only() {
        // The clean sibling `;` (the only 0.1.7-reproducible form) must
        // be recognized as the while terminator, in both file forms:
        // with trailing newline...
        let input = "Пока Истина Цикл\n    А = 1;\nКонецЦикла;\n";
        assert!(find_missing_semicolons(input).unwrap().is_empty());
        // ...and at EOF without a trailing newline.
        let eof_form = "Пока Истина Цикл\n    А = 1;\nКонецЦикла;";
        assert!(find_missing_semicolons(eof_form).unwrap().is_empty());
    }

    #[test]
    fn while_without_semicolon_is_flagged() {
        let input = "Пока Истина Цикл\n    А = 1;\nКонецЦикла\n";
        let violations = find_missing_semicolons(input).unwrap();

        assert_eq!(violations, vec![violation(input, "КонецЦикла", 3)]);
    }

    // --- compound ends (bsl-ls parity) ---

    #[test]
    fn compound_end_without_semicolon_is_flagged() {
        // if/try/for/for_each carry their own trailing optional(';')
        // INSIDE the node; `КонецЕсли` without `;` is flagged (the
        // official bsl-ls fixture flags exactly this).
        let input = "Если Истина Тогда\n    А = 1;\nКонецЕсли\n";
        let violations = find_missing_semicolons(input).unwrap();

        assert_eq!(violations, vec![violation(input, "КонецЕсли", 3)]);
    }

    #[test]
    fn compound_end_with_semicolon_is_not_flagged() {
        let input = "Если Истина Тогда\n    А = 1;\nКонецЕсли;\n";
        assert!(find_missing_semicolons(input).unwrap().is_empty());
    }

    // --- trailing comment before the insertion point (probe (h)) ---

    #[test]
    fn trailing_comment_precedes_insertion_point() {
        // Probe (h): the comment is a SIBLING of the statement, and the
        // statement span ENDS before the comment starts. The stop token
        // ends before the trailing comment begins — the fix (Task 3)
        // inserts `;` between them.
        let input = "Если Истина Тогда\n    А = 1;\nКонецЕсли // c\n";
        let violations = find_missing_semicolons(input).unwrap();

        assert_eq!(violations.len(), 1);
        let stop = violations[0].stop_token_span;
        assert_eq!(stop, span_of(input, "КонецЕсли"));
        // stop token ends strictly before the comment starts
        let comment_start = input.find("// c").unwrap();
        assert!(stop.end_byte < comment_start);
        assert_eq!(violations[0].line, 3);
    }

    // --- string literals containing `;` (probe (n1)/(n2)) ---

    #[test]
    fn string_literal_with_semicolon_is_flagged() {
        // Without `;` the last child is the expression (string) —
        // flagged; with `;` the last child is the `;` token — not
        // flagged. A `;` inside a string literal never terminates.
        let input = "А = \"1;2\"";
        let violations = find_missing_semicolons(input).unwrap();

        assert_eq!(violations, vec![violation(input, "\"1;2\"", 1)]);

        let terminated = "А = \"1;2\";";
        assert!(find_missing_semicolons(terminated).unwrap().is_empty());
    }

    // --- multi-line statement: stop-token line (probe (i)) ---

    #[test]
    fn multiline_statement_reports_stop_token_line() {
        // Probe (i): return_statement starts on row 1, its expression
        // ends on row 2 — the violation line is the stop-token line (3,
        // 1-based), not the statement start line (2).
        let input = "Функция Ф()\n    Возврат Ф(А,\n        Б)\nКонецФункции";
        let violations = find_missing_semicolons(input).unwrap();

        assert_eq!(
            violations,
            vec![SemicolonViolation {
                stop_token_span: span_of(input, "Ф(А,\n        Б)"),
                line: 3,
            }]
        );
    }

    // --- check-mode parse-error policy: per-statement skip ---

    #[test]
    fn skips_statements_with_parse_errors() {
        // `Сообщить(1 2)` parses as a call_statement with an ERROR node
        // INSIDE its arguments (missing comma): the statement is skipped
        // individually (bsl-ls `Trees.treeContainsErrors(ctx)` parity).
        // The clean `А = 1` after it is still flagged.
        let input = "Сообщить(1 2)\nА = 1";
        let violations = find_missing_semicolons(input).unwrap();

        let at = input.find("А = 1").unwrap() + "А = ".len();
        assert_eq!(
            violations,
            vec![SemicolonViolation {
                stop_token_span: BslByteSpan::new(at, at + "1".len()),
                line: 2,
            }]
        );
    }

    #[test]
    fn missing_nodes_do_not_skip_a_statement() {
        // Probe (r9): `~Метка` without `:` parses as label_statement
        // with a MISSING `:` node. MISSING nodes are not blockers
        // (ANTLR recovers missing tokens inline): the statement is
        // still flagged without `;`.
        let input = "Процедура П()\n    ~Метка\nКонецПроцедуры";
        let violations = find_missing_semicolons(input).unwrap();

        assert_eq!(violations, vec![violation(input, "Метка", 2)]);
    }

    // --- preprocessor / region keywords ---

    #[test]
    fn preprocessor_and_region_keywords_not_flagged() {
        // The preprocessor region node itself is never flagged, but its
        // statement children ARE visited (probe (f)/(t3)): `А = 1`
        // inside the region is flagged, `#Область`/`#КонецОбласти` and
        // the region node are not.
        let input = "#Область Имя\nА = 1\n#КонецОбласти\n";
        let violations = find_missing_semicolons(input).unwrap();

        let at = input.find("А = 1").unwrap() + "А = ".len();
        assert_eq!(
            violations,
            vec![SemicolonViolation {
                stop_token_span: BslByteSpan::new(at, at + "1".len()),
                line: 2,
            }]
        );
    }

    #[test]
    fn preproc_if_region_body_is_flagged() {
        // Probe (t3): a `#Если` region parses as ONE preprocessor node
        // with the body statements as its children.
        let input = "#Если Сервер Тогда\n    X = 1\n#КонецЕсли\n";
        let violations = find_missing_semicolons(input).unwrap();

        let at = input.find("X = 1").unwrap() + "X = ".len();
        assert_eq!(
            violations,
            vec![SemicolonViolation {
                stop_token_span: BslByteSpan::new(at, at + "1".len()),
                line: 2,
            }]
        );

        // with `;` present the body is clean
        let terminated = "#Если Сервер Тогда\n    X = 1;\n#КонецЕсли\n";
        assert!(find_missing_semicolons(terminated).unwrap().is_empty());
    }

    // --- var forms (probe (g)/(m); РАСХОЖДЕНИЕ-2 decision) ---

    #[test]
    fn module_level_var_behavior_follows_probe() {
        // Probe (g): module-level `Перем X` parses as `var_definition`
        // — not a statement in either model (bsl-ls `moduleVar` is not
        // a statement) — never flagged, with or without `;`.
        assert!(find_missing_semicolons("Перем X\n").unwrap().is_empty());
        assert!(find_missing_semicolons("Перем X;\n").unwrap().is_empty());

        // Probe (m): procedure-level `Перем X` parses as
        // `var_statement` (a statement in the tree-sitter model), but
        // bsl-ls parses it as `subVar` — NOT a statement — and never
        // flags it. Parity excludes `var_statement` from the semicolon
        // set (orchestrator decision on РАСХОЖДЕНИЕ-2): procedure-level
        // `Перем` is not checked, with or without `;`.
        let proc = "Процедура П()\n    Перем X\nКонецПроцедуры";
        assert!(find_missing_semicolons(proc).unwrap().is_empty());
        let terminated = "Процедура П()\n    Перем X;\nКонецПроцедуры";
        assert!(find_missing_semicolons(terminated).unwrap().is_empty());
    }

    // --- verbatim bsl-ls fixture parity (AC-3) ---

    /// Verbatim content of bsl-ls v1.0.7 fixture
    /// `src/test/resources/diagnostics/SemicolonPresenceDiagnostic.bsl`.
    /// The bsl-ls test asserts exactly 2 diagnostics with stop-token
    /// ranges (3,6,3,7) — the `0` of `А = 0` on row 3 — and (4,0,4,9) —
    /// the `КонецЕсли` keyword on row 4 (LSP 0-based lines/columns).
    const BSL_LS_FIXTURE: &str = "А = 0;\nЕсли Истина Тогда\n  А = 0;\n  А = 0           // Диагностика должна сработать здесь\nКонецЕсли         // и здесь\n\n#Область ИмяОбласти\n\n#КонецОбласти\n\nАсинх Процедура а()\n    Существует = Ждать ФайлНаДиске.СуществуетАсинх();\nКонецПроцедуры\n\nПроцедура ОшибкаРазбора()\n    Для ЭлементСтруктуры Из КакаятоСтруктура Цикл // Здесь ошибки не будет, т.к. ошибка разбора\n\n    КонецЦикла;  // Здесь ошибки не будет, т.к. ошибка разбора\nКонецПроцедуры\n";

    /// Reference matrix for the fixer tests (Task 3): every mutation
    /// input the fixer must handle, used by both the targeted tests and
    /// the idempotence loop. Mirrors the Task 2 detector cases plus the
    /// BOM/CRLF/EOF matrix required by the plan.
    fn fixer_mutation_inputs() -> Vec<String> {
        vec![
            // EOF form (issue main case, probe (d)/(e))
            "\tВозврат \"Булево\"".to_owned(),
            // while without `;` (sibling terminator missing)
            "Пока Истина Цикл\n    А = 1;\nКонецЦикла\n".to_owned(),
            // compound end without `;`
            "Если Истина Тогда\n    А = 1;\nКонецЕсли\n".to_owned(),
            // trailing comment before the insertion point (probe (h))
            "Если Истина Тогда\n    А = 1;\nКонецЕсли // c\n".to_owned(),
            // string literal containing `;` (probe (n1))
            "А = \"1;2\"".to_owned(),
            // multi-line statement: stop token on another row (probe (i))
            "Функция Ф()\n    Возврат Ф(А,\n        Б)\nКонецФункции".to_owned(),
            // two statements on one line, first unterminated (probe (b))
            "А = 0 Б = 1;\n".to_owned(),
            // preprocessor region body (probe (f)/(t3))
            "#Если Сервер Тогда\n    X = 1\n#КонецЕсли\n".to_owned(),
            // MISSING node does not block the statement (probe (r9)):
            // the label case is in the matrix and runs in
            // `fix_is_idempotent` (idempotence holds there as a no-op
            // refusal), but it is excluded from
            // `fix_clears_all_violations_from_the_detector` because the
            // fixed output (`~Метка;`) re-parses as an ERROR node.
            "Процедура П()\n    ~Метка\nКонецПроцедуры".to_owned(),
            // two missing semicolons in one file (insertion count)
            "Если Истина Тогда\n    А = 1\nКонецЕсли\nБ = 2\n".to_owned(),
            // CRLF line endings
            "Если Истина Тогда\r\n    А = 1;\r\nКонецЕсли\r\n".to_owned(),
            // BOM + CRLF + EOF form
            "\u{feff}\r\n\tВозврат \"Булево\"".to_owned(),
            // BOM + LF + trailing comment
            "\u{feff}Если Истина Тогда\n    А = 1;\nКонецЕсли // c\n".to_owned(),
        ]
    }

    /// Clean inputs (no violations): the fix must return them
    /// byte-for-byte unchanged.
    fn fixer_clean_inputs() -> Vec<String> {
        vec![
            "\tВозврат \"Булево\";".to_owned(),
            "Пока Истина Цикл\n    А = 1;\nКонецЦикла;\n".to_owned(),
            "Если Истина Тогда\n    А = 1;\nКонецЕсли;\n".to_owned(),
            "А = \"1;2\";".to_owned(),
            "А = 0;;".to_owned(),
            "Перем X\n".to_owned(),
            "Процедура П()\n    Перем X\nКонецПроцедуры".to_owned(),
            "// Если Тогда".to_owned(),
            "А = 1; Б = 2; )".to_owned(),
            "\u{feff}А = 0;\n".to_owned(),
            // `А = 0\n;` — the `;` on the next row is still a child of
            // the statement (probe (q1)): terminated, no fix.
            "А = 0\n;\n".to_owned(),
            // `КонецЕсли // c\n;` — comment child between ENDIF and `;`
            // (probe (u6)): terminated via comment-skipping scan.
            "Если Истина Тогда\n    А = 1;\nКонецЕсли // c\n;\n".to_owned(),
        ]
    }

    #[test]
    fn fix_inserts_semicolon_at_stop_token_before_comment() {
        // The insertion point is stop_token_span.end_byte: BEFORE the
        // trailing comment on the statement's line (probe (h)).
        let input = "Если Истина Тогда\n    А = 1;\nКонецЕсли // c\n";
        let output = fix_missing_semicolons(input).unwrap();
        assert_eq!(output, "Если Истина Тогда\n    А = 1;\nКонецЕсли; // c\n");
    }

    #[test]
    fn fix_inserts_at_every_violation_and_nowhere_else() {
        // Two violations in one file: exactly two `;` inserted, at the
        // two stop-token end bytes, nothing else changes.
        let input = "Если Истина Тогда\n    А = 1\nКонецЕсли\nБ = 2\n";
        let output = fix_missing_semicolons(input).unwrap();
        assert_eq!(
            output,
            "Если Истина Тогда\n    А = 1;\nКонецЕсли;\nБ = 2;\n"
        );
    }

    #[test]
    fn fix_is_idempotent() {
        // fix(fix(x)) == fix(x) on the whole matrix (Task 2 cases +
        // BOM + CRLF + EOF form — the EOF form is the mandatory matrix
        // element: after insertion the predicate must see the inserted
        // `;` as the terminator, probe (e)).
        for input in fixer_mutation_inputs() {
            let once = fix_missing_semicolons(&input).unwrap();
            assert!(
                once != input || find_missing_semicolons(&input).unwrap().is_empty(),
                "input changed without violations: {input:?}"
            );
            let twice = fix_missing_semicolons(&once).unwrap();
            assert_eq!(once, twice, "not idempotent for {input:?}");
        }
    }

    #[test]
    fn fix_clears_all_violations_from_the_detector() {
        // After the fix the detector must find no violations on the
        // fixed output (insertion-only: the fixed text re-parses with
        // every inserted `;` recognized as its statement's terminator).
        // The label-MISSING case is excluded here: its fix output is
        // ERROR-shaped (`~Метка;` parses as an ERROR node), and the fix
        // correctly refuses that output on the second run (unsafe
        // ERROR) — covered by `fix_label_missing_colon_output_refused`.
        for input in fixer_mutation_inputs()
            .into_iter()
            .filter(|input| !input.contains("~Метка"))
        {
            let once = fix_missing_semicolons(&input).unwrap();
            let remaining = find_missing_semicolons(&once).unwrap();
            assert!(
                remaining.is_empty(),
                "violations remain after fix for {input:?}:\n{remaining:?}"
            );
        }
    }

    #[test]
    fn fix_label_missing_colon_output_refused() {
        // Probe (r9) follow-up: `~Метка` without `:` parses as
        // label_statement with a MISSING `:`; inserting `;` after
        // `Метка` yields `~Метка;` which does NOT parse as
        // label_statement — the region becomes a plain ERROR node. The
        // second fix run must refuse that output (unsafe ERROR), not
        // insert again: the fix stays idempotent as a NO-OP refusal.
        let input = "Процедура П()\n    ~Метка\nКонецПроцедуры";
        let once = fix_missing_semicolons(input).unwrap();
        assert_eq!(once, "Процедура П()\n    ~Метка;\nКонецПроцедуры");
        assert!(has_unsafe_parse_errors(&once).unwrap());
        let twice = fix_missing_semicolons(&once).unwrap();
        assert_eq!(once, twice, "second run must not touch the refused file");
    }

    #[test]
    fn fix_leaves_clean_inputs_unchanged_byte_for_byte() {
        for input in fixer_clean_inputs() {
            let output = fix_missing_semicolons(&input).unwrap();
            assert_eq!(output, input, "clean input changed: {input:?}");
        }
    }

    #[test]
    fn fix_refuses_files_with_unsafe_parse_errors() {
        // `Сообщить(1 2)`: the call_statement holds an ERROR node in
        // its arguments (missing comma). Check mode skips the statement
        // individually; fix mode refuses the whole file — the returned
        // text is byte-for-byte the input, and the refusal is visible
        // to the handler through `has_unsafe_parse_errors`.
        let input = "Сообщить(1 2)\nА = 1";
        assert!(has_unsafe_parse_errors(input).unwrap());
        let output = fix_missing_semicolons(input).unwrap();
        assert_eq!(output, input, "unsafe file must stay unchanged");

        // Unterminated procedure (ERROR region led by PROCEDURE_KEYWORD,
        // not a safe form): refused as well.
        let proc = "Процедура А()\nА=1; А=2;";
        assert!(has_unsafe_parse_errors(proc).unwrap());
        assert_eq!(fix_missing_semicolons(proc).unwrap(), proc);

        // Clean file: no unsafe errors.
        assert!(!has_unsafe_parse_errors("А = 1;\nБ = 2;\n").unwrap());
    }

    #[test]
    fn fix_refuses_but_safe_error_forms_do_not_block() {
        // Safe ERROR forms (the OSPL precedent): MISSING nodes, bare-`;`
        // ERRORs (not reproducible on 0.1.7 — defensive), keyword-led
        // compound ERROR regions, and preproc-containing ERROR regions.
        //
        // The bsl-ls fixture's `ОшибкаРазбора` procedure: `Для … Цикл`
        // with an empty body parses as a FOR_KEYWORD-led ERROR region
        // (safe form) — the file still gets fixed (its `А = 0` and
        // `КонецЕсли` violations are inserted), and the fix output
        // stays idempotent.
        let input = "А = 0;\nЕсли Истина Тогда\n  А = 0;\n  А = 0           // Диагностика должна сработать здесь\nКонецЕсли         // и здесь\n\n#Область ИмяОбласти\n\n#КонецОбласти\n\nАсинх Процедура а()\n    Существует = Ждать ФайлНаДиске.СуществуетАсинх();\nКонецПроцедуры\n\nПроцедура ОшибкаРазбора()\n    Для ЭлементСтруктуры Из КакаятоСтруктура Цикл // Здесь ошибки не будет, т.к. ошибка разбора\n\n    КонецЦикла;  // Здесь ошибки не будет, т.к. ошибка разбора\nКонецПроцедуры\n";
        assert!(!has_unsafe_parse_errors(input).unwrap());
        let once = fix_missing_semicolons(input).unwrap();
        assert_ne!(once, input);
        let twice = fix_missing_semicolons(&once).unwrap();
        assert_eq!(once, twice, "second run must be a no-op");

        // MISSING nodes alone (probe (r9), `~Метка` without `:`): safe
        // for the fix — the file is not refused before the first run.
        let missing = "Процедура П()\n    ~Метка\nКонецПроцедуры";
        assert!(!has_unsafe_parse_errors(missing).unwrap());

        // Unterminated compound at EOF (probe (v5)): IF_KEYWORD-led
        // ERROR region — safe form, fix not refused.
        let unterminated_if = "Если Истина Тогда А = 1;";
        assert!(!has_unsafe_parse_errors(unterminated_if).unwrap());
    }

    #[test]
    fn fix_leaves_lone_cr_files_unchanged() {
        // Lone `\r` line endings (CR-only or mixed) are out of contract
        // (see `bsl_parser::has_lone_cr`): the fix returns the input
        // unchanged — the handler distinguishes the refusal by calling
        // `has_lone_cr` itself.
        let cr_only = "А = 1\rБ = 2\r";
        let output = fix_missing_semicolons(cr_only).unwrap();
        assert_eq!(output, cr_only, "CR-only input must stay unchanged");

        let mixed = "А = 1;\r\nБ = 2;\rВ = 3;\n";
        let output = fix_missing_semicolons(mixed).unwrap();
        assert_eq!(output, mixed, "mixed lone-CR input must stay unchanged");

        // The check helper is the shared `bsl_parser::has_lone_cr`.
        assert!(has_lone_cr(cr_only));
        assert!(has_lone_cr(mixed));
        assert!(!has_lone_cr("А = 1;\r\nБ = 2;\r\n"));
    }

    #[test]
    fn fix_preserves_bom_and_crlf() {
        // BOM survives byte-for-byte; CRLF line endings are untouched
        // (insertion-only at stop-token end bytes — no line-ending
        // normalization happens anywhere).
        let input = "\u{feff}Если Истина Тогда\r\n    А = 1;\r\nКонецЕсли // c\r\n".to_owned();
        let output = fix_missing_semicolons(&input).unwrap();
        let expected = "\u{feff}Если Истина Тогда\r\n    А = 1;\r\nКонецЕсли; // c\r\n".to_owned();
        assert_eq!(output, expected);
        assert!(output.starts_with('\u{feff}'));
        assert!(output.contains("КонецЕсли; // c\r\n"));

        // BOM + EOF form (no trailing newline): the `;` lands at the
        // very end, after the BOM-led file's last statement token.
        let eof_input = "\u{feff}\r\n\tВозврат \"Булево\"".to_owned();
        let output = fix_missing_semicolons(&eof_input).unwrap();
        assert_eq!(output, "\u{feff}\r\n\tВозврат \"Булево\";");
    }

    #[test]
    fn bsl_ls_fixture_parity() {
        let violations = find_missing_semicolons(BSL_LS_FIXTURE).unwrap();

        // Detector order is document order, parent before children
        // (bsl-ls visitor parity): the if_statement comes first, then
        // its inner assignment.
        //
        // Row 4: `КонецЕсли` — stop token is the ENDIF keyword (LSP
        // range (4,0,4,9): columns 0..9 on 0-based row 4).
        let row4 = "КонецЕсли         // и здесь";
        let row4_start = BSL_LS_FIXTURE.find(row4).unwrap();
        assert_eq!(
            violations[0],
            SemicolonViolation {
                stop_token_span: BslByteSpan::new(row4_start, row4_start + "КонецЕсли".len()),
                line: 5, // 0-based LSP row 4 → 1-based line 5
            }
        );
        // Row 3: `  А = 0           // Диагностика ...` — stop token is
        // the `0` (LSP range (3,6,3,7): columns 6..7 on 0-based row 3).
        let row3 = "  А = 0           // Диагностика должна сработать здесь";
        let row3_start = BSL_LS_FIXTURE.find(row3).unwrap();
        let zero_at = row3_start + "  А = ".len();
        assert_eq!(
            violations[1],
            SemicolonViolation {
                stop_token_span: BslByteSpan::new(zero_at, zero_at + "0".len()),
                line: 4, // 0-based LSP row 3 → 1-based line 4
            }
        );
        // Exactly 2 diagnostics — the bsl-ls test size assertion.
        assert_eq!(violations.len(), 2);
    }
}
