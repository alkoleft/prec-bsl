# prec-bsl PRD

## 1. Executive Summary

`prec-bsl` is a Rust hook package for `prek`/`pre-commit` users who keep 1C:Enterprise BSL sources in Git and need the same operational mechanics as `precommit4onec` without requiring OScript on every workstation. The product provides a `prec-bsl` CLI binary, a `.pre-commit-hooks.yaml` manifest for `prek`, and a staged-file processing pipeline that reads Git index changes, applies built-in BSL/XML/EDT fixers and checks, restages modified files, and exits with deterministic hook results.

## 2. Problem Statement

### Who has this problem?

Primary users are 1C/BSL developers and platform teams maintaining Designer or EDT source repositories. Secondary users are CI maintainers who want the same checks in local hooks and automated validation.

### What is the problem?

The current reference implementation, `/usr/share/oscript/lib/precommit4onec`, is useful but tied to OScript distribution, global/local OScript configuration, and dynamically loaded `.os` scenario files. Teams adopting `prek` need a Rust-native hook repository that installs and runs as a normal `language: rust` hook while preserving the familiar `precommit4onec` behavior for staged files and explicit full-tree rule execution.

### Why is it painful?

- Developers need BSL-specific checks before commit, not only generic whitespace hooks.
- The hook must process only staged changes during a commit, then add generated or modified files back to the index.
- 1C repositories may contain multiple source roots, Designer XML, EDT `.mdo`/`.form`, external reports/processings/extensions, and test extensions.
- Teams need predictable CI/local parity and no workstation dependency on OScript for the common path.

### Evidence

- `precommit4onec precommit` reads `git diff --name-status --staged --no-renames`, maps Git statuses to domain statuses, processes each staged file through configured scenarios, appends post-processing files to the processing queue, and restages modified paths.
- `precommit4onec exec-rules` runs selected rules over one or more source directories, supports `-rules`, `-source-dir`, and `-cfg-file`, and aggregates critical errors at the end.
- The default `v8config.json` enables 19 built-in scenarios and stores scenario settings under the historic key `Precommt4onecСценарии`.
- `prek` hook repositories use the upstream `.pre-commit-hooks.yaml` manifest, and Rust hooks are installed through `cargo install --bins --locked`.

## 3. Target Users & Personas

### Primary Persona: 1C Repository Maintainer

- Role: senior 1C developer or tech lead responsible for source cleanliness.
- Goals: prevent broken BSL/XML/EDT sources from entering Git, keep pre-commit behavior close to the existing `precommit4onec` setup, avoid local OScript drift.
- Pain points: mixed source formats, generated files, Cyrillic paths, local/global settings, hook failures that do not explain which file/rule failed.

### Secondary Persona: CI Maintainer

- Role: DevOps or platform engineer maintaining GitHub/GitLab pipelines.
- Goals: run the same rules in CI as developers run locally, pin hook revisions, cache Rust hook installation.
- Pain points: non-reproducible global tools, unclear exit codes, hooks that silently modify files without reporting what changed.

## 4. Strategic Context

`prek` positions itself as a fast Rust implementation compatible with pre-commit configurations and hooks. That makes a Rust BSL hook package the right distribution shape: this repository should be consumed by `prek` as a standard hook repository, not as a fork or patch of `prek`.

The initiative matters because BSL projects have domain-specific source hygiene requirements that generic hook collections do not cover. A Rust implementation can keep the common checks dependency-light, fast, and easier to run in CI, while preserving the established `precommit4onec` mental model for teams already using it.

## 5. Solution Overview

`prec-bsl` will provide:

- A Rust CLI binary named `prec-bsl`.
- A root `.pre-commit-hooks.yaml` with at least one hook id, `prec-bsl`, configured as `language: rust` and `entry: prec-bsl prek-hook`.
- A `prek-hook` command that runs under `prek` but uses the Git index as the source of truth for commit-mode processing.
- A `run` or `precommit` command that mirrors `precommit4onec precommit`: repository path, source directory, staged-file discovery, scenario pipeline, post-processing queue, restaging changed files.
- An `exec-rules` command that mirrors explicit rule execution over one or more source directories.
- A `v8config.json` domain configuration file compatible with the existing `Precommt4onecСценарии` shape.

