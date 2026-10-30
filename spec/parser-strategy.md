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
  `ctx.preprocessor() != null` / `ctx.exception != null` semantics). The
  grammar 0.1.x has no `_empty_statement` node: a bare `;` is an ERROR node
  in three forms — a top-level ERROR-`;` resets the tracking; an EOF-swallowed
  ERROR-`;` child of a statement resets at the statement's end row; a
  top-level ERROR-`;` immediately after a `while_statement` is a grammar
  artifact (0.1.6 `while_statement` lacks `optional(';')`) and is skipped
  entirely.
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
  statements of the chain get the anchor indent + one tab. `Иначе` is NOT
  pattern 2 (an `Иначе X = 1;` line with no other statement on the row is
  not a violation at all).
- The edit replaces the whitespace prefix before the statement start (the
  byte range from the end of the previous content on the line up to the
  statement start) with `<line_ending><indent>` — the file's dominant line
  ending is used, no trailing whitespace remains (double spaces collapse),
  and multiline statements move as a whole with continuation rows untouched.
- `;` is never added to or removed from a moved statement (missing `;` is
  the prospective SemicolonPresence issue #10 territory), and empty
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
   missing terminator in its quickfix. Missing-`;` handling is the
   prospective issue #10 (SemicolonPresence) territory.
3. Check mode reports one dry-run result enumerating the violating rows,
   not one diagnostic per flagged statement as bsl-ls does.

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
