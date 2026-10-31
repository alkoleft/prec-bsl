# Completed implementation tasks: T1-T43, T47-T51, T54

Archived from `spec/IMPLEMENTATION_TODO.md` on 2026-05-26 to keep the active
implementation ledger focused on open work. Task text below preserves the
original acceptance criteria, validation notes, completion evidence and
dependencies. Records completed later (T50 on 2026-10-29, T51 and T54 on
2026-10-31) are appended below in their original form.

## Milestone 0: Bootstrap Hook Package

### T1. DONE: Define public hook manifest

Create the root `.pre-commit-hooks.yaml` manifest.

Acceptance criteria:

- Hook id `prec-bsl` exists.
- Hook uses `language: rust`.
- Hook entry is `prec-bsl prek-hook`.
- Hook uses `always_run: true` and `pass_filenames: false`.
- Manifest does not embed project-specific `v8config.json` settings.

Validation:

- `test -f .pre-commit-hooks.yaml`
- `rg -n "prec-bsl|language: rust|prek-hook|pass_filenames: false" .pre-commit-hooks.yaml`

Completion evidence:

- 2026-05-06: Added `.pre-commit-hooks.yaml` with `prec-bsl` Rust hook entry
  `prec-bsl prek-hook`, `always_run: true`, and `pass_filenames: false`.

Dependencies:

- None.

### T2. DONE: Add CLI skeleton and command contract

Replace the placeholder binary with a CLI that exposes the v1 command surface.

Acceptance criteria:

- `prec-bsl prek-hook [--config <path>] [--source-dir <path>] [--rules <list>] [--format text|json]` exists.
- `prec-bsl exec-rules <repo> [--config <path>] [--source-dir <list>] [--rules <list>] [--format text|json]` exists.
- `--format` accepts `text` and `json`.
- Unknown commands and invalid arguments return non-zero with a clear message.

Validation:

- `cargo test`
- `cargo run -- --help`
- `cargo run -- prek-hook --help`
- `cargo run -- exec-rules --help`
- A command-contract test asserts that `exec-rules` accepts positional `<repo>`.

Completion evidence:

- 2026-05-06: Replaced the placeholder binary with a CLI skeleton for
  `prek-hook` and `exec-rules`; added focused command-contract tests covering
  public options, `exec-rules <repo>`, invalid `--format`, unknown commands, and
  missing `<repo>`.

Dependencies:

- T1 can be implemented independently, but CLI names must match the manifest.

## Milestone 1: Parity Baseline

### T3. DONE: Capture reference precommit4onec scenario inventory

Record the reference scenario names, unsupported decisions, and execution-order
evidence needed for parity work.

Acceptance criteria:

- Required v1 scenarios are represented in code-facing fixtures or test data.
- `РазборОбычныхФормНаИсходники` is explicitly represented as unsupported.
- Scenario ids can be matched with and without `.os` suffix.
- The historic config key `Precommt4onecСценарии` is preserved in fixtures.

Validation:

- `cargo test scenario`
- `rg -n "Precommt4onecСценарии|РазборОбычныхФормНаИсходники" .`

Completion evidence:

- 2026-05-06: Added `spec/reference-scenario-inventory.md`,
  scenario metadata now collected by the root facade catalog in `src/lib.rs`, and
  `tests/fixtures/scenario_inventory/reference-v8config.json` to preserve the
  reference `precommit4onec` scenario order, required v1 list, `.os`
  normalization, explicit unsupported `РазборОбычныхФормНаИсходники`, and
  historic `Precommt4onecСценарии` fixture key.
- Verification passed: `cargo test scenario`, `cargo test`, and
  `rg -n "Precommt4onecСценарии|РазборОбычныхФормНаИсходники" .`.

Dependencies:

- T2.

### T4. DONE: Build golden fixture harness

Add a fixture test harness that can compare input, output, diagnostics, and
idempotence for fixer and checker scenarios.

Acceptance criteria:

- Fixtures support Cyrillic paths and scenario names.
- Fixer tests can assert first-run modifications and second-run clean state.
- Checker tests can assert diagnostics without modifying input.
- Expected output files are deterministic and reviewable in Git.

Validation:

- `cargo test fixtures`

Completion evidence:

- 2026-05-06: Added `tests/support` golden fixture helpers and
  `tests/fixture_harness.rs` contract tests for Cyrillic fixer/checker fixtures.
  The harness compares input, expected output, expected diagnostics, and fixer
  idempotence while checker fixtures assert no input modification.
- Verification passed: `cargo test fixtures` and `cargo test`.

Dependencies:

- T3.

### T5. DONE: Add RAT acceptance harness with copy-only safety

Add tests or scripts that use `/home/alko/develop/open-source/rat` as a
read-only external corpus.

Acceptance criteria:

- Tests never mutate the real RAT checkout.
- Required source roots can be copied into a temp directory.
- RAT config parsing is tested against the live `v8config.json`.
- Repository-local unsupported scenarios are reported clearly.
- Mutating checks run only against temporary copies.

Validation:

- `cargo test rat`
- Manual safety check:
  `GIT_OPTIONAL_LOCKS=0 git -C /home/alko/develop/open-source/rat status --short`

Completion evidence:

- 2026-05-06: Added RAT acceptance tests that copy required source roots into
  `target/rat-acceptance` temporary directories, verify BSL/EDT source files
  and Cyrillic paths are preserved in the copy, and assert the real RAT checkout
  status is unchanged after the test.
- Added live `rat/v8config.json` parsing coverage for `GLOBAL`,
  `Precommt4onecСценарии`, disabled `РазборОбычныхФормНаИсходники`, and clear
  diagnostics for enabled repository-local scenarios whose dynamic `.os`
  execution is unsupported in v1.
- Verification passed: `cargo test rat`, `cargo test`, `git diff --check`, and
  manual safety check
  `GIT_OPTIONAL_LOCKS=0 git -C /home/alko/develop/open-source/rat status --short`
  with the pre-existing `?? .build/` entry only.

Dependencies:

- T4.

Non-goals:

- A full green run of live RAT `v8config.json` with repository-local `.os`
  scenarios.

## Milestone 2: Core Architecture Layers

### T6. DONE: Implement configuration model and resolver

Implement parsing and resolution for the three-layer configuration contract.

Acceptance criteria:

- CLI `--config` overrides default discovery.
- Default discovery checks `v8config.json` in repository root.
- Built-in defaults are available when no config exists.
- `GLOBAL` and `Precommt4onecСценарии` parse successfully.
- Global scenarios, disabled scenarios, scenario settings, repository scenario
  settings, and project overrides are represented.
- Project-specific settings fully override base settings for matching source
  subpaths.
- Enabled `РазборОбычныхФормНаИсходники` fails validation with a clear message.

Validation:

- `cargo test config`

Completion evidence:

- 2026-05-06: Added `src/config.rs` with typed `v8config.json` parsing,
  default discovery from repository root, explicit `--config` path precedence,
  built-in defaults, optional CLI rule override resolution, normalized scenario
  ids, repository scenario settings, base/project scenario settings, and
  project-path override lookup.
- Validation now rejects enabled `РазборОбычныхФормНаИсходники` and enabled
  repository-local scenarios with clear diagnostics, while unknown disabled
  scenario ids remain parse-compatible warnings for `0.x`; credential-like
  keys and non-repository-relative local-scenario/project paths are rejected.
- Verification passed: `cargo test config` and `cargo test`.
- 2026-05-26: Clarified and fixed the optional default config contract:
  missing repository `v8config.json` and missing explicit `--config v8config.json`
  use built-in defaults, while missing custom explicit config paths remain hard
  errors.

Dependencies:

- T3.

### T7. DONE: Implement Git index collection and restaging layer

Implement Git staged-file discovery and restaging as a separate module.

Acceptance criteria:

- Uses `git diff --name-status --staged --no-renames`.
- Maps Git statuses to added, modified, deleted, renamed, copied, and unknown.
- Uses `core.quotePath=false` or equivalent handling for Cyrillic paths.
- Deleted files are represented without requiring file contents.
- Modified and generated paths can be restaged after fixer scenarios.
- Git failures are reported as blocking diagnostics.

Validation:

- `cargo test git_index`

Completion evidence:

- 2026-05-06: Added `src/git_index.rs` as a separate Git boundary module with
  typed staged-file statuses, `git -c core.quotePath=false diff --name-status
  --staged --no-renames` collection, Git-quoted path decoding for Cyrillic and
  quoted paths, explicit deleted-file representation, literal-pathspec
  `git add --` restaging for modified/generated paths, and blocking diagnostics
  for Git failures.
- Verification passed: `cargo fmt --check`, `cargo test git_index`, and
  `cargo test`.

Dependencies:

- T2.

### T8. DONE: Implement source root and file classification

Add source-root resolution and file type classification for Designer, EDT, and
external artifacts.

Acceptance criteria:

- Supports explicit `--source-dir`.
- Supports multiple source roots for `exec-rules`.
- Classifies `.bsl`, `.mdo`, `.form`, `Configuration.mdo`, XML metadata, and
  unsupported files.
- Preserves source-root context per file.
- Missing source roots are reported.

Validation:

- `cargo test source_root`
- `cargo test file_classification`

Completion evidence:

- 2026-05-07: Added `src/source_files.rs` as a separate source-root and file
  classification boundary with comma-separated source-dir parsing, explicit and
  default source-root resolution, blocking diagnostics for missing roots,
  canonicalized in-repository root context with blocking diagnostics for roots
  outside the repository, recursive source traversal for multiple `exec-rules`
  roots, staged-file classification without requiring deleted-file contents,
  source-root context preservation per file, and file kind classification for
  BSL, EDT `.mdo`, EDT `.form`, `Configuration.mdo`, XML metadata, and
  unsupported files.
- Verification passed: `cargo test source_root` and
  `cargo test file_classification`.

Dependencies:

- T6.

### T9. DONE: Implement scenario registry and execution pipeline

Create the shared scenario registry, execution context, queue, and result model.

