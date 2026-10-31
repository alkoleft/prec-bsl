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

- `tree-sitter-bsl` 0.1.7 provides BSL language support for tree-sitter.
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
- `ПроверкаДублейПроцедурИФункций`: collect procedure and function names from
  the syntax tree, with a narrow preprocessor branch context for duplicate
  coexistence checks.
- `ПроверкаКорректностиИнструкцийПрепроцессора`: validate preprocessor nodes and error nodes where the grammar covers directives.
- `ПроверкаКорректностиОбластей`: inspect `#Область` / `#КонецОбласти` tokens where grammar coverage is sufficient, with a lexical stack fallback if needed.
- `ОдинОператорНаСтроку` (detection only, see the scenario contract below)
- Diagnostics that need byte ranges for source spans.

Use text/lexical processing instead of tree-sitter when preserving exact textual parity is the primary behavior:

- `УдалениеЛишнихКонцевыхПробелов`
- `УдалениеЛишнихПустыхСтрок`
- `ДобавлениеПробеловПередКлючевымиСловами`
- `ИсправлениеНеКаноническогоНаписания`
- `ПроверкаНецензурныхСлов`
- `ВставкаКопирайтов`
- `ПроверкаДлиныСтрок` (check-only, see the scenario contract below)
- `ПробелыВокругОператоров` and `ЛишниеЗапятыеВКонце` (whole-file span fixers,
  see the scenario contract below)

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

### External artifact parsing boundary

`РазборОтчетовОбработокРасширений` is a platform-dependent scenario for
external report, processing, and extension artifacts. The initial T30 Rust
slice establishes the explicit runtime boundary and does not execute 1C or
mutate source files yet.

The scenario applies only to files with `.epf`, `.erf`, or `.cfe` extensions.
Other source files are skipped by this scenario. Supported external artifacts
require a 1C platform executable before any unpacking can be attempted:

- `.epf` / `.erf` artifacts require a future
  `/DumpExternalDataProcessorOrReportToFiles` execution contract;
- `.cfe` artifacts require a future extension load/dump execution contract;
- missing platform executable discovery is reported as a hard dependency
  failure for the processed artifact, not as a parser or configuration error;
- if an executable is discovered, the T30 boundary still reports that unpacking
  execution is not implemented in this slice and does not run the platform;
- no files, generated directories, or Git restaging paths are modified by this
  initial boundary.

The scenario setting
`НастройкиСценариев.РазборОтчетовОбработокРасширений.ВерсияПлатформы` is
parsed as an optional version selector when
`ИспользоватьНастройкиПоУмолчанию = false`. Empty or default settings mean
that any supported platform executable discovered on `PATH` may satisfy the
dependency check. A non-empty version selector restricts discovery to candidate
paths that contain that version string.

Real unpacking, output-directory cleanup, generated-file queueing, and fixture
baselines for platform execution require a later spec task that defines the
exact command line, temporary infobase policy, and mutation safety contract.

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

The fixer collects every direct XML `<id>...</id>` value in the EDT form
description and treats ids from form elements, attributes, columns, commands
and related form metadata as one shared id space. Element names are not used for
selection or matching. Duplicate ids are corrected deterministically:

- if an id is used by multiple elements and none of those elements is borrowed
  from a base form, all but the last occurrence receive free ids;
- free ids are the lowest positive integers not currently used, then
  monotonically increasing ids after the current maximum;
- if a sibling `BaseForm/Form.form` exists for the EDT form, ids matching the
  same indexed XML path keep the base form id and are treated as borrowed;
- duplicate groups containing borrowed elements keep borrowed ids and reassign
  the non-borrowed elements;
- non-positive numeric ids, for example `-1` or `0`, are treated as missing
  valid ids and receive free positive ids through the same deterministic
  allocator;

The scenario must skip non-`Form.form` files and `BaseForm/Form.form` files when
they are processed directly. When an owning `Form.form` has a sibling
`BaseForm/Form.form`, the base form may also be corrected and reported as a
modified path. The fixer must preserve unrelated XML text, report modified
files, and prove idempotence through focused tests. XML parse errors,
non-numeric ids, and overflowing positive ids are hard failures for the
processed file.

### Metadata object/file synchronization contract

`СинхронизацияОбъектовМетаданныхИФайлов` is an XML/EDT checker for
configuration description files and their immediate metadata object files.

The v1 Rust slice applies to:

- EDT `Configuration/Configuration.mdo`;
- Designer `Configuration.xml`;
- staged EDT object description files such as
  `CommonModules/Имя/Имя.mdo`;
- staged Designer object description files such as `CommonModules/Имя.xml`.

For non-configuration metadata files, the scenario locates the owning
configuration description under the same source root and checks that
description. During full-tree `exec-rules`, non-configuration metadata files
are skipped so diagnostics are not duplicated for every object file; the
configuration description file is the full-tree check entrypoint.

The checker validates the configuration description XML through the shared
XML/EDT parser boundary before inspecting it. It extracts metadata object
references from EDT root-level composition elements whose values have the
`Type.Name` shape, and from Designer `ChildObjects` elements at their normal
configuration nesting depth, where the element name is the metadata type and
text is the metadata object name.

For each supported metadata type, the checker compares configuration
references with filesystem entries:

- EDT objects must have `<TypeDirectory>/<ObjectName>/<ObjectName>.mdo`;
- Designer objects must have `<TypeDirectory>/<ObjectName>.xml`;
- referenced object directories/files that are absent are hard failures;
- unreferenced object files or directories are hard failures;
- object directory or file names that differ only by case are hard failures;
- `Sequence` references are accepted without requiring object files, matching
  the reference scenario exception.

The scenario does not modify, generate, delete, or restage files in v1.
Repair/generation behavior is intentionally not part of this checker unless a
later spec task defines an exact file-writing contract. Hook mode therefore
blocks through hard-failure diagnostics and must leave `modified_paths` empty.
Composition sorting and duplicate metadata removal remain owned by
`СортировкаСостава` and `УдалениеДублейМетаданных`.

### Metadata tree and subsystem composition sorting compatibility

`СортировкаДереваМетаданных` and `СортировкаСоставаПодсистем` are supported
compatibility scenario ids for existing `precommit4onec` configs that split
sorting intent more narrowly than the built-in `СортировкаСостава` rule.

Reference evidence comes from the local `precommit4onec`
`СортировкаСостава.os` implementation:

- configuration files are detected as EDT `Configuration/Configuration.mdo` or
  Designer `Configuration.xml`;
- subsystem files are detected under `Subsystems` as EDT subsystem `.mdo`
  metadata files or Designer subsystem `.xml` metadata files;
- EDT subsystem composition is sorted inside repeated `content` elements;
- Designer subsystem composition is sorted inside `Properties/Content`
  `xr:Item` elements;
- sorting keeps the same prefix-bucket setting as `СортировкаСостава`
  (`УчитываяПрефикс`) and the same disabled-object setting
  (`ОтключенныеОбъекты`) where it applies.

`СортировкаДереваМетаданных` is scoped only to configuration composition.
`СортировкаСоставаПодсистем` is scoped only to subsystem composition. The
broader built-in `СортировкаСостава` rule may continue to cover both
configuration and subsystem composition through the same Rust-native XML/EDT
implementation.

### Duplicate metadata removal contract

