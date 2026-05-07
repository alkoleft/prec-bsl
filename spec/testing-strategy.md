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

Historical and project-local variants of this config may include:

- `ИспользоватьСценарииРепозитория = true`
- `КаталогЛокальныхСценариев = "tools/pre-commit"`
- repository-local scenario ids such as:
  - `СортировкаДереваМетаданных.os`
  - `СортировкаСоставаПодсистем.os`
  - `ДобавлениеТестовВРасширение`

The live RAT config must be re-read in each run because it can drift. As of the
T37 implementation baseline, the live `ГлобальныеСценарии` list no longer
contains `СортировкаДереваМетаданных.os` or
`СортировкаСоставаПодсистем.os`; compatibility coverage for those ids is
therefore a synthetic RAT-config fixture built from the live file in memory,
without writing to `/home/alko/develop/open-source/rat`.

Dynamic execution of repository-local `.os` scenarios is out of v1 scope. Therefore:

- Use the live `rat/v8config.json` as a config parsing and diagnostics fixture.
- A full green `prec-bsl prek-hook --config /home/alko/develop/open-source/rat/v8config.json` is not required until local scenario handling is designed.
- Unknown or repository-local scenario diagnostics must clearly name the unsupported scenario and explain that dynamic local `.os` execution is not supported in v1.
- `СортировкаДереваМетаданных` and `СортировкаСоставаПодсистем` are exceptions
  to the generic local-scenario diagnostic: they are explicit Rust-native
  compatibility scenarios backed by focused XML/EDT fixture tests.

For green-path acceptance, generate a test-specific `v8config.json` from the required v1 scenario list and run it against a temporary copy of the `rat` source roots.

## precommit4onec Reference Test Cases

The legacy `precommit4onec` test cases from
`/home/alko/develop/open-source/v8-utils/precommit4onec/tests` are imported as
a local reference corpus under:

```text
tests/fixtures/precommit4onec-reference
```

This corpus is used as parity evidence and fixture material for incremental
Rust scenario tests. It is not executed through OScript in the built-in Rust v1
hook path.

The import intentionally preserves legacy fixture bytes. Two legacy cases are
known non-text or non-strict-JSON inputs and must remain explicit in tests:

- `fixtures/ПроверкаСообщенияКоммита/v8config.json` contains an unescaped
  regex `\d` in the original JSON fixture.
- `fixtures/ЗащищенныеФайлы/Module.bsl` and
  `fixtures/СинхронизацияОбъектовМетаданныхИФайлов/EDT/src/CommonForms/ФормаКонстант/Form.oform`
  are binary or non-UTF-8 fixture payloads.

The `precommit4onec_reference` integration test guards the imported corpus
inventory, validates ordinary JSON fixtures, and verifies that expected text
fixture classes remain valid UTF-8 while preserving the explicit legacy
exceptions above.

### Executable Reference Mapping

Every executable test case registered with `ВсеТесты.Добавить(...)` in the
imported legacy `.os` test modules must be mapped here before it is treated as
ported, already covered, blocked or out of scope. The
`precommit4onec_reference` integration test extracts those method names from
the imported modules and fails when a method is missing from this mapping.

Mapping statuses:

- `covered`: existing or newly added Rust tests protect the same observable
  behavior.
- `blocked`: the legacy case belongs to a supported area but depends on an open
  prerequisite task before parity can be claimed.
- `out-of-scope`: the case belongs to an explicitly unsupported v1 surface,
  OScript-only helper behavior, local `.os` execution or config-editor
  internals.

#### `ТестВыполнениеСценариев.os`

| Legacy method | Mapping |
| --- | --- |
| `ТестДолжен_ПодготовитьДанныеИВыполнитьСценарийВставкиКопирайта` | `covered`: copyright fixer behavior and idempotence are covered by `tests/copyright.rs`; full legacy Git/temp orchestration is represented by Rust hook/pipeline tests. |
| `ТестДолжен_ПодготовитьДанныеИВыполнитьСценарийРазбораОбычныхФорм` | `out-of-scope`: `РазборОбычныхФормНаИсходники` is explicitly unsupported in v1. |
| `ТестДолжен_ПодготовитьДанныеИВыполнитьСценарийРазбораОтчетовОбработокРасширений` | `blocked`: platform-dependent runtime execution is owned by the separated `РазборОтчетовОбработокРасширений` boundary and remains constrained by T44 cleanup/deleted-path decisions. |

#### `ТестНастройкиРепозитория.os`