Acceptance criteria:

- Scenario lookup accepts names with and without `.os`.
- Pipeline respects configured scenario enable/disable decisions.
- Enabled scenarios execute in the resolved configured order captured by parity
  fixtures.
- Post-processing files can be appended to the processing queue.
- Scenario results distinguish modifications, warnings, hard failures, skips,
  and unsupported scenarios.
- Critical errors are accumulated and printed after traversal.
- Hook mode returns `0` only when no blocking diagnostics and no unreviewed
  modifications remain.

Validation:

- `cargo test scenario_pipeline`
- A scenario-order contract test asserts normalized `.os` and non-`.os` ids keep
  the resolved execution order.

Completion evidence:

- 2026-05-07: Added `src/scenario_pipeline.rs` with a reference scenario
  registry, normalized lookup, execution context, post-processing queue,
  scenario result statuses for modifications, warnings, hard failures, skips
  and unsupported scenarios, critical-result accumulation, modified-path
  aggregation, and hook/exec-rules exit-code aggregation. Required scenarios
  without implementation handlers fail closed as hard failures instead of
  passing silently.
- Added explicit deleted-file dispatch gating: scenarios receive deleted files
  only when registered with deleted-file capability; other scenarios produce a
  skip result without requiring file contents or invoking the handler.
- Added focused `scenario_pipeline` tests for normalized `.os`/non-`.os`
  execution order, project-specific scenario order, post-processing queue
  append behavior, result status distinctions, traversal continuing after hard
  failures, unregistered scenario diagnostics, deleted-file dispatch gating,
  and hook/exec-rules exit aggregation.
- Verification passed: `cargo fmt --check`, `cargo test scenario_pipeline`,
  and `cargo test`.

Dependencies:

- T6, T7, T8.

### T10. DONE: Implement diagnostics and output formats

Add text and JSON output for hook and CI use.

Acceptance criteria:

- Text output groups messages by rule and file.
- Modified files are listed separately from hard failures.
- JSON output includes rule id, path, severity, modification flag, message, and
  source span when available.
- Output is deterministic for stable test snapshots.

Validation:

- `cargo test output`

Completion evidence:

- 2026-05-07: Added `src/output.rs` as a separate output boundary for
  rendering `PipelineReport` in deterministic text and JSON formats. Text
  output groups messages by rule and file, lists modified files separately
  from hard failures, and includes the computed exit code. JSON output includes
  rule id, path, severity, modification flag, message, modified paths, and
  optional source byte spans.
- Added optional `SourceSpan` support to `ScenarioResult` for future
  parser-backed diagnostics without changing the output boundary.
- Verification passed: `cargo fmt --check`, `cargo test output`, and
  `cargo test`.

Dependencies:

- T9.

## Milestone 3: Low-Risk Text Fixers and Checks

### T11. DONE: Implement trailing whitespace fixer

Scenario: `УдалениеЛишнихКонцевыхПробелов`.

Acceptance criteria:

- Removes trailing spaces and tabs from ordinary code lines without changing
  unrelated text.
- Preserves whitespace-only alignment lines and BSL comment text, including
  inline `//` comments.
- Preserves line endings according to fixture expectations.
- Is idempotent.
- Reports modified files.

Validation:

- `cargo test trailing_whitespace`

Completion evidence:

- 2026-05-07: Added `src/text_fixers.rs` with the lexical
  `УдалениеЛишнихКонцевыхПробелов` implementation, registered it in the
  reference scenario registry, and added `tests/trailing_whitespace.rs` coverage
  for the Cyrillic golden fixture, modified-file reporting, hook-mode blocking
  after unreviewed modifications, second-run idempotence, and LF/CRLF
  preservation.
- 2026-05-26: Updated the fixer contract to preserve whitespace-only alignment
  lines and BSL comments while continuing to trim ordinary code lines.
- Verification passed: `cargo fmt --check`, `cargo test trailing_whitespace`,
  and `cargo test`.
- Independent reviewer pass returned `APPROVED` with no missing verification.

Dependencies:

- T4, T9.

### T12. DONE: Implement extra blank line fixer

Scenario: `УдалениеЛишнихПустыхСтрок`.

Acceptance criteria:

- Removes excessive blank lines according to parity fixtures.
- Preserves meaningful module spacing.
- Is idempotent.

Validation:

- `cargo test empty_lines`

Completion evidence:

- 2026-05-07: Added the lexical `УдалениеЛишнихПустыхСтрок` implementation to
  `src/text_fixers.rs`, registered it in the reference scenario registry, and
  added `tests/empty_lines.rs` plus Cyrillic golden fixtures covering
  excessive blank-line removal, preservation of meaningful single blank-line
  spacing, CRLF behavior, modified-file reporting, hook-mode blocking after
  unreviewed modifications, and second-run idempotence.
- Verification passed: `cargo fmt --check`, `cargo test empty_lines`, and
  `cargo test`.

Dependencies:

- T11.

### T13. DONE: Implement keyword spacing fixer

Scenario: `ДобавлениеПробеловПередКлючевымиСловами`.

Acceptance criteria:

- Matches parity fixtures for required BSL keywords.
- Avoids changes inside string literals and comments where parity requires.
- Is idempotent.

Validation:

- `cargo test keyword_spacing`

Completion evidence:

- 2026-05-07: Added the lexical
  `ДобавлениеПробеловПередКлючевымиСловами` implementation to
  `src/text_fixers.rs`, registered it in the reference scenario registry, and
  added `tests/keyword_spacing.rs` plus Cyrillic golden fixtures covering
  `Экспорт` spacing after a closing parenthesis, mixed-case keyword
  preservation, single-line and multiline comment/string safety,
  modified-file reporting, hook-mode blocking after unreviewed modifications,
  and second-run idempotence.
- Verification passed: `cargo fmt --check`, `cargo test keyword_spacing`, and
  `cargo test`.

Dependencies:

- T12.

### T14. DONE: Implement canonical spelling fixer

Scenario: `ИсправлениеНеКаноническогоНаписания`.

Acceptance criteria:

- Normalizes known non-canonical keyword spellings from fixtures.
- Handles Russian and English spellings covered by specs.
- Avoids string/comment rewrites unless parity requires them.
- Is idempotent.

Validation:

- `cargo test canonical_spelling`

Completion evidence:

- 2026-05-07: Added the lexical
  `ИсправлениеНеКаноническогоНаписания` implementation to `src/text_fixers.rs`,
  registered it in the reference scenario registry, and added
  `tests/canonical_spelling.rs` plus Cyrillic golden fixtures covering Russian
  and English keyword spellings, string/comment safety, modified-file reporting,
  hook-mode blocking after unreviewed modifications, and second-run idempotence.
- Added explicit reference-keyword-scope coverage for directives, annotations,
  platform contexts, loops, declarations, logical aliases, literals and handler
  keywords; accepted reference aliases such as `Или`, `Не`, `ИСТИНА`, `ЛОЖЬ`,
  `ЗНАЧ`, `НЕОПРЕДЕЛЕНО`, and `Null` remain unchanged.
- Verification passed: `cargo fmt --check`, `cargo test canonical_spelling`,
  and `cargo test`.
- Independent reviewer pass returned `APPROVED` after the keyword-scope
  verification gap was fixed.

Dependencies:

- T13.

### T15. DONE: Implement profanity checker

Scenario: `ПроверкаНецензурныхСлов`.

Acceptance criteria:

- Loads the configured profanity dictionary when present.
- Reports matched words with file path and rule id.
- Handles missing dictionary according to config/default policy.
- Does not modify files.

Validation:

- `cargo test profanity`

Completion evidence:

- 2026-05-07: Added `src/text_checkers.rs` with the lexical
  `ПроверкаНецензурныхСлов` implementation, registered it in the reference
  scenario registry, and documented the dictionary-setting policy in
  `spec/configuration.md`.
- The checker loads `ФайлСНецензурнымиСловами` when configured, falls back to
  repository-root `НецензурныеСлова.txt` only when the setting is absent,
  reports missing/invalid configured dictionaries as hard failures, skips when
  the default dictionary is absent, reports dictionary matches as warnings with
  rule id and file path, and never modifies source files.
- Added `tests/profanity.rs` coverage for configured dictionary matches,
  default dictionary lookup, absent-default skip, missing configured dictionary
  failure, non-string setting failure, and empty setting failure.
- Verification passed: `cargo fmt --check`, `cargo test profanity`, and
  `cargo test`.
- Independent reviewer pass found a blank-setting policy gap; it was fixed and
  covered by `profanity_fails_when_dictionary_setting_is_empty`.

Dependencies:

- T6, T9.

### T16. DONE: Implement copyright insertion fixer

Scenario: `ВставкаКопирайтов`.

Acceptance criteria:

- Inserts configured copyright text according to parity fixtures.
- Handles files with existing headers.
- Is idempotent.

Validation:

- `cargo test copyright`

Completion evidence:

- 2026-05-07: Added the lexical `ВставкаКопирайтов` implementation to
  `src/text_fixers.rs`, registered it in the reference scenario registry, and
  documented the scenario settings policy in `spec/configuration.md`.
- The fixer loads configured repository-relative `ПутьКФайлуКопирайта` or the
  default `COPYRIGHT`, inserts or replaces `//©` copyright blocks, skips modules
  with configured `ИсключаемыеТеги` / `ИсключаемыеТэги`, reports ambiguous
  copyright shapes as hard failures, preserves final line endings when rewriting,
  reports modified files, and is idempotent.
- Added `tests/copyright.rs` plus Cyrillic golden fixtures covering first-run
  insertion, second-run idempotence, stale-header replacement, skip tags,
  ambiguous marker blocks, missing default/configured copyright files, invalid
  repository-relative paths, and Windows/backslash path rejection.
- Verification passed: `cargo fmt --check`, `cargo test copyright`, and
  `cargo test`.
