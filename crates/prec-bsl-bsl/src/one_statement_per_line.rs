//! Detection of multiple statements per line (`ОдинОператорНаСтроку`).
//!
//! Transliteration of the bsl-ls `OneStatementPerLineDiagnostic`
//! `previousLineNumber` tracking, adapted to the tree-sitter-bsl 0.1.6
//! tree shapes (see `.superpowers/gh-issue/6/tree-dumps.md` § 11):
//!
//! - statement nodes are visited in document order, descending into
//!   compound statements and containers (`source_file`,
//!   `procedure_definition`, `function_definition`, `if_statement`,
//!   `elseif_clause`, `else_clause`, `while_statement`, `for_statement`,
//!   `for_each_statement`, `try_statement`) and into ERROR regions that
//!   still hold valid statement nodes inside;
//! - `previous_row: Option<usize>` — a statement whose start row equals
//!   the tracked row is a violation (the first statement of a chain is
//!   never flagged);
//! - a `preprocessor` statement resets the tracking (bsl-ls
//!   `ctx.preprocessor() != null`);
//! - a bare `;` is an ERROR node in 0.1.6 (no `_empty_statement`):
//!   a top-level ERROR-`;` resets the tracking (bsl-ls empty statement),
//!   an EOF-swallowed ERROR-`;` child of a statement fires a reset at the
//!   statement's END row, and a top-level ERROR-`;` right after a
//!   `while_statement` is a grammar artifact (0.1.6 `while_statement`
//!   lacks `optional(';')`) — in the ANTLR model that `;` is the while
//!   statement's own terminator, so it is skipped entirely to keep
//!   bsl-ls parity;
//! - an ERROR region led by `IF_KEYWORD`/`WHILE_KEYWORD`/`TRY_KEYWORD`/
//!   `FOR_KEYWORD` is an unterminated compound statement recovered inline
//!   by ANTLR (missing end keyword): it occupies a statement slot and its
//!   inner statements stay visible;
//! - any other ERROR region resets the tracking (bsl-ls
//!   `ctx.exception != null`);
//! - MISSING nodes do not invalidate a statement (ANTLR recovers missing
//!   tokens inline).

use crate::bsl_parser::{BslByteSpan, BslParser, BslParserError, ParsedBsl};

/// Statement kinds from the grammar `_statement` (grammar.js:250).
const STATEMENT_KINDS: &[&str] = &[
    "execute_statement",
    "call_statement",
    "assignment_statement",
    "return_statement",
    "try_statement",
    "rise_error_statement",
    "var_statement",
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
    "preprocessor",
    "await_statement",
];

