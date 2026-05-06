# prec-bsl Implementation Todo

This ledger tracks executable implementation work for the current v1 scope from
`spec/prd-prec-bsl.md`, `spec/configuration.md`, `spec/parser-strategy.md`, and
`spec/testing-strategy.md`. Reference scenario inventory evidence is captured in
`spec/reference-scenario-inventory.md`.

Roadmap style:

- Statuses are backlog-only for now: every task starts as `TODO`.
- Sequencing is parity-first and layer-first: establish reference behavior and
  shared architecture before filling scenario implementations.
- Scenario ordering is risk-based: start with deterministic text fixers, then
  parser-backed diagnostics, XML/EDT transformations, metadata scenarios, and
  platform-dependent work.
- This file is the implementation ledger, not a replacement for the specs.

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
  `src/scenarios.rs`, and
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

- Removes trailing spaces and tabs without changing unrelated text.
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

### T16. TODO: Implement copyright insertion fixer

Scenario: `ВставкаКопирайтов`.

Acceptance criteria:

- Inserts configured copyright text according to parity fixtures.
- Handles files with existing headers.
- Is idempotent.

Validation:

- `cargo test copyright`

Dependencies:

- T6, T14.

## Milestone 4: Parser-Backed BSL Scenarios

### T17. TODO: Integrate shared tree-sitter-bsl parser module

Add the parser foundation required by syntax-aware scenarios.

Acceptance criteria:

- Uses `tree-sitter = "0.25"` and `tree-sitter-bsl = "0.1"` unless compatibility
  validation changes the decision.
- Parser initialization is shared.
- UTF-8 source byte offsets are preserved.
- Parse errors are exposed to scenarios without forcing a global hard failure.

Validation:

- `cargo test bsl_parser`

Dependencies:

- T10.

### T18. TODO: Implement goto checker

Scenario: `ЗапретИспользованияПерейти`.

Acceptance criteria:

- Detects `Перейти` / `goto` as syntax, not raw text.
- Reports source spans.
- Does not flag string literals or comments.

Validation:

- `cargo test goto`

Dependencies:

- T17.

### T19. TODO: Implement duplicate procedure/function checker

Scenario: `ПроверкаДублейПроцедурИФункций`.

Acceptance criteria:

- Collects procedure and function names from syntax trees.
- Detects duplicates according to parity fixtures.
- Reports all duplicate definitions with paths and spans.

Validation:

- `cargo test duplicate_methods`

Dependencies:

- T17.

### T20. TODO: Implement preprocessor instruction checker

Scenario: `ПроверкаКорректностиИнструкцийПрепроцессора`.

Acceptance criteria:

- Uses parser coverage for preprocessor nodes and error nodes.
- Defines which parse errors are blocking for this scenario.
- Covers broken syntax and incomplete directive fixtures.

Validation:

- `cargo test preprocessor`

Dependencies:

- T17.

### T21. TODO: Implement region correctness checker

Scenario: `ПроверкаКорректностиОбластей`.

Acceptance criteria:

- Checks `#Область` / `#КонецОбласти` balance and ordering.
- Uses parser coverage where sufficient.
- Uses a scenario-specific lexical stack fallback where parser coverage is not
  enough.
- Reports precise diagnostics.

Validation:

- `cargo test regions`

Dependencies:

- T17, T20.

### T22. TODO: Implement unit test processing scenario

Scenario: `ОбработкаЮнитТестов`.

Acceptance criteria:

- Locates procedures/functions and loader methods using parser-backed discovery.
- Matches required parity fixtures for test modules.
- Reports or modifies only the files expected by fixtures.
- Is idempotent where it modifies files.

Validation:

- `cargo test unit_tests_processing`

Dependencies:

- T17, T21.

## Milestone 5: XML and EDT Scenarios

### T23. TODO: Add XML/EDT parser and writer layer

Implement structured XML handling for metadata scenarios.

Acceptance criteria:

- Reads `.mdo`, `.form`, and relevant XML metadata files.
- Avoids regex-only XML rewrites for structured transformations.
- Preserves formatting well enough for deterministic fixture diffs.
- Reports parse errors with paths.

Validation:

- `cargo test xml_edt`

Dependencies:

- T8, T10.

