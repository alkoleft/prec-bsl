# Reference Scenario Inventory

## Decision

Use the local `precommit4onec` installation as the v1 scenario inventory
reference:

```text
/usr/share/oscript/lib/precommit4onec
```

The inventory baseline is captured in code-facing fixture
`tests/fixtures/scenario_inventory/reference-v8config.json` and Rust scenario
metadata in `src/scenarios.rs`.

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

## Explicit Unsupported Scenario

- `РазборОбычныхФормНаИсходники`

This scenario exists in the reference installation and default config, but is
unsupported by `prec-bsl` v1 product decision. If enabled by configuration,
validation must fail with a clear diagnostic instead of silently skipping it.

## Historic Config Key

The fixture preserves the misspelled historic key:

```text
Precommt4onecСценарии
```

This is a compatibility contract for `v8config.json` parsing and must not be
renamed in persisted v1 config fixtures.