- Independent reviewer pass found malformed-marker and Windows/backslash path
  policy gaps; both were fixed and covered by focused regressions. Follow-up
  reviewer pass confirmed the remaining scope stayed within T16 after the
  stricter backslash path policy was added.

Dependencies:

- T6, T14.

## Milestone 4: Parser-Backed BSL Scenarios

### T17. DONE: Integrate shared tree-sitter-bsl parser module

Add the parser foundation required by syntax-aware scenarios.

Acceptance criteria:

- Uses `tree-sitter = "0.25"` and `tree-sitter-bsl = "0.1"` unless compatibility
  validation changes the decision.
- Parser initialization is shared.
- UTF-8 source byte offsets are preserved.
- Parse errors are exposed to scenarios without forcing a global hard failure.

Validation:

- `cargo test bsl_parser`

Completion evidence:

- 2026-05-07: Added `src/bsl_parser.rs` with shared `tree-sitter-bsl`
  parser initialization, `tree-sitter = "0.25"` and `tree-sitter-bsl = "0.1"`
  dependencies, UTF-8 source byte-length preservation, typed byte spans for
  parse error nodes, and a parse API that exposes syntax errors without turning
  them into parser failures.
- Verification passed: `cargo fmt --check`, `cargo test bsl_parser`, and
  `cargo test`.
- Independent reviewer pass returned `APPROVED`.

Dependencies:

- T10.

### T18. DONE: Implement goto checker

Scenario: `ЗапретИспользованияПерейти`.

Acceptance criteria:

- Detects `Перейти` / `goto` as syntax, not raw text.
- Reports source spans.
- Does not flag string literals or comments.

Validation:

- `cargo test goto`

Completion evidence:

- 2026-05-07: Added `src/bsl_checkers.rs` with the parser-backed
  `ЗапретИспользованияПерейти` implementation, registered it in the reference
  scenario registry, and added `tests/goto.rs` coverage.
- The checker matches `tree-sitter-bsl` `goto_statement` syntax, reports hard
  failures with source spans anchored to `GOTO_KEYWORD`, detects both
  `Перейти` and `goto`, ignores comments and string literals, skips non-BSL
  files, and never modifies source files.
- Verification passed: `cargo fmt --check`, `cargo test goto`, and
  `cargo test`.
- Independent explorer pass confirmed the grammar shape and recommended
  anchoring diagnostics on `GOTO_KEYWORD`.

Dependencies:

- T17.

### T19. DONE: Implement duplicate procedure/function checker

Scenario: `ПроверкаДублейПроцедурИФункций`.

Acceptance criteria:

- Collects procedure and function names from syntax trees.
- Detects duplicates according to parity fixtures.
- Reports all duplicate definitions with paths and spans.

Validation:

- `cargo test duplicate_methods`

Completion evidence:

- 2026-05-07: Added the parser-backed
  `ПроверкаДублейПроцедурИФункций` implementation to `src/bsl_checkers.rs`,
  registered it in the reference scenario registry, and added
  `tests/duplicate_methods.rs` coverage.
- The checker collects `procedure_definition` and `function_definition` names
  from `tree-sitter-bsl` syntax trees, detects duplicates case-insensitively
  across procedures and functions, reports every duplicate definition as a hard
  failure with file path and source span anchored to the method name, skips
  non-BSL files, and never modifies source files.
- Verification passed: `cargo fmt --check`, `cargo test duplicate_methods`,
  and `cargo test`.
- Independent explorer pass confirmed the grammar node strategy:
  `procedure_definition` / `function_definition` with the required `name`
  field.

Dependencies:

- T17.

### T20. DONE: Implement preprocessor instruction checker

Scenario: `ПроверкаКорректностиИнструкцийПрепроцессора`.

Acceptance criteria:

- Uses parser coverage for preprocessor nodes and error nodes.
- Defines which parse errors are blocking for this scenario.
- Covers broken syntax and incomplete directive fixtures.

Validation:

- `cargo test preprocessor`

Completion evidence:

- 2026-05-07: Added the parser-backed
  `ПроверкаКорректностиИнструкцийПрепроцессора` implementation to
  `src/bsl_checkers.rs`, registered it in the reference scenario registry, and
  documented the T20 parser/error-node contract in `spec/parser-strategy.md`.
- The checker reports preprocessor-related tree-sitter `ERROR` and missing
  nodes as hard failures with source spans, skips non-BSL files, does not modify
  files, ignores ordinary BSL parse errors outside preprocessor/annotation
  constructs, and uses a narrow case-insensitive line stack for
  `#Если` / `#ИначеЕсли` / `#Иначе` / `#КонецЕсли` ordering and balance where
  published `tree-sitter-bsl` 0.1.x does not model nested directive blocks.
- Added `tests/preprocessor.rs` coverage for valid directive blocks,
  incomplete `#Если`, missing directive expressions, unmatched/duplicate branch
  ordering, malformed annotations, comments/string literals, ordinary BSL parse
  errors, non-BSL file skipping, and hook/exec-rules exit behavior.
- Verification passed: `cargo fmt --check`, `cargo test preprocessor`,
  `cargo test`, and `git diff --check`.
- Independent reviewer pass returned `APPROVED` after case-insensitive fallback
  and missing verification gaps were fixed.

Dependencies:

- T17.

### T21. DONE: Implement region correctness checker

Scenario: `ПроверкаКорректностиОбластей`.

Acceptance criteria:

- Checks `#Область` / `#КонецОбласти` balance and ordering.
- Uses parser coverage where sufficient.
- Uses a scenario-specific lexical stack fallback where parser coverage is not
  enough.
- Reports precise diagnostics.

Validation:

- `cargo test regions`

Completion evidence:

- 2026-05-07: Added the parser-backed
  `ПроверкаКорректностиОбластей` implementation to `src/bsl_checkers.rs`,
  registered it in the reference scenario registry, and documented the T21
  parser/fallback contract in `spec/parser-strategy.md`.
- The checker validates `#Область` / `#КонецОбласти` and English
  `#Region` / `#EndRegion` pairs with a narrow case-insensitive lexical stack
  fallback for precise balance diagnostics, reports hard failures with source
  spans, skips non-BSL files, ignores comments/string literals and ordinary
  non-region BSL parse errors, and never modifies source files.
- Added `tests/regions.rs` coverage for valid nested regions, missing end
  regions, unmatched end regions, missing region names, case-insensitive
  Russian directives, English directives, comments/string literals, ordinary
  BSL parse errors, non-BSL file skipping, and hook/exec-rules exit behavior.
- Verification passed: `cargo fmt --check`, `cargo test regions`,
  `cargo test`, and `git diff --check`.
- Independent reviewer pass found missing English-directive coverage and
  ledger evidence gaps; both were fixed before completion.

Dependencies:

- T17, T20.

### T22. DONE: Implement unit test processing scenario

Scenario: `ОбработкаЮнитТестов`.

Acceptance criteria:

- Locates procedures/functions and loader methods using parser-backed discovery.
- Matches required parity fixtures for test modules.
- Reports or modifies only the files expected by fixtures.
- Is idempotent where it modifies files.

Validation:

- `cargo test unit_tests_processing`

Completion evidence:

- 2026-05-07: Added the parser-backed `ОбработкаЮнитТестов` implementation in
  `src/unit_tests_processing.rs`, registered it in the reference scenario
  registry, and documented the scenario contract in `spec/parser-strategy.md`.
- The fixer applies only to BSL modules under a `tests` path component,
  discovers exported `@unit-test:` procedures/functions and the existing
  `ИсполняемыеСценарии` loader through `tree-sitter-bsl`, replaces or inserts
  the `#Область ТестыAPI` loader region, reports modified files, skips
  non-BSL/non-test files, and fails without modifying the module when BSL parse
  errors are present.
- Added `tests/unit_tests_processing.rs` plus Cyrillic golden fixtures covering
  loader insertion, existing-loader replacement, idempotence, no annotated
  method clean runs, skip behavior, and parse-error hard failure.
- Verification passed: `cargo fmt --check`, `cargo test unit_tests_processing`,
  `cargo test`, and `git diff --check`.

Dependencies:

- T17, T21.

## Milestone 5: XML and EDT Scenarios

### T23. DONE: Add XML/EDT parser and writer layer

Implement structured XML handling for metadata scenarios.

Acceptance criteria:

- Reads `.mdo`, `.form`, and relevant XML metadata files.
- Avoids regex-only XML rewrites for structured transformations.
- Preserves formatting well enough for deterministic fixture diffs.
- Reports parse errors with paths.

Validation:

- `cargo test xml_edt`

Completion evidence:

- 2026-05-07: Added `src/xml_edt.rs` as a separate XML/EDT boundary backed by
  `quick-xml` structured event parsing and writing, exported it from
  `src/lib.rs`, and added `tests/xml_edt.rs` coverage for `.mdo`, `.form`,
  `Configuration.mdo`, XML metadata files, unsupported file kinds, deterministic
  clean roundtrip output, parse errors with repo paths and byte positions,
  empty documents, multiple root elements, and trailing text outside the root.
- Verification passed: `cargo fmt --check`, `cargo test xml_edt`, and
  `cargo test`.
- Independent reviewer pass found malformed document-shape gaps; they were
  fixed with parser/writer-boundary regressions. Follow-up review found no
  remaining T23 correctness or verification risks.

Dependencies:

- T8, T10.

### T24. DONE: Implement XML form correction

Scenario: `КорректировкаXMLФорм`.

Acceptance criteria:

- Discovers `Form.form` files.
- Validates XML through the shared XML/EDT parser boundary before rewriting.
- Corrects duplicate EDT form element ids deterministically.
- Preserves sibling `BaseForm/Form.form` ids for matching extension form
  elements and reports ambiguous base-form matches as hard failures.
- Is idempotent.

Validation:

- `cargo test xml_forms`

Completion evidence:

- 2026-05-07: Added `src/xml_forms.rs` with the EDT `Form.form`
  `КорректировкаXMLФорм` implementation, registered it in the reference
  scenario registry, and documented the XML form correction contract in
  `spec/parser-strategy.md`.
