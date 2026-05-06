# BSL Parser Strategy

## Decision

Use `tree-sitter-bsl` for BSL syntax-aware scenarios in v1.

The parser is not a replacement for every `precommit4onec` mechanic. It is the default foundation for BSL module parsing, node discovery, and syntax-aware diagnostics. XML/EDT metadata scenarios still use XML/EDT-specific parsers, and platform-dependent scenarios still use 1C runtime tooling.

## Dependency

Initial dependency target:

```toml
[dependencies]
tree-sitter = "0.25"
tree-sitter-bsl = "0.1"
```

Rationale:

- `tree-sitter-bsl` 0.1.6 provides BSL language support for tree-sitter.
- The crate exposes `LANGUAGE` for `tree_sitter::Parser` and `NODE_TYPES` for grammar metadata.
- The upstream README demonstrates the Rust integration with `tree-sitter = "0.25"` and `tree-sitter-bsl = "0.1"`.
- The grammar includes BSL procedure/function definitions, `goto`/`Перейти`, preprocessor directives, regions, annotations, and compilation directives.

Do not upgrade to a newer `tree-sitter` major/minor API without a compatibility check against `tree-sitter-bsl`.

## Local Grammar Workspace

Local checkout:

```text
/home/alko/develop/open-source/tree-sitter-bsl
```

Use this checkout when `prec-bsl` acceptance fixtures expose parser gaps that block required scenarios.

Workflow:

1. Reproduce the parser gap with a minimal BSL corpus fixture in `tree-sitter-bsl`.
2. Update `grammar.js` and regenerate parser artifacts in the grammar repository.
3. Verify `tree-sitter-bsl` tests first.
4. Temporarily consume the local grammar from `prec-bsl` with a path dependency only while developing the parser fix.
5. Prefer a released `tree-sitter-bsl` crate version for normal `prec-bsl` releases. If a path or git dependency is unavoidable, document it in release notes and keep it temporary.

Do not patch around parser gaps in `prec-bsl` with broad regex fallbacks until the gap has been evaluated against the grammar repository. Scenario-specific lexical fallback is still allowed where `spec/parser-strategy.md` explicitly permits it.

## Where To Use It

Use `tree-sitter-bsl` for:

- `ЗапретИспользованияПерейти`: detect `Перейти` / `goto` as syntax, not as raw text.
- `ПроверкаДублейПроцедурИФункций`: collect procedure and function names from the syntax tree.
- `ПроверкаКорректностиИнструкцийПрепроцессора`: validate preprocessor nodes and error nodes where the grammar covers directives.
- `ПроверкаКорректностиОбластей`: inspect `#Область` / `#КонецОбласти` tokens where grammar coverage is sufficient, with a lexical stack fallback if needed.
- `ОбработкаЮнитТестов`: locate procedures/functions and loader methods more safely than line regexes.
- Diagnostics that need byte ranges for source spans.

Use text/lexical processing instead of tree-sitter when preserving exact textual parity is the primary behavior:

- `УдалениеЛишнихКонцевыхПробелов`
- `УдалениеЛишнихПустыхСтрок`
- `ДобавлениеПробеловПередКлючевымиСловами`
- `ИсправлениеНеКаноническогоНаписания`
- `ПроверкаНецензурныхСлов`
- `ВставкаКопирайтов`

### Canonical spelling fixture scope

`ИсправлениеНеКаноническогоНаписания` is a lexical text-parity fixer for known
reference keywords. It must normalize keyword spellings outside comments and
string literals, preserve unrelated text, and prove idempotence through golden
fixtures. The initial fixture matrix covers Russian keywords and the English
`NULL` spelling from the reference scenario:

- control flow and declarations: `Если`, `Тогда`, `Иначе`, `ИначеЕсли`,
  `КонецЕсли`, `Для`, `Каждого`, `Цикл`, `КонецЦикла`, `Пока`, `Попытка`,
  `Исключение`, `КонецПопытки`, `Процедура`, `КонецПроцедуры`, `Функция`,
  `КонецФункции`, `Возврат`;
- logical and literal spellings: `И`, `ИЛИ`/`Или`, `НЕ`/`Не`,
  `Истина`/`ИСТИНА`, `Ложь`/`ЛОЖЬ`, `Знач`/`ЗНАЧ`,
  `Неопределено`/`НЕОПРЕДЕЛЕНО`, `NULL`/`Null`;
- directives and annotations: `#Если`, `#Тогда`, `#Иначе`, `#ИначеЕсли`,
  `#КонецЕсли`, `#Область`, `#КонецОбласти`, `&НаКлиенте`, `&НаСервере`,
  `&НаСервереБезКонтекста`, `&НаКлиентеНаСервереБезКонтекста`,
  `&НаКлиентеНаСервере`;
- platform contexts and other reference words: `Клиент`, `НаКлиенте`,
  `НаСервере`, `ТолстыйКлиентОбычноеПриложение`,
  `ТолстыйКлиентУправляемоеПриложение`, `Сервер`, `ВнешнееСоединение`,
  `ТонкийКлиент`, `ВебКлиент`, `Выполнить`, `По`, `Прервать`,
  `Продолжить`, `Из`, `Новый`, `Перейти`, `Перем`, `ВызватьИсключение`,
  `ДобавитьОбработчик`, `УдалитьОбработчик`, `Знач`.

Use XML/EDT/platform-specific mechanisms instead of tree-sitter for:

- `КорректировкаXMLФорм`
- `ОтключениеПолнотекстовогоПоиска`
- `ОтключениеРазрешенияИзменятьФорму`
- `РазборОтчетовОбработокРасширений`
- `СинхронизацияОбъектовМетаданныхИФайлов`
- `СортировкаСостава`
- `УдалениеДублейМетаданных`

## Implementation Contract

- Create a shared BSL parser module instead of each scenario initializing its own parser ad hoc.
- Parse UTF-8 text and keep source byte offsets for diagnostics and rewrites.
- Treat `tree.root_node().has_error()` as a scenario-specific signal, not always as a global hard failure. Some existing checks should still report the best available diagnostics on partially invalid modules.
- Keep fixture parity against `precommit4onec` for every scenario that switches from regex scanning to AST traversal.
- Do not use tree-sitter to reformat or regenerate BSL source. Use it only to identify syntax ranges and semantic structure.

## Open Validation Tasks

- Build a fixture matrix for Russian and English keyword spellings.
- Verify behavior on modules with broken syntax, incomplete regions, and preprocessing directives.
- Verify parser coverage for extension annotations and compilation directives used in real 1C repositories.
- Decide which scenarios may fail fast on tree-sitter parse errors and which must continue with lexical fallback.
- Run the RAT corpus parser coverage against both the published crate and the local grammar checkout before accepting grammar changes.

## References

- `tree-sitter-bsl` docs: <https://docs.rs/tree-sitter-bsl/latest/tree_sitter_bsl/>
- `tree-sitter-bsl` source/repository: <https://github.com/alkoleft/tree-sitter-bsl>
- `tree-sitter-bsl` crate metadata: <https://docs.rs/crate/tree-sitter-bsl/latest/source/Cargo.toml>
- Local `tree-sitter-bsl` checkout: `/home/alko/develop/open-source/tree-sitter-bsl`