/// Containers that are not statements themselves but hold statements.
const CONTAINER_KINDS: &[&str] = &[
    "source_file",
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

/// Compound keywords that lead an ERROR region wrapping an unterminated
/// compound statement (`Если`/`Пока`/`Попытка`/`Для`).
const COMPOUND_KEYWORD_KINDS: &[&str] = &[
    "IF_KEYWORD",
    "WHILE_KEYWORD",
    "TRY_KEYWORD",
    "FOR_KEYWORD",
];

/// One flagged statement: several statements share its row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementViolation {
    /// Byte span of the statement node, including the trailing `;`
    /// (diagnostic span parity with bsl-ls ranges).
    pub statement_span: BslByteSpan,
    /// 0-based start row of the statement (LSP convention).
    pub row: usize,
    /// True when the statement follows an `if`/`elseif` branch opener on
    /// the same row (`Тогда`), i.e. the previous tracked statement was an
    /// `if_statement` or an IF/ELSIF-led ERROR compound region whose
    /// `THEN_KEYWORD` child row equals this row. Used by the fixer to add
    /// one extra indent level. `else_clause` does NOT set this.
    pub after_block_keyword: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum EventKind {
    /// Normal statement: add-or-flush, then `previous_row = row`.
    Statement,
    /// Reset trackers: preprocessor, exception region, bare `;`.
    Reset,
    /// Empty `;` terminally swallowed by the preceding statement (EOF
    /// form): evaluated at the END row of the enclosing statement.
    EmptySemiAtEnd,
}

/// Tree-sitter node reference with its event kind. `Tree` is not `Sync`
/// and cannot be stored in a `Vec` across the `Node<'tree>` borrow, so
/// events carry `(kind, start_byte, end_byte, row, end_row)` tuples
/// resolved from the node at collection time. (tree-sitter 0.25 `Node`
/// borrows the `Tree`; a `Vec<Node>` in one scope is fine — see
/// `collect_events`.)
struct Event {
    /// Whether the event node is a block-opener compound (if_statement,
    /// elseif_clause, or keyword-led ERROR region) — needed for
    /// `after_block_keyword`.
    is_block_opener: bool,
    /// Row of the `THEN_KEYWORD` child of the block opener, if any.
    then_row: Option<usize>,
    kind: EventKind,
    row: usize,
    end_row: usize,
    span: BslByteSpan,
}

/// Pending violations of the current tracked row, flushed when a
/// statement with a different row (or a reset) is processed — mirroring
/// the bsl-ls add-then-clear order.
struct PendingItem {
    span: BslByteSpan,
    row: usize,
    after_block_keyword: bool,
}

/// Previous statement context used for `after_block_keyword`: the kind of
/// the node that last set `previous_row`.
#[derive(Clone, Copy, PartialEq)]
enum PreviousKind {
    Statement,
    BlockOpener,
}

struct Tracker {
    violations: Vec<StatementViolation>,
    pending: Vec<PendingItem>,
    previous_row: Option<usize>,
    /// The tracking node that set `previous_row` — block opener or not.
    previous_kind: PreviousKind,
    previous_then_row: Option<usize>,
}

impl Tracker {
    fn new() -> Self {
        Self {
            violations: Vec::new(),
            pending: Vec::new(),
            previous_row: None,
            previous_kind: PreviousKind::Statement,
            previous_then_row: None,
        }
    }

    /// add-or-flush (bsl-ls visitStatement does this for every statement).
    fn process(&mut self, event: &Event) {
        let row = match event.kind {
            EventKind::EmptySemiAtEnd => event.end_row,
            _ => event.row,
        };
        if self.previous_row == Some(row) {
            // after_block_keyword must be captured at push time: the
            // previous statement in tracking (the node that set
            // previous_row) is if_statement or an IF/ELSIF-led ERROR
            // compound region whose THEN_KEYWORD child row equals this
            // row (tree-dumps § 11.6).
            let after_block_keyword = self.previous_kind == PreviousKind::BlockOpener
                && self.previous_then_row == Some(row);
            self.pending.push(PendingItem {
                span: event.span,
                row: event.row,
                after_block_keyword,
            });
        } else {
            self.flush();
        }
        match event.kind {
            EventKind::Statement => {
                self.previous_row = Some(row);
                self.previous_kind = if event.is_block_opener {
                    PreviousKind::BlockOpener
                } else {
                    PreviousKind::Statement
                };
                self.previous_then_row = event.then_row;
            }
            EventKind::Reset | EventKind::EmptySemiAtEnd => {
                self.pending.clear();
            }
        }
    }

    fn flush(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        for item in pending {
            self.violations.push(StatementViolation {
                statement_span: item.span,
                row: item.row,
                after_block_keyword: item.after_block_keyword,
            });
        }
    }
}

/// Collect the tracking events for a parsed tree, in document order.
fn collect_events(node: tree_sitter::Node<'_>, source: &str, out: &mut Vec<Event>) {
    if is_statement_kind(node.kind()) {
        if node.kind() == "preprocessor" {
            out.push(reset_event(node));
            return;
        }
        // EOF-swallowed bare `;` inside a statement => statement event +
        // EmptySemiAtEnd at the statement's END row.
        if has_embedded_error_semi(node, source) {
            let mut event = statement_event(node);
            event.kind = EventKind::Statement;
            out.push(event);
            let mut empty = reset_event(node);
            empty.kind = EventKind::EmptySemiAtEnd;
            out.push(empty);
            return;
        }
        out.push(statement_event(node));
        // composite statements: descend into their statement children
    } else if node.is_error() {
        if is_error_semi(node, source) {
            // Grammar artifact: `;` immediately after while_statement —
            // 0.1.6 while_statement lacks optional(';') (unlike if/try/for).
            // In the ANTLR model that `;` is the while statement's own
            // terminator (statement: ... SEMICOLON?), NOT an empty
            // statement. Skipping it preserves bsl-ls parity (pending
            // statements must NOT be cleared).
            let prev_is_while = node
                .prev_sibling()
                .is_some_and(|p| p.kind() == "while_statement");
            if prev_is_while {
                return;
            }
            out.push(reset_event(node));
            return; // bare `;` — nothing inside
        }
        // Unterminated compound (missing КонецЕсли/КонецПопытки/
        // КонецЦикла): tree-sitter wraps the whole region in ERROR, but
        // ANTLR recovers a missing token INLINE (no ctx.exception) — the
        // compound statement keeps its row and participates in tracking.
        if is_compound_error_region(node) {
            let mut event = statement_event(node);
            event.is_block_opener = true;
            event.then_row = child_keyword_row(node, "THEN_KEYWORD");
            out.push(event);
            // fall through: descend into children
        } else {
            // Real error region (e.g. preproc inside a call): Reset at
            // start row, then descend (structure inside still visited).
            out.push(reset_event(node));
        }
    } else if !is_container_kind(node.kind()) {
        return; // line_comment, expressions etc. — no statements inside
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_events(cursor.node(), source, out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn statement_event(node: tree_sitter::Node<'_>) -> Event {
    let is_block_opener = node.kind() == "if_statement" || node.kind() == "elseif_clause";
    Event {
        is_block_opener,
        then_row: if is_block_opener {
            child_keyword_row(node, "THEN_KEYWORD")
        } else {
            None
        },
        kind: EventKind::Statement,
        row: node.start_position().row,
        end_row: node.end_position().row,
        span: BslByteSpan::new(node.start_byte(), node.end_byte()),
    }
}

fn reset_event(node: tree_sitter::Node<'_>) -> Event {
    Event {
        is_block_opener: false,
        then_row: None,
        kind: EventKind::Reset,
        row: node.start_position().row,
        end_row: node.end_position().row,
        span: BslByteSpan::new(node.start_byte(), node.end_byte()),
    }
}

/// Row of the direct named child with `kind_name` (e.g. THEN_KEYWORD),
/// if present.
fn child_keyword_row(node: tree_sitter::Node<'_>, kind_name: &str) -> Option<usize> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind_name {
            return Some(child.start_position().row);
        }
    }
    None
}

fn is_statement_kind(kind: &str) -> bool {
    STATEMENT_KINDS.contains(&kind)
}

fn is_container_kind(kind: &str) -> bool {
    CONTAINER_KINDS.contains(&kind)
}

fn is_error_semi(node: tree_sitter::Node<'_>, source: &str) -> bool {
    node.is_error()
        && node
            .utf8_text(source.as_bytes())
            .is_ok_and(|text| text == ";")
}

/// An ERROR region that starts with a compound-statement keyword
/// (Если/Пока/Попытка/Для) = unterminated compound statement that ANTLR
/// would recover inline (missing end keyword) — NOT ctx.exception.
fn is_compound_error_region(node: tree_sitter::Node<'_>) -> bool {
    let mut cursor = node.walk();
    let mut children = node.children(&mut cursor);
    children
        .next()
        .is_some_and(|first| COMPOUND_KEYWORD_KINDS.contains(&first.kind()))
}

fn has_embedded_error_semi(node: tree_sitter::Node<'_>, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if is_error_semi(child, source) {
            return true;
        }
    }
    false
}