- The fixer validates XML through the shared XML/EDT parser boundary, collects
  form elements from XML events, corrects duplicate element ids with lazy
  free-id allocation, preserves matching sibling `BaseForm/Form.form` ids,
  reports modified current/base form paths, skips direct base-form and
  non-`Form.form` files, and reports malformed XML, invalid ids, and ambiguous
  base-form matches as hard failures.
- Added `tests/xml_forms.rs` coverage for first-run modification and
  second-run idempotence, compact XML layout, sibling base-form binding,
  modified base-form reporting, ambiguous base matches, invalid numeric ids,
  multiple duplicate groups, large and `u64::MAX` id allocation, malformed XML,
  and skip behavior.
- Verification passed: `cargo fmt --check`, `cargo test xml_forms`,
  `cargo test`, and `git diff --check`.
- Independent review found line-layout, eager free-id allocation, missing
  BaseForm/invalid-id/multiple-group coverage, and `u64::MAX` edge gaps; all
  were fixed and covered before completion.

Dependencies:

- T23.

### T25. DONE: Implement full-text search disabling

Scenario: `ОтключениеПолнотекстовогоПоиска`.

Acceptance criteria:

- Discovers relevant EDT/Designer metadata files.
- Disables full-text search settings according to parity fixtures.
- Is idempotent.

Validation:

- `cargo test disable_full_text_search`

Completion evidence:

- 2026-05-07: Added `src/full_text_search.rs` with the XML/EDT
  `ОтключениеПолнотекстовогоПоиска` implementation, registered it in the
  reference scenario registry, and documented the scenario contract in
  `spec/parser-strategy.md` and `spec/configuration.md`.
- The fixer validates metadata XML through the shared XML/EDT parser boundary,
  rewrites `Use` to `DontUse` only inside `fullTextSearch` /
  `xr:FullTextSearch` elements, skips EDT form and non-metadata files, honors
  `МетаданныеДляИсключения` path and attribute exclusions, reports invalid
  setting shapes and XML parse errors as hard failures, reports modified files,
  and is idempotent.
- Added `tests/disable_full_text_search.rs` coverage for EDT metadata,
  Designer/XR metadata tag names, attribute and tabular-section exclusions,
  empty path exclusions, non-metadata skipping, invalid settings, malformed XML,
  hook exit behavior, and second-run idempotence.
- Verification passed: `cargo fmt --check`, `cargo test disable_full_text_search`,
  and `cargo test`.
- Independent reviewer pass found exclusion-matching gaps around ancestor object
  and table names; both were fixed with regression coverage. Final focused
  reviewer pass returned `APPROVED`.

Dependencies:

- T23.

### T26. DONE: Implement form-change permission disabling

Scenario: `ОтключениеРазрешенияИзменятьФорму`.

Acceptance criteria:

- Applies required metadata transformations.
- Handles missing properties clearly.
- Is idempotent.

Validation:

- `cargo test disable_form_change`

Completion evidence:

- 2026-05-07: Added `src/form_change_permission.rs` with the XML/EDT
  `ОтключениеРазрешенияИзменятьФорму` implementation, registered it in the
  reference scenario registry, and documented the scenario contract in
  `spec/parser-strategy.md`.
- The fixer validates XML through the shared XML/EDT parser boundary, applies
  only to EDT `Form.form` and Designer `Form.xml`, rewrites
  `allowFormCustomize` / `Customizable` `true` values to `false`, inserts
  missing Designer `<Customizable>false</Customizable>` after
  `WindowOpeningMode`, reports modified files, and is idempotent.
- Invalid boolean values, CDATA values, mixed-content boolean properties, and
  malformed XML are hard failures. Non-form `.xml` / `.form` files and other
  source kinds remain clean or skipped.
- Added `tests/disable_form_change.rs` coverage for EDT and Designer
  modifications, idempotence, missing Designer property insertion with anchor
  line-ending preservation, invalid values, malformed XML, mixed-content
  properties, non-form XML/EDT clean behavior, and pipeline skip behavior.
- Verification passed: `cargo fmt --check`, `cargo test disable_form_change`,
  `cargo test`, and `git diff --check`.
- Independent reviewer passes found scope and XML edge-case gaps; they were
  fixed with focused regression coverage before completion.

Dependencies:

- T23.

## Milestone 6: Metadata and Composition Scenarios

### T27. DONE: Implement metadata-object/file synchronization

Scenario: `СинхронизацияОбъектовМетаданныхИФайлов`.

Acceptance criteria:

- Detects metadata objects and corresponding files.
- Validates the owning configuration description when either the configuration
  file or a staged metadata object description file is processed.
- Reports missing object files/directories, stale files/directories, and
  case-only path/name mismatches as deterministic hard failures.
- Does not modify, generate, delete, or restage files in the v1 checker slice;
  generated/repaired file behavior requires a later explicit spec contract.
- Produces deterministic diagnostics.

Validation:

- `cargo test metadata_sync`

Completion evidence:

- 2026-05-07: Added `src/metadata_sync.rs` with the XML/EDT
  `СинхронизацияОбъектовМетаданныхИФайлов` checker and registered it in the
  reference scenario registry.
- Documented and implemented the v1 validation-only contract: configuration
  descriptions and staged metadata object files trigger owning configuration
  sync checks; missing object files/directories, stale files/directories, and
  case-only path/name mismatches are deterministic hard failures; no files are
  modified, generated, deleted, or restaged in this slice.
- Added focused `tests/metadata_sync.rs` coverage for clean EDT and Designer
  configurations, EDT extension configuration without `languages`, nested
  Designer `ChildObjects` that must be ignored, missing/deleted/stale object
  files, Cyrillic case-only mismatches, malformed XML, full-tree skip behavior,
  and empty `modified_paths`.
- Verification passed: `cargo fmt --check`, `cargo test metadata_sync`,
  `cargo test`, `git diff --check`, and a read-only RAT smoke against
  `exts/rat/src` with `--rules СинхронизацияОбъектовМетаданныхИФайлов`.
- Independent reviewer passes found Designer nesting, EDT extension,
  filesystem IO, and nested `ChildObjects` gaps; all were fixed before
  completion.

Dependencies:

- T9, T23.

### T28. DONE: Implement composition sorting

Scenario: `СортировкаСостава`.

Acceptance criteria:

- Sorts metadata composition according to parity fixtures.
- Preserves valid XML/EDT structure.
- Is idempotent.

Validation:

- `cargo test composition_sort`

Completion evidence:

- 2026-05-07: Added `src/composition_sort.rs` with the XML/EDT
  `СортировкаСостава` initial slice and registered it in the reference
  scenario registry.
- Documented and implemented the v1 configuration-description contract:
  `Configuration/Configuration.mdo` and `Configuration.xml` are sorted through
  source-root-relative identity, XML is validated through the shared XML/EDT
  parser boundary before rewriting, direct EDT composition references and
  Designer `ChildObjects` entries are sorted by the reference metadata-kind
  order from `languages` / `Language` through `paletteColors` / `PaletteColor`
  and then by object name, using the `v8-context-fixture` EDT/Designer
  configuration files as the metadata-kind order baseline. UID-like references
  containing `-` remain in place, prefix buckets from `УчитываяПрефикс` are
  honored, and `ОтключенныеОбъекты = Конфигурация` skips the configuration
  slice.
- Added `tests/composition_sort.rs` coverage for EDT and Designer sorting,
  hook-mode modified-path reporting, second-run idempotence, prefix buckets,
  disabled configuration settings, non-configuration skip behavior, invalid
  settings, malformed XML, and source-root-relative configuration identity.
- Verification passed: `cargo fmt --check`, `cargo test composition_sort`,
  `cargo test`, and `git diff --check`.
- Independent reviewer pass found a source-root-relative identity gap for
  nested `Configuration/Configuration.mdo`; it was fixed with regression
  coverage. Follow-up reviewer pass found no remaining T28 correctness,
  T29/local `.os`, or platform-runtime scope issues after ledger evidence was
  added.

Dependencies:

- T23.

### T29. DONE: Implement duplicate metadata removal

Scenario: `УдалениеДублейМетаданных`.

Acceptance criteria:

- Detects duplicate metadata entries according to fixtures.
- Removes or reports duplicates according to parity behavior.
- Is idempotent where it modifies files.

Validation:

- `cargo test duplicate_metadata`

Completion evidence:

- 2026-05-07: Added `src/duplicate_metadata.rs` with the XML/EDT
  `УдалениеДублейМетаданных` implementation and registered it in the reference
  scenario registry.
- Documented and implemented the v1 configuration-description contract:
  `Configuration/Configuration.mdo` and `Configuration.xml` are validated
  through the shared XML/EDT parser boundary, direct composition entries are
  de-duplicated with the last matching source-shape occurrence preserved,
  UID-like references containing `-` and non-configuration files are skipped,
  modified files are reported, and the fixer is idempotent.
- Added `tests/duplicate_metadata.rs` coverage for EDT and Designer
  de-duplication, hook-mode modified-path reporting, second-run idempotence,
  source-root-relative configuration identity, UID-like reference preservation,
  different source-shape preservation, non-configuration skip behavior, and
  malformed XML hard failures.
- Verification passed: `cargo fmt --check`, `cargo test duplicate_metadata`,
  `cargo test`, and `git diff --check`.
- Independent explorer pass confirmed the legacy scenario's narrow
  source-shape de-duplication behavior; the implementation and spec were kept
  within that T29 scope.

Dependencies:

- T23, T28.

## Milestone 7: Platform-Dependent Scenario

### T30. DONE: Implement external reports/processings/extensions scenario boundary

Scenario: `РазборОтчетовОбработокРасширений`.

Acceptance criteria:

- Scenario is registered as required in v1.
- 1C platform executable discovery is explicit.
- Missing platform/runtime inputs are reported as environment skip or dependency
  error, not parser/config failure.
