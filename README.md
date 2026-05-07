# prec-bsl

`prec-bsl` is a Rust hook package for `prek` and `pre-commit` users who keep
1C:Enterprise BSL sources in Git. It preserves the staged-file processing model
of `precommit4onec` for the built-in v1 scenario set without requiring OScript
for ordinary hook execution.

## Install With prek

Add the hook repository to the consuming project's `prek.toml`:

```toml
[[repos]]
repo = "https://github.com/<org>/prec-bsl"
rev = "v0.1.0"

[[repos.hooks]]
id = "prec-bsl"
args = ["--config", "v8config.json"]
```

The repository manifest exposes the hook as `prec-bsl prek-hook` with
`language: rust`, `always_run: true`, and `pass_filenames: false`. Commit mode
uses the staged Git index as its source of truth instead of runner-provided
filenames.

## Install With pre-commit

Use the same hook id from `.pre-commit-config.yaml`:

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

Project-specific scenario settings stay in `v8config.json`. Hook args should be
limited to small stable options such as `--config`, `--source-dir`, `--rules`,
and `--format`.

## CLI

Commit hook mode:

```text
prec-bsl prek-hook [--config <path>] [--source-dir <path>] [--rules <list>] [--format text|json]
```

Explicit source-root validation mode:

```text
prec-bsl exec-rules <repo> [--config <path>] [--source-dir <list>] [--rules <list>] [--format text|json]
```

Example CI-style run:

```bash
prec-bsl exec-rules . --source-dir fixtures/configuration/src,exts/rat/src --rules УдалениеЛишнихКонцевыхПробелов,ПроверкаКорректностиОбластей
```

## v1 Release Notes

- Built-in Rust scenarios cover the required v1 scenario set documented in
  `spec/IMPLEMENTATION_TODO.md`.
- `РазборОбычныхФормНаИсходники` is intentionally unsupported and fails config
  validation when enabled.
- Repository-local `.os` scenarios are not executed in v1. Enabled local or
  unknown scenarios produce clear unsupported diagnostics.
- `РазборОтчетовОбработокРасширений` is registered as a required scenario with
  an explicit 1C platform dependency boundary. The current v1 slice reports
  missing runtime or not-yet-implemented unpack execution clearly and does not
  run 1C from the ordinary hook path.
