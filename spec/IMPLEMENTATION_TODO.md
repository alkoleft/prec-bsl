# prec-bsl Implementation Todo

This ledger tracks executable implementation work for the current v1 scope from
`spec/prd-prec-bsl.md`, `spec/configuration.md`, `spec/parser-strategy.md`, and
`spec/testing-strategy.md`. Reference scenario inventory evidence is captured in
`spec/reference-scenario-inventory.md`.

Completed task history is archived in
`spec/archive/completed-tasks-t1-t43-t47.md`. Keep this file focused on active
or intentionally deferred work.

Roadmap style:

- Active statuses are `TODO`, `IN PROGRESS`, or `BLOCKED`; completed work moves
  to `spec/archive/`.
- Sequencing is parity-first and layer-first: establish reference behavior and
  shared architecture before filling scenario implementations.
- Scenario ordering is risk-based: start with deterministic text fixers, then
  parser-backed diagnostics, XML/EDT transformations, metadata scenarios, and
  platform-dependent work.
- This file is the implementation ledger, not a replacement for the specs.

## Milestone 15: Extension Lexical Fixers

> Task id note: the first draft of this record used id T51, which collided
> with the already-merged LineLength task in Milestone 14. This record is
> renumbered to T54; no other ledger entry references the old id.

### T54. DONE: ПробелыВокругОператоров + ЛишниеЗапятыеВКонце extension fixers