The internal model should separate:

- Git status collection and restaging.
- Configuration resolution.
- Project/source-root resolution.
- Scenario registry and scenario execution.
- File type classification.
- Text/XML transformations.
- Diagnostics and exit-code aggregation.

## 6. Success Metrics

### Primary Metric

Adoption readiness: a 1C repository can replace an existing `precommit4onec` hook with `prek` + `prec-bsl` and get equivalent results for the required v1 scenario set.

Target: 100% parity on staged-file selection, scenario ordering, post-processing queue behavior, and restaging behavior for covered scenarios.

### Secondary Metrics

- Hook install path works through `prek` without requiring OScript for covered scenarios.
- `prec-bsl exec-rules` can run selected rules over multiple source roots.
- Diagnostics include rule id, path, severity, and whether a file was modified.
- Golden fixture tests cover Designer XML and EDT source layouts.
- Acceptance tests use `/home/alko/develop/open-source/rat` as the primary real-world EDT corpus without mutating that checkout.

### Guardrail Metrics

- No silent file changes: modified paths must be reported and should return non-zero in hook mode so the user can review/re-run.
- No hidden fallback to OScript for built-in Rust scenarios.
- No network access during hook execution.

## 7. User Stories & Requirements

### Epic Hypothesis

We believe that a Rust-native `prec-bsl` hook package for `prek` will reduce workstation setup friction and preserve BSL source hygiene because teams can install it as a standard Rust hook while keeping the core `precommit4onec` mechanics.

### Story 1: Install through prek

As a repository maintainer, I want to reference `prec-bsl` from `.pre-commit-config.yaml`/`prek.toml`, so developers can install and run the hook with `prek`.

Acceptance criteria:

- The repository contains `.pre-commit-hooks.yaml`.
- The hook id is stable and documented.
- The hook uses `language: rust`, and the binary name matches `Cargo.toml`.
- The hook supports invocation from `prek` while collecting staged file state from Git.

### Story 2: Process staged files like precommit4onec

As a developer, I want the hook to process the files staged in Git, so only commit-relevant BSL/XML/EDT files are checked or fixed.

Acceptance criteria:

- The command collects `git diff --name-status --staged --no-renames`.
- Git statuses are mapped to `added`, `modified`, `deleted`, `renamed`, `copied`, and unknown.
- Deleted files are passed only to scenarios that explicitly need deletion context.
- New files generated by scenarios can be appended to the processing queue.
- Modified files/directories are restaged after successful fixer scenarios.

### Story 3: Preserve scenario configuration semantics

As a maintainer, I want to enable, disable, and configure scenarios using familiar concepts, so migration from `precommit4onec` is explicit.

Acceptance criteria:

- Config supports global scenario list, disabled scenario list, scenario settings, repository scenario settings, and project-specific overrides.
- Project-specific settings fully override base settings for matching source subpaths.
- Scenario names can be referenced with or without `.os` suffix for compatibility.
- The misspelled historic key `Precommt4onecСценарии` is accepted by the JSON parser.

### Story 4: Run explicit rules over source roots

As a CI maintainer, I want to run selected rules over the whole source tree, so CI can validate more than the staged diff.

Acceptance criteria:

- `exec-rules` accepts repository path, comma-separated source roots, comma-separated rule names, and optional config path.
- Missing source roots are reported.
- Multiple source roots preserve the correct source-root context per file.
- Critical errors are accumulated and printed after traversal.

### Story 5: Provide required v1 scenarios

As a BSL developer, I want all selected `precommit4onec` scenarios covered in v1, so migration does not lose existing repository checks.

Acceptance criteria:

- v1 implements these scenarios:
  - `ВставкаКопирайтов`
  - `УдалениеЛишнихКонцевыхПробелов`
  - `УдалениеЛишнихПустыхСтрок`
  - `ДобавлениеПробеловПередКлючевымиСловами`
  - `ИсправлениеНеКаноническогоНаписания`
  - `ЗапретИспользованияПерейти`
  - `КорректировкаXMLФорм`
  - `ОбработкаЮнитТестов`
  - `ОтключениеПолнотекстовогоПоиска`
  - `ОтключениеРазрешенияИзменятьФорму`
  - `ПроверкаДублейПроцедурИФункций`
  - `ПроверкаКорректностиИнструкцийПрепроцессора`
  - `ПроверкаКорректностиОбластей`
  - `ПроверкаНецензурныхСлов`
  - `РазборОтчетовОбработокРасширений`
  - `СинхронизацияОбъектовМетаданныхИФайлов`
  - `СортировкаСостава`
  - `УдалениеДублейМетаданных`
- Each scenario has fixtures proving parity against representative BSL files.
- Fixers are idempotent.
- `РазборОбычныхФормНаИсходники` is explicitly not implemented.

### Story 6: Support XML/EDT and platform-dependent scenarios

As a repository maintainer, I want Designer XML, EDT metadata, and external report/processing/extension handling to be covered with clear runtime boundaries.

Acceptance criteria:

- The scenario registry marks `РазборОбычныхФормНаИсходники` as unsupported by product decision.
- `РазборОтчетовОбработокРасширений` is implemented as a required scenario with explicit 1C platform/runtime dependency handling.
- XML/EDT transformations use structured XML parsing where possible instead of regex-only rewrites.

### Story 7: Explain hook results

As a developer, I want clear output when a commit is blocked, so I can fix or restage quickly.

Acceptance criteria:

- Output groups messages by rule and file.
- File modifications are listed separately from hard failures.
- Exit code is `0` only when no blocking diagnostics and no unreviewed modifications remain.
- `--json` output is available for CI.

## 8. Out of Scope

Not included in the first release:

- Dynamic execution of repository-local `.os` scenarios without OScript.
- Full interactive `configure -config` parity.
- `v8unpack` integration for ordinary form binary unpacking.
- `РазборОбычныхФормНаИсходники`.
- A custom `prek` language backend or fork of `prek`.

Future consideration:

- Compatibility bridge that shells out to OScript for explicitly configured legacy scenarios.
- Richer BSL parser integration for syntax-aware checks.

## 9. Dependencies & Risks

### Dependencies

- Rust toolchain supported by `prek` hook installation.
- Git CLI available in hook execution environment.
- `tree-sitter` and `tree-sitter-bsl` for BSL syntax-aware scenarios.
- XML parser crate for XML/EDT transformations.
- 1C platform executable discovery for `РазборОтчетовОбработокРасширений`.
- Encoding strategy for UTF-8, UTF-8 BOM, and legacy text files.
- Fixture corpus covering Designer and EDT source layouts.
- Read-only access to `/home/alko/develop/open-source/rat` for real-world acceptance tests.

### Risks & Mitigations

- Risk: regex parity for BSL code rewrites differs from OScript behavior.
  - Mitigation: build golden fixtures from local `precommit4onec` outputs before replacing behavior.
- Risk: `prek` passes filenames while `precommit4onec` uses Git staged diff.
  - Mitigation: keep Git-index discovery as default in `prek-hook`, with an explicit `--use-passed-filenames` escape only if needed later.
- Risk: restaging generated directories surprises users.
  - Mitigation: list restaged paths and return non-zero after modifications in hook mode.
- Risk: XML transformations corrupt metadata formatting.
  - Mitigation: start XML/EDT scenarios after text scenarios, use fixture diffs, and prefer structured parsing.
- Risk: Cyrillic paths or Git quote behavior differ by OS.
  - Mitigation: set up tests with Cyrillic filenames and use Rust path APIs plus `git -c core.quotePath=false` output handling.

