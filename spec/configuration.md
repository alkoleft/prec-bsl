# prec-bsl Configuration Contract

## Decision

`v8config.json`-style settings must not be stored wholesale in `.pre-commit-hooks.yaml`.

Use three separate configuration layers:

1. Hook repository manifest: `.pre-commit-hooks.yaml`.
2. End-user hook selection: `prek.toml` or `.pre-commit-config.yaml`.
3. `prec-bsl` domain configuration: `v8config.json`.

This keeps `prec-bsl` compatible with `prek` and upstream `pre-commit` while preserving the richer `precommit4onec` settings model.

## Why Not Put v8config Settings Into .pre-commit-hooks.yaml?

`.pre-commit-hooks.yaml` belongs to the hook repository, not to each consuming 1C project. It is the manifest that tells `prek`/`pre-commit` which hooks exist and how to install/run them. It supports fields such as `id`, `name`, `entry`, `language`, `files`, `args`, `env`, `always_run`, `pass_filenames`, `stages`, and similar hook metadata.

That means it can safely contain:

- stable hook ids;
- binary entrypoint;
- hook language;
- default filter/runner behavior;
- minimal default CLI args that are valid for every consumer.

It should not contain:

- repository-specific source roots;
- scenario enable/disable lists for a concrete project;
- per-scenario settings;
- project override maps;
- local paths to profanity/copyright/config files;
- 1C platform connection/runtime settings.

Technically, `args` could pass some settings, but encoding the whole `v8config.json` model as CLI arguments would make large configs unreadable, hard to validate, hard to migrate, and unsuitable for project-specific overrides.

## Layer 1: Hook Repository Manifest

File: `.pre-commit-hooks.yaml` in the `prec-bsl` repository.

Purpose: publish hook entrypoints for consumers.

Recommended manifest:

```yaml
- id: prec-bsl
  name: prec-bsl
  description: Run BSL source hygiene checks and fixers
  entry: prec-bsl prek-hook
  language: rust
  always_run: true
  pass_filenames: false
```

Rationale:

- `always_run: true` lets `prec-bsl` inspect the Git index itself, including deletions and metadata-only situations.
- `pass_filenames: false` avoids relying on pre-commit file filtering as the source of truth.
- The Rust command remains responsible for reading `git diff --name-status --staged --no-renames`.

Optional additional hooks can expose explicit modes:

```yaml
- id: prec-bsl-exec-rules
  name: prec-bsl exec-rules
  description: Run selected BSL rules over configured source roots
  entry: prec-bsl exec-rules
  language: rust
  always_run: true
  pass_filenames: false
```

## Layer 2: End-User prek/pre-commit Selection

File: `prek.toml` or `.pre-commit-config.yaml` in the consuming project.

Purpose: pin hook version and pass small, stable execution options.

YAML example:

```yaml
repos:
  - repo: https://github.com/<org>/prec-bsl
    rev: v0.1.5
    hooks:
      - id: prec-bsl
        args:
          - --config
          - v8config.json
```

TOML example:

```toml
[[repos]]
repo = "https://github.com/<org>/prec-bsl"
rev = "v0.1.5"

[[repos.hooks]]
id = "prec-bsl"
args = ["--config", "v8config.json"]
```

Allowed here:

- `--config <path>`;
- `--source-dir <path>` for simple single-root repositories;
- `--rules <rule1,rule2>` for explicit hook variants;
- `--format text|json`;
- `--mode all|changed|index|worktree` for the `check`/`fix` commands;
- `--dry-run` for the `fix` command;
- `--restage` for the `fix` command;
- `--ignore-merge-commit` when a consuming repository wants merge commits to
  bypass `prec-bsl` hook processing;
- `--ignore-remote-merge-commit` when only merge commits whose `MERGE_HEAD`
  SHA is reachable from `refs/remotes/*` should bypass `prec-bsl` hook
  processing;
- `--profile <name>` if profiles are added later.

`--ignore-merge-commit` and `--ignore-remote-merge-commit` are mutually
exclusive.

Not allowed here:

- full scenario settings;
- large JSON/TOML blobs;
- secrets;
- 1C infobase credentials.

## Layer 3: prec-bsl Domain Configuration

Default file: `v8config.json`.

Discovery:

1. CLI `--config <path>` when the file exists.
2. `v8config.json` in repository root.
3. Built-in defaults.

`v8config.json` is optional. If neither repository discovery nor an explicit
`--config v8config.json` path finds that file, `prec-bsl` uses built-in
defaults: the supported v1 scenario set is enabled and scenario settings are
empty/default. A missing explicit custom config path, for example
`--config custom-v8config.json`, remains a configuration error.