- Mutating behavior runs only in controlled temp/fixture environments in tests.
- Hook diagnostics clearly explain runtime dependency failures.

Validation:

- `cargo test external_artifacts`

Completion evidence:

- 2026-05-07: Added `src/external_artifacts.rs` with the initial
  platform-dependent `РазборОтчетовОбработокРасширений` boundary and
  registered it in the reference scenario registry.
- Documented and implemented the T30 boundary contract: `.epf`, `.erf`, and
  `.cfe` files are classified as external artifacts, non-external files are
  skipped, scenario settings validate `ИспользоватьНастройкиПоУмолчанию` and
  `ВерсияПлатформы` for processed artifacts, 1C platform discovery is explicit
  through executable candidates on `PATH`, missing runtime is reported as a
  hard dependency failure, and discovered runtime still does not execute 1C in
  this slice.
- The boundary does not modify artifacts, create generated directories, enqueue
  post-processing files, or restage paths. Real unpacking and mutation safety
  require a later explicit spec task.
- Added `tests/external_artifacts.rs` coverage for missing platform
  dependency diagnostics, non-external skip behavior, invalid settings policy,
  version-constrained platform discovery, discovered-runtime non-execution, and
  empty modified paths.
- Verification passed: `cargo fmt --check`, `cargo test external_artifacts`,
  `cargo test`, and `git diff --check`.
- Independent reviewer pass returned `APPROVED`.

Dependencies:

- T9, T10.

Non-goals:

- Implementing `РазборОбычныхФормНаИсходники`.

## Milestone 8: End-to-End Hook and CI Readiness

### T31. DONE: Wire `prek-hook` end-to-end

Connect config resolution, Git index discovery, source classification, scenario
pipeline, restaging, diagnostics, and exit codes for hook mode.

Acceptance criteria:

- `prek-hook` ignores filenames passed by `prek` and uses the Git index.
- Staged modified files are processed.
- Deleted files are passed only to scenarios that need deletion context.
- Generated files can be processed and restaged.
- Modified paths are reported.
- Exit code follows the PRD contract.

Validation:

- `cargo test prek_hook`
- Manual smoke in a temporary Git repo.

Completion evidence:

- 2026-05-07: Wired the public `prek-hook` path through Git top-level
  discovery, config/source-root resolution, staged Git-index discovery, source
  classification, scenario pipeline execution, modified-path restaging, and
  deterministic text/JSON report rendering.
- 2026-05-25: Added source-root auto-discovery for `prek-hook` when
  `--source-dir` is omitted. Hook mode now prefers root `v8project.yaml`
  `source-set` entries, resolves EDT source sets to their `src` subdirectory,
  falls back to staged EDT `Configuration/Configuration.mdo` and Designer
  `Configuration.xml` markers when the manifest is absent, ignores unsupported
  staged files when no source root applies, and keeps an explicit
  `--source-dir` override authoritative.
- `prek-hook` now ignores runner-passed positional filenames, including
  non-UTF-8 and single-dash path edge cases, while unknown long options remain
  CLI errors. Commit-mode processing continues to use the Git index as the
  source of truth.
- Added `tests/prek_hook.rs` end-to-end coverage with real temporary Git
  repositories for staged fixer processing, modified-path reporting, restaging,
  hook exit code `1` after unreviewed modifications, ignored passed filenames,
  repository-subdirectory invocation, and deleted-file skip behavior for
  scenarios without deleted-file capability.
- Verification passed: `cargo fmt --check`, `cargo test prek_hook`,
  `cargo test`, `git diff --check`, and a manual temporary-Git-repo smoke for
  `prec-bsl prek-hook --rules УдалениеЛишнихКонцевыхПробелов`.
- Independent reviewer passes found non-UTF-8 and single-dash runner filename
  edge cases; both were fixed with focused regressions. Final focused reviewer
  pass returned `APPROVED`.

Dependencies:

- T6, T7, T8, T9, T10, T11.

### T32. DONE: Wire `exec-rules` end-to-end

Connect explicit whole-tree rule execution for CI and local validation.

Acceptance criteria:

- Accepts repository path, comma-separated source roots, comma-separated rule
  names, and optional config path.
- Missing source roots are reported.
- Multiple source roots preserve source-root context.
- Critical errors are accumulated and printed after traversal.

Validation:

- `cargo test exec_rules`
- Manual smoke against a temp copy of selected RAT roots.

Completion evidence:

- 2026-05-07: Added `tests/exec_rules.rs` end-to-end binary coverage for
  `prec-bsl exec-rules <repo>` from outside the target repository, explicit
  `--config` resolution, comma-separated `--source-dir` and `--rules` values,
  source-root-relative XML/EDT scenario context across multiple roots, missing
  source-root diagnostics, and critical failure accumulation after traversal.
- Verification passed: `cargo fmt --check`, `cargo test exec_rules`,
  `cargo test`, and a manual smoke against a temporary copy of
  `/home/alko/develop/open-source/rat/fixtures/configuration` with
  `--rules УдалениеЛишнихКонцевыхПробелов`. The real RAT checkout status was
  unchanged before/after the smoke; it already contained unrelated local
  changes in `AGENTS.md`, `CLAUDE.md`, and
  `exts/rat/src/CommonModules/РатАлгоритмы/Module.bsl`.
- Independent reviewer pass found a missing comma-separated `--rules`
  verification gap; it was fixed with binary-level coverage and the follow-up
  reviewer pass returned `APPROVED`.

Dependencies:

- T6, T8, T9, T10.

### T33. DONE: Run RAT parser coverage acceptance

Run the parser coverage baseline over RAT `.bsl` files.

Acceptance criteria:

- Parser initialization succeeds.
- Parse errors are counted and reported with paths.
- Results compare published crate behavior with the local grammar checkout only
  when parser gaps block required scenarios.

Validation:

- `cargo test rat_parser_coverage`

Completion evidence:

- 2026-05-07: Added read-only RAT parser coverage acceptance in
  `tests/rat_acceptance.rs` for the documented parser roots
  `fixtures/configuration/src`, `exts/rat/src`, and `tests/src`.
- The acceptance initializes the shared `tree-sitter-bsl` parser, parses all
  discovered RAT `.bsl` files, counts parser error nodes, and writes a
  deterministic report to `target/rat-acceptance/rat-parser-coverage.txt`.
- Published crate baseline on the current RAT checkout: 242 `.bsl` files
  covered, 53 files with parse errors, and 169 parser error nodes. These gaps
  were recorded as coverage data only; no local grammar checkout comparison was
  required because T33 did not identify a parser gap blocking a required
  scenario.
- Verification passed: `cargo fmt --check`, `cargo test rat_parser_coverage`,
  `cargo test`, `git diff --check`, and read-only RAT status probe with
  `GIT_OPTIONAL_LOCKS=0 git -C /home/alko/develop/open-source/rat status --short`.

Dependencies:

- T5, T17.

### T34. DONE: Run text fixer idempotence acceptance on RAT copy

Run low-risk text fixer scenarios on a temporary RAT copy.

Acceptance criteria:

- First run reports modified files where applicable.
- Second run is clean for the same scenario set.
- Real RAT checkout remains unchanged.

Validation:

- `cargo test rat_text_idempotence`
- `git -C /home/alko/develop/open-source/rat status --short`

Completion evidence:

- 2026-05-07: Added RAT text fixer idempotence acceptance in
  `tests/rat_acceptance.rs`. The test copies the required RAT source roots to a
  `target/rat-acceptance` temporary directory, writes deterministic copyright
  text into the copy, runs the text fixer scenario set through
  `prec-bsl exec-rules`, asserts the first run reports modified files, reruns
  the same scenario set on the same copy, and asserts the second run is clean.
- The real RAT checkout status is captured before and after the mutating
  acceptance run with `GIT_OPTIONAL_LOCKS=0` and must remain unchanged.
- Verification passed: `cargo fmt --check`, `cargo test rat_text_idempotence`,
  `cargo test`, `git diff --check`, and read-only RAT status probe with
  `GIT_OPTIONAL_LOCKS=0 git -C /home/alko/develop/open-source/rat status --short`.
- Independent plan review returned `APPROVED` before implementation.

Dependencies:

- T5, T11, T12, T13, T14, T16.

### T35. DONE: Run XML/EDT acceptance on RAT copy

Run XML/EDT scenarios on a temporary RAT copy with `.mdo` and `.form` files.

Acceptance criteria:

- `Configuration.mdo` and object `.mdo` files are discovered.
- `Form.form` files are discovered.
- Scenario outputs are checked by golden diff or idempotence.
- Real RAT checkout remains unchanged.

Validation:

- `cargo test rat_xml_edt`
- `git -C /home/alko/develop/open-source/rat status --short`

Completion evidence:

- 2026-05-07: Added RAT XML/EDT acceptance in
  `tests/rat_acceptance.rs`. The test copies required RAT source roots into a
  `target/rat-acceptance` temporary directory, uses the production source-root
  and file-classification layer to prove `Configuration.mdo`, object `.mdo`,
  and `Form.form` discovery, seeds XML/EDT fixer probes only in the temp copy,
  and verifies `ОтключениеПолнотекстовогоПоиска` plus
  `ОтключениеРазрешенияИзменятьФорму` first-run modifications and second-run
  idempotence.
- The acceptance also records the current RAT
  `КорректировкаXMLФорм` boundary for copied `Form.form` files with `id = -1`:
  the scenario fails closed with a deterministic hard-failure diagnostic and
  leaves the temp-copy form unchanged.
- Verification passed: `cargo fmt --check`, `cargo test rat_xml_edt`,
  `cargo test`, `git diff --check`, and read-only RAT status probe with
  `GIT_OPTIONAL_LOCKS=0 git -C /home/alko/develop/open-source/rat status --short`.
- Independent plan review returned `APPROVED` before implementation.

Dependencies:

- T5, T23, T24, T25, T26.

### T36. DONE: Prepare v1 release readiness checklist

Create the final v1 acceptance checklist after all required scenarios are
implemented.

Acceptance criteria:

