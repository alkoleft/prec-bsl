# v1 Release Readiness Checklist

This checklist is the T36 release-readiness artifact for the current v1 scope.
It summarizes the implementation state tracked in
`spec/IMPLEMENTATION_TODO.md` and the durable contracts in
`spec/prd-prec-bsl.md`, `spec/configuration.md`,
`spec/parser-strategy.md`, and `spec/testing-strategy.md`.

## Scenario Coverage

| Scenario | v1 status | Evidence |
| --- | --- | --- |
| `ВставкаКопирайтов` | Implemented | T16, `cargo test copyright` |
| `ДобавлениеПробеловПередКлючевымиСловами` | Implemented | T13, `cargo test keyword_spacing` |
| `ЗапретИспользованияПерейти` | Implemented | T18, `cargo test goto` |
| `ИсправлениеНеКаноническогоНаписания` | Implemented | T14, `cargo test canonical_spelling` |
| `КорректировкаXMLФорм` | Implemented | T24, `cargo test xml_forms` |
| `ОбработкаЮнитТестов` | Implemented | T22, `cargo test unit_tests_processing` |
| `ОтключениеПолнотекстовогоПоиска` | Implemented | T25, `cargo test disable_full_text_search` |
| `ОтключениеРазрешенияИзменятьФорму` | Implemented | T26, `cargo test disable_form_change` |
| `ПроверкаДублейПроцедурИФункций` | Implemented | T19, `cargo test duplicate_methods` |
| `ПроверкаКорректностиИнструкцийПрепроцессора` | Implemented | T20, `cargo test preprocessor` |
| `ПроверкаКорректностиОбластей` | Implemented | T21, `cargo test regions` |
| `ПроверкаНецензурныхСлов` | Implemented | T15, `cargo test profanity` |
| `РазборОтчетовОбработокРасширений` | Accepted runtime boundary | T30, `cargo test external_artifacts` |
| `СинхронизацияОбъектовМетаданныхИФайлов` | Implemented | T27, `cargo test metadata_sync` |
| `СортировкаСостава` | Implemented | T28, `cargo test composition_sort` |
| `УдалениеДублейМетаданных` | Implemented | T29, `cargo test duplicate_metadata` |
| `УдалениеЛишнихКонцевыхПробелов` | Implemented | T11, `cargo test trailing_whitespace` |
| `УдалениеЛишнихПустыхСтрок` | Implemented | T12, `cargo test empty_lines` |

Unsupported by product decision:

- `РазборОбычныхФормНаИсходники` remains explicit in the scenario inventory and
  fails configuration validation when enabled.
- Repository-local or unknown `.os` scenarios are not executed in v1 and must
  produce unsupported diagnostics rather than silent skips.

## Hook and CLI Readiness

- `.pre-commit-hooks.yaml` contains hook id `prec-bsl`, entry
  `prec-bsl prek-hook`, `language: rust`, `always_run: true`, and
  `pass_filenames: false`.
- `prec-bsl prek-hook` supports `--config`, `--source-dir`, `--rules`, and
  `--format text|json`.
- `prec-bsl exec-rules <repo>` supports `--config`, comma-separated
  `--source-dir`, comma-separated `--rules`, and `--format text|json`.
- Hook mode uses Git staged-file discovery through
  `git diff --name-status --staged --no-renames`.
- Modified files are reported and restaging is explicit in the Git layer.
- Text and JSON output include deterministic diagnostics and exit-code
  behavior.

## Acceptance Baseline

- `cargo test` must be green before release tagging.
- `cargo run -- prek-hook --help` and `cargo run -- exec-rules --help` must
  print the documented CLI contracts.
- RAT parser, text-fixer, and XML/EDT acceptance checks are covered by T33,
  T34, and T35 and must continue to treat `/home/alko/develop/open-source/rat`
  as an external read-only corpus.

## Release Note Callouts

- v1 covers built-in Rust scenarios only. Dynamic repository-local `.os`
  execution is outside v1.
- `РазборОтчетовОбработокРасширений` has a deliberate runtime boundary:
  missing 1C platform/runtime is reported clearly, and real unpacking execution
  requires a later explicit spec task.
- `v8config.json` remains the domain configuration surface, including the
  historic key `Precommt4onecСценарии`.
