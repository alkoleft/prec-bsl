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