prec-bsl upstream issue: `alkoleft/prec-bsl` #9 (part of #3, «проверка vs
фикс» table). Resolved 2026-10-31; see the completion evidence below. The
scenario contracts live in `spec/parser-strategy.md` (section "Operator
spacing and trailing comma fixers contract") and
`spec/reference-scenario-inventory.md` (Extension Scenarios).

Resolution:

- Two lexical extension fixers (opt-in through `ГлобальныеСценарии`, `.os`
  suffix lookup supported) in `crates/prec-bsl-bsl/src/text_fixers.rs`:
  `ПробелыВокругОператоров` (bsl-ls MissingSpace operator subset
  `+ - * / % = < > <> <= >=`, unary `-`/`+` after `[ ( , =` or file start
  exempt) and `ЛишниеЗапятыеВКонце` (bsl-ls ExtraCommas, whole trailing
  comma chains removed in one pass, comments preserved). Both run over one
  shared whole-file scanner (literal/comment/directive tracker + paren
  depth) and one `apply_spans` byte-splice routine; the pre-existing
  per-line fixers keep their own trackers (deferred DRY, documented).
- bsl-ls 1.0.7 parity on the yaxunit control corpus (env-gated
  `tests/spacing_parity.rs`, pinned commit `d21ce6ff`, jar SHA-256 pinned in
  the baseline): pre-fix 104 MissingSpace / 2 ExtraCommas → post-fix
  ExtraCommas 0, MissingSpace 84 fully inside the residue policy
  (81 comma/semicolon + 2 unary sign + 1 keyword, mechanical
  classification); corpus idempotence proven (second pass: zero changes).
- Deliberate non-parity documented in `spec/parser-strategy.md`: unary
  exemption, non-operator MissingSpace classes, broken code, paren-group
  commas, numeric exponents, keyword-adjacent unary signs.

Completion evidence:

- 2026-10-31 (branch `issue-9-operator-spacing-trailing-commas`): fixers,
  scanner and unit tests in commits `f5b092e` and `231bdfc`; review-fix
  rounds (date literals, BOM directives, whitespace-only gap verification)
  in commits `1fc43ba` and `3b3204a`; parity test and baseline in commit
  `99b894c`; parity enforcement hardening in commit `3f723a0`; specs in
  this commit. Gates: `cargo test --workspace` 481 passed / 0 failed;
  `cargo fmt --check` clean; env-gated parity run green (pre 104/2 → post
  84/0, residue 81/2/1, fixers changed 16 of 350 tracked `.bsl` files,
  second pass idempotent). Implementation plan (kept in the main checkout
  outside version control, `docs/superpowers/plans/`):
  `2026-10-31-issue-9-operator-spacing-trailing-commas.md`.

Validation:

- `cargo test --workspace`
- `cargo fmt --check`
- env-gated parity test (`PREC_BSL_PARITY_CORPUS`, bsl-ls 1.0.7 jar)

Dependencies:

- T38, T39 (crate layout and scenario definition binding).

## Milestone 11: Upstream Issue Regression Backlog

These tasks capture risks found during the 2026-05-07 review of open
`precommit4onec` GitHub issues. They are tracked as explicit parity/regression
work so upstream problems are either fixed in `prec-bsl` or intentionally kept
outside the Rust v1 scope.

Completed task T50 (resolved 2026-10-29, `УдалениеЛишнихПустыхСтрок` trailing
blank run at EOF, `alkoleft/prec-bsl` #11) and task T51 (resolved 2026-10-30,
`ПроверкаПереносовСтрок` check + fix + settings, IncorrectLineBreak parity,
`alkoleft/prec-bsl` #5) are archived with the Milestone 11 history below and
in `spec/archive/completed-tasks-t1-t43-t47.md`.

### T44. TODO: Preserve OScript-only issue boundaries in diagnostics

Upstream issues: `precommit4onec` #2, #42, #43.

Current finding:

- Missing OScript dependencies, `#Использовать` failures, repository-local
  `.os` runtime execution, and external-artifact temporary-directory cleanup
  are not part of the ordinary built-in Rust v1 hook/check path.
- Platform execution is allowed only for explicitly specified scenarios such as
  `РазборОтчетовОбработокРасширений` and must remain separate from pure Rust
  parser/config/scenario paths.

Acceptance criteria:

- Unsupported repository-local `.os` scenarios keep producing clear blocking
  diagnostics instead of being silently ignored or attempted through OScript.
- The built-in Rust path does not start OScript or 1C runtime processes for
  config parsing, BSL checks, XML/EDT transformations, or ordinary hook mode.
- Future `РазборОтчетовОбработокРасширений` work must define its temporary
  directory cleanup and deleted-path behavior before enabling platform runtime
  execution.

Validation:

- `cargo test config`
- `cargo test rat`
- `cargo test --workspace`

Dependencies:

- T5, T6, T30.

### T45. TODO: Port executable precommit4onec test cases into Rust parity tests

Current finding:

- The legacy `precommit4onec` `tests` tree is imported under
  `tests/fixtures/precommit4onec-reference/` and guarded for corpus integrity.
- The executable test-case intent from those legacy tests has not yet been
  ported into Rust integration tests.
- Some legacy expectations may already be covered by existing focused
  `prec-bsl` tests and should be updated there instead of duplicated.
- 2026-05-07 progress: `spec/testing-strategy.md` now maps every executable
  legacy `.os` test method to `covered`, `blocked` or `out-of-scope`, and
  `tests/precommit4onec_reference.rs` verifies that mapping stays complete.
- T45 remains open while T40-T44 are still open because the remaining blocked
  parity groups depend on those upstream-regression and OScript/platform
  boundary tasks.

Acceptance criteria:

- Inventory the imported `precommit4onec` test cases by scenario, fixture
  inputs, expected modifications, expected diagnostics and unsupported
  OScript/platform dependencies.
- Map each imported test case to one of:
  - already covered by an existing `prec-bsl` test;
  - covered after updating an existing `prec-bsl` test or fixture;
  - needs a new Rust parity test;
  - intentionally out of v1 scope with documented reason.
- Port executable expectations for supported built-in Rust scenarios into
  focused Rust integration tests, preserving Cyrillic paths and scenario names.
- Prefer updating existing scenario tests when they already protect the same
  behavior; add new tests only for uncovered behavior.
- Keep unsupported repository-local `.os` and platform-runtime cases as
  explicit diagnostics or documented non-goals, not silently skipped parity
  gaps.
- Update `spec/testing-strategy.md` with the resulting mapping and the rules for
  using `tests/fixtures/precommit4onec-reference/` as parity evidence.

Validation:

- `cargo test precommit4onec_reference`
- Scenario-specific tests for each ported group.
- `cargo test --workspace`

Dependencies:

- T4, T37, T40, T41, T42, T43, T44.

## Milestone 12: Configuration UX Research

These tasks are research and design work, not implementation work. They should
compare configuration shapes from the point of view of real hook users before
changing persisted config contracts.

### T46. TODO: Research the future scenario settings format

Current finding:

- `v8config.json` remains required as the backward-compatibility layer for
  existing `precommit4onec` users.
- The project still needs a separate decision about the most convenient native
  `prec-bsl` settings structure and storage format for new projects.
- The decision should be based on configuration ergonomics for common
  pre-commit cases, not on the current implementation shape.

Research questions:

- Should the native config be JSON, TOML, YAML, or another format, considering
  `prek` / `pre-commit` users, editor support, comments, schema validation and
  copy-paste ergonomics?
- Which structure is easiest for these cases:
  - enable a small named rule set;
  - disable one noisy scenario from the default set;
  - configure one scenario with several settings;
  - define path-specific/project-specific overrides;
  - run separate pre-commit hook variants such as quick checks, full checks and
    fixers;
  - migrate from an existing `v8config.json` without losing compatibility;
  - explain unsupported or legacy `.os` scenarios clearly.
- How should native config relate to CLI `--rules`, `--source-dir` and
  pre-commit hook arguments?
- Should there be profiles, rule groups, severity levels, explicit fixer/checker
  modes, or only scenario enable/disable lists?
- What migration path should exist between `v8config.json` compatibility config
  and the native config, if both are present?

Acceptance criteria:

- Produce a short design note comparing at least JSON, TOML and YAML for native
  `prec-bsl` configuration.
- Include concrete examples for default adoption, minimal opt-out, strict CI,
  path-specific overrides, and migration from `v8config.json`.
- State which layer owns each concern: hook manifest, end-user pre-commit config,
  native `prec-bsl` config, and compatibility `v8config.json`.
- Recommend one native format and one scenario-settings structure, with explicit
  tradeoffs.
- Preserve `v8config.json` as a compatibility input in the recommendation.
- Do not implement parser, CLI or behavior changes as part of this task unless a
  later implementation task is created and approved.

Validation:

- Design note reviewed against `spec/configuration.md` and
  `spec/prd-prec-bsl.md`.
- No code changes required.

Dependencies:

- T6.

## Milestone 14: prec-bsl Issue Scenarios

Implementation work triggered by `alkoleft/prec-bsl` GitHub issues that extend
the scenario set beyond the v1 reference inventory.

### T51. DONE: Implement `ПроверкаДлиныСтрок` (LineLength) check-only scenario

prec-bsl issue: `alkoleft/prec-bsl` #4 (part of the Sonar/bsl-ls scenario
tracking issue #3).

Current finding:

- The scenario set lacked a line-length diagnostic equivalent to the
  `bsl-language-server` LineLength check, and the issue also required the
  line-break semantics for authors to be captured in spec.

Resolution (implemented per the contract in `spec/parser-strategy.md`,
section "LineLength checker contract"):

- Lexical check-only scenario in `crates/prec-bsl-bsl/src/text_checkers.rs`:
  pure detector `find_long_lines` counting Unicode scalars after
  `trim_end()`, strictly `>` limit, BOM stripped on line 1, `\r` excluded,
  multiline literal `|`-continuations skipped, code after a closing quote on
  a continuation line counted (lone `;` excluded).
- Setting `НастройкиСценариев.ПроверкаДлиныСтрок.МаксимальнаяДлинаСтроки`
  (`serde_json` positive integer, default 150, validated in the handler).
- Stable EN message `String length {len} exceeded maximum {limit} at line
  {n}` with Russian translation in `text_message`; Warning severity
  (check exit 1, hook not blocked).
- Registered with `ScenarioSupport::Compatibility` (opt-in through
  `ГлобальныеСценарии`), captured in `spec/reference-scenario-inventory.md`.
- Line-break rules for authors documented in `spec/line-break-rules.md`
  (advisory, canon yaxunit PR #708 / commit `79d950ae`); enforcement is the
  prospective issue #5 (`IncorrectLineBreak`) work, not part of this task.

Acceptance criteria:

- Long lines produce Warning diagnostics with length, limit, and 1-based
  line number; check exits 1; files (including CRLF) are never modified.
- The scenario is opt-in and absent from default configuration.
- Unit tests cover the counting semantics; integration tests cover
  pipeline/CLI contracts and setting validation.
- Line-break rules are captured in `spec/line-break-rules.md`.

Validation:

- `cargo test -p prec-bsl-bsl --lib`
- `cargo test --test line_length`
- `cargo test --workspace`

Completion evidence:

- 2026-10-30: Detector, handler, and unit tests landed in commit `b4b28f0`;
  registry registration and message translations in commit `36a37a5`;
  integration/CLI tests in commit `1f25c43`; rustfmt pass in commit
  `d65ff99`; specs in commit `ed91514` (branch `issue-4-line-length`).
  Implementation plan (kept in the main checkout outside version control,
  `docs/superpowers/plans/`): `2026-10-30-issue-4-line-length.md`.
- 2026-10-30: bsl-ls parity snapshot validation landed with the committed
  baseline `tests/fixtures/line-length/bsl-ls-line-length-baseline.json`
  (58 diagnostics, bsl-ls v1.0.7, yaxunit `d21ce6ff`, limit 150) and the
  env-gated `tests/line_length_parity.rs` (run with
  `PREC_BSL_PARITY_CORPUS=<yaxunit checkout>`; skips on corpus drift). The
  validation run found and fixed a real detector gap: lone `\r` (CR-only
  legacy Designer endings) is now a line terminator matching the bsl-ls
  reader. Final parity: 58/58 fingerprints matched, 0 missing, 0 unexpected
  over all 350 tracked `.bsl` files.

Dependencies:

- T38, T39 (crate layout and scenario definition binding).

### T52. DONE: Implement `ОдинОператорНаСтроку` (OneStatementPerLine) check-and-fix scenario

prec-bsl issue: `alkoleft/prec-bsl` #6 (part of the Sonar/bsl-ls scenario
tracking issue #3). Implementation plan (kept in the main checkout outside
version control, `docs/superpowers/plans/`):
`2026-10-30-issue-6-one-statement-per-line.md`, tasks 1-5.

Current finding:

- The scenario set lacked a one-statement-per-line diagnostic and fixer
  equivalent to the `bsl-language-server` OneStatementPerLine check, with
  the fix indent contract (+1 tab after `Тогда`) from the issue.

Resolution (implemented per the contract in `spec/parser-strategy.md`,
section "One statement per line contract"):

- Tree-sitter detection transliterating the bsl-ls
  `OneStatementPerLineDiagnostic` `previousLineNumber` semantics:
  statement walk in document order, violation when a statement's start row
  equals the previous statement's start row, first statement on a line
  never flagged; preprocessor statements, exception/ERROR-reset forms and
  bare `;` reset the tracking and are not flagged; `elseif_clause` is a
  track-only slot that is never flagged itself. Parity proven in module
  tests against both bsl-ls test fixtures (5 and 2 violations with exact
  LSP ranges).
- Text-splice fixer moving every flagged statement to its own line:
  pattern 1 = statement line indent byte-for-byte; pattern 2 (statement on
  a `Тогда` row of `Если`/`ИначеЕсли`) = anchor line indent + one tab;
  mixed chains after a block opener all get anchor + 1 tab; `Иначе` is not
  pattern 2; the edit replaces the whitespace prefix with the file line
  ending + indent (no trailing whitespace), multiline statements move
  whole, `;` is never added or removed, empty statements produce no blank
  lines; second run is a no-op (idempotence).
- Parse-error policy: known safe ERROR forms do not block; other errors
  intersecting violation rows degrade to a Warning result without
  modification.
- Handler: check + fix in one pass through `apply_text_modification`
  (`WritePolicy::DryRun` gives check mode: a single dry-run result with
  proposed content and the violating rows enumerated). Non-BSL files are
  skipped.
- Registered with `ScenarioSupport::Compatibility` (opt-in through
  `ГлобальныеСценарии`, `.os`-suffix lookup supported), captured in
  `spec/reference-scenario-inventory.md`.
- Deliberate non-parity with bsl-ls is fixed in the contract: fix indent
  after `Тогда` = +1 tab (vs the bsl-ls quickfix same-indent), `;` not
  added to moved statements (issue #10 territory), check mode = single
  dry-run result (not per-statement diagnostics).

Acceptance criteria:

- Detection parity with bsl-ls OneStatementPerLine on both reference
  fixtures (embedded verbatim in unit tests).
- Hook mode modifies and reports the violating rows; the second hook run
  is clean; check mode never writes files.
- Golden fixtures cover chain, if-then, mixed, crlf, spaces, multiline and
  strings-comments cases with idempotence.
- The scenario is opt-in and absent from default configuration.

Validation:

- `cargo test -p prec-bsl-bsl one_statement`
- `cargo test --test one_statement_per_line`
- `cargo test --workspace`

Completion evidence:

- 2026-10-30: grammar facts gate (plan Task 1, tree-dump artifact outside
  git) confirmed the 0.1.6 tree shapes and proved the algorithm parity by
  simulation before implementation. Finder and unit tests landed in commit
  `063ad96` (test rename follow-up `5a8a3ac`); fixer in commit `4c3959f`;
  handler, registration and integration/golden tests in commit `2746317`; CR-only guard in commit `783fc9c`;
  specs in this commit (branch `issue-6-one-statement-per-line`).
- 2026-10-30 (tree-sitter-bsl 0.1.7 adaptation, Cargo.lock bump kept):
  probed 0.1.7 deltas with the standalone probe crate
  (`/tmp/opencode/dateprobe2/probe`, depends on the workspace crate):
  date literals parse as a clean `date` node; a bare `;` is a real
  `_empty_statement` node (kind `";"`, prec(-1)) in statement position,
  not an ERROR; `#Область … #КонецОбласти` parses as ONE `preprocessor`
  node whose body holds the region's statements (the old walk returned
  after the region reset and lost the region-body violations — main
  bsl-ls fixture regained rows 8/8); `while_statement` still lacks
  `optional(';')` (its trailing `;` is a sibling node and must not reset
  tracking); the trailing `;` of if/try/for/for_each is held inside the
  compound node after `ENDIF/ENDTRY/ENDDO` and must not be treated as an
  empty statement. `call_statement` was already in STATEMENT_KINDS (the
  task prompt's claim was wrong — no change needed). Module updated:
  `preprocessor` added to containers with a fall-through descent after
  the region reset; `is_empty_statement` decides statement position via
  parent container kind plus prev-sibling exclusions
  (`COMPOUND_TERMINATOR_KEYWORDS`, `while_statement`). ANTLR/bsl-ls
  traces: `А=1; А=2;; А=3;` yields exactly one violation (`А=3`) and the
  fix moves only it — replacing the old 0.1.6 guess that moved `А=1`.
  Parse-error integration input replaced with the unterminated-procedure
  form (`Процедура А()\nА=1; А=2;`): ERROR region led by
  PROCEDURE_KEYWORD, not compound-led, no PREPROC children → relevant.
  The 0.1.7 bump also changed `ПроверкаКорректностиИнструкцийПрепроцессора`
  and `ПроверкаКорректностиОбластей`: unterminated `#Если` blocks and
  stray region directives now parse as single ERROR regions led by the
  directive keyword, and the tree walk double-reported the lexical
  balance diagnostics — both checkers now skip those ERROR regions (the
  balance stack owns the directive family; unit RED/GREEN tests added in
  `bsl_checkers.rs`). Gates: `cargo test --workspace` 435 passed / 0
  failed; `cargo fmt --check` clean; clippy introduces no new warnings
  in touched files (one `needless_option_as_deref` in new code fixed).
- 2026-10-30 (post-merge review of PR #18, branch
  `fix-ospl-review-findings`): two confirmed fixer bugs, both reproduced
  by RED tests before the fix. (1) A UTF-8 BOM before an indented row-0
  chain broke the indent scan (`line_start`/`indent_end`/`prefix_start`
  stopped at the BOM bytes): the moved statement was dedented. Row 0 now
  computes `indent_base` after a leading `\xEF\xBB\xBF`, keeping the BOM
  byte-for-byte and the indent after it. (2) `block_opener_rows` was
  row-based: with ordinary statements before an inline
  `Если … Тогда` (`А=1; Б=2; Если В Тогда Г=3;`) every violation on the
  row wrongly got the extra tab. The extra tab is now scoped to the
  opener chain: the pattern-2 statement and later violations on the row
  (byte-start >= the first pattern-2 violation's start on that row);
  earlier statements keep the line indent. Detection semantics unchanged
  (both bsl-ls parity fixtures still 5/2). Gates: `cargo test -p
  prec-bsl-bsl one_statement` 36/36 (4 new), `cargo test --workspace`
  439 passed / 0 failed, `cargo fmt --check` clean, clippy clean in the
  touched file. Idempotence loop extended with both review inputs.

Dependencies:

- T38, T39 (crate layout and scenario definition binding).

### T53. DONE: Enable all supported scenarios in built-in defaults

Maintainer decision (2026-10-30, branch `default-all-supported-scenarios`):
users get every supported scenario out of the box instead of the required v1
subset only.

Resolution:

- `built_in_defaults_with_catalog` and the mirrored
  `RawScenarioConfig::into_domain` fallback (config file without the
  `ГлобальныеСценарии` key) now enable `catalog.supported()` = RequiredV1 +
  Compatibility, so `СортировкаДереваМетаданных`, `СортировкаСоставаПодсистем`,
  `ПробелыВКомментариях`, `ПробелыПослеЗапятых`, `ПроверкаДлиныСтрок` and
  `ОдинОператорНаСтроку` run without opting in.
- Unsupported scenarios (`РазборОбычныхФормНаИсходники`, `ОбработкаЮнитТестов`,
  platform-dependent/repo-local ids) stay excluded from defaults.
- Opt-out paths unchanged: `ОтключенныеСценарии` subtracts at every level,
  explicit `ГлобальныеСценарии` (including `[]`) still narrows/disables;
  project sections keep inheriting the default set.
- Specs rewritten: `spec/configuration.md` (built-in defaults paragraph),
  `spec/reference-scenario-inventory.md` (extension-scenario registration
  contract), README configuration section.

Acceptance criteria:

- Built-in defaults contain exactly RequiredV1 + Compatibility and no
  unsupported ids (unit test over `EXPECTED_REQUIRED_V1` +
  `EXPECTED_COMPATIBILITY`).
- A hook run with built-in defaults fixes `ОдинОператорНаСтроку` violations;
  `ОтключенныеСценарии` turns it off without restating the whole list.
- Tests pinning explicit-config narrowing semantics (e.g. a non-empty
  `ГлобальныеСценарии` list without the scenario) keep passing unchanged.

Validation:

- `cargo test --workspace` (436 passed / 0 failed at completion)
- `cargo fmt --check`
- clippy: no new warnings in touched files

Dependencies:

- T38, T39, T51, T52 (scenario registry and the extension scenarios that are
  now on by default).
