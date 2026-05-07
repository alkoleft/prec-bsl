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

### Full-text search disabling contract

`ОтключениеПолнотекстовогоПоиска` is an XML/EDT fixer for metadata description
files. The initial v1 slice applies to `Configuration.mdo`, EDT object `.mdo`
files, and Designer XML metadata files. It skips BSL modules, EDT form files,
and unsupported files.

The fixer validates XML through the shared XML/EDT parser boundary before
rewriting text. It replaces text content equal to `Use`, case-insensitively,
inside `fullTextSearch` and `xr:FullTextSearch` elements with `DontUse`.
Other similarly named metadata properties, such as
`fullTextSearchOnInputByString`, are not part of this scenario.

The optional scenario setting
`НастройкиСценариев.ОтключениеПолнотекстовогоПоиска.МетаданныеДляИсключения`
is a map from repository-relative metadata file path to an array of metadata
attribute names. A matching empty array skips the whole file. A non-empty array
keeps `Use` for matching attributes and disables it for the rest of the file.
For tabular-section attributes, both `Реквизит` and `Таблица.Реквизит` names
may be matched. Repository-relative paths are matched with normalized
separators and may include or omit a leading slash for compatibility with the
reference scenario.

Invalid setting shapes are hard failures for the processed file. XML parse
errors are hard failures. The fixer must preserve unrelated XML text, report
modified files, and prove idempotence through focused tests.

### Form customization disabling contract

`ОтключениеРазрешенияИзменятьФорму` is an XML/EDT fixer for form description
files. The initial v1 slice applies to EDT `Form.form` files and Designer XML
`Form.xml` files.

The fixer validates XML through the shared XML/EDT parser boundary before
rewriting text. For EDT forms, it replaces plain text equal to `true`,
case-insensitively, inside `allowFormCustomize` elements with `false`. EDT
forms without `allowFormCustomize`, or with an existing `false` value, are
clean.

For Designer XML forms, it replaces plain text equal to `true`,
case-insensitively, inside `Customizable` elements with `false`. If the
`Customizable` element is absent and the file contains a `WindowOpeningMode`
element, the fixer inserts `<Customizable>false</Customizable>` immediately
after the `WindowOpeningMode` element, preserving that line's indentation and
line-ending style. Designer XML files without either form property are clean.

`Customizable` / `allowFormCustomize` values other than `true` or `false`,
CDATA inside those properties, and XML parse errors are hard failures for the
processed file. The fixer must preserve unrelated XML text, report modified
files, skip non-form/non-XML files, and prove idempotence through focused
tests.

### XML form correction contract

`КорректировкаXMLФорм` is an XML/EDT fixer for form description files. The
initial v1 slice applies to EDT `Form.form` files and uses the shared XML/EDT
parser boundary to validate XML before rewriting text.

The fixer collects form elements from ordered `<name>...</name>` and
`<id>...</id>` pairs under the current XML element path. Duplicate element ids
are corrected deterministically:

- if an id is used by multiple elements and none of those elements is borrowed
  from a base form, all but the last occurrence receive free ids;
- free ids are the lowest positive integers not currently used, then
  monotonically increasing ids after the current maximum;
- if a sibling `BaseForm/Form.form` exists for the EDT form, elements matching
  the same XML path and name keep the base form id and are treated as borrowed;
- duplicate groups containing borrowed elements keep borrowed ids and reassign
  the non-borrowed elements;
- if more than one current-form element matches the same base-form path/name,
  the scenario reports a hard failure instead of guessing.

The scenario must skip non-`Form.form` files and `BaseForm/Form.form` files when
they are processed directly. When an owning `Form.form` has a sibling
`BaseForm/Form.form`, the base form may also be corrected and reported as a
modified path. The fixer must preserve unrelated XML text, report modified
files, and prove idempotence through focused tests. XML parse errors, invalid
numeric ids, and ambiguous base-form matches are hard failures for the
processed file.

## Implementation Contract