Canonical JSON shape:

```json
{
  "GLOBAL": {
    "version": "2.0",
    "ФорматEDT": false,
    "ВерсияПлатформы": ""
  },
  "Precommt4onecСценарии": {
    "ИспользоватьСценарииРепозитория": false,
    "КаталогЛокальныхСценариев": "",
    "ГлобальныеСценарии": [
      "ВставкаКопирайтов.os",
      "ДобавлениеПробеловПередКлючевымиСловами.os",
      "ЗапретИспользованияПерейти.os",
      "ИсправлениеНеКаноническогоНаписания.os",
      "КорректировкаXMLФорм.os",
      "ОтключениеПолнотекстовогоПоиска.os",
      "ОтключениеРазрешенияИзменятьФорму.os",
      "ПроверкаДублейПроцедурИФункций.os",
      "ПроверкаКорректностиИнструкцийПрепроцессора.os",
      "ПроверкаКорректностиОбластей.os",
      "ПроверкаНецензурныхСлов.os",
      "РазборОтчетовОбработокРасширений.os",
      "СинхронизацияОбъектовМетаданныхИФайлов.os",
      "СортировкаСостава.os",
      "УдалениеДублейМетаданных.os",
      "УдалениеЛишнихКонцевыхПробелов.os",
      "УдалениеЛишнихПустыхСтрок.os"
    ],
    "ОтключенныеСценарии": [],
    "НастройкиСценариев": {
      "ПроверкаНецензурныхСлов": {
        "ФайлСНецензурнымиСловами": "НецензурныеСлова.txt"
      }
    },
    "Проекты": {
      "configuration": {
        "ИспользоватьСценарииРепозитория": false,
        "ГлобальныеСценарии": [
          "УдалениеЛишнихКонцевыхПробелов.os",
          "УдалениеЛишнихПустыхСтрок.os"
        ],
        "ОтключенныеСценарии": [],
        "НастройкиСценариев": {}
      }
    }
  }
}
```

Keep the historic misspelling `Precommt4onecСценарии` because it is the compatibility contract with `precommit4onec`. Rust code may use normalized internal structs, but the persisted v1 config file remains compatible JSON.

## Scenario Scope

Required in v1:

- `ВставкаКопирайтов`
- `ДобавлениеПробеловПередКлючевымиСловами`
- `ЗапретИспользованияПерейти`
- `ИсправлениеНеКаноническогоНаписания`
- `КорректировкаXMLФорм`
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

Supported compatibility scenarios:

- `СортировкаДереваМетаданных`
- `СортировкаСоставаПодсистем`

These ids are accepted for migration from existing `precommit4onec` configs,
including names with the optional `.os` suffix. They are explicit Rust-native
compatibility entries, not dynamic repository-local `.os` execution. They are
not added to the built-in v1 default scenario list.

`СортировкаДереваМетаданных` applies the configuration composition behavior
from the reference `СортировкаСостава.os` scenario to `Configuration.mdo` /
`Configuration.xml`. For EDT `Configuration.mdo` and Designer
`Configuration.xml`, sorting covers both object names inside the same metadata
kind and the order of metadata kinds themselves; the kind order follows the
configuration reference fixture order from `languages` / `Language` through
`paletteColors` / `PaletteColor`, including `subsystems` / `Subsystem` and
`interfaces` / `Interface`, where `filterCriteria` / `FilterCriterion`
precedes `commonModules` / `CommonModule` and `xDTOPackages` / `XDTOPackage`
follows `exchangePlans` / `ExchangePlan`.
`СортировкаСоставаПодсистем` applies the subsystem composition behavior from
the same reference scenario to EDT subsystem `.mdo` files and Designer subsystem
`.xml` files.

Unsupported by product decision:

- `РазборОбычныхФормНаИсходники`
- `ОбработкаЮнитТестов`

If `v8config.json` enables either scenario, `prec-bsl` must fail configuration
validation with a clear message instead of silently ignoring it.

## CLI Contract

`prec-bsl prek-hook`:

```text
prec-bsl prek-hook [--config <path>] [--source-dir <path>] [--rules <list>] [--format text|json] [--ignore-merge-commit|--ignore-remote-merge-commit]
```

Behavior:

- Runs in commit mode.
- Reads staged Git changes from the repository.
- Uses config discovery unless `--config` is provided. The default
  `--config v8config.json` path is optional and falls back to built-in defaults
  when the file is absent.