`УдалениеДублейМетаданных` is an XML/EDT fixer for configuration composition
lists. The initial v1 Rust slice applies to configuration description files:

- EDT `Configuration/Configuration.mdo`;
- Designer `Configuration.xml`.

The fixer validates XML through the shared XML/EDT parser boundary before
rewriting text. For EDT configuration files, it inspects direct root-level
composition reference elements after the first direct `languages` element,
whose text has the `Type.Name` shape, such as
`<commonModules>CommonModule.Модуль</commonModules>`. For Designer
configuration files, it inspects direct elements inside
`MetaDataObject/Configuration/ChildObjects`, such as
`<CommonModule>Модуль</CommonModule>`.

For v1 parity, duplicate identity is the XML element name, normalized text
content, and exact direct element source shape after surrounding XML structure
is validated. When duplicates are present, the last occurrence is preserved
and earlier occurrences with the same source shape are removed, matching the
reference scenario's repeated first-match removal loop. The scenario does not
sort entries, does not remove UID-like broken references containing `-`, and
does not inspect nested `ChildObjects` blocks, EDT entries before `languages`,
or non-configuration metadata files in this slice.

The fixer must preserve unrelated XML text, report modified files, and prove
idempotence through focused tests. XML parse errors are hard failures for the
processed file.

### Metadata composition sorting contract

`СортировкаСостава` is an XML/EDT fixer for metadata composition lists. The
initial v1 Rust slice applies to configuration description files:

- EDT `Configuration/Configuration.mdo`;
- Designer `Configuration.xml`.

The fixer validates XML through the shared XML/EDT parser boundary before
rewriting text. For EDT configuration files, it sorts direct root-level
composition reference elements whose text has the `Type.Name` shape, such as
`<commonModules>CommonModule.Модуль</commonModules>`. For Designer
configuration files, it sorts direct elements inside
`MetaDataObject/Configuration/ChildObjects`, such as
`<CommonModule>Модуль</CommonModule>`.
The metadata-kind order baseline is the EDT and Designer fixtures in
`/home/alko/develop/open-source/v8-context-fixture`, not the older imported
`precommit4onec-reference/fixtures/СортировкаСостава/Configuration/После`
composition order.

Sorting is deterministic and preserves the existing XML element text:

- EDT `Configuration.mdo` and Designer `Configuration.xml` metadata type
  groups are reordered by the reference configuration kind order from
  `languages` / `Language` through `paletteColors` / `PaletteColor`, while
  unrelated configuration properties remain outside the sortable runs;
- sortable object references within each metadata type are ordered by their
  full reference text using the reference `precommit4onec`
  `ТаблицаЗначений.Сортировать("СтрокаСортировки")` string ordering, not
  Rust Unicode scalar ordering or natural numeric ordering;
- this reference-compatible order treats `_`, digits, Cyrillic/Latin letters
  and case according to the 1C string sort used by the legacy scenario, so
  names such as `ФорматПФР70_2010XML` are covered by explicit regression tests;
- references whose object name starts with one of
  `НастройкиСценариев.СортировкаСостава.УчитываяПрефикс` are sorted in prefix
  buckets after non-prefixed references, preserving the configured prefix
  order;
- `languages` / `Language`, `subsystems` / `Subsystem`, and
  `interfaces` / `Interface` participate in the reference configuration kind
  order;
- references containing `-` are treated as broken UID-like references and are
  left in their original slots.

`НастройкиСценариев.СортировкаСостава.ОтключенныеОбъекты` accepts either a
string array or a comma-separated string. If it contains `Конфигурация`
case-insensitively, configuration description files are skipped. Invalid
setting shapes are hard failures for the processed file.

The scenario skips non-configuration metadata files in this slice. Sorting
subsystem, functional option, defined type, exchange plan, and other object
description compositions belongs to a later explicit task. The fixer must
preserve unrelated XML text, report modified files, and prove idempotence
through focused tests. XML parse errors are hard failures for the processed
file.

### LineLength checker contract (`ПроверкаДлиныСтрок`)

`ПроверкаДлиныСтрок` is a lexical, check-only scenario: it never modifies,
reformats, or restages source files. Line breaking itself is author work
guided by `spec/line-break-rules.md`; no fixer exists because line breaking
changes code structure.

Length semantics (per source line, mirroring `bsl-language-server` LineLength):

- The length is counted in Unicode scalar values (`chars().count()`), not
  bytes and not grapheme clusters. A Cyrillic character is 1; a tab is 1.
- Trailing whitespace is excluded: the line is `trim_end()`-ed before
  counting. Leading whitespace (indentation) is counted.
- The line is flagged when its length is strictly greater than the limit
  (`length > limit`); a line of exactly the limit length is clean.
- A line-1 BOM (`\u{feff}`) is stripped and not counted; line endings are
  split on `\r\n`, `\n`, and a lone `\r` (Java `readLine` semantics, mirroring
  the `bsl-language-server` reader), so the terminator never enters the
  counted text and legacy CR-only Designer dumps are still split into lines.
  The input is assumed to be UTF-8 (shared with the other text scenarios:
  the handler reads via `fs::read_to_string` and fails on non-UTF-8 files);
  CP1251-encoded dumps are out of scope.
- Comment lines (`//` ...) and preprocessor directive lines are counted as
  ordinary lines; quotes inside a `//` comment never open a string literal.
- Multiline string literal continuations: while a string literal stays open
  (unclosed `"` on an earlier line), a line starting with `|` is a
  continuation and is skipped entirely, at any length. The opening line of
  the literal is an ordinary line and is flagged when too long. Code after
  the closing `"` on a continuation line is counted: the whole line is
  flagged when over the limit, except when the remainder after the quote is
  only `;` and/or whitespace, which is excluded. `""` escape sequences do
  not close the literal.
- Non-BSL files are skipped by the scenario.

The optional scenario setting
`НастройкиСценариев.ПроверкаДлиныСтрок.МаксимальнаяДлинаСтроки` is a
`serde_json` number that must be a positive integer (`as_u64()` plus
`usize::try_from`; a float like `150.0` or a non-number is invalid). Any
invalid shape is a hard failure for the processed file with the message
`max line length setting МаксимальнаяДлинаСтроки must be a positive integer`.
When the setting is absent, the default limit is `150` (constant
`DEFAULT_MAX_LINE_LENGTH`). The setting is validated in the scenario handler,
not in the config layer: the config crate stores arbitrary JSON for scenario
settings and does not type-check them, and a registry of setting types has no
second consumer yet.

The stable EN message contract (used by `text_message` and fingerprints) is:

```text
String length {len} exceeded maximum {limit} at line {n}
```

with the Russian presentation translation
`длина строки {len} превышает максимально допустимую {limit} (строка {n})`.
Severity is Warning: `prec-bsl check` exits 1 when violations are present,
hook mode is not blocked by this scenario.

Deliberate non-parity with `bsl-language-server` LineLength:

- bsl-ls (Java) counts UTF-16 code units, this checker counts Unicode
  scalars. The counts diverge only on astral-plane characters (e.g. emoji),
  which are practically absent from BSL code outside string literals (and
  literal continuations are skipped anyway).
- The default limit is 150, while bsl-ls defaults to 120 in code. 150 is the
  bsl-ls Sonar profile value and the requested `prec-bsl` default.
