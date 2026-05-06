# prec-bsl Implementation Todo

This ledger tracks executable implementation work for the current v1 scope from
`spec/prd-prec-bsl.md`, `spec/configuration.md`, `spec/parser-strategy.md`, and
`spec/testing-strategy.md`.

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

### T2. TODO: Add CLI skeleton and command contract

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

Dependencies:

- T1 can be implemented independently, but CLI names must match the manifest.

## Milestone 1: Parity Baseline

### T3. TODO: Capture reference precommit4onec scenario inventory

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

Dependencies:

- T2.

### T4. TODO: Build golden fixture harness

Add a fixture test harness that can compare input, output, diagnostics, and
idempotence for fixer and checker scenarios.

Acceptance criteria:

- Fixtures support Cyrillic paths and scenario names.
- Fixer tests can assert first-run modifications and second-run clean state.
- Checker tests can assert diagnostics without modifying input.
- Expected output files are deterministic and reviewable in Git.

Validation:

- `cargo test fixtures`

Dependencies:

- T3.

### T5. TODO: Add RAT acceptance harness with copy-only safety

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
- Manual safety check: `git -C /home/alko/develop/open-source/rat status --short`

Dependencies:

- T4.

Non-goals:

- A full green run of live RAT `v8config.json` with repository-local `.os`
  scenarios.

## Milestone 2: Core Architecture Layers

### T6. TODO: Implement configuration model and resolver

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

Dependencies:

- T3.

### T7. TODO: Implement Git index collection and restaging layer

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

Dependencies:

- T2.

### T8. TODO: Implement source root and file classification

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

- `cargo test source_root file_classification`

Dependencies:

- T6.

### T9. TODO: Implement scenario registry and execution pipeline

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

Dependencies:

- T6, T7, T8.

### T10. TODO: Implement diagnostics and output formats

Add text and JSON output for hook and CI use.

Acceptance criteria:

- Text output groups messages by rule and file.
- Modified files are listed separately from hard failures.
- JSON output includes rule id, path, severity, modification flag, message, and
  source span when available.
- Output is deterministic for stable test snapshots.

Validation:

- `cargo test output`

Dependencies:

- T9.

## Milestone 3: Low-Risk Text Fixers and Checks

### T11. TODO: Implement trailing whitespace fixer

Scenario: `УдалениеЛишнихКонцевыхПробелов`.

Acceptance criteria:

- Removes trailing spaces and tabs without changing unrelated text.
- Preserves line endings according to fixture expectations.
- Is idempotent.
- Reports modified files.

Validation:

- `cargo test trailing_whitespace`

Dependencies:

- T4, T9.

### T12. TODO: Implement extra blank line fixer

Scenario: `УдалениеЛишнихПустыхСтрок`.

Acceptance criteria:

- Removes excessive blank lines according to parity fixtures.
- Preserves meaningful module spacing.
- Is idempotent.

Validation:

- `cargo test empty_lines`

Dependencies:

- T11.

### T13. TODO: Implement keyword spacing fixer

Scenario: `ДобавлениеПробеловПередКлючевымиСловами`.

Acceptance criteria:

- Matches parity fixtures for required BSL keywords.
- Avoids changes inside string literals and comments where parity requires.
- Is idempotent.

Validation:

- `cargo test keyword_spacing`

Dependencies:

- T12.

### T14. TODO: Implement canonical spelling fixer

Scenario: `ИсправлениеНеКаноническогоНаписания`.

Acceptance criteria:

- Normalizes known non-canonical keyword spellings from fixtures.
- Handles Russian and English spellings covered by specs.
- Avoids string/comment rewrites unless parity requires them.
- Is idempotent.

Validation:

- `cargo test canonical_spelling`

Dependencies:

- T13.

### T15. TODO: Implement profanity checker

Scenario: `ПроверкаНецензурныхСлов`.

Acceptance criteria:

- Loads the configured profanity dictionary when present.
- Reports matched words with file path and rule id.
- Handles missing dictionary according to config/default policy.
- Does not modify files.

Validation:

- `cargo test profanity`

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