- CLI `--source-dir` and `--rules` are overrides for simple hook configuration.
- If `--ignore-merge-commit` is set and Git reports an active merge commit
  state through `MERGE_HEAD`, exits successfully before staged-file discovery,
  scenario execution or restaging.
- If `--ignore-remote-merge-commit` is set, the same early successful exit
  applies only when at least one non-empty SHA from `MERGE_HEAD` is reachable
  from `refs/remotes/*`.
- `--ignore-merge-commit` and `--ignore-remote-merge-commit` cannot be used in
  the same hook invocation.
- If `--source-dir` is omitted, discovers source roots from root
  `v8project.yaml` `source-set` entries when the manifest is present. EDT
  source sets resolve to their `src` subdirectory. Only source roots containing
  staged source files participate in the hook run.
- If `v8project.yaml` is absent, falls back to staged source paths by locating
  the nearest EDT `Configuration/Configuration.mdo` or Designer
  `Configuration.xml` marker. Unsupported staged files, such as hook
  configuration files, do not create source roots.
- Staged files outside discovered source roots are ignored unless they are
  source-like files that require a source root marker; those receive a clear
  source-root diagnostic instead of being processed relative to the repository
  root.

`prec-bsl exec-rules` (deprecated):

```text
prec-bsl exec-rules <repo> [--config <path>] [--source-dir <list>] [--rules <list>] [--format text|json]
```

Behavior:

- Deprecated alias for `fix --mode all`. Prints a deprecation warning to
  stderr on every run, but keeps the historic behavior and exit codes
  (including writing files) so existing consumers keep working.
- Runs over configured source roots rather than only the staged index.
- If `--source-dir` is provided, runs only over those explicit source roots.
- If `--source-dir` is omitted and root `v8project.yaml` is present, resolves
  source roots from supported `source-set` entries. EDT source sets resolve to
  their `src` subdirectory.
- If `--source-dir` is omitted and `v8project.yaml` is absent, uses the
  repository root as the default full-tree source root.
- Files and directories whose names start with `.` are excluded from
  full-tree traversal, including `.git` and other hidden working files.
- Used for CI, migration checks, and manual validation.

`prec-bsl check`:

```text
prec-bsl check <repo> [--config <path>] [--source-dir <list>] [--rules <list>] [--format text|json] [--mode <m>]
```

Behavior:

- Diagnostics only: never writes to files. Fixers run in memory and report
  would-be modifications. In text format a per-file line diff (`+`/`-` lines
  under «Предлагаемые изменения (не записаны)») is printed; in JSON format
  the diff trailer is omitted because each result already carries
  `proposed_content`.
- "Violation" means a fixer would have changed the file or a checker scenario
  produced a diagnostic result. Warnings count as violations: unlike the hook
  contract (where warnings never block a commit), `check`/`fix` exit `1` on
  any reported diagnostic, including warnings such as profanity matches.
- Exit codes: `0` - no violations; `1` - violations found; `2` - execution
  error (unresolvable repo path, invalid configuration, Git failure).
- Uses the same config/source-root resolution as `exec-rules`/`fix`.

`prec-bsl fix`:

```text
prec-bsl fix <repo> [--config <path>] [--source-dir <list>] [--rules <list>] [--format text|json] [--mode <m>] [--dry-run] [--restage]
```

Behavior:

- Applies fixers to files.
- `--dry-run` shows what would change (per-file line diff in text format)
  without writing. In JSON format the diff trailer is omitted: each result
  already carries `proposed_content`.
- `--restage` adds the fixed files back to the Git index after applying, the
  same restaging step the `prek-hook` commit mode performs. It is the only
  way to ask `prec-bsl` to update staged content; without the flag the Git
  index is never touched by `fix`.
- `--restage` is only accepted by `fix`. `check` rejects it as an unknown
  option (CLI parse error).
- `--restage` together with `--dry-run` is a warning printed to stderr, not an
  error: restaging has no effect without writing, so the dry-run runs as
  usual, and no file or index entry is modified. This keeps scripts that
  always pass `--restage` working.
- Warnings count as violations, same as `check`.
- Exit codes: `0` - no fixes needed; `1` - fixes applied (in dry-run: there
  is something to apply) or diagnostics (including warnings) reported;
  `2` - execution error, including a failed restage.
  Hook-compatible semantics.

`--mode` file selection (both `check` and `fix`, default `all`):

- `all` - every source file in the resolved source roots (equivalent to the
  historic `exec-rules` traversal).
- `changed` - everything changed relative to `HEAD`: staged, unstaged, and
  untracked files. New files are included by product decision.