- Create a shared BSL parser module instead of each scenario initializing its own parser ad hoc.
- Parse UTF-8 text and keep source byte offsets for diagnostics and rewrites.
- Treat `tree.root_node().has_error()` as a scenario-specific signal, not always as a global hard failure. Some existing checks should still report the best available diagnostics on partially invalid modules.
- Keep fixture parity against `precommit4onec` for every scenario that switches from regex scanning to AST traversal.
- Do not use tree-sitter to reformat or regenerate BSL source. Use it only to identify syntax ranges and semantic structure.

### Preprocessor instruction checker contract

`ПроверкаКорректностиИнструкцийПрепроцессора` is parser-backed and does not
modify source text.

Blocking diagnostics for this scenario are limited to tree-sitter `ERROR` or
missing nodes that are preprocessor-related:

- the error node is inside a `preprocessor` node;
- the error subtree contains `preprocessor`, `preproc`, `annotation`, or
  `PREPROC_*` nodes;
- the error source span starts with a preprocessor or annotation marker (`#` or
  `&`).

Ordinary BSL parse errors outside preprocessor/annotation constructs are not
reported by this scenario. They may be handled by a later syntax scenario if one
is accepted.

The published `tree-sitter-bsl` 0.1.x grammar recognizes preprocessor
instructions as individual nodes and does not yet model every directive as a
nested block. After parser evaluation, this scenario may add a narrow
line-oriented stack for `#Если` / `#ИначеЕсли` / `#Иначе` / `#КонецЕсли`
ordering and balance. This fallback is limited to directive lines and must not
rewrite or parse ordinary BSL statements.

Initial fixtures must cover a valid `#Если ... Тогда ... #КонецЕсли` block,
incomplete directives, missing directive expressions, standalone unmatched
directive branches such as `#Иначе`, comments/string literals containing
preprocessor-looking text, and non-BSL file skipping.

### Region correctness checker contract

`ПроверкаКорректностиОбластей` is parser-backed and does not modify source
text.

The checker validates `#Область` / `#КонецОбласти` and English
`#Region` / `#EndRegion` pairs. It must report blocking diagnostics for:

- unmatched closing region directives;
- opening region directives without a matching closing directive;
- region-related tree-sitter `ERROR` or missing nodes where parser coverage
  gives a precise span.

The published `tree-sitter-bsl` 0.1.x grammar models valid regions as
preprocessor nodes, but malformed region balance can be easier to report
precisely with a narrow lexical stack. This fallback is limited to directive
lines, is case-insensitive, ignores comments and string literals, and must not
rewrite source text.

Initial fixtures must cover valid nested regions, missing `#КонецОбласти`,
unmatched `#КонецОбласти`, case-insensitive directives, comments/string
literals containing region-looking text, ordinary non-region BSL parse errors,
and non-BSL file skipping.

### Unit test processing fixer contract

`ОбработкаЮнитТестов` is parser-backed and may modify BSL test modules.

The scenario applies only to BSL modules whose repository path or source-root
relative path contains a `tests` directory component, case-insensitively.
Non-BSL files and BSL modules outside test paths are skipped.

Test methods are exported procedure or function definitions discovered from the
`tree-sitter-bsl` syntax tree. A method is included when the line immediately
before the definition is a line comment containing `@unit-test:`. Comments and
string literals elsewhere in the module must not create test entries.
Existing loader methods named `ИсполняемыеСценарии` are also detected from the
syntax tree before the surrounding loader region is replaced.

When at least one test method is found, the scenario generates the
`#Область ТестыAPI` loader region with an exported
`ИсполняемыеСценарии(ДополнительныеПараметры = Неопределено)` function. The
function returns a `Массив` and appends discovered test method names in source
order through `ИсполняемыеСценарии.Добавить("<method>");`.

If a `ТестыAPI` region already exists, the scenario replaces that region. If no
loader region exists but a `#Область Тесты` region exists, the loader region is
inserted before it. Otherwise the loader region is prepended to the module.

The fixer must preserve unrelated module text, report modified files, and prove
idempotence through fixtures or focused tests. It must not generate a loader
region when no annotated exported test methods are present. If the BSL parser
reports syntax errors, the scenario must return a hard failure and must not
modify the module.

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