- A `;` immediately after a string token on an ordinary (non-continuation)
  line is counted here, while bsl-ls excludes it. This does not surface on
  the control corpus used for parity validation.

Tree-sitter is deliberately not used for this scenario: its columns are byte
offsets (breaking Cyrillic parity) and the 0.1.x grammar has gaps for
multiline string literals.

Parity scope statement:

- Parity with bsl-ls is validated as a snapshot on a pinned corpus commit
  (committed baseline fixture
  `tests/fixtures/line-length/bsl-ls-line-length-baseline.json` capturing 58
  diagnostics from bsl-ls v1.0.7 at yaxunit `d21ce6ff`, limit 150). There is
  no continuous parity guarantee against upstream changes; the semantic
  contract is carried by the unit and integration tests.
- The env-gated regression test `tests/line_length_parity.rs` re-validates the
  snapshot when `PREC_BSL_PARITY_CORPUS` points at a readable yaxunit checkout
  whose `git rev-parse HEAD` equals the baseline `corpus_commit`; on corpus
  drift it skips with baseline-refresh instructions rather than failing. The
  detector runs over all tracked `.bsl` files of the corpus
  (`git -c core.quotePath=false ls-files`) and compares fingerprint sets
  `(path, line, chars)` in both directions, so it catches both false
  negatives and false positives. The 2026-10-30 validation run found and
  fixed one real detector gap: legacy CR-only Designer endings were not
  treated as line terminators (58/58 matched after the fix, 0 missing,
  0 unexpected).
- The string-literal state tracker is private to the LineLength checker.
  It is extracted for reuse only when a second consumer appears.

### One statement per line contract (`ОдинОператорНаСтроку`)

`ОдинОператорНаСтроку` (prec-bsl issue #6) is a tree-sitter-backed
check-and-fix extension scenario. tree-sitter is used only to identify
statement spans and structure; the fix is a text splice that never
regenerates or reformats the source. The scenario is implemented in
`crates/prec-bsl-bsl/src/one_statement_per_line.rs`.

Detection contract (parity with `bsl-language-server`
OneStatementPerLineDiagnostic):

- The reference is the bsl-ls diagnostic source and its tests: fixtures
  `OneStatementPerLineDiagnostic.bsl` (5 violations, LSP ranges
  (3,8,3,14), (8,18,8,32), (8,33,8,37), (12,5,12,9), (12,10,12,14)) and
  `OneStatementPerLineDiagnosticEndFile.bsl` (2 violations, ranges
  (1,5,1,9), (1,10,1,14)) from `OneStatementPerLineDiagnosticTest.java`.
  Both fixtures are embedded verbatim in the module unit tests; a runnable
  bsl-ls is not required.
- The algorithm is a transliteration of the bsl-ls `previousLineNumber`
  tracking: statement nodes are visited in document order (pre-order,
  descending into compound statements and containers — `source_file`,
  procedure/function definitions, `if_statement`, `elseif_clause`,
  `else_clause`, `while_statement`, `for_statement`, `for_each_statement`,
  `try_statement`, and ERROR regions that still hold valid statements); a
  statement whose start row equals the previously tracked statement's start
  row is a violation. The first statement on a line is never flagged
  (`А=1; А=2; А=3;` flags `А=2;` and `А=3;`).
- Preprocessor statements, exception-equivalent ERROR regions and bare `;`
  reset the row tracking and are never flagged themselves (bsl-ls
  `ctx.preprocessor() != null` / `ctx.exception != null` semantics). Reset
  semantics are an exact transliteration of the bsl-ls `visitStatement`
  exception branch (`statementsPerLine.clear()`): a reset clears any pending
  violations of the tracked row without emitting them, so e.g.
  `А = 1; Б = 2; )` yields no diagnostics in bsl-ls either (parity, not a
  gap). Since tree-sitter-bsl 0.1.7 a bare `;` is a real
  `_empty_statement` node (kind `";"`, prec(-1)): it resets the tracking
  exactly in statement position — a `;` whose previous sibling ends an
  if/try/for/for_each compound (`КонецЕсли`/`КонецЦикла`/`КонецПопытки`)
  is that compound's own `optional(';')` terminator, and a `;` right
  after a `while_statement` is the while's terminator (0.1.7
  `while_statement` still lacks `optional(';')`) — neither may clear
  pending violations (ANTLR parity: in `BSLParser.g4` both are part of
  the preceding statement, not the `statement: SEMICOLON` alternative).
  0.1.7 also parses `#Область … #КонецОбласти` as ONE `preprocessor`
  node whose body holds the region's statements: the reset fires at the
  region start row and the body statements are then visited inside —
  equivalent to the ANTLR model, where region contents are statements of
  the surrounding code block between two preprocessor statements. The
  0.1.6 ERROR-`;` fallback forms are kept for other error shapes.
- An ERROR region led by `IF_KEYWORD`/`WHILE_KEYWORD`/`TRY_KEYWORD`/
  `FOR_KEYWORD` is an unterminated compound statement that ANTLR would
  recover inline: it occupies a statement slot (its row participates in
  tracking, so a body statement on the `Тогда` row is still flagged) and its
  inner statements stay visible. MISSING nodes do not invalidate a
  statement.
- `elseif_clause` participates in tracking as a track-only slot: its row can
  make a following body statement a violation, but the clause itself is
  never flagged (bsl-ls never flags `elsifBranch`).
- The diagnostic span is the statement node span including the trailing
  `;` (bsl-ls range parity). `;` and `Тогда` inside string literals and
  comments are never detected as violations (they are string/comment nodes,
  not statements).

Fix contract (text splice, applied over original byte offsets):

- Pattern 1 (the previous statement on the line is not a block opener):
  target indent is the statement's own line indent prefix, copied
  byte-for-byte.
- Pattern 2 (the statement follows `Тогда` of `Если`/`ИначеЕсли` on the same
  row): target indent is the anchor line indent prefix plus one `\t`.
- Mixed chain after a block opener (`Если X Тогда А = 1; Б = 2;`): all
  statements of the chain get the anchor indent + one tab. The extra tab is
  scoped to the opener chain — the pattern-2 statement and every violation
  after it on the row — not to the row as a whole: ordinary statements
  preceding an inline `Если … Тогда` on the same row
  (`А=1; Б=2; Если В Тогда Г=3;`) keep the line indent. `Иначе` is NOT
  pattern 2 (an `Иначе X = 1;` line with no other statement on the row is
  not a violation at all).
- The edit replaces the whitespace prefix before the statement start (the
  byte range from the end of the previous content on the line up to the
  statement start) with `<line_ending><indent>` — the file's dominant line
  ending is used, no trailing whitespace remains (double spaces collapse),
  and multiline statements move as a whole with continuation rows untouched.
- `;` is never added to or removed from a moved statement (missing `;`
  is handled by the `ПроверкаТочекЗапятой` contract below), and empty
  statements (`;;`) produce no blank lines.
- The fix is idempotent: a second run over fixed output produces zero
  changes (unit `fix(fix(x))`, golden `assert_fixer_fixture` and the
  integration double run).

Parse-error policy: known safe ERROR forms (MISSING nodes, the bare-`;`
forms above, keyword-led compound ERROR regions and preprocessor-containing
ERROR regions) do not block detection or fixing — the bsl-ls fixtures
themselves contain `;;` and must stay clean of warnings. Any other parse
error intersecting the row of a detected violation makes the result
unreliable: the scenario returns a Warning and leaves the file unchanged
instead of silently modifying unreliable input.