/// Find all one-statement-per-line violations (bsl-ls parity).
pub fn find_violations(input: &str) -> Result<Vec<StatementViolation>, BslParserError> {
    let mut parser = BslParser::new()?;
    let parsed = parser.parse(input)?;
    let mut events = Vec::new();
    collect_events(parsed.tree().root_node(), input, &mut events);

    let mut tracker = Tracker::new();
    for event in &events {
        tracker.process(event);
    }
    // Final flush (bsl-ls visitFile).
    tracker.flush();
    Ok(tracker.violations)
}

/// True when a parse error node intersects the row of some violation,
/// i.e. the detection result cannot be fully trusted for this input and
/// the scenario must take the warning path. Error nodes of known safe
/// forms (MISSING nodes, bare-`;` ERRORs, keyword-led compound ERROR
/// regions and the preprocessor regions inside them) are excluded: they
/// are already handled by the tracking rules and keep bsl-ls fixture
/// parity. Perfect generality is not required — the fixtures and the
/// treedump probe cases must pass.
pub fn has_relevant_parse_errors(input: &str) -> Result<bool, BslParserError> {
    let mut parser = BslParser::new()?;
    let parsed = parser.parse(input)?;
    if !parsed.has_errors() {
        return Ok(false);
    }
    let violations = find_violations(input)?;
    if violations.is_empty() {
        return Ok(false);
    }
    let violation_rows: Vec<usize> = violations.iter().map(|v| v.row).collect();
    Ok(has_unsafe_error_at_rows(&parsed, &violation_rows))
}

/// Collect error nodes (excluding MISSING) that intersect a violation row
/// and are not of a safe form.
fn has_unsafe_error_at_rows(parsed: &ParsedBsl, rows: &[usize]) -> bool {
    let root = parsed.tree().root_node();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_error()
            && !is_safe_error_region(node)
            && rows.iter().any(|row| row_intersects(node, *row))
        {
            return true;
        }
        // MISSING nodes are always safe (ANTLR inline recovery).
        if !node.is_missing() {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.has_error() {
                    stack.push(child);
                }
            }
        }
    }
    false
}

