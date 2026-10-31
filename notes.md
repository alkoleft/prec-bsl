# Notes — issue #9 Task 3 (bsl-ls spacing parity) + итоги Tasks 1–2

Факты и находки воркера Task 3. План:
`docs/superpowers/plans/2026-10-31-issue-9-operator-spacing-...md` (в
основном репо). История ветки: Task 0 — 970bdb4; Task 1 — f5b092e + ревью-фикс
1fc43ba; Task 2 — 231bdfc + ревью-фикс 3b3204a; Task 3 — см. `git log` (коммит
`test(parity): bsl-ls spacing parity on yaxunit corpus (issue #9)`).

## Сделано (Task 3)

- `tests/fixtures/spacing-parity/bsl-ls-spacing-baseline.json`: corpus_commit
  d21ce6ff…, jar 1.0.7 (URL релиза + sha256
  9f62765edd344d66456da24c906eaf623a03c56e90e5aafee466200100909f64 —
  совпадает с digest ассета bsl-language-server-1.0.7-exec.jar в GitHub
  release), command, pre/post-fix counts, residue_policy с механическим
  правилом классификации.
- `tests/spacing_parity.rs`: env-gated (`PREC_BSL_PARITY_CORPUS`),
  skip без env / без jar `/tmp/opencode/bsl_ls.jar` / при несовпадении commit;
  (a) pre-fix сверка counts с baseline; (b) фиксеры operator → commas по
  каждому tracked .bsl; (c) post-fix: ExtraCommas == 0, MissingSpace —
  механическая классификация остатка; (d) идемпотентность второго прохода.
  Тест мутирует корпус и возвращает его в pristine (`git checkout -- .`)
  в конце — повторные запуски не требуют ручного сброса; перед стартом
  требует pristine worktree (fail с понятным сообщением, если корпус грязный).

## Ключевые факты (проверены прогонами)

- **Pre-fix counts на clone НЕ ~22420/13 из плана, а 104 MissingSpace / 2
  ExtraCommas.** Числа плана — с анализа рабочей копии yaxunit с untracked
  build/-артефактами (350 tracked .bsl + ~1457 untracked = 1808 файлов
  analyzed). Baseline зафиксирован по фактическому clone-прогону — это
  сознательное отклонение от плана, зафиксировано в поле `notes` baseline.
- bsl-ls analyze по 350 tracked .bsl: 3937 диагностик всего, из них
  MissingSpace 104, ExtraCommas 2. Прогон ~15 с (3.5 мин CPU).
- **Post-fix: ExtraCommas 0, MissingSpace 84.** Фиксеры меняют 16 файлов
  (14 — только операторные правки, 2 — только хвостовые запятые;
  25 строк затронуто). Новых MissingSpace/ExtraCommas после фикса НЕТ
  (diff pre→post по этим кодам: только удаления).
- **Остаток 84 классифицируется полностью, 0 вне residue_policy:**
  - `,`/`;` справа — 81 (bsl-ls `listForCheckRight = ", ;"`; наш фиксер их
    сознательно не трогает — прерогатива ПробелыПослеЗапятых);
  - unary `-` после `[` — 2 (`Значения[-Инд]` ×2: ЮТОбщийСлужебный:84,
    ОМ_ЮТКоллекции:588);
  - keyword `НЕ` перед `(` — 1 (`Интеграция/Module.bsl:54`
    `Если НЕ(Ответ.КодСостояния >= 200…`).
- Механическое правило (locale-независимо, по символу в LSP-диапазоне на
  post-fix тексте): token `-`/`+` и prev non-ws символ ∈ `[ ( , =`/начало
  файла → unary; token `,`/`;` → comma; token из букв/цифр/`_` → keyword;
  прочее → FAIL. Текст сообщения НЕ парсится.
- **Keyword-семантика bsl-ls 1.0.7** (из исходника v1.0.7
  MissingSpaceDiagnostic.java, зафиксировано в residue_policy):
  KEYWORDS_WITH_LEFT_RIGHT_SPACE = {ТО, IN, ИЛИ, AND}; LEFT = {Экспорт,
  Тогда, Do}; RIGHT = {Если, ИначеЕсли, Пока, Для, НЕ, Каждый}. Corpus-факт:
  keyword-диагностики имеют диапазон ровно на keyword-токене (например `НЕ`,
  2 UTF-16 юнита) — классификация «токен из identifier-символов» их ловит.