- `index` - staged files only (the same Git layer the `prek-hook` commit mode
  uses).
- `worktree` - files changed outside the index: unstaged plus untracked.

Mode file sets are intersected with the source roots and path classification
(`classify_repo_path`): non-source files are filtered out as usual. Deleted
files (Git status `D`) are skipped with the standard
"scenario does not handle deleted files" message.

### Staged-content semantics

Every `check`/`fix` mode reads and fixes (in `fix`) the **working tree** copy
of each selected file on disk. Staged (Git index) content is never
overwritten silently.

`--mode` values are file **selection** filters based on Git status, not a
choice of which content version to process: `--mode index` selects staged
files, but analyzes and fixes their current working-tree content, not the
index version.

`--restage` is the only way to request that fixed content is written back to
the index (the manual-run equivalent of the `prek-hook` restaging wrapper).
By default (no flag) `fix` only modifies the working tree and leaves the index
untouched, which is backward compatible with existing scripts.

Example for a file with combined `MM` status where both the staged version and
the working tree contain a violation (for example, trailing whitespace):

| Command | Working tree | Staged (index) | Status after |
| --- | --- | --- | --- |
| `fix --mode index` | fixed | untouched: violation remains | `MM` (the staged part still differs from `HEAD`, and the fixed working tree now differs from the index) |
| `fix --mode index --restage` | fixed | fixed: fixed content lands in the index | clean vs `HEAD` |

## Configuration Precedence

Highest to lowest:

1. CLI arguments from end-user hook config.
2. `v8config.json`.
3. Built-in defaults.

Within domain config:

1. Matching project settings fully override base scenario settings for that project path.
2. `disabled` has priority over `enabled`.
3. Scenario names are normalized by trimming optional `.os`.

There is no separate v1 compatibility flag named
`ИспользоватьГлобальныеСценарии`. Local `precommit4onec` reference code and
fixtures use `ГлобальныеСценарии` as the enabled scenario list and
`ОтключенныеСценарии` as the exclusion list. To disable all global scenarios,
list the enabled scenarios under `ОтключенныеСценарии` or provide an empty
project-specific `ГлобальныеСценарии` override. Adding another boolean switch
for this behavior is outside the supported v1 surface.

## Validation Rules

- Unknown top-level JSON keys are warnings in `0.x`, errors in `1.0`.
- Unknown enabled scenario ids are errors.
- Unknown disabled scenario ids are warnings, because existing `v8config.json` files can keep disabled legacy or local scenario names.
- Paths are repository-relative unless absolute paths are explicitly allowed by a setting.
- Config must not contain credentials.
- `v8config.json` parser must preserve `precommit4onec` behavior and should
  not emit migration hints to another domain format in v1.

## Scenario Setting Policies

### ПроверкаНецензурныхСлов

Dictionary setting:

- `НастройкиСценариев.ПроверкаНецензурныхСлов.ФайлСНецензурнымиСловами`

Behavior:

- If the setting is present, the path is resolved relative to repository root.
- If the setting is present but empty or not a string, the scenario reports a
  hard failure for the processed file.
- If the configured dictionary file is missing or unreadable, the scenario
  reports a hard failure for the processed file.
- If the setting is absent, the scenario looks for `НецензурныеСлова.txt` in
  repository root.
- If the default dictionary file is absent, the scenario reports a skipped
  result and does not block by itself.
- The checker reports matched dictionary words with rule id and file path.
- The checker never modifies source files.

### ОтключениеПолнотекстовогоПоиска

Exclusion setting:

- `НастройкиСценариев.ОтключениеПолнотекстовогоПоиска.МетаданныеДляИсключения`

Behavior:

- If the setting is absent or empty, the scenario disables every supported
  `fullTextSearch` / `xr:FullTextSearch` property in the processed metadata
  file.
- If the setting contains the processed repository-relative metadata path with
  an empty array, the scenario skips that file.
- If the setting contains the processed repository-relative metadata path with
  a non-empty array, the scenario keeps matching metadata attributes enabled
  and disables the rest.
- Path keys are repository-relative, use normalized separators, and may include
  or omit a leading slash for compatibility with the reference scenario.
- Invalid setting shapes are hard failures for the processed file.

### ВставкаКопирайтов

Copyright settings:

- `НастройкиСценариев.ВставкаКопирайтов.ПутьКФайлуКопирайта`
- `НастройкиСценариев.ВставкаКопирайтов.ИсключаемыеТеги`
- `НастройкиСценариев.ВставкаКопирайтов.ИсключаемыеТэги`
- `НастройкиСценариев.ВставкаКопирайтов.ИгнорироватьМодулиОбъектовПоставки`

