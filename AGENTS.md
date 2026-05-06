# Project Rules for Agents

These rules apply to the whole repository.

## Project Context

`prec-bsl` is a Rust hook package for `prek`/`pre-commit` users who keep
1C:Enterprise BSL sources in Git. The project provides a `prec-bsl` CLI binary,
a `.pre-commit-hooks.yaml` hook manifest, and a staged-file processing pipeline
that preserves the operational mechanics of `precommit4onec` without requiring
OScript for the built-in Rust v1 scenario set.

The project owns:

- hook repository manifest behavior for `prek`/`pre-commit`;
- CLI contracts for `prek-hook` and `exec-rules`;
- `v8config.json` parsing and resolution for the supported compatibility
  surface;
- Git staged-file discovery, processing queue behavior and restaging;
- source-root resolution and BSL/XML/EDT file classification;
- built-in scenario registry, fixers, diagnostics and output formats;
- fixture and acceptance baselines for the required v1 scenarios.

The project does not own `prek` internals, `pre-commit` internals, OScript
runtime behavior, repository-local `.os` scenario execution, or general 1C
platform orchestration. Platform execution is allowed only for explicitly
specified scenarios such as `РазборОтчетовОбработокРасширений`, and must remain
separate from pure Rust hook/check paths.

Use `spec/` as the durable project source of truth. Start from the relevant
specification files before changing behavior or tasks:

- `spec/prd-prec-bsl.md`
- `spec/configuration.md`
- `spec/parser-strategy.md`
- `spec/testing-strategy.md`
- `spec/IMPLEMENTATION_TODO.md`

`spec/IMPLEMENTATION_TODO.md` is the active implementation ledger. When chat,
README, code comments, copied prompts or task text conflict with `spec/`,
reconcile the relevant spec before implementation.

## Context Boundaries

Keep context boundaries explicit and narrow.

- Separate hook manifest, CLI argument parsing, configuration loading,
  Git-index access, source-root discovery, file classification, scenario
  registry, scenario execution, diagnostics/output and test harness concerns.
- Treat the Git index as the source of truth for commit-mode processing:
  `prek` filenames are not the contract for staged-file selection.
- Keep `.pre-commit-hooks.yaml` as hook repository metadata only. Do not encode
  repository-specific `v8config.json` settings, source roots, secrets or large
  scenario settings there.
- Keep end-user hook selection (`prek.toml` or `.pre-commit-config.yaml`)
  limited to small stable CLI options such as `--config`, `--source-dir`,
  `--rules` and `--format`.
- Keep `v8config.json` as the domain configuration layer, including the
  historic compatibility key `Precommt4onecСценарии`.
- Keep built-in Rust scenario behavior separate from unsupported
  repository-local `.os` execution. Unsupported enabled scenarios must produce
  clear diagnostics instead of being silently ignored.
- Keep BSL text fixers, `tree-sitter-bsl` syntax-aware checks, XML/EDT
  transformations and platform-dependent scenarios in separate modules.
- Use structured XML parsing for XML/EDT transformations where practical
  instead of broad regex rewrites.
- Keep runtime 1C process execution out of ordinary hook, parser and config
  paths unless the active task and spec explicitly require it.
- Validation belongs at boundaries: CLI parsing, configuration loading,
  filesystem input, Git command results, source-root discovery, parser/XML
  input, scenario registry lookup and output serialization.

## Modeling Rules

- Scenario identity must match names with and without the `.os` suffix.
- Preserve the required v1 scenario list and the explicit unsupported decision
  for `РазборОбычныхФормНаИсходники`.
- Keep scenario configuration semantics compatible with the supported
  `precommit4onec` surface: global scenarios, disabled scenarios, scenario
  settings, repository scenario settings and project-specific overrides.
- Project-specific settings fully override base settings for matching source
  subpaths when that contract is implemented.
- Represent staged file status explicitly: added, modified, deleted, renamed,
  copied and unknown.
- Deleted files must be representable without requiring file contents.
- Preserve source-root context per processed file.
- Scenario results must distinguish modifications, warnings, hard failures,
  skips and unsupported scenarios.
- Hook mode exits with `0` only when there are no blocking diagnostics and no
  unreviewed modifications remain.
- No silent file changes: modified paths must be reported and restaging must be
  explicit in the Git layer.
- Use typed domain structures instead of repeated primitive tuples, stringly
  typed markers or duplicated ad hoc checks.

## Parser and Runtime Rules

- Use `tree-sitter-bsl` for syntax-aware BSL scenarios listed in
  `spec/parser-strategy.md`.
