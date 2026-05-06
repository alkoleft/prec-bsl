# Testing Strategy

## Decision

Use `/home/alko/develop/open-source/rat` as the primary real-world acceptance corpus for `prec-bsl`.

The `rat` repository must be treated as an external fixture, not as a workspace where `prec-bsl` tests freely rewrite files. Tests that can modify files must run against a temporary copy or a generated fixture subset.

## RAT Corpus

Repository path:

```text
/home/alko/develop/open-source/rat
```

Observed project traits:

- `v8project.yaml` exists.
- `v8config.json` exists.
- Source format is EDT.
- Source sets:
  - `fixtures/configuration`
  - `exts/rat`
  - `tests`
- Source corpus includes `.bsl`, `.mdo`, `.form`, and `Configuration.mdo`.
- `v8config.json` has `GLOBAL.ФорматEDT = true`.
- `v8config.json` has `GLOBAL.ВерсияПлатформы = "8.3.20.1996"`.
- Current working tree may contain untracked `.build/`; `prec-bsl` tests must not depend on or clean that directory.

## RAT Config Caveat

The current `rat/v8config.json` is a valuable compatibility fixture, but not a direct green-path v1 config fixture.

It includes:

- `ИспользоватьСценарииРепозитория = true`
- `КаталогЛокальныхСценариев = "tools/pre-commit"`
- repository-local scenario ids such as:
  - `СортировкаДереваМетаданных.os`
  - `СортировкаСоставаПодсистем.os`
  - `ДобавлениеТестовВРасширение`

Dynamic execution of repository-local `.os` scenarios is out of v1 scope. Therefore:

- Use the live `rat/v8config.json` as a config parsing and diagnostics fixture.
- A full green `prec-bsl prek-hook --config /home/alko/develop/open-source/rat/v8config.json` is not required until local scenario handling is designed.
- Unknown or repository-local scenario diagnostics must clearly name the unsupported scenario and explain that dynamic local `.os` execution is not supported in v1.

For green-path acceptance, generate a test-specific `v8config.json` from the required v1 scenario list and run it against a temporary copy of the `rat` source roots.

## Required Acceptance Checks

### Parser Coverage

Run `tree-sitter-bsl` over all RAT `.bsl` files in:

- `fixtures/configuration/src`
- `exts/rat/src`
- `tests/src`

Acceptance:

- Parser initialization succeeds.
- Parse errors are counted and reported with paths.
- Syntax-aware scenarios can choose whether parse errors are blocking or fallback-compatible according to `spec/parser-strategy.md`.

### Text Fixer Idempotence

Run text fixer scenarios on a temporary RAT copy.

Acceptance:

- First run reports modified files where applicable.
- Second run over the same temporary copy is clean for the same scenario set.
- No changes are written to the real `/home/alko/develop/open-source/rat` checkout.

### XML/EDT Coverage

Run XML/EDT scenarios on a temporary RAT copy containing `.mdo` and `.form` files.

Acceptance:

- `Configuration.mdo` and object `.mdo` files are discovered.
- `Form.form` files are discovered.
- Scenario output can be diffed against golden outputs or checked for idempotence.

### Config Compatibility

Run config parsing against the live `rat/v8config.json`.

Acceptance:

- `GLOBAL` and `Precommt4onecСценарии` parse successfully.
- Disabled base scenarios such as `РазборОбычныхФормНаИсходники` do not fail merely because they are listed under `ОтключенныеСценарии`.
- Enabled repository-local scenarios are reported as unsupported in v1 unless a local-scenario compatibility mode is later accepted.

### Platform-Dependent Scenario

For `РазборОтчетовОбработокРасширений`, use RAT only when the required 1C platform executable and runtime inputs are available.

Acceptance:

- Missing platform/runtime is reported as an environment skip or explicit dependency error, not as a parser/config failure.
- Tests that require platform execution are separated from pure Rust unit and fixture tests.

## Test Safety Rules

- Never run mutating acceptance tests directly on `/home/alko/develop/open-source/rat`.
- Copy only the needed source roots into a temp directory.
- Run Git status probes against RAT with `GIT_OPTIONAL_LOCKS=0` so safety checks
  do not refresh or lock the external checkout index.
- Keep generated artifacts under `target/` or a test temp directory.
- Do not clean, reset, or modify the RAT repository.
- Do not rely on RAT's untracked `.build/`.

## PRD Impact

The acceptance baseline now includes a real EDT 1C repository:

- Unit tests validate individual scenario logic.
- Fixture tests validate generated edge cases.
- RAT acceptance tests validate behavior against a real source tree and real `v8config.json` compatibility constraints.