fn row_intersects(node: tree_sitter::Node<'_>, row: usize) -> bool {
    node.start_position().row <= row && row <= node.end_position().row
}

/// Safe error forms (tree-dumps § 11.2-11.4):
/// - a bare `;` ERROR (empty statement, already a tracking reset);
/// - an ERROR-`;` right after a while_statement (grammar artifact);
/// - a compound keyword-led ERROR region (unterminated compound recovered
///   inline);
/// - an ERROR region containing preprocessor keywords (preproc inside an
///   expression: bsl-ls also resets there, no warning needed).
fn is_safe_error_region(node: tree_sitter::Node<'_>) -> bool {
    if node.is_missing() {
        return true;
    }
    if is_compound_error_region(node) {
        return true;
    }
    // preprocessor-containing error region (e.g. `#Если` inside a call):
    // resets tracking exactly like bsl-ls preprocessor handling.
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind().starts_with("PREPROC_") {
            return true;
        }
    }
    false
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

    fn violation(input: &str, needle: &str, row: usize, after_block_keyword: bool) -> StatementViolation {
        StatementViolation {
            statement_span: span_of(input, needle),
            row,
            after_block_keyword,
        }
    }

    #[test]
    fn chain_of_three_assignments_flags_second_and_third() {
        let input = "А=1; А=2; А=3;";
        let violations = find_violations(input).unwrap();

        assert_eq!(
            violations,
            vec![
                violation(input, "А=2;", 0, false),
                violation(input, "А=3;", 0, false),
            ]
        );
    }

    #[test]
    fn complete_if_then_flags_both_statements() {
        let input = "Если Истина Тогда Сообщить(А=1); F=0; КонецЕсли;";
        let violations = find_violations(input).unwrap();

        assert_eq!(
            violations,
            vec![
                violation(input, "Сообщить(А=1);", 0, true),
                violation(input, "F=0;", 0, false),
            ]
        );
    }

    #[test]
    fn eof_swallowed_double_semicolon_is_not_a_violation() {
        let input = "А = 0;;";
        assert!(find_violations(input).unwrap().is_empty());
        assert!(!has_relevant_parse_errors(input).unwrap());
    }

    #[test]
    fn endfile_fragment_after_eof_semicolon_still_tracks() {
        // Fragment of OneStatementPerLineDiagnosticEndFile.bsl (rows 0-1):
        // the EOF-swallowed `;` of `А = 0;;` resets at end row 0, then the
        // chain on row 1 is flagged from its second statement on.
        let input = "А = 0;;\nФ=1; У=2; Е=3;";
        let violations = find_violations(input).unwrap();

        assert_eq!(
            violations,
            vec![
                violation(input, "У=2;", 1, false),
                violation(input, "Е=3;", 1, false),
            ]
        );
        assert!(!has_relevant_parse_errors(input).unwrap());
    }

    #[test]
    fn if_body_chain_on_then_row_flags_first_body_statement() {
        // Row 1 holds both statements; the first one follows `Тогда`.
        let input = "Если Истина Тогда\n  А = 0;А = 1; // comment\nКонецЕсли";
        let violations = find_violations(input).unwrap();

        assert_eq!(
            violations,
            vec![violation(input, "А = 1;", 1, false)]
        );
    }

    #[test]
    fn unterminated_if_error_wrap_keeps_inner_statements_flagged() {
        // Two statements in an unterminated `Если` body: tree-sitter wraps
        // the region in ERROR, but the compound is recovered inline by
        // ANTLR — both statements are flagged and no relevant parse error
        // remains (safe form, tree-dumps § 11.3).
        let input = "Если X Тогда А = 1; Б = 2;";
        let violations = find_violations(input).unwrap();

        assert_eq!(
            violations,
            vec![
                violation(input, "А = 1;", 0, true),
                violation(input, "Б = 2;", 0, false),
            ]
        );
        assert!(!has_relevant_parse_errors(input).unwrap());
    }

    /// Chosen policy (documented): a stray non-keyword ERROR region (e.g.
    /// a lone `)`) resets the tracking, so the pending violation for
    /// `Б = 2;` is dropped — matching the Reset semantics of bsl-ls
    /// `ctx.exception != null`. Since the violation is dropped, no
    /// tracked violation row remains for the error node to intersect,
    /// and `has_relevant_parse_errors` is false: the input degrades to
    /// "no diagnostics", the same way bsl-ls yields nothing after a
    /// ctx.exception reset. (The alternative — keeping the pending
    /// violation and flagging the error as relevant — is the plan's other
    /// allowed branch; this one keeps find_violations and the relevance
    /// check consistent on the same dropped state.)
    #[test]
    fn stray_paren_error_drops_pending_violation_and_is_relevant() {
        let input = "А = 1; Б = 2; )";
        assert!(find_violations(input).unwrap().is_empty());
        assert!(!has_relevant_parse_errors(input).unwrap());
    }

    #[test]
    fn semicolon_inside_string_literal_is_ignored() {
        let input = "А = \"1;2\";";
        assert!(find_violations(input).unwrap().is_empty());
    }

    #[test]
    fn two_statements_with_string_literals_flag_second() {
        let input = "А = \"1;2\"; Б = \"Тогда\";";
        let violations = find_violations(input).unwrap();

        assert_eq!(
            violations,
            vec![violation(input, "Б = \"Тогда\";", 0, false)]
        );
    }

    #[test]
    fn comment_only_line_is_not_a_statement() {
        let input = "// Если Тогда";
        assert!(find_violations(input).unwrap().is_empty());
    }

    #[test]
    fn async_procedure_body_has_no_violations() {
        // `Асинх` parses cleanly (ASYNC_KEYWORD + await_expression); the
        // single body statement is alone on its row.
        let input = "Асинх Процедура а()\n    Существует = Ждать ФайлНаДиске.СуществуетАсинх();\nКонецПроцедуры";
        assert!(find_violations(input).unwrap().is_empty());
    }

    // --- Full bsl-ls fixture parity (contents embedded verbatim) ---

    /// Verbatim content of bsl-ls fixture
    /// `OneStatementPerLineDiagnostic.bsl`
    /// (5 violations, rows 3, 8, 8, 12, 12; LSP ranges (3,8,3,14),
    /// (8,18,8,32), (8,33,8,37), (12,5,12,9), (12,10,12,14) — see
    /// OneStatementPerLineDiagnosticTest.java).
    const BSL_LS_MAIN_FIXTURE: &str = r##"А = 0;;
