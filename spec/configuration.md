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
    rev: v0.1.0
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
rev = "v0.1.0"

[[repos.hooks]]
id = "prec-bsl"
args = ["--config", "v8config.json"]
```

Allowed here:

- `--config <path>`;
- `--source-dir <path>` for simple single-root repositories;
- `--rules <rule1,rule2>` for explicit hook variants;
- `--format text|json`;
- `--profile <name>` if profiles are added later.

Not allowed here:

- full scenario settings;
- large JSON/TOML blobs;
- secrets;
- 1C infobase credentials.

## Layer 3: prec-bsl Domain Configuration

Default file: `v8config.json`.

Discovery:

1. CLI `--config <path>`.
2. `v8config.json` in repository root.
3. Built-in defaults.

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
      "ОбработкаЮнитТестов.os",
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

Unsupported by product decision:

- `РазборОбычныхФормНаИсходники`

If `v8config.json` enables `РазборОбычныхФормНаИсходники`, `prec-bsl` must fail configuration validation with a clear message instead of silently ignoring it.

## CLI Contract

`prec-bsl prek-hook`:

```text
prec-bsl prek-hook [--config <path>] [--source-dir <path>] [--rules <list>] [--format text|json]
```

Behavior:

- Runs in commit mode.
- Reads staged Git changes from the repository.
- Uses config discovery unless `--config` is provided.
- CLI `--source-dir` and `--rules` are overrides for simple hook configuration.

`prec-bsl exec-rules`:

```text
prec-bsl exec-rules <repo> [--config <path>] [--source-dir <list>] [--rules <list>] [--format text|json]
```

Behavior:

- Runs over configured source roots rather than only the staged index.
- Used for CI, migration checks, and manual validation.

## Configuration Precedence

Highest to lowest:

1. CLI arguments from end-user hook config.
2. `v8config.json`.
3. Built-in defaults.

Within domain config:

1. Matching project settings fully override base scenario settings for that project path.
2. `disabled` has priority over `enabled`.
3. Scenario names are normalized by trimming optional `.os`.

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