- All required scenarios are implemented or have an explicit accepted runtime
  boundary.
- Unsupported scenarios fail validation clearly.
- `cargo test` is green.
- Hook manifest is present and matches CLI command names.
- Docs include minimal install and usage examples for `prek`.
- Release notes call out unsupported local `.os` execution and platform
  dependencies.

Validation:

- `cargo test`
- `cargo run -- prek-hook --help`
- `cargo run -- exec-rules --help`

Completion evidence:

- 2026-05-07: Added `spec/release-readiness-checklist.md` with the final v1
  acceptance checklist, required scenario coverage matrix, hook/CLI readiness
  notes, RAT acceptance baseline, and release note callouts for unsupported
  local `.os` execution, `РазборОбычныхФормНаИсходники`, platform dependency
  boundaries, and `v8config.json` as the domain configuration surface.
- Added `README.md` with minimal `prek` and `pre-commit` install examples,
  public CLI usage, and short v1 release notes.
- Added a focused registry regression asserting every required v1 scenario has
  a concrete reference handler rather than the placeholder implementation.
- Verification passed: `cargo fmt --check`, focused readiness regression,
  `cargo test`, `cargo run -- prek-hook --help`,
  `cargo run -- exec-rules --help`, and `git diff --check`.

Dependencies:

- T31, T32, T33, T34, T35.

## Milestone 9: Compatibility Follow-ups

### T37. DONE: Support metadata-tree and subsystem-composition sorting scenarios

Add explicit support for the additional `precommit4onec` scenarios that appear
in existing project configs but are not part of the current built-in v1 required
set:

- `СортировкаДереваМетаданных.os`
- `СортировкаСоставаПодсистем.os`

Motivation:

- Existing `precommit4onec` projects, including historical RAT config snapshots
  and similar compatibility corpora, may keep these scenario ids in
  `ГлобальныеСценарии`.
- v1 previously reported them as unsupported because they were outside the
  built-in required scenario set.
- A migration-friendly release should support these concrete scenarios directly
  instead of requiring users to remove them from existing `v8config.json`
  immediately.

Acceptance criteria:

- The behavior of `СортировкаДереваМетаданных` is specified from reference
  evidence before implementation.
- The behavior of `СортировкаСоставаПодсистем` is specified from reference
  evidence before implementation.
- Both scenario ids are recognized with and without the `.os` suffix.
- Both scenarios are registered as supported compatibility scenarios rather than
  generic repository-local dynamic `.os` execution.
- Implementation uses Rust-native XML/EDT processing where practical and keeps
  broad repository-local `.os` execution unsupported.
- RAT `v8config.json` is covered as a compatibility fixture without requiring
  users to remove these two scenarios and without mutating
  `/home/alko/develop/open-source/rat`.
- README and `spec/configuration.md` document the migration path for existing
  `precommit4onec` configs that include these scenarios.

Validation:

- `cargo test config`
- `cargo test rat`
- New focused tests for both scenarios in global and project-specific settings.
- Focused XML/EDT fixture tests for the accepted behavior of each scenario.
- Read-only RAT status probe:
  `GIT_OPTIONAL_LOCKS=0 git -C /home/alko/develop/open-source/rat status --short`

Completion evidence:

- 2026-05-07: Registered `СортировкаДереваМетаданных` and
  `СортировкаСоставаПодсистем` as explicit supported compatibility scenarios
  outside the built-in v1 default list, with lookup accepting ids with and
  without `.os`.
- Specified the behavior from reference `precommit4onec`
  `СортировкаСостава.os`: metadata-tree sorting uses the configuration
  composition branch; subsystem composition sorting uses EDT `content` and
  Designer `Properties/Content` `xr:Item` branches.
- Implemented Rust-native XML/EDT handling for EDT and Designer subsystem
  composition sorting while keeping unknown repository-local `.os` scenarios
  unsupported.
- Updated RAT compatibility coverage to account for the current live
  `rat/v8config.json`: it no longer contains these two ids in
  `ГлобальныеСценарии`, so tests build a synthetic in-memory RAT config
  fixture and keep the checkout read-only.
- Updated README, `spec/configuration.md`, `spec/parser-strategy.md`,
  `spec/testing-strategy.md`, and `spec/reference-scenario-inventory.md`.
- Verification passed: `cargo test config`, `cargo test scenarios`,
  `cargo test scenario_pipeline`, `cargo test composition_sort`,
  `cargo test compatibility`, `cargo test rat`,
  `GIT_OPTIONAL_LOCKS=0 git -C /home/alko/develop/open-source/rat status --short`,
  `cargo fmt --check`, and `git diff --check`.

Dependencies:

- T36.

Non-goals:

- Reclassifying unknown repository-local `.os` scenarios as silently supported.
- Implementing a generic repository-local `.os` execution adapter.
- Implementing `РазборОбычныхФормНаИсходники`.

## Milestone 10: Workspace Modularity

### T38. DONE: Split mixed responsibility modules into focused crates

Convert the single-crate implementation into a workspace with internal crates
that match the existing context boundaries from the PRD and agent rules.

Motivation:

- `src/scenario_pipeline.rs` currently mixes the execution model with concrete
  BSL, XML/EDT, metadata, composition and platform-dependent scenario handler
  registration.
- Large modules such as `src/config.rs`, `src/text_fixers.rs`,
  `src/bsl_checkers.rs`, `src/metadata_sync.rs`, and
  `src/composition_sort.rs` are context-sized implementation units and should
  not all live in the binary package root.
- Future scenario work should be able to depend on the immediate context it
  needs without pulling Git, CLI or unrelated XML/BSL rules.

Acceptance criteria:

- Root package remains the public `prec-bsl` binary/library facade so existing
  integration tests can continue using `prec_bsl::<module>`.
- Workspace members separate at least these contexts:
  - scenario inventory and configuration;
  - Git-index access;
  - source-root and file classification;
  - scenario execution pipeline;
  - output rendering;
  - BSL parser/checker/fixer scenarios;
  - XML/EDT and metadata scenarios.
- `ScenarioRegistry` no longer hard-codes concrete handler imports from every
  rule module; reference handler wiring is owned by the facade or a rule crate.
- CLI contracts, output contracts, scenario ids, config semantics, RAT safety
  and hook exit behavior remain unchanged.
- No new dynamic `.os` execution, parser fallback, runtime 1C execution, or
  compatibility bridge is introduced.

Validation:

- `cargo fmt --check`
- `cargo test`
- `cargo test scenario_pipeline`
- `cargo test config`
- `cargo test rat`

Completion evidence:

- 2026-05-07: Converted the root package into a Cargo workspace while keeping
  the public `prec-bsl` binary/library facade.
- Added focused internal crates:
  - `prec-bsl-config` for `v8config.json` parsing and resolution;
  - `prec-bsl-git` for staged-file collection and restaging;
  - `prec-bsl-source` for source roots and file classification;
  - `prec-bsl-pipeline` for scenario execution, queueing, result aggregation
    and exit-code semantics;
  - `prec-bsl-output` for text/JSON report rendering;
  - `prec-bsl-bsl` for BSL parser, lexical fixers and parser-backed BSL
    scenarios;
  - `prec-bsl-xml` for XML/EDT, metadata and composition scenarios;
  - `prec-bsl-platform` for the external report/processing/extension runtime
    boundary.
- Kept scenario catalog assembly in the root facade instead of a separate
  inventory crate, so implemented scenarios are defined in their owning rule
  modules and config receives catalog metadata explicitly.
- Moved reference handler wiring out of `prec-bsl-pipeline` into the facade
  `reference_registry()` function, so the pipeline crate no longer imports
  concrete BSL/XML/platform rule handlers.
- Split large crate roots after the workspace move:
  - `prec-bsl-config` now separates error, model, raw parsing, path matching,
    resolution, validation and tests;
  - `prec-bsl-pipeline` now separates execution models, registry contracts,
    runner logic and tests;
  - Git, source and output crates keep inline library code separate from their
    test modules.
- Kept existing test-facing module paths through facade re-exports such as
  `prec_bsl::config`, `prec_bsl::source_files`, `prec_bsl::text_fixers` and
  `prec_bsl::xml_edt`.
- Moved member-crate test fixtures back under the workspace `target/` tree and
  ignored accidental `crates/*/target/` directories to keep generated test
  artifacts out of the workspace crates.
- Verification passed: `cargo fmt --all --check`, `cargo test`,
  `cargo test --workspace`, `cargo test --workspace scenario_pipeline`,
  `cargo test --workspace config`, `cargo test rat`, and `git diff --check`.

Dependencies:

- T37.

### T39. DONE: Bind executable scenario definitions next to handlers

Remove the loose string-based handler wiring left after the workspace split.
Each implemented rule module should expose a complete executable scenario
definition near its handler function, while config receives scenario metadata
through an explicit catalog.

Motivation:

- Scenario metadata and handler functions were still connected by separate
  string constants in `reference_registry()`.
- A supported scenario could remain described in the inventory while its
  handler was forgotten or accidentally bound to a different id.

Acceptance criteria:

- `prec-bsl-config` owns catalog-neutral `ScenarioMetadata`, `ScenarioSupport`,
  `ScenarioCatalog` and id-normalization types.
- Runtime `ScenarioDefinition` includes metadata, handler and deleted-file
  capability, and provides constructors for required and compatibility
  executable scenarios.
- BSL, XML/EDT, metadata, composition and platform rule modules expose nearby
  `*_SCENARIO` constants that bind their ids, source files, support level and
  concrete handlers in one place.
- The root facade catalog collects executable definitions plus metadata-only
  non-executable decisions; config/default resolution receives this catalog
  explicitly instead of depending on a separate scenario-inventory crate.
- `reference_registry()` collects executable definitions only; it does not
  manually pair free strings with handlers.
- A regression test fails if any required or compatibility scenario in the
  reference inventory is not bound to a registered handler.

Completion evidence:

