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

## Milestone 17: CLI Rule Exclusions

Implementation work for the `--exclude-rules` CLI flag, tracked by
`alkoleft/prec-bsl` issue #14. Completed tasks move to `spec/archive/`.

### T56. DONE: Implement `--exclude-rules` for the `check`/`fix` commands

prec-bsl issue: `alkoleft/prec-bsl` #14. Implementation plan (kept in the main
checkout outside version control, `docs/superpowers/plans/`):
`2026-11-01-issue-14-exclude-rules.md`.

Current finding:

- Users need a one-off way to exclude a noisy rule from a run without
  rewriting the persistent `v8config.json` or restating the full `--rules`
  list: `--rules "А,Б,В" --exclude-rules "Б"` must run `А,В`.

Contract (specified in `spec/configuration.md` — Layer 2 allowed args, CLI
signatures, "Configuration Precedence", "Validation Rules" and "Decision
Table" — before implementation):

- The flag is accepted only by `check`/`fix`; `prek-hook` and `exec-rules`
  reject it as an unknown parameter.
- CLI exclusions subtract from the effective (resolved) rule list after
  `--rules` and independently of `ОтключенныеСценарии`; the root list and
  every project `Some`-section are subtracted, `None`-sections inherit the
  subtracted root.
- Unknown (`None`) and unsupported names in the exclude list are validation
  errors (exit `2`), not warnings: CLI input is typed right now, so a typo
  would silently return the excluded rule to the run (persistent-config
  disabled lists keep legacy-name warnings).
- Excluding a known rule that is not in the current set is a silent no-op;
  excluding every rule of the set leaves an empty set and `check`/`fix` exit
  `0` (deliberate opt-out).

Acceptance criteria (from issue #14):

- `--exclude-rules` removes the rule from the default set, from an explicit
  `--rules` list, and stacks with `ОтключенныеСценарии`.
- `prek-hook`/`exec-rules` reject the flag with the existing
  «неизвестный параметр» error.
- Resolve-level unit tests plus `check`/`fix` e2e tests pin the behavior.

Validation:

- `cargo test -p prec-bsl-config --lib`
- `cargo test --test check_fix_modes`
- `cargo test --workspace`
- `cargo clippy --all-targets -- -D warnings`
- `cargo fmt --check`

Dependencies:

- T53 (built-in defaults that the exclusion subtracts from).

## Milestone 16: PR Review Fix Backlog

Review-driven fixes for merged scenario work. Each record references the
pull request that triggered it and is removed to `spec/archive/` once the
fix and its verification land.

### T55. TODO: Review fixes PR #21: trailing comma region gap validation, no-retry, indent preservation

prec-bsl issue: `alkoleft/prec-bsl` #9 (review findings on the PR #21
implementation of the `ЛишниеЗапятыеВКонце` fixer).

Current finding:

- The merged trailing comma region scan accepted any bytes between the
  previous region element and a `//` comment (a `#` directive or a dangling
  `|` line could precede the comment inside a region), retried commas of a
  rejected region as fresh start candidates, and deleted the whole
  whitespace gap between a comment and the closing `)` — collapsing the
  leading indent of the closing line.

Acceptance criteria:

- The gap between a comma/previous region element and a `//` comment is
  verified as whitespace-only: a non-whitespace gap (`#` directives,
  dangling `|` lines) rejects the whole region and its commas are not
  removed.
- Commas of a rejected region are never retried as start candidates
  (no-retry: the scan walks the rejected region to its end and the outer
  loop resumes after the last examined comma).
- The gap between a comment and the closing `)` is verified but not
  deleted: the leading indent of the closing `)` line is preserved; the gap
  after a comma is still deleted as one whitespace span.
- The review example pairs stay unchanged:
  `Метод(А,\n#Если Сервер Тогда\n#КонецЕсли\n// к\n)` and
  `Метод(А,\n#Если Сервер Тогда\n#КонецЕсли\n, )`.
- The fixers stay idempotent on the fixed output.

Validation:

- `cargo test -p prec-bsl-bsl --lib`
- `cargo test --test trailing_commas`
- `cargo test --workspace`

Dependencies:

- T54 (the fixers being fixed; archived).

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

### T56. DONE: Implement `ПроверкаТочекЗапятой` (SemicolonPresence) check-and-fix scenario

prec-bsl issue: `alkoleft/prec-bsl` #10 (part of the Sonar/bsl-ls scenario
tracking issue #3).

Scope: tree-sitter check (per-statement Warning diagnostics on statements
missing the terminating `;`, bsl-ls `SemicolonPresence` parity including
compound statement ends) + insertion-only fix (`;` inserted at the
stop-token end before trailing comments), registered as a
`ScenarioSupport::Compatibility` extension and enabled in built-in defaults.

Contract: `spec/parser-strategy.md` → "Semicolon presence contract
(`ПроверкаТочекЗапятой`)" (detector scope, stop-token span/line, while-sibling
terminator in both `;` forms, mode-specific parse-error policy, idempotent
insertion-only fix, non-parity list, parity scope statement).