Behavior:

- If `ПутьКФайлуКопирайта` is present, the path is resolved relative to
  repository root.
- If `ПутьКФайлуКопирайта` is present but empty, not a string, absolute, or
  outside the repository, the scenario reports a hard failure for the processed
  file.
- If the configured copyright file is missing or unreadable, the scenario
  reports a hard failure for the processed file.
- If the setting is absent, the scenario looks for `COPYRIGHT` in repository
  root.
- If the default `COPYRIGHT` file is absent, the scenario reports a skipped
  result and does not block by itself.
- The copyright file text is trimmed before insertion.
- BSL modules without a copyright block receive the configured copyright text,
  followed by one blank line and the trimmed original module text. If the
  original module ended with a line ending, the rewritten module keeps a final
  line ending.
- BSL modules with exactly one existing copyright block, identified by two
  `//©` marker lines, keep the block when it already matches the configured
  copyright text and replace it when it differs.
- BSL modules with an ambiguous copyright shape, for example more than one
  block or only one marker line, report a hard failure instead of rewriting.
- `ИсключаемыеТеги` and historic typo `ИсключаемыеТэги` are accepted as string
  arrays. If absent, the default skip tag is `// IMPORT`.
- A module containing a configured skip tag as a line prefix is skipped and is
  not modified.
- `ИгнорироватьМодулиОбъектовПоставки` is parsed-compatible in v1, but this
  pure BSL text fixer does not infer supplier-object ownership. Supplier module
  skipping belongs to a later metadata-aware boundary.
- The fixer reports modified files and is idempotent.

### СортировкаСостава

Composition sorting settings:

- `НастройкиСценариев.СортировкаСостава.ОтключенныеОбъекты`
- `НастройкиСценариев.СортировкаСостава.УчитываяПрефикс`

Behavior:

- `ОтключенныеОбъекты` accepts either a string array or a comma-separated
  string. Empty values are ignored.
- If `ОтключенныеОбъекты` contains `Конфигурация` case-insensitively, the
  scenario skips `Configuration.mdo` / `Configuration.xml`.
- `УчитываяПрефикс` accepts either a string array or a comma-separated string.
  Empty values are ignored.
- Object references whose object name starts with a configured prefix are
  sorted in that prefix bucket after non-prefixed references; prefix bucket
  order follows the configured setting order.
- Invalid setting shapes are hard failures for the processed file.

### РазборОтчетовОбработокРасширений

Platform settings:

- `НастройкиСценариев.РазборОтчетовОбработокРасширений.ИспользоватьНастройкиПоУмолчанию`
- `НастройкиСценариев.РазборОтчетовОбработокРасширений.ВерсияПлатформы`

Behavior:

- If `ИспользоватьНастройкиПоУмолчанию` is absent or `true`, the initial
  platform dependency check does not require a specific platform version.
- If `ИспользоватьНастройкиПоУмолчанию = false` and `ВерсияПлатформы` is a
  non-empty string, platform executable discovery is restricted to candidate
  paths containing that version string.
- Empty `ВерсияПлатформы` means no version restriction.
- Invalid setting shapes are hard failures for the processed external artifact.
- The T30 boundary does not store platform credentials or execute 1C. Runtime
  execution settings belong to a later explicit spec task.

## Decision Table

| Setting kind | `.pre-commit-hooks.yaml` | `prek.toml` / `.pre-commit-config.yaml` | `v8config.json` |
| --- | --- | --- | --- |
| Hook id | yes | selects only | no |
| Rust entrypoint | yes | override only if needed | no |
| Version pin | no | yes | no |
| Config path | no | yes, via `args` | no |
| Source dirs | no | simple override only | yes |
| Enabled/disabled scenarios | no | simple override only | yes |
| Scenario settings | no | no | yes |
| Project overrides | no | no | yes |
| `v8config.json` domain config | no | path only | yes |

## PRD Impact

The PRD open question about domain configuration is resolved for v1:

- Canonical domain config: `v8config.json`.
- `prek.toml` / `.pre-commit-config.yaml`: hook selection and small CLI args only.
- `.pre-commit-hooks.yaml`: hook repository manifest only.

This v1 compatibility decision does not settle the future native `prec-bsl`
configuration format. The follow-up research task in
`spec/IMPLEMENTATION_TODO.md` must evaluate configuration ergonomics for new
projects while preserving `v8config.json` as a backward-compatibility input.