| Legacy method | Mapping |
| --- | --- |
| `Тест_ИспользованиеГлобальныхНастроек` | `covered`: built-in defaults and missing config fallback are covered by config resolution tests. |
| `Тест_ИспользованиеЛокальныхНастроек` | `covered`: historic key parsing, local scenario list parsing and id normalization are covered by config tests and RAT config acceptance. |
| `Тест_ОтключенныеНастройки` | `covered`: disabled scenario filtering and unsupported-disabled compatibility are covered by config and RAT acceptance tests. |
| `Тест_ОтключенныеНастройкиИПереопределенныеГлобальныеСценарии` | `covered`: global-scenario override plus disabled-scenario filtering are covered by config tests; broader global-disable compatibility remains tracked by T43. |
| `Тест_НастройкиПроектов` | `covered`: project-specific override resolution by source subpath is covered by config tests. |
| `Тест_НастройкиСценариев` | `covered`: per-project scenario settings fully override base settings in config tests. |

#### `ТестПроверкаСценариевОбработки.os`

| Legacy method | Mapping |
| --- | --- |
| `СортировкаСостава_Configuration` | `covered`: configuration composition sorting and idempotence are covered by `tests/composition_sort.rs`. |
| `СортировкаСостава_DefinedTypes` | `blocked`: additional composition branches are tracked by T41 before parity can be claimed. |
| `СортировкаСостава_ExchangePlans` | `blocked`: additional composition branches are tracked by T41 before parity can be claimed. |
| `СортировкаСостава_FunctionalOptions` | `blocked`: additional composition branches are tracked by T41 before parity can be claimed. |
| `СортировкаСостава_Subsystems` | `covered`: subsystem composition sorting and compatibility alias behavior are covered by `tests/composition_sort.rs` and RAT acceptance. |
| `СортировкаСостава_CommonAttributes` | `blocked`: additional composition branches are tracked by T41 before parity can be claimed. |
| `СортировкаСостава_EventSubscriptions` | `blocked`: additional composition branches are tracked by T41 before parity can be claimed. |
| `ТипыФайлов_ЗащищенныеМодулиНеОпределяютсяКакФайлИсходников` | `blocked`: protected-module classification is not yet a Rust source-classification contract. |
| `ТестДолжен_ПроверитьЧтоСинхронизацияОбъектовМетаданныхВызываетИсключение` | `covered`: metadata/filesystem consistency diagnostics are covered by `tests/metadata_sync.rs`. |
| `ТестДолжен_ПроверитьЧтоСинхронизацияОбъектовМетаданныхДляПВХВызываетИсключение` | `covered`: missing referenced metadata diagnostics are covered by `tests/metadata_sync.rs`. |
| `ТестДолжен_ПроверитьЧтоСценарийПроверкиДублейПроцедурВызываетИсключение` | `blocked`: the parameterized duplicate fixture is covered by `tests/duplicate_methods.rs`, but `ПроверкаДублейПроцедурНегативныйТест.bsl` exposes a parser/parity gap that must be resolved through the parser strategy before claiming full legacy parity; preprocessor-branch false positives remain tracked by T40. |
| `ТестДолжен_ПроверитьЧтоСценарийПроверкиДублейПроцедурОбработаетФайл` | `covered`: reference positive duplicate-method fixture is covered by `tests/duplicate_methods.rs`. |
| `ТестДолжен_ПроверитьЧтоСценарийПроверкиДублейПроцедурНеОбработаетНесуществующийФайл` | `covered`: missing files are reported as boundary input errors through pipeline/file reading tests rather than scenario success. |
| `ТестДолжен_ПроверитьЧтоСценарийПроверкиДублейПроцедурНеОбработаетНеИсходник` | `covered`: non-BSL skip behavior is covered by `tests/duplicate_methods.rs`. |
| `ТестДолжен_ПроверитьЧтоСценарийРазбораОтчетовОбработокРасширенийИмеетМетодПолученияНастроек` | `covered`: Rust scenario metadata/settings are exposed through executable scenario definitions rather than OScript reflection. |
| `ТестДолжен_ПроверитьЧтоСценарийОтключенияПолнотекстовогоПоискаВозвращаетНастройки` | `covered`: settings parsing and full-text-search behavior are covered by config tests and `tests/disable_full_text_search.rs`. |
| `ТестДолжен_ПроверитьЧтоСценарийРазбораОтчетовОбработокРасширенийВозвращаетНастройки` | `covered`: platform scenario settings are parsed and carried as structured scenario settings; runtime execution remains separated. |
| `ТестДолжен_ПроверитьЧтоСценарийИсправлениеНеКаноническогоНаписанияИсправляетФайл` | `covered`: canonical spelling fixer is covered by golden and reference fixture tests. |
| `ТестДолжен_ПроверитьЧтоСценарийИсправлениеНеКаноническогоНаписанияНеИндексируетНеизмененные` | `covered`: idempotence/no second modification is covered by `tests/canonical_spelling.rs`. |
| `ТестДолжен_ПроверитьЧтоСценарийИсправлениеНеКаноническогоНаписанияИсправляетТолькоНаписание` | `covered`: canonical spelling tests assert lexical replacement without changing comments, strings or identifier parts. |
| `ТестДолжен_ПроверитьЧтоСценарийИсправлениеНеКаноническогоНаписанияИгнорируетМодулиРасширенияСКонтролемИзменений` | `covered`: reference change-control module skip is covered by `tests/canonical_spelling.rs`. |
| `ТестДолжен_ПроверитьЗагрузкуСценариевПоИмени` | `covered`: scenario lookup accepts ids with and without `.os` suffix and deduplicates through config/catalog normalization. |
| `ТестДолжен_ПроверитьИзменениеТегаКастомизацииФормы` | `covered`: form change permission behavior is covered by `tests/disable_form_change.rs`. |
| `ТестДолжен_ПроверитьЧтоСценарийВставкиКопирайтовНеОбновляетКопирайтВФайлахПоставки` | `covered`: copyright skip/update behavior is covered by `tests/copyright.rs`; parent-configuration fixture parity remains evidence for future widening. |
| `ТестДолжен_ПроверитьЧтоСценарийЗапретаИспользованияПерейтиНеСрабатываетНаСтроку` | `covered`: reference `Перейти` string-literal fixtures are covered by `tests/goto.rs`. |
| `ТестДолжен_ПроверитьЧтоСценарийКорректировкаXMLФормУчитываетСвязьФормРасширенийСФормамиКонфигурации` | `covered`: base-form duplicate-id behavior is covered by `tests/xml_forms.rs`. |
| `ТестДолжен_ПроверитьЧтоСценарийСортировкаПравРолейИзменяетПорядокОбъектов` | `out-of-scope`: role-right sorting is not in the required v1 scenario list. |
| `ТестДолжен_ПроверитьЧтоСообщениеКоммитаСоответствуетМаске` | `out-of-scope`: commit-message validation is outside the Rust v1 hook/check surface. |
| `ТестДолжен_ПроверитьЧтоСообщениеКоммитаОбрабатываютФайлыТолькоВУказанномКаталоге` | `out-of-scope`: commit-message validation is outside the Rust v1 hook/check surface. |
| `ТестДолжен_ПроверитьЧтоВыполняютсяПринудительноУказанныеЛокальныеСценарии` | `out-of-scope`: dynamic repository-local `.os` execution is unsupported in v1 and must produce diagnostics instead of being executed. |
| `ТестДолжен_ПроверитьЧтоВыполняютсяВсеЛокальныеСценарии` | `out-of-scope`: dynamic repository-local `.os` execution is unsupported in v1 and must produce diagnostics instead of being executed. |