- **bsl-ls недетерминизм вне наших кодов:** повторные прогоны на идентичном
  корпусе расходятся в UnusedLocalMethod/UnusedLocalVariable/MagicNumber
  (±11 позиций, дерево/кэши). MissingSpace/ExtraCommas стабильны во всех
  прогонах. Поэтому parity-тест сравнивает только два кода; шум зафиксирован
  в baseline `known_noise`.
- **bsl-ls path quirk:** URI в отчёте — `file://<CWD-относительный>/…` с `..`
  сегментами, когда корпус вне CWD процесса. Тест резолвит лексически +
  canonicalize против canonical corpus-пути. Paths percent-encoded
  (кириллица) — декодируется в тесте.
- LSP `character` — UTF-16 code units (кириллица = 1 юнит на символ, но
  правило реализовано честно через len_utf16).

## Отклонения от плана / решения

- Pre-fix counts: план ожидал ~22420/13; фактические clone-числа 104/2
  зафиксированы в baseline (замечание о происхождении расхождения — в
  `notes`-поле baseline). Остаток полностью уложился в residue_policy —
  расширять non-parity-список или чинить фиксер не потребовалось;
  residue-analysis.md не создавался (нет предмета).
- Прогон (c)-классификации в тесте идёт по post-fix тексту файлов корпуса
  (читаются с диска после записи) — как в плане.
- `Новый Структура("", )` → `Новый Структура("")` и
  `.Когда("ВыполнитьЗапрос",)` → `.Когда("ВыполнитьЗапрос")` — обе
  ExtraCommas устранены фиксером хвостовых запятых.
- 2 файла corpus (МокитоСлужебный, ЮТТестовыеДанные) меняются фиксером, но
  не имели bsl-ls диагностик до фикса: `Возврат -1` → `Возврат - 1` и
  `Возврат -СлучайноеЧисло(...)` → `Возврат - СлучайноеЧисло(...)` — bsl-ls
  1.0.7 такие `-` НЕ флагает (noSpaceLeft у него LPAREN-exempt только для
  левой стороны; для `Возврат-А` empirika плана говорила «флагается», на
  корпусе фактически не флагается `Возврат -1`). Это сверх-паритет (фиксер
  правит больше, чем флагает bsl-ls) — семантика из плана (unary-exemption
  набор `[ ( , =` не включает `Возврат`), не нарушение. Новых диагностик
  не создаёт.

## Проверки (Task 3, факт)

- `cargo test --test spacing_parity` без env — skip, ok.
- `PREC_BSL_PARITY_CORPUS=/tmp/opencode/parity-yaxunit-9 cargo test --test
  spacing_parity -- --nocapture` — **ok** (pre 104/2, post 84/0, residue
  unary 2 + comma 81 + keyword 1, idempotent 0 изменений), корпус
  восстанавливается сам.
- `cargo test --workspace` без env — **481 passed / 0 failed** (было 480).
- `cargo test --workspace` с env — **481 passed / 0 failed**.
- `cargo fmt --check` — clean. `cargo clippy --test spacing_parity` — 0
  замечаний в файле; pre-existing drift в других crates не тронут
  (prec-bsl-config/model.rs:89, prec-bsl-source:581, bsl_checkers.rs:344 —
  тот же набор, что до задачи).

## Осталось (Task 4)

- Спека/доки: reference-scenario-inventory (+2 extension-записи, переписать
  «MissingSpace (comma subset)»), parser-strategy (Deliberate non-parity:
  unary, сломанный код, `,`/`;`/keywords; MEDIUM-1 из ревью Task 2 round 1 —
  запятая перед `)` paren-группы `А = (Б + В, )`; deferred-DRY per-line
  трекеры), README (extension-сценарии).
- Риски: pre-existing clippy drift заблокирует `-D warnings` CI-gate всей
  ветки (вне задачи). Оригинал yaxunit не мутировался (проверено);
  disposable clone `/tmp/opencode/parity-yaxunit-9` остаётся для повторных
  прогонов.
