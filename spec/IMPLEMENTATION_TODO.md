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

## Milestone 11: Upstream Issue Regression Backlog

These tasks capture risks found during the 2026-05-07 review of open
`precommit4onec` GitHub issues. They are tracked as explicit parity/regression
work so upstream problems are either fixed in `prec-bsl` or intentionally kept
outside the Rust v1 scope.

Completed task T50 (resolved 2026-10-29, `УдалениеЛишнихПустыхСтрок` trailing
blank run at EOF, `alkoleft/prec-bsl` #11) is archived with the Milestone 11
history below and in `spec/archive/completed-tasks-t1-t43-t47.md`.

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
  handler, registration and integration/golden tests in commit `2746317` (reworded from `3c4eccd`);
  specs in this commit (branch `issue-6-one-statement-per-line`).

Dependencies:

- T38, T39 (crate layout and scenario definition binding).