#### `ТестРедакторНастроек.os`

| Legacy method | Mapping |
| --- | --- |
| `Тест_СброситьНастройкиРепозитория` | `out-of-scope`: interactive/config-editor mutation behavior is not part of the v1 CLI/config parsing contract. |
| `Тест_СброситьГлобальныеНастройки` | `out-of-scope`: interactive/config-editor mutation behavior is not part of the v1 CLI/config parsing contract. |
| `Тест_ПолучитьСтандартнуюСтруктуруНастроек` | `out-of-scope`: OScript editor structure helpers are not public Rust API. |
| `Тест_ОбновитьНастройки` | `out-of-scope`: interactive/config-editor mutation behavior is not part of the v1 CLI/config parsing contract. |

#### `ТестФайловыеОперации.os`

| Legacy method | Mapping |
| --- | --- |
| `ТестДолжен_ПрочитатьФайл` | `out-of-scope`: OScript file helper behavior is not a public Rust contract. |
| `ТестДолжен_ЗаписатьФайл` | `out-of-scope`: OScript file helper behavior is not a public Rust contract. |
| `ТестДолжен_ПроверитьПоискКаталогов` | `out-of-scope`: OScript file helper behavior is not a public Rust contract. |
| `ТестДолжен_ПроверитьНовыйФайл` | `out-of-scope`: OScript file helper behavior is not a public Rust contract. |

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
- Enabled repository-local scenarios are reported as unsupported in v1 unless
  the scenario id is one of the explicitly supported compatibility entries
  (`СортировкаДереваМетаданных`, `СортировкаСоставаПодсистем`).

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
