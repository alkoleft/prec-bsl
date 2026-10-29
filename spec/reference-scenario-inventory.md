# Reference Scenario Inventory

## Decision

Use the local `precommit4onec` installation as the v1 scenario inventory
reference:

```text
/usr/share/oscript/lib/precommit4onec
```

The inventory baseline is captured in code-facing fixture
`tests/fixtures/scenario_inventory/reference-v8config.json` and Rust scenario
metadata in the root facade catalog in `src/lib.rs`.

Implemented scenarios are defined as complete executable `*_SCENARIO` bindings
next to their handlers in the owning rule module. The root facade catalog
collects those definitions and adds explicit metadata-only inventory entries
only for non-executable decisions such as
`РазборОбычныхФормНаИсходники`.

## Evidence Sources

- Default configuration:
  `/usr/share/oscript/lib/precommit4onec/v8config.json`
- Built-in scenario directory:
  `/usr/share/oscript/lib/precommit4onec/src/СценарииОбработки`

The default `ГлобальныеСценарии` order is preserved as the reference execution
order for parity tests. Scenario lookup must accept both `ИмяСценария` and
`ИмяСценария.os` spellings.

## Required v1 Scenarios

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

## Supported Compatibility Scenarios

- `СортировкаДереваМетаданных`
- `СортировкаСоставаПодсистем`

These scenario ids are not separate files in the local `precommit4onec`
built-in scenario directory. They are migration compatibility ids whose
behavior is specified from the reference `СортировкаСостава.os` implementation:
metadata-tree sorting is the configuration composition branch, and subsystem
composition sorting is the subsystem branch. Lookup accepts both plain ids and
`.os` suffixes, but the ids remain explicit supported entries rather than
generic repository-local `.os` execution.

## Extension Scenarios (beyond the v1 reference inventory)

- `ПробелыВКомментариях`
- `ПробелыПослеЗапятых`

These are `prec-bsl` extensions modeled on the `bsl-language-server` diagnostics
`SpaceAtStartComment` and `MissingSpace` (comma subset). They are NOT part of
the required v1 `precommit4onec` inventory and are registered with
`ScenarioSupport::Compatibility` so that:

- the required v1 inventory list check in `src/lib.rs` stays unchanged;
- `built_in_defaults` does not enable them implicitly (only `RequiredV1`
  scenarios are enabled by default);
- users opt in by listing the ids in `ГлобальныеСценарии`.

Both are lexical text fixers with string/comment state tracking; see
`crates/prec-bsl-bsl/src/text_fixers.rs`.

## Explicit Unsupported Scenarios

- `РазборОбычныхФормНаИсходники`
- `ОбработкаЮнитТестов`

These scenarios exist in the reference installation and default config, but are
unsupported by `prec-bsl` v1 product decision. If enabled by configuration,
validation must fail with a clear diagnostic instead of silently skipping them.

## Historic Config Key

The fixture preserves the misspelled historic key:

```text
Precommt4onecСценарии
```

This is a compatibility contract for `v8config.json` parsing and must not be
renamed in persisted v1 config fixtures.