- 2026-05-07: Added config-level `ScenarioMetadata`, `ScenarioSupport`,
  `ScenarioCatalog` and id normalization, added runtime
  `ScenarioDefinition { metadata, handler, handles_deleted_files }`, and moved
  executable `*_SCENARIO` constants into the rule modules next to the handlers.
- Replaced facade `.with_handler(<id>, <fn>)` wiring with
  `ScenarioRegistry::with_definitions([...])`.
- Removed the separate `prec-bsl-scenarios` crate; the root facade now exposes
  compatibility helpers through `prec_bsl::scenarios` while config receives the
  root catalog explicitly.
- Added `reference_registry_binds_every_supported_reference_scenario_to_handler`
  coverage for all required v1 and explicit compatibility scenarios.
- Verification passed: `cargo fmt --all`, `cargo test --workspace
  scenario_pipeline`, `cargo test --workspace
  reference_registry_binds_every_supported_reference_scenario_to_handler`, and
  `cargo test --workspace reference_scenario_inventory`.

Dependencies:

- T38.

## Milestone 11: Upstream Issue Regression Backlog

These tasks capture risks found during the 2026-05-07 review of open
`precommit4onec` GitHub issues. They are tracked as explicit parity/regression
work so upstream problems are either fixed in `prec-bsl` or intentionally kept
outside the Rust v1 scope.

### T40. DONE: Make duplicate method detection preprocessor-branch aware

Upstream issue: `precommit4onec` #16.

Current finding:

- `ПроверкаДублейПроцедурИФункций` currently reports duplicate procedures or
  functions even when the same name appears in mutually exclusive
  `#Если` / `#Иначе` preprocessor branches.
- The issue is reproducible in `prec-bsl` through `exec-rules` on a fixture
  where two same-named methods are guarded by mutually exclusive branches.

Acceptance criteria:

- A method name repeated only across mutually exclusive branches of the same
  preprocessor conditional block is not reported as a duplicate.
- A method name repeated in the same active branch remains a hard failure.
- Nested conditional blocks are handled conservatively and deterministically.
- Parser errors remain scenario-specific diagnostics; no broad regex fallback is
  introduced.
- The checker still reports duplicates across procedures/functions
  case-insensitively when the definitions can coexist in one compiled variant.

Validation:

- Add focused fixtures to `tests/duplicate_methods.rs`.
- `cargo test duplicate_methods`
- `cargo test --workspace`

Completion evidence:

- 2026-05-07: Updated parser-backed duplicate method detection to keep method
  discovery in `tree-sitter-bsl` while adding a narrow line-oriented
  preprocessor branch context for duplicate coexistence checks.
- Same-name procedures/functions repeated only across mutually exclusive
  `#Если` / `#ИначеЕсли` / `#Иначе` branches are no longer reported as
  duplicates. Repeats in the same branch, repeats in malformed conditional
  blocks, and procedure/function pairs that can coexist still report hard
  failures with method-name spans.
- Documented the branch-context contract in `spec/parser-strategy.md` and
  updated the legacy parity mapping note in `spec/testing-strategy.md`.
- Added focused `tests/duplicate_methods.rs` coverage for exclusive branches,
  `#ИначеЕсли`, same-branch duplicates, nested conditionals, and malformed
  duplicate `#Иначе` structure.

Dependencies:

- T19, T20.

### T41. DONE: Verify EDT composition sort order against precommit4onec edge cases

Upstream issue: `precommit4onec` #39.

Current finding:

- `СортировкаСостава` currently uses Rust string ordering after the implemented
  metadata-type and prefix grouping rules.
- The upstream issue reports an EDT/reference-order mismatch for object names
  containing digits and underscores, such as `ФорматПФР70_2010XML`.

Acceptance criteria:

- Add parity fixtures covering EDT `Configuration.mdo` and subsystem
  composition names with digits, underscores, Cyrillic and Latin suffixes.
- Compare the expected order with local `precommit4onec` reference behavior
  before changing the comparator.
- If the comparator differs, encode the reference-compatible ordering rule in
  the XML/EDT composition boundary, not as an ad hoc test-only sort.
- Sorting remains idempotent and source-root-relative.

Validation:

- `cargo test composition_sort`
- `cargo test --workspace`

Completion evidence:

- 2026-05-07: Verified reference `precommit4onec`
  `СортировкаСостава.os` behavior: prefix buckets are selected from object
  names, while each bucket is sorted by the full reference text through
  `ТаблицаЗначений.Сортировать("СтрокаСортировки")`.
- Replaced Rust Unicode scalar ordering in the XML/EDT composition boundary
  with a reference-compatible string sort key for the covered configuration
  and subsystem composition scopes.
- Added focused `tests/composition_sort.rs` regressions for EDT
  `Configuration.mdo`, EDT subsystem `content`, and Designer subsystem
  `Properties/Content/xr:Item` names containing digits, underscores, Cyrillic
  and Latin suffixes such as `ФорматПФР70_2010XML`.
- Updated `spec/parser-strategy.md` and `spec/testing-strategy.md` with the
  durable comparator contract and legacy mapping conclusion.
- Verification passed: `cargo fmt --all --check`, `cargo test
  composition_sort`, and `cargo test --workspace`.

Dependencies:

- T28, T37.

### T42. DONE: Add line-ending regression matrix for mutating scenarios

Upstream issue: `precommit4onec` #36.

Current finding:

- BSL text fixers have focused LF/CRLF preservation coverage, but XML/EDT and
  cross-scenario mutating paths do not yet have a shared line-ending regression
  matrix.
- The current implementation should not normalize line endings accidentally
  when only a targeted BSL/XML/EDT value changes.

Acceptance criteria:

- Cover LF and CRLF inputs for every mutating built-in Rust scenario that
  rewrites files.
- Assert idempotence after the first rewrite for each line-ending style.
- For XML/EDT scenarios, verify unchanged surrounding text keeps its original
  line-ending style where the writer boundary promises text preservation.
- Hook/exec-rules modified-path reporting remains unchanged.

Validation:

- Add scenario-specific focused tests rather than one opaque mega-test.
- `cargo test --test trailing_whitespace`
- `cargo test --test empty_lines`
- `cargo test --test keyword_spacing`
- `cargo test --test canonical_spelling`
- `cargo test --test copyright`
- `cargo test --test unit_tests_processing`
- `cargo test --test composition_sort`
- `cargo test --test duplicate_metadata`
- `cargo test --test disable_full_text_search`
- `cargo test --test disable_form_change`
- `cargo test --test xml_forms`
- `cargo test --workspace`

Dependencies:

- T11, T12, T13, T14, T16, T22, T24, T25, T26, T28, T29.

Completion evidence:

- 2026-05-07: Added scenario-specific LF/CRLF regression tests for every
  mutating built-in Rust scenario that rewrites files:
  `УдалениеЛишнихКонцевыхПробелов`, `УдалениеЛишнихПустыхСтрок`,
  `ДобавлениеПробеловПередКлючевымиСловами`,
  `ИсправлениеНеКаноническогоНаписания`, `ВставкаКопирайтов`,
  `ОбработкаЮнитТестов`, `КорректировкаXMLФорм`,
  `ОтключениеПолнотекстовогоПоиска`,
  `ОтключениеРазрешенияИзменятьФорму`, `СортировкаСостава`,
  compatibility sorting aliases `СортировкаДереваМетаданных` /
  `СортировкаСоставаПодсистем`, and `УдалениеДублейМетаданных`.
- The new copyright matrix exposed a CRLF regression where inserted copyright
  header lines used the copyright file's LF line endings instead of the
  processed module's dominant line ending. Fixed
  `ВставкаКопирайтов` to normalize inserted copyright text to the target file's
  line-ending style while keeping second-run idempotence.
- Updated validation commands to use executable Cargo test targets and included
  the mutating canonical spelling and unit-test processing scenarios that were
  implied by the acceptance text.
- Verification passed: all focused T42 test targets, `cargo fmt --all --check`,
  `cargo test --workspace`, and `git diff --check`.

### T43. DONE: Clarify global-scenario disabling compatibility

Upstream issue: `precommit4onec` #24.

Current finding:

- `prec-bsl` already supports `ОтключенныеСценарии`; if all enabled scenarios
  are disabled, resolution can produce an empty scenario set.
- There is no separate `ИспользоватьГлобальныеСценарии` compatibility setting
  in the current v1 configuration contract.

Acceptance criteria:

- Inspect local reference `precommit4onec` configuration behavior for any
  `ИспользоватьГлобальныеСценарии` or equivalent flag before implementation.
- If the flag exists in the supported compatibility surface, document it in
  `spec/configuration.md` and implement parser/resolution behavior.
- If the flag is outside the supported v1 surface, document the explicit
  non-goal and add a regression test proving `ОтключенныеСценарии` can disable
  the whole global list without an exception.

Validation:

- `cargo test config`
- `cargo test scenario_pipeline`
- `cargo test --workspace`

Completion evidence:

- 2026-05-08: Inspected local `precommit4onec` reference sources and imported
  fixtures for `ИспользоватьГлобальныеСценарии`; no such compatibility flag is
  present. Reference behavior uses `ГлобальныеСценарии` as the enabled list and
  removes entries through `ОтключенныеСценарии`.
- Documented the explicit v1 non-goal in `spec/configuration.md`: there is no
  separate `ИспользоватьГлобальныеСценарии` flag, and disabling the whole list
  is represented through `ОтключенныеСценарии` or an empty project-specific
  `ГлобальныеСценарии` override.
- Added focused config and scenario-pipeline regressions proving that disabling
  every enabled scenario produces an empty enabled set and an empty pipeline
  result without an exception.

Dependencies:

- T6, T9.

## Milestone 13: Hook Execution Controls

### T47. DONE: Add merge-commit skip options for `prek-hook`

Add a small opt-in CLI control for repositories that want merge commits to
bypass `prec-bsl` processing while keeping normal commit hook behavior
unchanged.

Acceptance criteria:

- `prec-bsl prek-hook` accepts `--ignore-merge-commit`.
- `prec-bsl prek-hook` accepts `--ignore-remote-merge-commit`.
- Both merge-ignore options are mutually exclusive.
- The option is valid only for hook mode, not for `exec-rules`.
- When `--ignore-merge-commit` is present and Git reports an active merge
  state through `MERGE_HEAD`, hook mode exits with code `0` before staged-file
  discovery, scenario execution or restaging.
- When `--ignore-remote-merge-commit` is present, the same early successful
  exit applies only when at least one `MERGE_HEAD` SHA is reachable from
  `refs/remotes/*`; local merge commits continue through normal hook
  processing.
- Without the option, existing hook processing is unchanged.
- The command help, README and configuration contract document the option as a
  small end-user hook argument.

Validation:

- `cargo test prek_hook`
- `cargo test cli`
- `cargo fmt --check`
- `git diff --check`

Completion evidence:

- 2026-05-26: Added `--ignore-merge-commit` and
  `--ignore-remote-merge-commit` to `prek-hook`, documented them in the public
  CLI contract, enforced mutual exclusion, and covered both merge skip modes
  with temporary Git repository regressions that prove staged files are not
  modified or restaged. The remote-only regression also proves local
  `MERGE_HEAD` commits without matching `refs/remotes/*` continue through
  normal hook processing.

Dependencies:

- T31.

### T48. DONE: Reduce noisy human-readable hook output

Keep commit-hook output focused on information a developer can act on when a
commit is blocked.

Acceptance criteria:

- Text output continues to list modified files separately from hard failures.
- Text output does not duplicate hard failures in the grouped `Messages`
  section.
- Text output suppresses non-actionable `skipped` details when modifications,
  warnings, hard failures or unsupported diagnostics are present.
- Text output still shows `skipped` messages when they are the only explanation
  for processed files.
- JSON output keeps the full stable result list unchanged.

Validation:

- `cargo test -p prec-bsl-output`
- `cargo test --workspace`
- `cargo fmt --all --check`
- `git diff --check`

Completion evidence:

- 2026-05-26: Adjusted the text renderer in `prec-bsl-output` so the default
  hook/exec-rules text output shows concise summaries plus actionable grouped
  details, while JSON serialization still emits the complete per-result
  contract. Added focused renderer regressions for suppressed skip noise and
  the all-skipped explanatory case.

Dependencies:

- T10.

### T49. DONE: Localize human-readable text output to Russian

Make the default text output readable for Russian-speaking 1C developers while
preserving the machine-readable JSON contract.

Acceptance criteria:

- Text output uses Russian section labels for processed count, modified files,
  blocking failures, grouped messages and exit code.
- Text output uses Russian status labels for modifications, warnings, hard
  failures, skips and unsupported scenarios.
- Common built-in scenario messages are rendered in Russian at the presentation
  boundary.
- JSON output keeps the existing stable English keys and severity values.
- CLI error prefixes and merge-skip notices are Russian.

Validation:

- `cargo test -p prec-bsl-output`
- `cargo test -p prec-bsl --test exec_rules`
- `cargo test -p prec-bsl --test prek_hook`
- `cargo test -p prec-bsl --test rat_acceptance`
- `cargo test --workspace`
- `cargo fmt --all --check`
- `git diff --check`

Completion evidence:

- 2026-05-26: Added Russian text labels/statuses and presentation-boundary
  translations for common built-in diagnostics in `prec-bsl-output`, updated
  hook/exec-rules/RAT acceptance assertions, and kept JSON serialization
  unchanged.

Dependencies:

- T48.

### T50. DONE: `УдалениеЛишнихПустыхСтрок` trailing blank run at EOF

prec-bsl issue: `alkoleft/prec-bsl` #11 (bsl-ls parity divergence).

Current finding:

- `remove_extra_blank_lines` kept a trailing blank run of exactly 1 blank line
  at end of file: the final `flush_blank_run` only collapsed runs of 2 or more.
- Files ending with `\n\n` or `\r\n\r\n` were not fixed, diverging from
  `bsl-language-server` behavior where the file ends with exactly one line
  ending.

Resolution:

- Inside a file, runs of 2+ blank lines still collapse to one blank line;
  runs of exactly 1 blank line still stay.
- At end of file, a trailing blank run of any size (including 1) is dropped:
  the file ends with exactly one `\n` or `\r\n`.
- A file consisting only of blank lines becomes empty output `""`.
- A final line without a line ending is not a blank line and is left untouched.
- No scenario setting was added (KISS/YAGNI); this is the default v1
  behavior, captured in `spec/reference-scenario-inventory.md`.

Validation:

- `cargo test --workspace` (343 passed)
- `cargo fmt --check`
- `cargo clippy --workspace`: no new warnings in the changed files
  (`-D warnings` has pre-existing failures on master, unchanged by this task)
- CLI end-to-end: LF/CRLF trailing runs of size 1/2/3 collapse to one line
  ending; a second run reports no changes.

Completion evidence:

- 2026-10-29: Dropped the trailing blank run in
  `crates/prec-bsl-bsl/src/text_fixers.rs`, captured the blank-run contract
  in `spec/reference-scenario-inventory.md`, and added unit, integration and
  golden-fixture coverage in `tests/empty_lines.rs` and
  `tests/fixtures/golden/УдалениеЛишнихПустыхСтрок/хвостовой_пробельный_блок/`.

### T51. DONE: `ПроверкаПереносовСтрок` (IncorrectLineBreak parity, issue #5)

prec-bsl issue: `alkoleft/prec-bsl` #5 (bsl-ls `IncorrectLineBreak` parity,
parent tracking issue #3 — yaxunit migration).

Current finding (at task creation):

- `bsl-ls` `IncorrectLineBreakDiagnostic` flags hanging `;` / `)` / `);` / `,`
  at the start of a line and binary operators `И`/`ИЛИ`/`OR`/`AND`/`+`/`-`/`/`/
  `%`/`*` at the end of a line. The corpus run over yaxunit `.bsl` files
  reports ~317 findings (FIRST 268: `;` 259, `)` 9, `,` 1; END 49).
- No built-in Rust scenario covered this class; stateful line
  classification (multi-line `"` strings with `""` escaping, query regions
  inside string literals) was required to avoid the hundreds of false
  positives a naive per-line grep produces.

Resolution:

- Implemented `ПроверкаПереносовСтрок` as a
  `ScenarioSupport::Compatibility` scenario in
  `crates/prec-bsl-bsl/src/line_break.rs`: a stateful line classifier
  (`in_string` with `""` escaping, `in_query` from a query keyword to the
  closing quote), a per-line violation detector, and a mechanically safe
  fixer (glue `;`/`)`/`);` to the previous line, move a trailing operator to
  the next line) with idempotence and line-ending preservation.
- Settings `НастройкиСценариев.ПроверкаПереносовСтрок.*` (ASCII bsl-ls keys):
  `checkFirstSymbol`, `listOfIncorrectFirstSymbol`, `checkLastSymbol`,
  `listOfIncorrectLastSymbol`; invalid values produce per-file hard failures.
- Contract captured in `spec/parser-strategy.md` (stateful classifier,
  4 deliberate non-parity points, mode contract, fix mechanics, check-only
  tokens, known limitations), the extension inventory row in
  `spec/reference-scenario-inventory.md`, and the settings surface in
  `spec/configuration.md`.

Validation:

- `cargo test -p prec-bsl-bsl line_break` (46 module tests)
- `cargo test --test line_break` (16 tests, incl. CLI JSON, check-after-fix
  and the cross-scenario invariant)
- `YAXUNIT_CORPUS=... cargo test --test line_break_parity` against the
  committed yaxunit fingerprint baseline (auto-skip without the corpus)
- `cargo test --workspace` (416 passed)
- `cargo fmt --check`, `cargo clippy --all-targets` (no new findings in the
  task files)

Completion evidence:

- 2026-10-30: Detector, fixer, handler, settings and registration landed in
  `crates/prec-bsl-bsl/src/line_break.rs` (commits d053483, 1a69c40, 67e0c10,
  42702e8, d5101be, 5cbdb57, 23ab28a); the yaxunit parity baseline was
  committed as `tests/fixtures/parity/ilb-fingerprint.txt` with the
  auto-skipping corpus test `tests/line_break_parity.rs`.

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

- Two lexical extension fixers (Compatibility support: enabled through
  `ГлобальныеСценарии`, `.os`
  suffix lookup supported; since T53 the built-in defaults also enable every
  supported scenario, so they are default-on with opt-out through
  `ОтключенныеСценарии`) in `crates/prec-bsl-bsl/src/text_fixers.rs`:
  `ПробелыВокругОператоров` (bsl-ls MissingSpace operator subset
  `+ - * / % = < > <> <= >=`, unary `-`/`+` after an operator byte, `(`, `[`,
  `,`, the keyword `Возврат`/`Return` case-insensitively or file start
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
  commas, numeric exponents, unary signs after non-Возврат keywords.

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
- 2026-10-31 unary-context bugfix (human report after a `fix` run on
  yaxunit: `Возврат -1;` was rewritten to `Возврат - 1;`): the unary set was
  realigned with the bsl-ls 1.0.7 UNARY semantics — an operator byte
  (`+ - * / % = < >`), `(`, `[`, `,`, the keyword `Возврат`/`Return`
  case-insensitively, or file start; the scanner Code event grew
  `prev_code_index` so the fixer can scan the preceding word backwards.
  Unary signs are now left fully untouched (no space to the right, matching
  `checkSpaceToRightOfUnary = false`). Gates after the fix:
  `cargo test --workspace` 482 passed / 0 failed; env-gated parity run
  green with unchanged counts (pre 104/2 → post 84/0, residue 81/2/1).

Validation:

- `cargo test --workspace`
- `cargo fmt --check`
- env-gated parity test (`PREC_BSL_PARITY_CORPUS`, bsl-ls 1.0.7 jar)

Dependencies:

- T38, T39 (crate layout and scenario definition binding).