Plan: `docs/superpowers/plans/2026-10-31-issue-10-semicolon-presence.md`
(working tree only, not committed).

Acceptance criteria (from the issue and plan):

- `Возврат "Булево"` / `Возврат СписокОбластей` without `;` (EOF form, the
  issue's control cases): `check` reports a Warning with the stop-token line;
  `fix` appends `;`; a second run is clean and idempotent.
- Verbatim bsl-ls fixture `SemicolonPresenceDiagnostic.bsl`: exactly 2
  diagnostics on the bsl-ls ranges (`А = 0` stop token, `КонецЕсли`).
- Fingerprint parity with bsl-ls v1.0.7 on the pinned yaxunit corpus
  (`tests/fixtures/semicolon-parity/bsl-ls-semicolon-baseline.json`),
  0 unexpected / 0 missing outside documented residue classes.
- Post-fix bsl-ls pass over a disposable corpus clone: `SemicolonPresence`
  is zero outside refused-by-policy files and residue classes; no new
  foreign diagnostics.
- Fix-order convergence with `ОдинОператорНаСтроку` in both permutations.

Dependencies: T38/T39 (registry), T52 (OSPL — statement-walking pattern and
`has_lone_cr` extraction), T53 (built-in defaults include the new scenario
automatically).

Resolution (2026-10-31):

- Implemented in `crates/prec-bsl-bsl/src/semicolon_presence.rs` (detector
  `find_missing_semicolons`, insertion-only `fix_missing_semicolons`,
  whole-file `has_unsafe_parse_errors`, two-mode handler) and registered in
  `src/lib.rs` (definitions, metadata, `EXPECTED_COMPATIBILITY`,
  built-in defaults).
- All acceptance criteria verified with green runs: workspace tests, fmt,
  clippy clean on the touched files; bsl-ls v1.0.7 parity on the pinned
  yaxunit commit — 67/67 diagnostics matched (0 unexpected / 0 missing,
  residue classes empty), post-fix `SemicolonPresence` 0 with no new stable
  diagnostics; fix-order convergence with `ОдинОператорНаСтроку` in both
  permutations; idempotence including the EOF form, BOM and CRLF.
- Contract details and the parity snapshot numbers live in
  `spec/parser-strategy.md` → "Semicolon presence contract
  (`ПроверкаТочекЗапятой`)".

Deferred DRY (from the contract's note), inventory of what is duplicated
from `one_statement_per_line.rs` and why it stays:

- `SEMICOLON_STATEMENT_KINDS` vs OSPL `STATEMENT_KINDS`: the sets differ
  (`semicolon` excludes `var_statement` for bsl-ls `subVar` parity and has
  no use for several OSPL-only kinds) — justified duplication.
- statement-walking skeleton (document-order descent, container kinds):
  OSPL's walker is fused with its `previousLineNumber` event semantics;
  extraction would not remove real duplication yet — justified.
- grammar facts (safe ERROR shapes, bare-`;` forms): boundary case; kept
  local until the third tree-sitter statement-walking scenario.
- `has_lone_cr` was extracted to `bsl_parser` (`pub(crate)`) and shared by
  both scenarios (done during this task).
- First extraction hazard to watch: OSPL rows are 0-based, semicolon lines
  are 1-based — any shared walker must reconcile this first.
- Extraction trigger: the third tree-sitter statement-walking scenario.

