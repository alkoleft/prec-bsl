//! Detection of multiple statements per line (`ОдинОператорНаСтроку`).
//!
//! Transliteration of the bsl-ls `OneStatementPerLineDiagnostic`
//! `previousLineNumber` tracking, adapted to the tree-sitter-bsl 0.1.7
//! tree shapes (0.1.6 facts in `.superpowers/gh-issue/6/tree-dumps.md`
//! § 11, 0.1.7 deltas probed in /tmp/opencode/dateprobe2):
//!
//! - statement nodes are visited in document order, descending into
//!   compound statements and containers (`source_file`,
//!   `preprocessor` regions, `procedure_definition`,
//!   `function_definition`, `if_statement`, `elseif_clause`,
//!   `else_clause`, `while_statement`, `for_statement`,
//!   `for_each_statement`, `try_statement`) and into ERROR regions that
//!   still hold valid statement nodes inside;
//! - `previous_row: Option<usize>` — a statement whose start row equals
//!   the tracked row is a violation (the first statement of a chain is
//!   never flagged);
//! - a `preprocessor` statement resets the tracking (bsl-ls
//!   `ctx.preprocessor() != null`). 0.1.7 parses `#Область … #КонецОбласти`
//!   as ONE `preprocessor` node whose repeat(`_definition`) body holds
//!   the region's statements: the reset fires at the region START row,
//!   then the body statements are visited inside (statement events after
//!   the reset — exactly the ANTLR model, where region contents are
//!   statements of the surrounding code block between two preprocessor
//!   statements);
//! - a bare `;` is a real `_empty_statement` node in 0.1.7 (kind
//!   `";"`, prec(-1)): it resets the tracking and is never flagged —
//!   mirroring the bsl-ls empty statement branch
//!   (`ctx.getChildCount() == 1 && ctx.SEMICOLON() != null` clears
//!   pending without updating `previousLineNumber`). The reset fires
//!   only in statement position (`is_empty_statement`): a `;` after
//!   `КонецЕсли`/`КонецЦикла`/`КонецПопытки` is the compound's own
//!   `optional(';')` terminator, and a `;` right after a
//!   `while_statement` is the while's terminator (0.1.7 while still
//!   lacks `optional(';')`) — neither may clear pending violations.
//!   The 0.1.6 ERROR-`;` forms remain as fallbacks for other error
//!   shapes;
//! - an ERROR region led by `IF_KEYWORD`/`WHILE_KEYWORD`/`TRY_KEYWORD`/
//!   `FOR_KEYWORD` is an unterminated compound statement recovered inline
//!   by ANTLR (missing end keyword): it occupies a statement slot and its
//!   inner statements stay visible;
//! - any other ERROR region resets the tracking (bsl-ls
//!   `ctx.exception != null`);
//! - MISSING nodes do not invalidate a statement (ANTLR recovers missing
//!   tokens inline).

use std::fs;

use prec_bsl_pipeline::{
    ScenarioDefinition, ScenarioExecutionContext, ScenarioResult, ScenarioRun,
};
use prec_bsl_source::SourceFileKind;

use crate::bsl_parser::{BslByteSpan, BslParser, BslParserError, ParsedBsl};

/// Statement kinds from the grammar `_statement` (grammar.js:303 in
/// tree-sitter-bsl 0.1.7). `call_statement` is a real statement kind
/// since 0.1.7 (0.1.6 embedded calls in other shapes). `_empty_statement`
/// (grammar.js:326, kind `";"`) is handled separately in
/// `collect_events` as a reset.
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
/// 0.1.7 `preprocessor` regions hold `repeat(_definition)` bodies
/// (`#Область` … `#КонецОбласти`), so the collector must descend into
/// them after firing the region reset.
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

/// Compound keywords that lead an ERROR region wrapping an unterminated
/// compound statement (`Если`/`Пока`/`Попытка`/`Для`).
const COMPOUND_KEYWORD_KINDS: &[&str] =
    &["IF_KEYWORD", "WHILE_KEYWORD", "TRY_KEYWORD", "FOR_KEYWORD"];