Если Истина Тогда

  А = 0;А = 1; // Диагностика должна сработать здесь
КонецЕсли

#Область ИмяОбласти

Если Истина Тогда Сообщить(А=1); F=0; КонецЕсли;

#КонецОбласти

А=1; А=2; А=3;

Процедура А()
 УспешноПодключено = ПодключитьВнешнююКомпоненту(
		#Если Клиент Тогда
			"C:\Projects\ETPAddin\Bin\Debug-Win32\AddInNative\AddInNative.dll",
		#Иначе
			"C:\Projects\ETPAddin\Bin\Debug-x64\AddInNative\AddInNative.dll",
		#КонецЕсли
			"ETP",
			ТипВнешнейКомпоненты.Native);
КонецПроцедуры"##;

    /// Verbatim content of bsl-ls fixture
    /// `OneStatementPerLineDiagnosticEndFile.bsl`
    /// (2 violations, rows 1, 1; LSP ranges (1,5,1,9), (1,10,1,14)).
    const BSL_LS_ENDFILE_FIXTURE: &str = r#"А = 0;;
Ф=1; У=2; Е=3;

Асинх Процедура а()
    Существует = Ждать ФайлНаДиске.СуществуетАсинх();
КонецПроцедуры"#;

    #[test]
    fn bsl_ls_main_fixture_parity() {
        let input = BSL_LS_MAIN_FIXTURE;
        let violations = find_violations(input).unwrap();

        assert_eq!(
            violations,
            vec![
                // row 3: `  А = 0;А = 1;` — second statement flagged;
                // previous row-3 statement is `А = 0;`, not a block opener
                violation(input, "А = 1;", 3, false),
                // row 8: `Если Истина Тогда Сообщить(А=1); F=0; КонецЕсли;`
                // — first body statement right after `Тогда` on the same row
                violation(input, "Сообщить(А=1);", 8, true),
                violation(input, "F=0;", 8, false),
                // row 12: `А=1; А=2; А=3;` — flags on 2nd and 3rd
                violation(input, "А=2;", 12, false),
                violation(input, "А=3;", 12, false),
            ]
        );
        assert!(!has_relevant_parse_errors(input).unwrap());
    }

    #[test]
    fn bsl_ls_endfile_fixture_parity() {
        let input = BSL_LS_ENDFILE_FIXTURE;
        let violations = find_violations(input).unwrap();

        assert_eq!(
            violations,
            vec![
                violation(input, "У=2;", 1, false),
                violation(input, "Е=3;", 1, false),
            ]
        );
        assert!(!has_relevant_parse_errors(input).unwrap());
    }
}