### T24. TODO: Implement XML form correction

Scenario: `КорректировкаXMLФорм`.

Acceptance criteria:

- Discovers `Form.form` files.
- Applies parity transformations.
- Is idempotent.

Validation:

- `cargo test xml_forms`

Dependencies:

- T23.

### T25. TODO: Implement full-text search disabling

Scenario: `ОтключениеПолнотекстовогоПоиска`.

Acceptance criteria:

- Discovers relevant EDT/Designer metadata files.
- Disables full-text search settings according to parity fixtures.
- Is idempotent.

Validation:

- `cargo test disable_full_text_search`

Dependencies:

- T23.

### T26. TODO: Implement form-change permission disabling

Scenario: `ОтключениеРазрешенияИзменятьФорму`.

Acceptance criteria:

- Applies required metadata transformations.
- Handles missing properties clearly.
- Is idempotent.

Validation:

- `cargo test disable_form_change`

Dependencies:

- T23.

## Milestone 6: Metadata and Composition Scenarios

### T27. TODO: Implement metadata-object/file synchronization

Scenario: `СинхронизацияОбъектовМетаданныхИФайлов`.

Acceptance criteria:

- Detects metadata objects and corresponding files.
- Appends generated or repaired files to the processing queue when needed.
- Restages generated or modified files in hook mode.
- Produces deterministic diagnostics.

Validation:

- `cargo test metadata_sync`

Dependencies:

- T9, T23.

### T28. TODO: Implement composition sorting

Scenario: `СортировкаСостава`.

Acceptance criteria:

- Sorts metadata composition according to parity fixtures.
- Preserves valid XML/EDT structure.
- Is idempotent.

Validation:

- `cargo test composition_sort`

Dependencies:

- T23.

### T29. TODO: Implement duplicate metadata removal

Scenario: `УдалениеДублейМетаданных`.

Acceptance criteria:

- Detects duplicate metadata entries according to fixtures.
- Removes or reports duplicates according to parity behavior.
- Is idempotent where it modifies files.

Validation:

- `cargo test duplicate_metadata`

Dependencies:

- T23, T28.

## Milestone 7: Platform-Dependent Scenario

### T30. TODO: Implement external reports/processings/extensions scenario boundary

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

Dependencies:

- T9, T10.

Non-goals:

- Implementing `РазборОбычныхФормНаИсходники`.

## Milestone 8: End-to-End Hook and CI Readiness

### T31. TODO: Wire `prek-hook` end-to-end

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

Dependencies:

- T6, T7, T8, T9, T10, T11.

### T32. TODO: Wire `exec-rules` end-to-end

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

Dependencies:

- T6, T8, T9, T10.

### T33. TODO: Run RAT parser coverage acceptance

Run the parser coverage baseline over RAT `.bsl` files.

Acceptance criteria:

- Parser initialization succeeds.
- Parse errors are counted and reported with paths.
- Results compare published crate behavior with the local grammar checkout only
  when parser gaps block required scenarios.

Validation:

- `cargo test rat_parser_coverage`

Dependencies:

- T5, T17.

### T34. TODO: Run text fixer idempotence acceptance on RAT copy

Run low-risk text fixer scenarios on a temporary RAT copy.

Acceptance criteria:

- First run reports modified files where applicable.
- Second run is clean for the same scenario set.
- Real RAT checkout remains unchanged.

Validation:

- `cargo test rat_text_idempotence`
- `git -C /home/alko/develop/open-source/rat status --short`

Dependencies:

- T5, T11, T12, T13, T14, T16.

### T35. TODO: Run XML/EDT acceptance on RAT copy

Run XML/EDT scenarios on a temporary RAT copy with `.mdo` and `.form` files.

Acceptance criteria:

- `Configuration.mdo` and object `.mdo` files are discovered.
- `Form.form` files are discovered.
- Scenario outputs are checked by golden diff or idempotence.
- Real RAT checkout remains unchanged.

Validation:

- `cargo test rat_xml_edt`
- `git -C /home/alko/develop/open-source/rat status --short`

Dependencies:

- T5, T23, T24, T25, T26.

### T36. TODO: Prepare v1 release readiness checklist

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

Dependencies:

- T31, T32, T33, T34, T35.