/// End keywords of the compound statements that carry their own trailing
/// `optional(';')` inside the statement node (if/try/for/for_each in
/// grammar.js): a `;` right after one of them is that terminator, not an
/// empty statement.
const COMPOUND_TERMINATOR_KEYWORDS: &[&str] = &["ENDIF_KEYWORD", "ENDDO_KEYWORD", "ENDTRY_KEYWORD"];

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
    /// Track-only slot (`elseif_clause`): participates in
    /// `previous_row` tracking (its row can make a following body
    /// statement a violation with `after_block_keyword`) but is never
    /// flagged itself — bsl-ls never flags `elsifBranch` (it is not a
    /// statement in the ANTLR grammar, only the plan's indent extension
    /// needs its THEN row).
    track_only: bool,
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
        if self.previous_row == Some(row) && !event.track_only {
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
    // `elseif_clause` occupies a statement slot in the ANTLR model
    // (elsifBranch is a compoundStatement alternative): its start row
    // participates in tracking, so a body statement on the ELSIF/THEN row
    // is a violation with `after_block_keyword` (tree-dumps § 11.6). The
    // slot is track-only: bsl-ls never flags `elsifBranch` itself.
    if node.kind() == "elseif_clause" {
        out.push(statement_event(node));
    }
    // `_empty_statement` (0.1.7): a bare `;` in statement position is a
    // real node with kind `";"` — the bsl-ls empty-statement branch
    // (childCount==1 with SEMICOLON): clear pending WITHOUT updating
    // previous_row, never flagged. Implemented as Reset (which never
    // updates previous_row). Statement position is decided by
    // `is_empty_statement`; every other `;` node (statement terminators,
    // compound trailing `;`) falls through to the generic leaf skip.
    if node.kind() == ";" && !node.is_error() && !node.is_missing() && is_empty_statement(node) {
        out.push(reset_event(node));
        return;
    }
    if is_statement_kind(node.kind()) {
        if node.kind() == "preprocessor" {
            // 0.1.7: a `#Область … #КонецОбласти` region parses as ONE
            // preprocessor node whose repeat(_definition) body holds the
            // region's statements. The reset fires at the region START
            // row, then the body statements are visited inside — the
            // ANTLR model, where region contents are statements of the
            // surrounding code block BETWEEN two preprocessor statements
            // (a `#Область` reset and a `#КонецОбласти` reset).
            out.push(reset_event(node));
            // fall through: descend into the region body
        } else if has_embedded_error_semi(node, source) {
            // EOF-swallowed bare `;` inside a statement => statement event +
            // EmptySemiAtEnd at the statement's END row.
            let mut event = statement_event(node);
            event.kind = EventKind::Statement;
            out.push(event);
            let mut empty = reset_event(node);
            empty.kind = EventKind::EmptySemiAtEnd;
            out.push(empty);
            return;
        } else {
            out.push(statement_event(node));
        }
        // composite statements: descend into their statement children
    } else if node.is_error() {
        if is_error_semi(node, source) {
            // Grammar artifact: `;` immediately after while_statement —
            // 0.1.6 while_statement lacks optional(';') (unlike if/try/
            // for; still true in 0.1.7). In the ANTLR model that `;` is
            // the while statement's own terminator
            // (statement: ... SEMICOLON?), NOT an empty statement.
            // Skipping it preserves bsl-ls parity (pending statements
            // must NOT be cleared).
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
        track_only: node.kind() == "elseif_clause",
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
        track_only: false,
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

/// True when a clean `;` node is an `_empty_statement` in statement
/// position (the ANTLR `statement: SEMICOLON` alternative): a direct
/// child of a container kind (or of an ERROR region recovered inline)
/// that does NOT continue a compound statement. A `;` whose previous
/// sibling ends an if/try/for/for_each compound (`КонецЕсли` etc.) is
/// that compound's own `optional(';')` terminator; a `;` right after a
/// `while_statement` is the while's terminator in the ANTLR model
/// (tree-sitter-bsl 0.1.7 `while_statement` still lacks
/// `optional(';')`). Neither is an empty statement: firing a Reset
/// there would clear pending violations that bsl-ls keeps.
fn is_empty_statement(node: tree_sitter::Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if !is_container_kind(parent.kind()) && !parent.is_error() {
        return false;
    }
    let prev = node.prev_sibling().map(|sibling| sibling.kind());
    match prev {
        // Trailing `optional(';')` of if/try/for/for_each — held INSIDE
        // the compound node, so the parent check alone cannot reject it.
        Some(kind) if COMPOUND_TERMINATOR_KEYWORDS.contains(&kind) => false,
        // Grammar artifact: `while_statement` has no `optional(';')`, so
        // its terminator appears as a sibling `;` node. In the ANTLR
        // model it is part of the while statement, not an empty one.
        Some("while_statement") => false,
        _ => true,
    }
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

/// True when the input contains a lone `\r` — a `\r` NOT followed by
/// `\n` (CR-only or mixed line endings). tree-sitter rows advance only
/// on `\n`, so detection on such files yields false violations and the
/// fixer would corrupt the text (inserting `\n` next to surviving `\r`,
/// wrong `line_indent` via `rfind('\n')`). Such files are out of
/// contract for this scenario: it warns and makes no changes.
fn has_lone_cr(input: &str) -> bool {
    input
        .as_bytes()
        .windows(2)
        .any(|pair| pair[0] == b'\r' && pair[1] != b'\n')
        || input.ends_with('\r')
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

/// Text splice edit moving one statement to its own line: replace the
/// whitespace prefix before the statement start with
/// `<line_ending><indent>`. Private fixer detail (plan Task 3).
struct MoveEdit {
    /// Byte range of the whitespace prefix to replace
    /// `[end of previous content on the line .. statement_start]`.
    prefix_start: usize,
    prefix_end: usize,
    indent: String,
}

/// Compute all `MoveEdit`s for the given violations. Each edit replaces
/// the whitespace prefix before the statement start with
/// `<line_ending><indent>`; the line ending is appended by the applier.
fn plan_move_edits(violations: &[StatementViolation], input: &str) -> Vec<MoveEdit> {
    // Pattern-2 chains: on a row holding a statement right after a block
    // opener's `Тогда` (`after_block_keyword`), the chain — that
    // statement and every violation after it on the row — moves to the
    // opener-line indent + one tab (the plan's mixed rule). Violations
    // BEFORE the chain start (ordinary statements preceding an inline
    // `Если ... Тогда` on the same row) are not part of the opener chain
    // and keep the line indent, so the scope is the byte position of the
    // first pattern-2 violation on the row, not row membership.
    let mut block_opener_chain_starts: Vec<(usize, usize)> = Vec::new();
    for violation in violations.iter().filter(|v| v.after_block_keyword) {
        match block_opener_chain_starts
            .iter_mut()
            .find(|(row, _)| *row == violation.row)
        {
            Some((_, chain_start)) => {
                *chain_start = (*chain_start).min(violation.statement_span.start_byte);
            }
            None => {
                block_opener_chain_starts.push((violation.row, violation.statement_span.start_byte))
            }
        }
    }

    let bytes = input.as_bytes();
    let mut edits = Vec::new();
    for violation in violations {
        let start = violation.statement_span.start_byte;
        let line_start = input[..start].rfind('\n').map_or(0, |at| at + 1);
        // A UTF-8 BOM (`\u{FEFF}`, 3 bytes) may lead row 0: it is not
        // whitespace, but it must be transparent for the row indent
        // computation — the indent scan and the line-prefix bound start
        // after it. The BOM itself always survives byte-for-byte: edits
        // only replace whitespace prefixes at/after `indent_base`.
        let indent_base = if line_start == 0 && bytes.starts_with(b"\xEF\xBB\xBF") {
            line_start + 3
        } else {
            line_start
        };
        // Indent prefix of the violation's own row (byte-for-byte).
        let mut indent_end = indent_base;
        while indent_end < start && (bytes[indent_end] == b' ' || bytes[indent_end] == b'\t') {
            indent_end += 1;
        }
        if indent_end == start {
            // The statement opens its line: there is no previous content
            // to break from (cannot happen for a tracked violation —
            // defensive skip, no edit is expressible as a move).
            continue;
        }
        // Whitespace prefix before the statement: from the end of the
        // previous content on the line up to the statement start. The
        // statement node starts at its first non-whitespace character, so
        // this range is exactly the whitespace to replace (collapses
        // double spaces, leaves no trailing whitespace).
        let mut prefix_start = start;
        while prefix_start > indent_base
            && (bytes[prefix_start - 1] == b' ' || bytes[prefix_start - 1] == b'\t')
        {
            prefix_start -= 1;
        }
        if prefix_start == indent_base {
            // Whitespace run swallowed the whole line indent — the line
            // has no previous content; skip instead of blanking the line.
            continue;
        }

        let line_indent = &input[indent_base..indent_end];
        let in_opener_chain = block_opener_chain_starts.iter().any(|(row, chain_start)| {
            *row == violation.row && violation.statement_span.start_byte >= *chain_start
        });
        let indent = if in_opener_chain {
            format!("{line_indent}\t")
        } else {
            line_indent.to_owned()
        };
        edits.push(MoveEdit {
            prefix_start,
            prefix_end: start,
            indent,
        });
    }
    edits
}

/// Apply the edits to the original text. All byte ranges refer to the
/// ORIGINAL input (never to partially-applied text), so application order
/// cannot shift later offsets — equivalent to applying right-to-left.
fn apply_move_edits(input: &str, edits: Vec<MoveEdit>, line_ending: &str) -> String {
    let mut edits = edits;
    edits.sort_by_key(|edit| edit.prefix_start);

    let extra: usize = edits
        .iter()
        .map(|edit| line_ending.len() + edit.indent.len())
        .sum();
    let mut output = String::with_capacity(input.len() + extra);
    let mut copied = 0usize;
    for edit in edits {
        output.push_str(&input[copied..edit.prefix_start]);
        output.push_str(line_ending);
        output.push_str(&edit.indent);
        copied = edit.prefix_end;
    }
    output.push_str(&input[copied..]);
    output
}

/// Fix multiple-statements-per-line violations: move every flagged
/// statement to its own line with the indent computed from the plan's
/// rules (pattern 1: line indent; pattern 2: block-opener line indent +
/// one tab; mixed chains after a block opener all get the opener indent +
/// one tab). Pure function: no violations (or relevant parse errors) →
/// the input is returned unchanged, byte-for-byte.
pub fn fix_one_statement_per_line(input: &str) -> Result<String, BslParserError> {
    // Lone-`\r` line endings are out of contract (see `has_lone_cr`):
    // tree-sitter's row model is LF-based, so detection rows — and any
    // fix built on them — would be wrong. Never normalize or fix such
    // files here; the handler reports a Warning instead.
    if has_lone_cr(input) {
        return Ok(input.to_owned());
    }
    let violations = find_violations(input)?;
    if violations.is_empty() {
        return Ok(input.to_owned());
    }
    // The fixer refuses to touch unreliable regions (handler warns in a
    // later task); `;` is never added or removed and empty statements
    // (`;;`) are never flagged, so they produce no blank lines.
    if has_relevant_parse_errors(input)? {
        return Ok(input.to_owned());
    }
    let line_ending = crate::text_fixers::dominant_line_ending(input);
    let edits = plan_move_edits(&violations, input);
    Ok(apply_move_edits(input, edits, line_ending))
}

// --- Handler (plan Task 4): scenario boundary over finder + fixer ---
//
// The handler consumes exactly the module's two public functions
// (`find_violations` for the violating rows of the message,
// `fix_one_statement_per_line` for the fixed content); everything else
// (`StatementViolation`, edits, indents) stays private to the module.

/// Extension scenario id (`precommit4onec` name without `.os`).
pub const ONE_STATEMENT_PER_LINE_RULE: &str = "ОдинОператорНаСтроку";

pub const ONE_STATEMENT_PER_LINE_SCENARIO: ScenarioDefinition = ScenarioDefinition::compatibility(
    ONE_STATEMENT_PER_LINE_RULE,
    "ОдинОператорНаСтроку.os",
    one_statement_per_line,
);

/// Scenario handler: check + fix in one pass. DryRun (check mode) yields a
/// single `modified_dry_run` result with the proposed fixed content and a
/// message enumerating the violating rows; Apply writes the fixed file.
/// Relevant parse errors degrade to a warning without modifying the file.
pub fn one_statement_per_line(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
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

    // Lone-`\r` line endings (CR-only or mixed) are out of contract:
    // tree-sitter rows advance only on `\n`, so detection would yield
    // false violations and the fix would corrupt the text. Warn and
    // leave the file untouched (checked before `find_violations`, so
    // check mode produces no false diagnostics either).
    if has_lone_cr(&input) {
        return ScenarioRun::single(ScenarioResult::warning(
            context.rule_id,
            context.file.repo_path.clone(),
            "файл использует неканонические переводы строк (CR); сценарий ОдинОператорНаСтроку пропущен",
        ));
    }

    let violations = match find_violations(&input) {
        Ok(violations) => violations,
        Err(error) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                error.to_string(),
            ));
        }
    };
    if violations.is_empty() {
        return ScenarioRun::clean();
    }

    // Unreliable tree: report instead of silently fixing unreliable input.
    if has_relevant_parse_errors(&input).unwrap_or(true) {
        return ScenarioRun::single(ScenarioResult::warning(
            context.rule_id,
            context.file.repo_path.clone(),
            "file has parse errors intersecting detected one-statement-per-line violations; fix them before running the scenario",
        ));
    }

    let output = match fix_one_statement_per_line(&input) {
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

    let rows = violating_rows(&violations);
    let message = format!("перенесены операторы на отдельные строки: {rows}");
    crate::text_fixers::apply_text_modification(context, &path, output, &message)
}

/// 1-based rows of the violations, deduplicated in document order,
/// formatted for the single modification message ("строка N" /
/// "строки N, M").
fn violating_rows(violations: &[StatementViolation]) -> String {
    let mut rows: Vec<usize> = Vec::new();
    for violation in violations {
        let row = violation.row + 1;
        if !rows.contains(&row) {
            rows.push(row);
        }
    }
    match rows.as_slice() {
        [single] => format!("строка {single}"),
        many => format!(
            "строки {}",
            many.iter()
                .map(|row| row.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
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

    fn violation(
        input: &str,
        needle: &str,
        row: usize,
        after_block_keyword: bool,
    ) -> StatementViolation {
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
    fn trailing_empty_statement_is_not_a_violation() {
        // 0.1.7: the second `;` of `А = 0;;` is a clean `_empty_statement`
        // node (kind `";"`) in statement position, not an EOF-swallowed
        // ERROR as in 0.1.6. It resets the tracking and is never flagged.
        let input = "А = 0;;";
        assert!(find_violations(input).unwrap().is_empty());
        assert!(!has_relevant_parse_errors(input).unwrap());
    }

    #[test]
    fn empty_statement_reset_keeps_tracking_for_the_next_row() {
        // Fragment of OneStatementPerLineDiagnosticEndFile.bsl (rows 0-1):
        // the trailing empty statement resets pending WITHOUT updating
        // previous_row (bsl-ls clears pending but keeps
        // previousLineNumber), so the chain on row 1 is flagged from its
        // second statement on.
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

    /// bsl-ls parity for a mid-chain empty statement (ANTLR trace of
    /// `А=1; А=2;; А=3;`): the empty `;` clears the pending violation of
    /// `А=2` on the tracked row without updating previousLineNumber, so
    /// exactly ONE violation survives (`А=3`) and the fix moves only it.
    #[test]
    fn empty_statement_drops_pending_violations_of_the_tracked_row() {
        let input = "А=1; А=2;; А=3;";
        let violations = find_violations(input).unwrap();

        assert_eq!(violations, vec![violation(input, "А=3;", 0, false)]);
        // Clean parse under 0.1.7: the bare `;` is a real node, so the
        // fixer may run (no warning path).
        assert!(!has_relevant_parse_errors(input).unwrap());
        let output = fix_one_statement_per_line(input).unwrap();
        assert_eq!(output, "А=1; А=2;;\nА=3;");
    }

    /// 0.1.7 parses date literals (`'00010101'`) as a clean `date` node
    /// (0.1.6 could not): assignments with date values behave exactly
    /// like any other statement chain.
    #[test]
    fn date_literal_assignments_flag_only_real_statements() {
        let input = "Поле1 = '00010101'; Поле2 = '20240101'; Поле3 = '00010102';";
        let violations = find_violations(input).unwrap();

        assert_eq!(
            violations,
            vec![
                violation(input, "Поле2 = '20240101';", 0, false),
                violation(input, "Поле3 = '00010102';", 0, false),
            ]
        );
        assert!(!has_relevant_parse_errors(input).unwrap());
        let output = fix_one_statement_per_line(input).unwrap();
        assert_eq!(
            output,
            "Поле1 = '00010101';\nПоле2 = '20240101';\nПоле3 = '00010102';"
        );
    }

    /// Grammar artifact guard: tree-sitter-bsl 0.1.7 `while_statement`
    /// still lacks `optional(';')` (unlike if/try/for/for_each), so the
    /// `;` after `КонецЦикла` appears as a SIBLING `;` node of the while
    /// statement. In the ANTLR model it is the while's own terminator,
    /// NOT an empty statement: treating it as a Reset would clear the
    /// pending violations of the while body (all on the while's row).
    #[test]
    fn while_trailing_semicolon_is_not_an_empty_statement() {
        let input = "Пока Истина Цикл А = 1; Б = 2; КонецЦикла;";
        let violations = find_violations(input).unwrap();

        assert_eq!(
            violations,
            vec![
                violation(input, "А = 1;", 0, false),
                violation(input, "Б = 2;", 0, false),
            ]
        );
        assert!(!has_relevant_parse_errors(input).unwrap());
        let output = fix_one_statement_per_line(input).unwrap();
        assert_eq!(output, "Пока Истина Цикл\nА = 1;\nБ = 2; КонецЦикла;");
    }

    #[test]
    fn if_body_chain_on_then_row_flags_first_body_statement() {
        // Row 1 holds both statements; the first one follows `Тогда`.
        let input = "Если Истина Тогда\n  А = 0;А = 1; // comment\nКонецЕсли";
        let violations = find_violations(input).unwrap();

        assert_eq!(violations, vec![violation(input, "А = 1;", 1, false)]);
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
    fn stray_paren_error_drops_pending_violation_and_is_not_relevant() {
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

    // --- Task 3: fixer (`fix_one_statement_per_line`) ---

    /// All mutation inputs used by the fixer tests (also used for the
    /// idempotence loop).
    fn fixer_mutation_inputs() -> Vec<&'static str> {
        vec![
            // pattern 1 chains
            "А = 1; Б = 2;",
            "\tА = 1; Б = 2;",
            // pattern 2 (block opener on the same row)
            "Если X Тогда Тип = \"1\";",
            "\tЕсли X Тогда А = 1;",
            // mixed chain after a block opener
            "Если X Тогда А = 1; Б = 2;",
            // elseif opener
            "Если А Тогда\n Б=1;\nИначеЕсли Б Тогда С=2;",
            // CRLF file
            "Если X Тогда А = 1;\r\nБ = 2; В = 3;\r\nКонецЕсли",
            // spaces indent
            "    Если X Тогда А = 1;",
            // multiline statement in a chain
            "А = 1; Б = Ф(x,\n y);",
            // double-space collapse before the statement
            "Если X Тогда  А = 1;",
            // review findings: statements before an inline opener keep
            // line indent (chain-scoped extra tab)
            "А=1; Б=2; Если В Тогда Г=3;",
            // review findings: BOM + first-row indent
            "\u{feff}\tА = 1; Б = 2;",
        ]
    }

    #[test]
    fn fix_chain_moves_second_statement_to_new_line_with_line_indent() {
        let output = fix_one_statement_per_line("А = 1; Б = 2;").unwrap();
        assert_eq!(output, "А = 1;\nБ = 2;");
    }

    #[test]
    fn fix_indented_chain_copies_line_indent_byte_for_byte() {
        let output = fix_one_statement_per_line("\tА = 1; Б = 2;").unwrap();
        assert_eq!(output, "\tА = 1;\n\tБ = 2;");
    }

    #[test]
    fn fix_if_then_body_gets_one_extra_tab() {
        let output = fix_one_statement_per_line("Если X Тогда Тип = \"1\";").unwrap();
        assert_eq!(output, "Если X Тогда\n\tТип = \"1\";");
    }

    #[test]
    fn fix_indented_if_then_adds_tab_to_line_indent() {
        let output = fix_one_statement_per_line("\tЕсли X Тогда А = 1;").unwrap();
        assert_eq!(output, "\tЕсли X Тогда\n\t\tА = 1;");
    }

    #[test]
    fn fix_mixed_chain_moves_all_body_statements_to_opener_indent_plus_tab() {
        let output = fix_one_statement_per_line("Если X Тогда А = 1; Б = 2;").unwrap();
        assert_eq!(output, "Если X Тогда\n\tА = 1;\n\tБ = 2;");
    }

    /// Review finding (PR #18): `block_opener_rows` was row-based, not
    /// chain-based. When ordinary statements PRECEDE an inline
    /// `Если ... Тогда` on the same row, only the statements of the
    /// block-opener chain (the pattern-2 target and the chain after it)
    /// get opener indent + tab; earlier statements on the row keep the
    /// line indent. Detection results stay unchanged — this is purely
    /// fixer indent scoping.
    #[test]
    fn fix_statements_before_inline_if_keep_line_indent() {
        let output = fix_one_statement_per_line("А=1; Б=2; Если В Тогда Г=3;").unwrap();
        assert_eq!(output, "А=1;\nБ=2;\nЕсли В Тогда\n\tГ=3;");
    }

    /// Indented variant of the chain-scoping rule: `Д=4;` continues the
    /// chain after the pattern-2 statement `Г=3;` and keeps the extra
    /// tab; `А=1;`/`Б=2;` before the opener keep the line indent.
    #[test]
    fn fix_chain_after_opener_keeps_extra_tab_statements_before_keep_line_indent() {
        let input = "\tА=1; Б=2; Если В Тогда Г=3; Д=4;";
        let output = fix_one_statement_per_line(input).unwrap();
        assert_eq!(output, "\tА=1;\n\tБ=2;\n\tЕсли В Тогда\n\t\tГ=3;\n\t\tД=4;");
    }

    #[test]
    fn fix_elseif_body_gets_extra_tab() {
        let input = "Если А Тогда\n Б=1;\nИначеЕсли Б Тогда С=2;";
        let output = fix_one_statement_per_line(input).unwrap();
        assert_eq!(output, "Если А Тогда\n Б=1;\nИначеЕсли Б Тогда\n\tС=2;");
    }

    #[test]
    fn fix_crlf_file_uses_crlf_insertions() {
        let input = "Если X Тогда А = 1;\r\nБ = 2; В = 3;\r\nКонецЕсли";
        let output = fix_one_statement_per_line(input).unwrap();
        assert_eq!(
            output,
            "Если X Тогда\r\n\tА = 1;\r\nБ = 2;\r\nВ = 3;\r\nКонецЕсли"
        );
    }

    #[test]
    fn fix_spaces_indent_file_copies_prefix_and_appends_tab() {
        let input = "    Если X Тогда А = 1;";
        let output = fix_one_statement_per_line(input).unwrap();
        assert_eq!(output, "    Если X Тогда\n    \tА = 1;");
    }

    #[test]
    fn fix_multiline_statement_moves_whole_span_continuation_untouched() {
        let input = "А = 1; Б = Ф(x,\n y);";
        let output = fix_one_statement_per_line(input).unwrap();
        assert_eq!(output, "А = 1;\nБ = Ф(x,\n y);");
    }

    #[test]
    fn fix_collapses_double_space_no_trailing_whitespace_left() {
        let input = "Если X Тогда  А = 1;";
        let output = fix_one_statement_per_line(input).unwrap();
        assert_eq!(output, "Если X Тогда\n\tА = 1;");
    }

    #[test]
    fn fix_leaves_clean_inputs_unchanged_byte_for_byte() {
        let inputs = [
            "А = \"1;2\";",
            "// Если Тогда",
            "А = 1;\nБ = 2;",
            "А = 0;;",
            // Stray `)` resets the tracking, so violations are dropped
            // and the fixer has nothing to do.
            "А = 1; Б = 2; )",
            // Relevant parse error (unterminated procedure: the whole
            // region is an ERROR node led by PROCEDURE_KEYWORD, not a
            // safe form, and it intersects the violation row of `А=2;`):
            // the fixer refuses to touch unreliable regions and returns
            // the input unchanged.
            "Процедура А()\nА=1; А=2;",
        ];
        for input in inputs {
            let output = fix_one_statement_per_line(input).unwrap();
            assert_eq!(output, input, "input changed: {input:?}");
        }
    }

    /// Regression guard for the elseif statement slot: a body statement
    /// sharing the row with `ИначеЕсли ... Тогда` is a violation, but the
    /// elseif clause itself is never flagged (track-only slot) — bsl-ls
    /// never flags `elsifBranch`, and flagging it would make the fixer
    /// indent the clause into the if-body.
    #[test]
    fn elseif_clause_on_shared_row_is_never_flagged_itself() {
        // `Б=1;` after `Тогда` (row 0) and `С=2;` after the ELSIF `Тогда`
        // (row 1) are flagged; the `ИначеЕсли` clause itself is NOT
        // (track-only): its span never appears in the violation list.
        let input = "Если А Тогда Б=1;\nИначеЕсли Б Тогда С=2;";
        let violations = find_violations(input).unwrap();
        assert_eq!(
            violations,
            vec![
                violation(input, "Б=1;", 0, true),
                violation(input, "С=2;", 1, true),
            ]
        );
        let elseif_span = span_of(input, "ИначеЕсли Б Тогда");
        assert!(!violations.iter().any(|v| v.statement_span == elseif_span));
        // The fixer moves the two body statements but never the clause.
        let output = fix_one_statement_per_line(input).unwrap();
        assert_eq!(output, "Если А Тогда\n\tБ=1;\nИначеЕсли Б Тогда\n\tС=2;");
    }

    #[test]
    fn fix_is_idempotent_on_all_mutation_inputs() {
        for input in fixer_mutation_inputs() {
            let once = fix_one_statement_per_line(input).unwrap();
            let twice = fix_one_statement_per_line(&once).unwrap();
            assert_eq!(once, twice, "not idempotent for {input:?}");
        }
    }

    #[test]
    fn fix_lone_cr_input_is_returned_unchanged_byte_for_byte() {
        // CR-only (or mixed with lone `\r`) line endings are out of
        // contract: tree-sitter rows advance only on `\n`, so detection
        // would produce false violations and the fix would corrupt the
        // text (inserting `\n` next to surviving `\r`). The fixer must
        // return the input unchanged.
        let input = "А = 1; Б = 2;\rВ = 3; Г = 4;";
        let output = fix_one_statement_per_line(input).unwrap();
        assert_eq!(output, input, "lone-CR input must stay unchanged");
        // Mixed: a lone `\r` anywhere disables the fixer even when the
        // file also contains CRLF/LF endings.
        let mixed = "А = 1; Б = 2;\r\nВ = 3;\rГ = 4;";
        let output = fix_one_statement_per_line(mixed).unwrap();
        assert_eq!(output, mixed, "mixed lone-CR input must stay unchanged");
    }

    #[test]
    fn fix_bom_prefixed_file_preserves_bom_and_stays_idempotent() {
        // A UTF-8 BOM must survive the fix byte-for-byte and the fixed
        // output must stay idempotent (pins previously untested behavior).
        let input = "\u{feff}А = 1; Б = 2;";
        let once = fix_one_statement_per_line(input).unwrap();
        assert_eq!(once, "\u{feff}А = 1;\nБ = 2;");
        let twice = fix_one_statement_per_line(&once).unwrap();
        assert_eq!(once, twice, "second run must be a no-op");
    }

    /// Review finding (PR #18): a BOM before an INDENTED first-line chain
    /// lost the indent — the row-0 indent scan stops at the BOM bytes
    /// (not whitespace), so `line_indent` came out empty and the moved
    /// statement was dedented. The BOM is transparent for the indent
    /// computation: the tab after it is the row indent and must be copied
    /// byte-for-byte to the moved statement, while the BOM itself (and
    /// the untouched first-line prefix) survives byte-for-byte.
    #[test]
    fn fix_bom_indented_first_line_keeps_tab_after_bom() {
        let input = "\u{feff}\tА = 1; Б = 2;";
        let once = fix_one_statement_per_line(input).unwrap();
        assert_eq!(once, "\u{feff}\tА = 1;\n\tБ = 2;");
        let twice = fix_one_statement_per_line(&once).unwrap();
        assert_eq!(once, twice, "second run must be a no-op");
    }

    /// Review finding (PR #18), BOM + pattern 2: the opener-line indent
    /// after the BOM must be kept AND the block-opener tab appended to
    /// it (`\t` line indent + one `\t` for the `Тогда` body).
    #[test]
    fn fix_bom_indented_if_then_keeps_tab_and_adds_extra_tab() {
        let input = "\u{feff}\tЕсли X Тогда А = 1;";
        let once = fix_one_statement_per_line(input).unwrap();
        assert_eq!(once, "\u{feff}\tЕсли X Тогда\n\t\tА = 1;");
        let twice = fix_one_statement_per_line(&once).unwrap();
        assert_eq!(once, twice, "second run must be a no-op");
    }

    /// Two inline `Если ... Тогда` openers on one row: the chain start is
    /// the MIN pattern-2 start on the row, so statements between/after
    /// both openers get opener indent + tab; the pre-opener `А=1;` keeps
    /// the line indent; the second inline `Если` piggybacks on the row
    /// (detection parity — it is not itself "fixed").
    #[test]
    fn fix_two_inline_openers_on_one_row_min_chain_start() {
        let input = "А=1; Если В Тогда Г=3; Д=4; Если Е Тогда Ж=5;";
        let once = fix_one_statement_per_line(input).unwrap();
        assert_eq!(
            once,
            "А=1;\nЕсли В Тогда\n\tГ=3;\n\tД=4; Если Е Тогда\n\tЖ=5;"
        );
        let twice = fix_one_statement_per_line(&once).unwrap();
        assert_eq!(once, twice, "second run must be a no-op");
    }

    /// BOM + CRLF line ending + indented first-line chain: the BOM must
    /// survive byte-for-byte, CRLF is used as the inserted line ending,
    /// and the tab indent after the BOM is preserved on the moved line.
    #[test]
    fn fix_bom_crlf_indented_first_line() {
        let input = "\u{feff}\r\n\tА = 1; Б = 2;";
        let once = fix_one_statement_per_line(input).unwrap();
        assert_eq!(once, "\u{feff}\r\n\tА = 1;\r\n\tБ = 2;");
        let twice = fix_one_statement_per_line(&once).unwrap();
        assert_eq!(once, twice, "second run must be a no-op");
    }

    #[test]
    fn fix_bsl_ls_main_fixture_is_stable_after_two_runs_and_moves_all_violations() {
        let input = BSL_LS_MAIN_FIXTURE;
        let before_rows: Vec<usize> = find_violations(input)
            .unwrap()
            .iter()
            .map(|v| v.row)
            .collect();
        assert!(!before_rows.is_empty());

        let once = fix_one_statement_per_line(input).unwrap();
        let twice = fix_one_statement_per_line(&once).unwrap();
        assert_eq!(once, twice, "second run must be a no-op");

        // No violation of the first run may survive the fix: after the fix
        // the second run's finder must not re-flag the moved statements.
        // (Violations may legitimately remain on rows the fixer cannot
        // express — but for this fixture the fix must clear them all.)
        assert!(
            find_violations(&once).unwrap().is_empty(),
            "violations remain after fix:\n{once}"
        );
    }
}