## 10. Open Questions

- Should hook mode return non-zero after fixers modify files, matching common pre-commit fixer behavior, or return zero after restaging to match the old direct hook experience?
- Do we need Windows command-shell parity in v1, or is POSIX + Git Bash enough for the first release?
- Should `.os` local scenarios be explicitly unsupported in config validation, or allowed only through a named compatibility adapter?

## 11. Source Mapping

### precommit4onec commands to preserve

- `precommit`: commit hook mode using staged Git changes.
- `exec-rules`: explicit full-tree or source-root rule execution.
- `configure`: config print/reset/edit semantics are documented, but only non-interactive config file support is required for v1.
- `install`: replaced by `prek install`; direct `.git/hooks/pre-commit` writing is not part of the new product.

### Scenario inventory

Required in v1:

- `ВставкаКопирайтов`
- `ДобавлениеПробеловПередКлючевымиСловами`
- `ЗапретИспользованияПерейти`
- `ИсправлениеНеКаноническогоНаписания`
- `КорректировкаXMLФорм`
- `ОбработкаЮнитТестов`
- `ОтключениеПолнотекстовогоПоиска`
- `ОтключениеРазрешенияИзменятьФорму`
- `ПроверкаДублейПроцедурИФункций`
- `ПроверкаКорректностиИнструкцийПрепроцессора`
- `ПроверкаКорректностиОбластей`
- `ПроверкаНецензурныхСлов`
- `РазборОтчетовОбработокРасширений`
- `СинхронизацияОбъектовМетаданныхИФайлов`
- `СортировкаСостава`
- `УдалениеДублейМетаданных`
- `УдалениеЛишнихКонцевыхПробелов`
- `УдалениеЛишнихПустыхСтрок`

Excluded by product decision:

- `РазборОбычныхФормНаИсходники`

## 12. Engineering Notes

Recommended Rust module layout:

```text
src/
  main.rs
  cli.rs
  git.rs
  config.rs
  file_types.rs
  bsl_parser.rs
  pipeline.rs
  diagnostics.rs
  scenarios/
    mod.rs
    bsl_text.rs
    bsl_checks.rs
    profanity.rs
    xml_metadata.rs
tests/
  fixtures/
```

Recommended hook manifest:

```yaml
- id: prec-bsl
  name: prec-bsl
  description: Run BSL source hygiene checks and fixers
  entry: prec-bsl prek-hook
  language: rust
  always_run: true
  pass_filenames: false
```

The manifest uses `always_run: true` and `pass_filenames: false` because commit-mode processing must use the Git index as the source of truth, including deletes and post-processing cases.

BSL parser strategy:

- Use `tree-sitter-bsl` for syntax-aware BSL checks and procedure/function discovery.
- Keep exact text fixers as lexical/text transformations where AST rewriting would risk parity drift.
- See `spec/parser-strategy.md` for scenario-level parser boundaries.

Testing strategy:

- Use generated fixtures for isolated edge cases.
- Use `/home/alko/develop/open-source/rat` as the main real-world EDT corpus.
- Run mutating checks only on temporary copies of RAT source roots.
- Treat RAT's repository-local `.os` scenarios as config diagnostics in v1, not as a green dynamic-execution requirement.

## 13. References

- Local reference implementation: `/usr/share/oscript/lib/precommit4onec`
- Local default config: `/usr/share/oscript/lib/precommit4onec/v8config.json`
- Configuration contract: `spec/configuration.md`
- BSL parser strategy: `spec/parser-strategy.md`
- Testing strategy: `spec/testing-strategy.md`
- `prek` repository: <https://github.com/j178/prek>
- `prek` language support: <https://prek.j178.dev/languages/>
- `prek` authoring hooks: <https://prek.j178.dev/authoring-hooks/>
- `prek` configuration reference: <https://prek.j178.dev/reference/configuration/>
