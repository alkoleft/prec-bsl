# Notes — issue #9 Task 2 (`ЛишниеЗапятыеВКонце`) + ревью-фикс round 1

Факты и находки воркера Task 2 и фикс-задачи по ревью
(`.superpowers/gh-issue/9/reviews/task2-round1.md`). План:
`docs/superpowers/plans/2026-10-31-issue-9-...md` (в основном репо
`/data/home/korav/projects/prec-bsl/docs/...`). Task 1
(`ПробелыВокругОператоров`) + его ревью-фикс — коммиты f5b092e, 1fc43ba;
Task 2 — 231bdfc; ревью-фикс — см. git log ветки.

## Сделано (Task 2, коммит 231bdfc)

- `crates/prec-bsl-bsl/src/text_fixers.rs`: `EXTRA_TRAILING_COMMAS_RULE`,
  `EXTRA_TRAILING_COMMAS_SCENARIO`, `extra_trailing_commas(ctx)`,
  `fix_extra_trailing_commas` = `apply_spans(scan_trailing_comma_edits)`.
- `src/lib.rs`: регистрация 5→6 (`EXPECTED_COMPATIBILITY`).
- `crates/prec-bsl-output`: RU-строка «удалены лишние запятые в конце».
- Тесты: unit + `tests/trailing_commas.rs` + golden-фикстура. Детали — в
  истории git log / ревью round 1.

## Ревью-фикс round 1 (эта задача)

### HIGH-1 — пробельность gap-диапазонов

Баг: сканер `scan()` не эмитит события для `#`-директив (строки 767–770)
и висячих `|`-строк (758–761: `|` на физическом начале строки). Их байты
попадают в gap между событиями региона; `push_space_deletion` удалял gap
без проверки → директива/`|`-строка молча стирались.

Воспроизведено (red): `Метод(А,\n#Если Сервер Тогда\n)` → `Метод(А)`
(директива стёрта); `Метод(А, // к\n#Область Х\n)` → `Метод(А // к\n)`
(директива стёрта); CRLF-вариант — то же; `Метод(А,\n|\n)` → `Метод(А)`
и `Метод(А,\n|   \n)` → `Метод(А)` (висячая `|`-строка стёрта). `|`-строка
с КОДОВЫМ содержимым (`Метод(А,\n|x\n)`) НЕ воспроизводится: `x` — кодовый
байт, прерывает регион ещё до гапа.

Фикс: `push_space_deletion(edits, text, start, end) -> bool` — режет
`text.as_bytes()[start..end]`, пустой gap = ok, не-пробельный байт
(не space/\t/\n/\r) → `false` без правки. Оба call-site в
`scan_trailing_comma_edits`: comma-ветка `if !push… { break; }`,
`)`-ветка `is_trailing = push…`. `false` ⇒ `is_trailing` не
устанавливается ⇒ все правки региона отбрасываются (существующий путь
`if !is_trailing { continue; }`).

Инвариант `start <= end` гарантирован порядком событий сканера (события
возрастают, comment end ≤ следующего кодового индекса) — паники на срезе
нет; пустой gap (`,)`) — early return true.

Семантика после фикса: регион с не-пробельным gap не хвостовой. Запятая
ПОСЛЕ директивы может быть хвостовой сама по себе
(`Метод(А,\n#Если\n,\n)` → `Метод(А,\n#Если\n)` — вторая запятая удалена,
директива сохранена): это отдельный регион от второй запятой, гап до `)`
чист. Идемпотентно.

### LOW-1 — названия тестов

`trailing_commas_pipeline_blocks_and_restages_only_once` и его сиблинг
`operator_spacing_pipeline_blocks_and_restages_only_once` не проверяют
restage (restage — прерогатива Git-слоя, `prec-bsl-git`). ОБА переименованы
в `…_blocks_and_second_run_is_clean` — честное имя того же поведения
(первый прогон блокирует Modified, второй чист), семантика не менялась.

## TDD-цепочка (ревью-фикс, факт)

- **Red unit**: `trailing_commas_rejects_regions_with_non_whitespace_gaps`
  (6 кейсов: directive LF/CRLF, directive-после-комментария, висячая `|`
  чистая/с пробелами, `,,\n#КонецОбласти\n`):
  `assertion 'left == right' failed: region with non-whitespace gap must
  stay byte-identical: "Метод(А,\n#Если Сервер Тогда\n)" left: "Метод(А)"
  right: "Метод(А,\n#Если Сервер Тогда\n)"` — 7 passed / 1 failed.
- **Green**: 110 passed / 0 failed (`cargo test -p prec-bsl-bsl`; 109 → +1).

## Проверки (ревью-фикс, факт)

- `cargo test -p prec-bsl-bsl`: **110 passed / 0 failed**.
- `cargo test --workspace`: exit 0, **480 passed / 0 failed** (50
  test-бинарей ok; было 479 → +1 новый red→green тест).
- `cargo fmt --check`: clean (один line-wrap сигнатуры
  `push_space_deletion` найден fmt и исправлен — collapsed в одну строку).
- `cargo clippy -p prec-bsl-bsl --all-targets` и `-p prec-bsl --all-targets`:
  0 замечаний в затронутых файлах (text_fixers.rs, tests/trailing_commas.rs,
  tests/operator_spacing.rs). Pre-existing drift вне задачи — тот же набор
  до/после: `bsl_checkers.rs:344`, `prec-bsl-config/model.rs:89`,
  `prec-bsl-source:581`.

## Семантика (зафиксирована тестами, актуально)

- Gap-спан удаляет ТОЛЬКО чистую пробельность (space/\t/\n/\r). Любой
  не-пробельный байт в gap (`#`-директива, висячая `|`-строка — сканер не
  эмитит их как события) ⇒ регион не хвостовой ⇒ правки отброшены, текст
  байт-в-байт.
- Пробелы ПЕРЕД комментарием в регионе сохраняются; комментарии целиком с
  терминатором; пробелы между комментарием/запятой и `)` — удаляются (если
  чисто пробельные).
- `Метод(А,\n Б)` — не хвостовая (кодовый токен); глубина 0 не трогается;
  date-литералы нетронуты.
- `)` на «чужой» глубине прерывает поиск; вложенные вызовы корректны.

## Осталось (Task 3–4)

- Task 3 parity: env-gated `tests/spacing_parity.rs`, jar
  `/tmp/opencode/bsl_ls.jar`, yaxunit d21ce6ff, порядок operator → commas.
  Фикс HIGH-1 на parity не влияет: директив внутри скобок вызова в корпусе
  нет (проверено ревьюером round 1).
- Task 4 spec (включая MEDIUM-1 из ревью round 1: запятая перед `)`,
  закрывающим paren-группу `А = (Б + В, )` — наша depth-семантика удаляет,
  bsl-ls ExtraCommas флагает только callParamList; задокументировать в
  Deliberate non-parity, parser-strategy.md).
- Риск (унаследован): pre-existing clippy-дрейф заблокирует CI-gate
  `-D warnings` для всей ветки — вне задачи.