Files with lone `\r` line endings (CR-only files, or mixed endings with a
`\r` not followed by `\n` anywhere) are out of contract for this scenario:
detection rows are LF-based (tree-sitter rows advance only on `\n`), so
detection would produce false violations and the fix would corrupt the
text. The scenario reports a single Warning ("файл использует
неканонические переводы строк (CR); сценарий ОдинОператорНаСтроку
пропущен") and makes no changes; such files are never normalized or fixed
here. Pure CRLF and LF files remain fully in contract. A UTF-8 BOM
survives the fix byte-for-byte and the fixed output stays idempotent; on
row 0 the BOM is transparent for the indent computation (an indent prefix
after a BOM is still the row indent and is copied to moved statements).

Check mode (`WritePolicy::DryRun`) yields a single dry-run result with the
proposed fixed content and one message enumerating the 1-based violating
rows; per-statement diagnostic results are not produced. Non-BSL files are
skipped.

Deliberate non-parity with `bsl-language-server` (explicit, accepted):

1. Pattern-2 fix indent is the anchor line indent + one tab, while the
   bsl-ls quickfix moves the statement with the same indent as the anchor
   line. The `prec-bsl` issue #6 contract (one indent level into the block)
   takes priority.
2. `;` is not added to moved statements that lack one; bsl-ls adds the
   missing terminator in its quickfix. Missing-`;` detection and insertion
   are implemented by the `ПроверкаТочекЗапятой` contract (see below).
3. Check mode reports one dry-run result enumerating the violating rows,
   not one diagnostic per flagged statement as bsl-ls does.

### Semicolon presence contract (`ПроверкаТочекЗапятой`)

`ПроверкаТочекЗапятой` (prec-bsl issue #10) is a tree-sitter check + fix
scenario modeled on the `bsl-language-server` `SemicolonPresence` diagnostic.
It is a `ScenarioSupport::Compatibility` extension beyond the required v1
`precommit4onec` inventory (enabled by default via `built_in_defaults`; opt
out through `ОтключенныеСценарии`), implemented in
`crates/prec-bsl-bsl/src/semicolon_presence.rs` over `bsl_parser::BslParser`
(tree-sitter-bsl 0.1.7).

Detector scope mirrors the bsl-ls `visitStatement` contract: every statement
node whose expression is not terminated by `;` is flagged, **including the
compound statement ends** (`КонецЕсли`, `КонецЦикла` bodies close via their
own `optional(';')`) — the official bsl-ls fixture
`SemicolonPresenceDiagnostic.bsl` flags both `А = 0` and `КонецЕсли`.
Excluded from the scope: preprocessor directives (bsl-ls skips contexts with
`preprocessor() != null`; the preprocessor region node itself is excluded,
but its statement children are visited — the regions hold statement bodies),
procedure/function definitions and their `КонецПроцедуры`/`КонецФункции`
brackets (not statements in either model), `elseif_clause`/`else_clause`
nodes (part of the enclosing `if`), and `var_statement` (procedure-level
`Перем`: tree-sitter treats it as a statement, but the bsl-ls ANTLR model
parses it as `subVar` — not a statement — so bsl-ls never flags it; the
semicolon set excludes it for parity).

Termination predicate (per statement node): scan the node's children in
reverse order, skipping `line_comment` extras; the statement is terminated
iff the first non-comment child encountered is the `;` token. For
`while_statement` — the grammar has no `optional(';')` there — the
terminator is the nearest next sibling, skipping comment siblings, and it
counts in either form: a clean `";"` node (`_empty_statement`) or an ERROR
node whose text is exactly `;` (the clean node is the only form reproducible
on 0.1.7 probes; the ERROR branch is a defensive 0.1.6 fallback for other
error shapes, kept from the `one_statement_per_line.rs` precedent). Diagnostic span and line: the **stop
token** (last significant non-comment child), byte span + 1-based line of
that token — not the statement start, so multi-line statements report the
line where the expression ends (bsl-ls `getStop()` range parity). String
literals containing `;` never count as terminators (the last child is the
literal, not the `;` token).

Parse-error policy is mode-specific. Check: per-statement — a statement
whose subtree contains a tree-sitter `ERROR` node is skipped individually
(mirrors bsl-ls `Trees.treeContainsErrors(ctx)`; MISSING nodes are not
blockers). Fix: whole-file refusal — any unsafe ERROR outside the known
safe forms blocks modification of the file entirely (the detector's own
`has_unsafe_parse_errors`; deliberately NOT the `ОдинОператорНаСтроку`
`has_relevant_parse_errors` row-intersection semantics, because insertion
offsets come from the tree and must not be computed over unreliable
regions). A lone-`\r` file produces a single Warning in both modes; an
unsafe-ERROR file produces the refusal Warning in Apply mode only — check
mode still reports the clean statements individually (per-statement skip,
bsl-ls parity), and in both refusal cases the file is left unchanged.

Fix mechanics: insertion-only. For every violation the fix inserts one `;`
at `stop_token_span.end_byte` (before the trailing comment on that line, if
any). Edits are computed over the original bytes, sorted, and applied in a
single copy pass; nothing else in the file changes. The fix is idempotent
(`fix(fix(x)) == fix(x)`), including the EOF form (`Возврат X` as the last
statement of the file): after insertion the predicate must see the inserted
`;` as the statement's terminator — validated by treedump probe before
implementation. A UTF-8 BOM survives byte-for-byte. Files with lone `\r`
line endings (CR-only or mixed without `\n`) are out of contract: single
Warning, no changes.

Result contract: severity Warning throughout. Check mode (DryRun) produces
one Warning per flagged statement with the stop-token source span; hook mode
is not blocked by this scenario. Apply mode (hook / fix) modifies the file
only when at least one `;` was inserted; the modified path is reported and
restaged explicitly. Stable EN message contract (prefix-strip template in
`text_message`): `missing semicolon at line ` for the per-statement warning
and `inserted missing semicolons` for the fix result.

Deliberate non-parity with `bsl-language-server` (explicit, accepted):

1. Preprocessor-disabled statements and `КонецПроцедуры;` artifacts are
   excluded by construction (bsl-ls: not statements / disabled contexts).
2. Statements inside tree-sitter ERROR regions may diverge from ANTLR
   error-recovery shapes; both engines skip errored statements, but the
   shapes differ. A representative case: a label-MISSING input (`~Метка`
   without `:`). On the pristine input both engines agree — bsl-ls
   reports `ParseError` and `SemicolonPresence` at the same stop token
   (`Метка`), and the detector flags it too. After the fix inserts `;`,
   the engines diverge: bsl-ls stops flagging it (`ParseError` remains),
   while the tree-sitter parse of `~Метка;` becomes an ERROR region, so
   the fixer refuses its own output as unsafe and idempotence for that
   input is a no-op refusal. Accepted as documented behavior; the case
   is pinned in the module's unit tests (`fix_is_idempotent` label
   exclusion and the detector's label probe).
3. Check-mode diagnostics carry the stop-token span (bsl-ls reports the
   stop-token range); message text is a stable EN template with the
   1-based line, not an LSP range object.

Parity scope statement:

- Parity with bsl-ls is validated as a snapshot on the pinned yaxunit corpus
  commit `d21ce6ff` (baseline fixture
  `tests/fixtures/semicolon-parity/bsl-ls-semicolon-baseline.json`,
  bsl-ls v1.0.7, SemicolonPresence enabled by default, LSP 0-based lines
  normalized to the detector's 1-based stop-token lines), plus the verbatim
  embedded bsl-ls fixture `SemicolonPresenceDiagnostic.bsl` as a unit
  assertion. Post-fix guarantee: after applying the fix to a disposable
  corpus clone, `SemicolonPresence` diagnostics are zero outside (a) files
  the fix refused by documented policy and (b) detector residue classes.
- Baseline recipe (reproducible; canonical invocation lives in
  `tests/semicolon_parity.rs::run_bsl_ls`): run
  `java -jar <bsl-ls v1.0.7> analyze -s <corpus> -r json -o <outdir> -q`
  (the output dir must be created first) with the corpus
  `.bsl-language-server.json`, filter `SemicolonPresence` diagnostics over
  tracked `.bsl` files only (untracked `build/` dump dirs are excluded),
  normalize LSP 0-based lines to 1-based. The env-gated parity test
  (`PREC_BSL_PARITY_CORPUS`, `tests/semicolon_parity.rs`) does not trust
  the corpus working copy: it clones the pinned commit into a disposable
  location (`git clone --shared` + checkout) and compares both the bsl-ls
  rerun and the detector fingerprint against the committed baseline, then
  repeats bsl-ls over the fixed clone (post-fix step) asserting
  `SemicolonPresence == 0` and zero new stable-code diagnostics.
- Measured snapshot (pinned commit, 350 tracked `.bsl` files, 21 files with
  violations): bsl-ls reports 67 `SemicolonPresence` diagnostics; the
  detector finds the same 67 — **0 unexpected / 0 missing, residue classes
  empty** (the `var_statement` exclusion and the error-skip policy account
  for all would-be divergences). Post-fix: 21 of 350 files changed,
  `SemicolonPresence` 0, new stable-code diagnostics 0 (only corpus-local
  noise diagnostics like `UnusedLocalMethod`/`UnusedLocalVariable` appear
  identically in the pristine run).
- The reference is the bsl-ls diagnostic source and its tests; continuous
  parity against upstream changes is not guaranteed.

Deferred DRY note: the statement-kind/container matrix and the
safe-ERROR-shape facts are deliberately duplicated from
`one_statement_per_line.rs` (the sets differ between the two scenarios).
Extraction of a shared statement-walking helper is deferred; trigger: the
third tree-sitter statement-walking scenario. `has_lone_cr` is shared via
`bsl_parser` (`pub(crate)`) instead of being copied.

### `ПроверкаПереносовСтрок` (IncorrectLineBreak parity) contract

`ПроверкаПереносовСтрок` is a lexical, stateful check + fix scenario modeled
on the `bsl-language-server` `IncorrectLineBreak` diagnostic. It is a
`ScenarioSupport::Compatibility` extension beyond the required v1
`precommit4onec` inventory (enabled by default via `built_in_defaults`;
opt out through `ОтключенныеСценарии`, narrow through an explicit
`ГлобальныеСценарии` list), implemented
in `crates/prec-bsl-bsl/src/line_break.rs` without tree-sitter.

The scanner core is a stateful line classifier, not a per-line port of the
bsl-ls comment/string machinery. bsl-ls computes the `IN_STRING` state per
line from the line text alone, which mis-tracks multi-line `"` literals.
The prec-bsl classifier carries the state between lines:

- `in_string` is opened by an unclosed `"` on a line and closed by the
  matching `"` on the same or a later line; `""` inside a literal is an
  escaped quote, not an exit; a literal that started as an in-string
  continuation keeps `|`-style continuation lines inside the string;
- a query region is opened by a query keyword (`ВЫБРАТЬ`/`ВЫБОРКА`/
  `ВЫБОРКИ`/`SELECT`, case-insensitive, word immediately after the opening
  `"`) inside a `"` literal and closed by the closing `"` of that literal
  (same line or a later line); interior and closing lines of the region
  (entry state `in_query` = true) are skipped entirely by the detector, and
  the fixer's END branch is gated by the same entry state of the previous
  line; the FIRST branch can still edit such a line when repairing a
  diagnosed token from the next line (the token's carrier line is the next
  line after the region's closing line and carries entry state `in_query` =
  false, so the gluing targets the region's closing line as the previous
  line — see fix mechanics below); the opening line of the region (entry
  state `in_query` = false when the line starts) remains FIRST-checkable —
  the region is skipped starting from the following lines, so a comma
  leading into the query-opening literal (`Запрос(\n\t, "ВЫБРАТЬ…`) is
  flagged on that opening line;
- `//` outside strings terminates the code part of a line.

FIRST violations are matches of the first-symbol pattern at the start of the
code part of a line (default bsl-ls pattern `\)|;|,\s*\S+|\);`, wrapped as
`^\s*(...)`, case-insensitive, the bsl-ls `(:?` typo corrected to `(?:`);
END violations are matches of the last-symbol pattern at the end of the code
part (default `ИЛИ|И|OR|AND|\+|-|/|%|\*`, wrapped as
`\s+(...)\s*(?://.*)?$`, the capturing group around the operator is the
reported span). Matches inside string literals, after `//` comments, or
inside query regions are not violations — with one exception for FIRST: a
match that *leads into* a string literal is valid when its code prefix (the
part of the match before the first string range) contains code, because the
hanging comma itself is the violation. With the default pattern
`,\s*\S+`, the greedy `\S+` swallows a quoted argument together with the
comma (`\t, "текст"` matches as one piece), so the detector accepts the
match on the strength of the comma before the literal — a comma leading to
a string argument is a violation exactly like a comma leading to code
(`\t, Арг`), while a match that starts *inside* a literal (a multiline
string continuation, where the string range starts at offset 0) has no
code prefix and stays rejected. When the line carries a `//`
comment, the first-symbol pattern is matched against the code part of the
line only (up to `comment_start`), so a match can never swallow comment
text: with the default pattern `,\s*\S+`, `\t, // пояснение` yields no
match at all — the comma must lead to code, not into comment text. Code
glued directly to the comment without a space (`\t,Арг//комм`) still
matches the code part (`\t,Арг`) and stays a violation, with the span
ending at the comment start. A match that ends at or before the comment
start (e.g. `\t; // комм`) remains a violation.

Deliberate non-parity with bsl-ls, by design:

1. Query regions are skipped as a whole region (from the opening query
   keyword to the closing quote), instead of the fragile bsl-ls
   "first line plus 2" window.
2. Check messages carry the violation line number
   (`incorrect line break at line N`) instead of the constant bsl-ls
   diagnostic text.
3. `""` escaping is handled correctly by the state machine, instead of the
   per-line `IN_STRING` quirk of bsl-ls.
4. The fixer has no bsl-ls counterpart: quickfix semantics are invented by
   prec-bsl and scoped to mechanically safe tokens (see below).

Mode contract:

- **DryRun (check)**: one `warning` per violation with the source span of
  the matched token, plus a single `modified_dry_run` result with
  `proposed_content` when the fixer would change the file. Text output
  renders the per-file diff preview; JSON output carries `proposed_content`.
  `check` counts warnings as violations and exits `1` when any are present.
  Span shape: for FIRST violations the span covers the whole match from the
  start of the line to the end of the matched token (the first-symbol
  pattern is anchored `^\s*(...)`, so the leading indentation is part of
  the span, e.g. `\t;` / `\t)`); for END violations the span covers only
  the trailing operator.
- **Apply (fix/hook/exec-rules)**: if the fixer changes the file, exactly
  one `modified` result (warnings are never emitted in Apply, following the
  shared fixer precedent); if not, the scenario reports no results
  (`ScenarioRun::clean()`), so a repeated run exits `0` (idempotence).
- Skip cases — violations the fixer deliberately leaves in place — are
  check-visible (exit `1` in check) and fix-silent (exit `0`, file
  unchanged): `check` is the mode that distinguishes a clean file from a
  skip-only file.

Settings (`НастройкиСценариев.ПроверкаПереносовСтрок.*`, ASCII keys as in
bsl-ls; see `spec/configuration.md` for validation): `checkFirstSymbol`,
`checkLastSymbol` (booleans gating both check and fix on the respective
side), `listOfIncorrectFirstSymbol`, `listOfIncorrectLastSymbol` (regex
sources wrapped by the compile-time anchors). Custom patterns affect the
check only; the fixer never operates on custom-pattern matches. Note the
consequence: Apply repairs known tokens independently of whether the
user's pattern diagnoses them, so a pattern that excludes a known token
(e.g. `listOfIncorrectFirstSymbol = "@"` while a line hangs a `;`) gets a
fix without a matching warning — the fixer's known-token mechanics and the
detector's pattern are separate inputs, and only the default patterns make
fix a subset of check.

Fix mechanics (known, mechanically safe tokens only):

- FIRST: a fixable token (`);`, `;`, `)` — longest match first; `,` is
  check-only and custom patterns do not add fixable tokens) at the start of
  the code part of a line is removed there and glued to the end of the
  previous line's body. The token is taken only when what follows it on the
  carrier line, up to the line end, is whitespace or a `//` comment
  (`rest.trim().is_empty() || rest.trim_start().starts_with("//")`); any
  other tail (e.g. `\t); хвост`) leaves the line unchanged (check-only —
  the fixer moves a hanging token, not arbitrary code). The carrier line is
  removed when nothing remains after the token; a remaining `//` comment
  keeps the carrier line with the original indent. Line endings are
  preserved exactly (LF/CRLF/mixed/final no-newline; `\r` never lands
  inside the glued token).
- END: a fixable trailing operator (`И`, `ИЛИ`, `OR`, `AND`, `+`, `-`, `/`,
  `%`, `*`) at the end of the **code part** of a line (a trailing `//`
  comment after the operator does not hide it — the detector pattern
  `\s+(...)\s*(?://.*)?$` accepts a trailing comment, and the fixer must see
  the violation the detector reports) is moved to the start of the next
  line's code, after its indent, in its original byte-exact spelling (word
  operators are matched case-insensitively so lowercase `и`/`or` are fixed
  too). The previous line must not carry the query-region entry state: the
  END branch is gated on the previous line's `in_query` entry state — the
  same field the detector uses to skip the line — so a line that closes a
  query region (e.g. `|ИЗ Т" +`, where `+` follows the closing quote of a
  region opened on an earlier line) is never touched by the END branch.
  The next line's gate uses the line's *entry state* `in_string` (the
  unclosed-literal state carried from the previous line), not the shape of
  its string ranges: a next line that *opens a fresh* string literal at
  column 0 (e.g. `А = Б +` followed by `"строка";`) also has a string range
  starting at byte 0, but it is code from the line's own start and is a
  valid target — the operator moves there (`А = Б` / `+ "строка";`). A next
  line that continues an unclosed literal (entry state `in_string`) is
  never touched, matching the detector's whole-line skip.
  The fixer repairs a subset of what the detector diagnoses **under the
  default settings**; under a custom pattern that excludes a known token,
  Apply can fix a token the check does not report (see the settings note
  above). Within the END branch it never edits a line whose previous line
  carries the query-region entry state (the interior and closing lines of
  the region).
  The FIRST branch is not gated by that state: it can glue a known token
  onto the closing line of a query region (e.g. `|А" И` followed by `\t);`
  glues `);` onto the region's closing line). This is still a repair of a
  diagnosed violation — the token's own violation was reported on the
  carrier line carrying the token — so the fix⊆check invariant holds; the
  detector skips the region's interior and closing lines only as whole-line
  units, while the closing line can still be edited by the FIRST branch
  when repairing a token diagnosed on the next line. When the
  previous line ends with a comment, the operator detaches from the code part and
  the comment stays on the previous line; the spaces between the code and
  `//` are preserved (the previous line's code part is rstripped, not the
  comment) so
  `Если А = Б И // комм` becomes `Если А = Б // комм`. The code part of the
  previous line is rstripped so `УдалениеЛишнихКонцевыхПробелов` stays
  clean.
- The scan state advances over the original line bodies, so edits cannot
  break the classification of following lines (the detector is not reused
  by the fixer; its positions go stale after edits). Each assembled line
  stores the scan state as of its original line start and the
  classification of its body **as currently assembled**: whenever a line
  is mutated (operator removal, token gluing, operator insertion after the
  indent, a carrier line being replaced by its comment remainder), its
  classification is recomputed from that entry state over the new body, so
  string/comment ranges never go stale relative to the text they describe.
  This includes the comment-remainder carrier produced by FIRST gluing:
  the remainder is reclassified from the carrier's own entry state, so the
  following pair (carrier, next line) reads the remainder's actual
  `comment_start` instead of the original body's.
- FIRST gluing is cascaded over carrier lines within a single pass: when
  gluing a token onto a previous line turns that line itself into a
  fixable-token carrier (e.g. `Вызов(Арг)` + `\t)` + `\t;` — gluing `;`
  onto `\t)` yields a `);` carrier), the carrier's token is immediately
  glued onto its own previous line, and so on up the chain. A chain of
  carriers converges in one run; a repeated fix run makes no further
  changes (single-pass convergence, guarded by the corpus smoke test). The
  corpus smoke test and the parity baseline in `tests/line_break_parity.rs`
  are validated against a `git archive` snapshot of the pinned corpus commit
  `d21ce6ff1b553e82fb1a4e96b752c01b45ffa10c`, not against the corpus working
  tree: local edits in the corpus checkout never affect the parity result,
  and a checkout that no longer contains the pinned commit skips the tests
  with a baseline-refresh hint instead of failing — but only when the
  corpus path is the implicit default; an explicitly set `YAXUNIT_CORPUS`
  whose repository lacks the pinned commit (or the exts/tests/fixtures
  roots) fails the tests instead of skipping: an explicitly requested
  parity run must not silently go green (the same rule as for a corpus path
  that is not a git repository at all).
- Fix skip conditions (violation remains for manual repair, still visible
  through check): the next line after a trailing operator is empty or a
  pure comment; gluing **any** fixable FIRST token onto a previous line
  that already ends with `;` (the `;;` protection gates on the previous
  line's trailing `;`, not on the glued token: a `)` glued after a `;`
  would produce the garbage `;)`, while a `)` glued after a `)` is a valid
  call continuation and must be glued); `,`-FIRST violations (including
  `,`-series runs, e.g. a `,,,,,` run — the corpus case is already broken
  code where gluing produces garbage); the previous line starts with a
  check-only `,`; the previous line carries a `//` comment (gluing into a
  comment would corrupt the file); violations that exist only under custom
  patterns and are not known tokens (the fixer does not read custom
  patterns at all — see check-only tokens below).

Check-only tokens: `,`-FIRST violations (the `,`-alternative of the default
first-symbol pattern, including comma-series runs) are reported by check
but never touched by the fixer. Custom patterns affect only the detector;
the fixer ignores them entirely and fixes only its known tokens — a
custom-pattern match that is also a known token (e.g. `;`) is still fixed.
Conversely, a known token that the custom pattern *excludes* from
diagnosis is still fixed by Apply without a paired warning (see the
settings note above: fix⊆check holds only under the default patterns).

Known limitations:

- Gluing `;`/`)`/`);` lengthens the previous line. A `LineLength`-style
  limit would flag some results; `LineLength` is not part of the required
  v1 inventory and no such prec-bsl scenario exists in v1, so no
  interaction is defined yet. The trade-off is accepted.
- The detector classifies text as written. Garbage in, garbage out: on
  invalid input, e.g. an `END`-operator line followed by an operator-less
  statement, a fixed file can still contain a dangling `;` sequence; the
  fixer repairs line-break structure, not syntax errors. In particular, on
  already-invalid input an `END` operator glued before a `;` that a later
  statement needs can leave a dangling `;` — the `;;` skip protection keeps
  the fixer from collapsing such runs.

### Operator spacing and trailing comma fixers contract (`ПробелыВокругОператоров`, `ЛишниеЗапятыеВКонце`)

`ПробелыВокругОператоров` and `ЛишниеЗапятыеВКонце` (both prec-bsl issue #9)
are whole-file lexical span fixers modeled on the `bsl-language-server`
MissingSpace (binary operator subset) and ExtraCommas diagnostics. Both are
implemented in `crates/prec-bsl-bsl/src/text_fixers.rs` over one shared
scanner and one shared span-application routine; neither uses tree-sitter
(textual parity is the primary contract, and the fixes must also work on
syntactically broken code — see deliberate non-parity below).

Scanner contract (single pass over the file bytes):

- One literal/comment/directive tracker produces the event stream: code bytes
  outside string literals, `//` comments and `#` preprocessor directives.
  Every code event carries its byte index, the previous non-whitespace code
  byte (`prev_code`, `None` at file start after a BOM), the byte index of
  that previous code byte (`prev_code_index`, `None` at file start — needed
  by the unary check to scan a word like `Возврат`/`Return` backwards from an
  identifier predecessor) and the count of currently unclosed `(` (`depth`).
  Comment events carry the comment byte span including the line terminator.
- Literal, date-literal and directive spans are not events: their bytes are
  simply skipped. A span fixer that needs those boundaries must extend this
  scanner rather than add a second tracker; gap ranges between events must
  be re-verified as whitespace before any deletion (the trailing-comma
  fixer enforces this, which is what keeps `#` directives inside broken
  calls untouched).
- String literals open at `"` with `""` escapes and continue across lines
  through `|` continuations at physical line starts. Date literals open at
  `'` and close at the paired `'` on the same line (an apostrophe operator
  does not exist in BSL; an unterminated `'` ends at end of line, and `"`
  inside an open date literal is content). `#` starts a preprocessor
  directive only at the physical start of a line (after insignificant spaces
  and tabs, and after a UTF-8 BOM on the first line), and directive bytes are
  not code. A leading UTF-8 BOM is not a code byte and is preserved.
- `fixer = apply_spans(input, scan_edits(input))`: each fixer collects
  non-overlapping sorted span edits — insert a single space at a byte
  position, or delete a half-open byte range — and `apply_spans` rewrites the
  text in one pass. Bytes not covered by any span are copied byte-for-byte,
  including CR-only line endings and BOM bytes; Cyrillic content is safe
  because spans are byte spans.
- Idempotence is structural: after the fixers run, every binary operator
  already has whitespace (space, tab, or EOL — EOL satisfies the space
  requirement and no space is inserted before an EOL) on both sides, and no
  trailing comma region remains, so a second pass emits no edits
  (`fix(fix(x)) == fix(x)`, proven by unit tests, golden fixtures and the
  corpus parity run).

`ПробелыВокругОператоров` semantics: for every operator byte set
`+ - * / % = < >` (the bsl-ls `DEFAULT_LIST_FOR_CHECK_LEFT_AND_RIGHT`,
including `<>`, `>=`, `<=` consumed as one operator by the immediately
following adjacent code byte), insert a single space on each side where the
adjacent byte is not already whitespace. Unary `-`/`+` — whose previous
non-whitespace code token is an operator byte (`+ - * / % = < >`), `(`, `[`,
`,` or the keyword `Возврат`/`Return` (case-insensitive; BSL is
case-insensitive), or file start — are exempt and never modified (the bsl-ls
1.0.7 UNARY set with `checkSpaceToRightOfUnary = false`: bsl-ls requires no
space to the right of a unary sign). `(` and `[` are kept in the exemption
set deliberately: bsl-ls does not flag `Метод(-А)` either, and keeping it
prevents the false rewrite `Метод(-А)` → `Метод( - А)`.
Keywords and `,`/`;` right-side spacing are not this fixer's classes (see
non-parity below).

`ЛишниеЗапятыеВКонце` semantics: a trailing comma region is the maximal
sequence of commas, whitespace (including line endings) and `//` comments that
starts at a comma with `depth > 0` and ends at the `)` closing that paren
depth, with no other code token in between. In a confirmed region all commas
are deleted (each comma is its own delete span) and the whitespace gap after a
comma is deleted as one span; comments, including their line terminators, are
preserved, and so is the space before a comment (it separates code from the
comment: `Метод(А, // к\n,)` → `Метод(А // к\n)`). The gap immediately after a
comment is verified as whitespace but never deleted: everything from the
comment's line terminator to the next region element — blank lines and the
leading indent of the closing line — survives, and from a post-comment gap
only comma bytes are removed (`Метод(А, // к\n\t,,);` → `Метод(А // к\n\t);`).
A comma followed by a code token (`Метод(А,\n Б)`), an empty middle parameter
before a code token (`Метод(А, , Б)`), a region that does not close with `)`
at the matching depth, and depth-0 commas are never touched. Because the
scanner emits no events for `#` directives and dangling `|` lines, their bytes
land in region gaps; every gap of a region — before a comment, after a
comment, between commas, before the closing `)` — must consist solely of
whitespace bytes, so a region containing a directive or any other
non-whitespace gap is rejected as a whole and stays byte-identical, commas
included (protection for `#` directives inside broken calls:
`Метод(А,\n#Если Сервер Тогда\n#КонецЕсли\n// к\n)` and
`Метод(А,\n#Если Сервер Тогда\n#КонецЕсли\n, )` both remain unchanged).
Commas of a rejected region are never retried as start candidates: the scan
walks a rejected region to its end — up to the `)` closing the region's depth
or to the first non-region code token, whichever comes first — and the outer
loop resumes after the last examined comma of that region, so any comma after
such a terminating token is seen by the outer loop as its own fresh start
candidate.

Deliberate non-parity with `bsl-language-server` (explicit, accepted; all
items are enforced or documented by `tests/spacing_parity.rs` and the
baseline `tests/fixtures/spacing-parity/bsl-ls-spacing-baseline.json`):

1. Unary `-`/`+` (previous non-whitespace code token is an operator byte
   `+ - * / % = < >`, `(`, `[`, `,`, the keyword `Возврат`/`Return`
   case-insensitively, or file start) is not a violation, and the fixer never
   emits edits for such a sign — neither left nor right (bsl-ls 1.0.7
   UNARY set with `checkSpaceToRightOfUnary = false` requires no space to
   the right of a unary sign). bsl-ls 1.0.7 flags `Значения[-Инд]` as
   missing space to the left of `-`; `prec-bsl` leaves it as is (issue #9
   position). Corpus residue: 2 diagnostics.
2. Non-operator MissingSpace classes are outside this fixer's scope:
   `,`/`;` right-side spacing (bsl-ls `DEFAULT_LIST_FOR_CHECK_RIGHT`) is
   partially covered by `ПробелыПослеЗапятых`, and keyword spacing (`ТО`,
   `ИЛИ`, `И`, `Если`, `НЕ`, `Каждого`, …) has no fixer in the current
   inventory. Corpus residue: 81 comma/semicolon and 1 keyword diagnostics.
3. Broken code: the lexical fix works where bsl-ls emits no ExtraCommas
   because of a parse error (for example `Процедура М(А,)` gets its trailing
   comma fixed, while bsl-ls emits ParseError and MissingSpace but no
   ExtraCommas on the unparsed node). This is a feature of
   the lexical approach, not a parity regression; the span model still
   refuses regions with non-whitespace gaps, which keeps `#` directives
   inside broken calls untouched.
4. Paren groups: a comma directly before a `)` that closes a paren group
   (`А = (Б + В, )`) is removed, while bsl-ls ExtraCommas flags only call
   parameter lists. The divergence matches the planned depth semantics and
   is parity-clean in effect: the post-fix code produces no new bsl-ls
   diagnostics.
5. Numeric exponents (`1e-5`) are not recognized as numeric tokens; the
   `-` is treated as a binary operator. The pattern is absent from the
   yaxunit control corpus (verified by `git grep`), so it does not surface in
   the parity run.
6. Unary signs after OTHER keywords (not `Возврат`/`Return`) are rewritten
   as binary: only `Возврат`/`Return` are in the bsl-ls UNARY keyword set,
   so e.g. `Если -1 = А Тогда` keeps `-` binary and gains spaces
   (`Если -1 = А Тогда` → `Если - 1 = А Тогда`). The pattern does not
   surface in the parity corpus; `Возврат -1` itself is unary context and
   is not rewritten (see non-parity item 1).

Parity scope statement:

- Parity with bsl-ls is validated as a snapshot: `tests/spacing_parity.rs`
  (env-gated through `PREC_BSL_PARITY_CORPUS`, skipped without the jar and a
  matching disposable yaxunit clone at pinned commit `d21ce6ff…`) runs
  bsl-ls 1.0.7 over the corpus before and after applying both fixers (fixed
  order: operator spacing first, then trailing commas). Pre-fix counts are
  104 MissingSpace / 2 ExtraCommas; post-fix ExtraCommas is 0 and post-fix
  MissingSpace is 84, fully classified by the baseline residue policy
  (81 comma/semicolon + 2 unary sign + 1 keyword; classification is
  mechanical by the token at the diagnostic range on the post-fix text and
  never parses bsl-ls message text). A second fixer pass over the fixed
  corpus changes nothing. There is no continuous parity guarantee against
  upstream bsl-ls changes; the semantic contract is carried by unit and
  integration tests, and the baseline records the run recipe, jar version
  and SHA-256.
- Deferred DRY: the per-line fixers (`ПробелыВКомментариях`,
  `ПробелыПослеЗапятых`, keyword/canonical spelling) keep their own private
  literal-state copies inside their line loops. Consolidating them onto the
  shared scanner is deliberately deferred: their per-line rewrite shapes are
  string building, not span edits, and a second consumer boundary did not
  justify the migration (KISS/YAGNI). New span-based fixers must use the
  single shared scanner instead of adding another tracker.

## Implementation Contract

- Create a shared BSL parser module instead of each scenario initializing its own parser ad hoc.
- Parse UTF-8 text and keep source byte offsets for diagnostics and rewrites.
- Treat `tree.root_node().has_error()` as a scenario-specific signal, not always as a global hard failure. Some existing checks should still report the best available diagnostics on partially invalid modules.
- Keep fixture parity against `precommit4onec` for every scenario that switches from regex scanning to AST traversal.
- Do not use tree-sitter to reformat or regenerate BSL source. Use it only to identify syntax ranges and semantic structure.

### Duplicate procedure/function checker contract

`ПроверкаДублейПроцедурИФункций` is parser-backed and does not modify source
text. Procedure and function definitions are discovered from the syntax tree,
and diagnostics are anchored to method-name byte spans.

The checker treats procedure and function names case-insensitively. Repeated
names are hard failures when the definitions can coexist in the same compiled
variant, including repeated definitions in the same preprocessor branch and
procedure/function pairs with the same name.

To avoid false positives from upstream issue `precommit4onec` #16, the checker
also builds a narrow line-oriented preprocessor branch context from
`#Если` / `#ИначеЕсли` / `#Иначе` / `#КонецЕсли` directive lines. Definitions
that repeat only across mutually exclusive branches of the same conditional
block are not reported as duplicates. Nested conditional blocks are handled
conservatively: definitions are considered mutually exclusive when any shared
conditional block has different branch ids; otherwise they can coexist.
Malformed conditional structure, such as repeated `#Иначе` in one block or a
missing `#КонецЕсли`, must not weaken duplicate detection; in that case the
affected conditional block is ignored for duplicate-suppression purposes.

This branch context is not a general BSL parser fallback and must not replace
tree-sitter method discovery. Parser and malformed-preprocessor diagnostics
remain scenario-specific: `ПроверкаКорректностиИнструкцийПрепроцессора`
continues to own directive validity checks.

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

Since tree-sitter-bsl 0.1.7 an unterminated `#Если` block or a stray
`#ИначеЕсли` / `#Иначе` / `#КонецЕсли` directive parses as ONE ERROR region
led by the directive keyword. The lexical balance stack owns the diagnostics
for this directive family (directive-level span plus the precise message,
e.g. `missing #КонецЕсли`), so the tree walk skips ERROR regions led by an
`#Если`-family keyword to avoid reporting the same directive twice. Other
preprocessor-related ERROR shapes (e.g. `#Если` inside a call argument list)
are still reported by the tree walk.

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
rewrite source text. Since tree-sitter-bsl 0.1.7 an unterminated `#Область`
or a stray `#КонецОбласти` also parses as ONE ERROR region led by the
directive keyword: the lexical stack owns those diagnostics (directive-level
span, precise message), so the tree walk skips ERROR regions led by a region
keyword to avoid double-reporting the same directive.

Initial fixtures must cover valid nested regions, missing `#КонецОбласти`,
unmatched `#КонецОбласти`, case-insensitive directives, comments/string
literals containing region-looking text, ordinary non-region BSL parse errors,
and non-BSL file skipping.

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