- Do not use tree-sitter to reformat or regenerate BSL source. Use it only to
  identify syntax ranges and semantic structure.
- Treat parser errors as scenario-specific signals. Some checks may continue
  with lexical fallback when the spec allows it.
- Do not patch around parser gaps with broad regex fallbacks before evaluating
  the gap against the local `tree-sitter-bsl` grammar workflow described in
  `spec/parser-strategy.md`.
- Text-parity fixers may stay lexical when exact text preservation is the main
  contract.
- Platform-dependent scenario tests must report missing platform/runtime as an
  environment skip or explicit dependency error, not as a parser/config failure.

## Design Principles

Optimize for simple, direct, maintainable code.

- KISS: choose the smallest design that expresses the current behavior clearly.
- YAGNI: do not add compatibility bridges, extension points, caches, generic
  pipelines or configuration knobs until there is a concrete requirement.
- DRY: extract shared behavior when duplication starts encoding the same rule in
  more than one place. Do not create premature abstractions for merely similar
  code.
- Do enough upfront design to define context boundaries, data contracts,
  invariants and failure modes before implementation. Do not turn this into
  speculative architecture for unvalidated future integrations.
- When two designs satisfy the same contract, choose the one with fewer moving
  parts.
- Keep modules talking through their immediate public interfaces. Avoid binding
  one context to another context's representation details.
- Each module/type should have one reason to change.
- Expose small interfaces focused on real consumers.
- Prefer Rust-native models and algorithms over reproducing OScript internals
  as public APIs.
- Use typed errors instead of panics for recoverable input, discovery, parsing,
  Git, scenario and output failures.

## Testing Rules

Test behavior, not implementation.

- Treat the unit of testing as a unit of behavior: a public CLI contract,
  config parse result, Git-index result, source discovery result, parser
  outcome, scenario result, diagnostic contract, serialized output or hook exit
  behavior.
- Do not test private implementation details, helper call order, internal
  struct layout or incidental decomposition.
- Favor externally observable contracts: returned data, statuses, order,
  errors, diagnostics, modified paths, serialized output and exit codes.
- Use small deterministic fixtures for parser and scenario behavior. Fixtures
  should represent real BSL, Designer XML, EDT `.mdo`/`.form`, external
  reports/processings/extensions or `v8config.json` structures.
- Regression tests should describe the user-visible or contract-visible
  behavior being protected.
- Fixer scenarios must prove idempotence where the spec requires it.
- Add broader tests when a change crosses module boundaries or changes a public
  contract; keep tests focused for local implementation changes.
- Use `/home/alko/develop/open-source/rat` only as an external read-only
  acceptance corpus. Mutating checks must run against a temporary copy or a
  generated fixture subset.
- Never clean, reset or modify the RAT repository from tests or acceptance
  scripts.

## Subagent Usage

- Use subagents for non-trivial implementation, parser, XML/EDT, platform,
  performance, architecture or cross-module changes when subagents are
  available and the task has meaningful behavioral or resource-impact risk.
- Prefer independent subagent passes for evidence gathering, test execution and
  code review before finalizing risky changes.
- Keep deterministic repository operations in the main session: spec updates,
  final verification, staging, commits and reconciliation of subagent findings.
- Delegate only bounded, self-contained work with clear read/write scope.
- Do not use subagents for trivial docs-only edits or tasks where delegation
  would add more coordination than value.

## Implementation Discipline

For non-trivial work, follow this order:

1. Read the relevant files in `spec/`, especially `spec/IMPLEMENTATION_TODO.md`
   and the specs referenced by the active task.
2. If the requested behavior is not covered, update the appropriate spec before
   implementation.
3. Add or update use cases, acceptance notes, fixtures or baseline expectations
   when behavior is visible through APIs, CLI, files, diagnostics, serialized
   output or hook exit codes.
4. Add or update the first active task in `spec/IMPLEMENTATION_TODO.md`,
   referencing the relevant spec files when needed.
5. Implement only that task and its direct verification unless the prompt
   explicitly asks for broader scope.
6. After verification, update `spec/` when implemented behavior, measurements,
   task status or durable conclusions changed.

Follow the active implementation ledger before adding new scope. Keep public
contracts provisional unless the specs explicitly stabilize them.

Keep implementation contexts separated into focused Rust modules. Do not grow a
large mixed `src/main.rs`/`src/lib.rs` with CLI, config, Git, parser, scenario,
diagnostics and test harness logic in one place once behavior expands beyond a
bootstrap slice.

Keep generated outputs, acceptance artifacts and experimental data out of source
files unless the plan asks for durable artifacts.

Do not introduce unrelated refactors while implementing a task.
