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

### `УдалениеЛишнихПустыхСтрок` blank-run contract

`prec-bsl` issue #11 (bsl-ls parity). Inside a file, a run of 2 or more
blank lines collapses to a single blank line (reference
`precommit4onec` behavior; runs of exactly 1 blank line stay). At end of
file, a trailing blank run of ANY size (including 1) is dropped entirely:
the file must end with exactly one line ending — the one already written by
the last non-blank line (`\n` or `\r\n` respectively). A file consisting only of blank
lines becomes an empty output with no bytes. A final line without a line
ending is not a blank line and is left untouched. No scenario setting
controls this; it is the default v1 behavior.

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
- `ПроверкаДлиныСтрок`
- `ОдинОператорНаСтроку`
- `ПроверкаПереносовСтрок`

These are `prec-bsl` extensions modeled on the `bsl-language-server` diagnostics
`SpaceAtStartComment`, `MissingSpace` (comma subset), `LineLength`,
`OneStatementPerLine`, and `IncorrectLineBreak`. They are NOT part of the required v1 `precommit4onec`
inventory and are registered with `ScenarioSupport::Compatibility` so that:

- the required v1 inventory list check in `src/lib.rs` stays unchanged;
- `built_in_defaults` enables them implicitly together with every other
  supported scenario (all supported = RequiredV1 + Compatibility), while
  unsupported scenarios stay excluded;
- users can turn them off through `ОтключенныеСценарии` or narrow the set with
  an explicit `ГлобальныеСценарии` list.

`ПробелыВКомментариях` and `ПробелыПослеЗапятых` are lexical text fixers with
string/comment state tracking; see `crates/prec-bsl-bsl/src/text_fixers.rs`.

`ПроверкаДлиныСтрок` (prec-bsl issue #4) is a lexical check-only scenario
modeled on the `bsl-language-server` LineLength diagnostic: it reports source
lines longer than the `МаксимальнаяДлинаСтроки` setting (default 150) as
Warning-severity diagnostics, never modifies files, and never blocks hook
mode. Line-break guidance for authors lives in `spec/line-break-rules.md`;
the counting semantics contract lives in `spec/parser-strategy.md`. The
lexical detector is implemented in `crates/prec-bsl-bsl/src/text_checkers.rs`.

`ОдинОператорНаСтроку` (prec-bsl issue #6) is a check-and-fix scenario
modeled on the `bsl-language-server` OneStatementPerLine diagnostic:
detection is the `previousLineNumber` statement-walk semantics
transliterated to `tree-sitter-bsl`, and the fix moves every flagged
statement to its own line with a computed indent. It modifies BSL modules
(hook mode reports modified paths and restaging is explicit), never adds or
removes `;`, and degrades to a Warning without modification when parse
errors intersect the detected violation rows. The full detection, fix and
deliberate non-parity contract lives in `spec/parser-strategy.md`, section
"One statement per line contract". The scenario is implemented in
`crates/prec-bsl-bsl/src/one_statement_per_line.rs`.

`ПроверкаПереносовСтрок` is a check + fix scenario with the four
`НастройкиСценариев.ПроверкаПереносовСтрок.*` settings. Its stateful line
classifier, mode contract, fix mechanics and deliberate non-parity with
bsl-ls are specified in the
`ПроверкаПереносовСтрок` (IncorrectLineBreak parity) contract section of
`spec/parser-strategy.md`; the settings are specified in
`spec/configuration.md`.

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
