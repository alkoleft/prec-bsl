# Notes — issue #9 Task 3 (bsl-ls spacing parity) + ревью-фикс round 1 + итоги Tasks 1–2

План: `docs/superpowers/plans/2026-10-31-issue-9-operator-spacing-...md`
(в основном репо). История ветки: Task 0 — 970bdb4; Task 1 — f5b092e +
ревью-фикс 1fc43ba; Task 2 — 231bdfc + ревью-фикс 3b3204a; Task 3 — 99b894c;
**ревью-фикс Task 3 (LOW-1..3 из `.superpowers/gh-issue/9/reviews/task3-round1.md`)
— 83e4608** `test(parity): enforce post-fix counts and drop-count in spacing
parity (issue #9)`. INFO-4 (temp-outdir в /tmp) и INFO-5 (контрактная защита
оригинала yaxunit) — принятые риски, не правились.

## Ревью-фикс Task 3 (LOW-1..3), что сделано

- **LOW-1:** baseline `mechanical_rule` — убран `.` из unary-набора; теперь
  ровно `{'[','(','=',','}` + file start, синхронно с тестом и фикстером
  (`previous_non_whitespace_char` + `Some('[' | '(' | ',' | '=') | None`).
  Фактических `.`-случаев на корпусе нет — правка prose-only, числа не
  меняются. JSON-валидность проверена (`json.load` ok).
- **LOW-2:** `read_baseline` расширен полями `post_missing_space`
  (`post_fix_counts.MissingSpace`), `residue_comma_semicolon`,
  `residue_unary_sign`, `residue_keyword`
  (`residue_policy.post_fix_residue_counts.*`, все из baseline, не хардкод).
  Новые assert'ы после ExtraCommas==0: post MissingSpace == baseline (84);
  residue comma/semicolon == 81, unary == 2, keyword == 1; сумма классов ==
  post MissingSpace (полное покрытие остатка). Это закрывает регрессию
  «фиксер чинит больше/меньше — тест проходит незаметно».
- **LOW-3:** тихий `continue` при пустом `relative` (нере-анкоренный URI)
  заменён на подсчёт `dropped_fileinfos` + `dropped_example` и panic при >0:
  «contains N fileinfo path(s) that could not be re-anchored onto the corpus
  (e.g. <URI>)». Обоснование: дропнутый fileinfo сдувает все counts, parity
  валидировался бы по неполным данным. Подсчёт ведётся по fileinfo с
  фильтруемыми кодами не связан — считается любой fileinfo с пустым
  relative (path вне corpus и вне CWD-резолва), пример URI печатается.
- Doc-комментарий шапки теста обновлён (assert (c) теперь включает сверку
  post-fix count + per-class residue counts).

## Негативные проверки ревью-фикса (TDD-ветки, факт)

- LOW-2, post-count: baseline `post_fix_counts.MissingSpace=83` (неверно) →
  panic «post-fix MissingSpace count must match the baseline snapshot...
  left: 84, right: 83». Corpus при FAILED-прогоне остаётся грязным (тест
  падает до restore) — восстановлен `git checkout -- .`; для повторных
  негативных прогонов это ожидаемо.
- LOW-2, residue: baseline `unary_sign=3` (неверно) → panic «post-fix
  unary-sign residue count must match the baseline snapshot. left: 2,
  right: 3».
- LOW-3: подмена `java` в PATH скриптом-заглушкой, возвращающей отчёт с
  URI `file:///somewhere/elsewhere/Module.bsl` → panic «contains 1 fileinfo
  path(s) that could not be re-anchored onto the corpus (e.g.
  file:///somewhere/elsewhere/Module.bsl)». Заглушка удалена.

## Ключевые факты Task 3 (проверены прогонами)

- **Pre-fix counts на clone НЕ ~22420/13 из плана, а 104 MissingSpace / 2
  ExtraCommas.** Числа плана — с анализа рабочей копии yaxunit с untracked
  build/-артефактами. Baseline зафиксирован по фактическому clone-прогону
  (сознательное отклонение, отмечено в `notes`-поле baseline).
- **Post-fix: ExtraCommas 0, MissingSpace 84** (фиксеры меняют 16 файлов).
  Остаток 84 = comma/semicolon 81 + unary 2 (`Значения[-Инд]` ×2) + keyword 1
  (`НЕ(` в Интеграция/Module.bsl:54); 0 вне residue_policy.
- Механическое правило (locale-независимо, по токену в LSP-диапазоне на
  post-fix тексте): `-`/`+` после `[ ( , =`/file start → unary; `,`/`;` →
  comma; identifier-токен → keyword; прочее → FAIL. Текст сообщения не
  парсится.
- **bsl-ls недетерминизм вне наших кодов** (UnusedLocalMethod/Variable/
  MagicNumber ±11) — поэтому сравниваются только MissingSpace/ExtraCommas
  (зафиксировано в baseline `known_noise`).
- **bsl-ls path quirk:** URI `file://<CWD-относительный>/…` с `..`
  сегментами, когда корпус вне CWD. Тест резолвит лексически + canonicalize
  против canonical corpus-пути; percent-encoding (кириллица) декодируется.
  LSP `character` — UTF-16 code units (учтено через len_utf16).
- Сверх-паритет `Возврат -1` → `Возврат - 1` (2 файла): bsl-ls 1.0.7
  `Возврат -1` НЕ флагает; unary-exemption набор `[ ( , =` не включает
  `Возврат` — семантика из плана, новых диагностик не создаёт.

## Проверки (ревью-фикс, факт)

- `PREC_BSL_PARITY_CORPUS=/tmp/opencode/parity-yaxunit-9 cargo test --test
  spacing_parity -- --nocapture` — **ok 33.4 c** (pre 104/2 → post 84/0,
  residue unary 2 / comma 81 / keyword 1, fixers changed 16/350, idempotent
  0), корпус восстановился сам (status pristine).
- `cargo test --test spacing_parity` без env — skip (ok, 0.00s).
- `cargo test --workspace` без env — **481 passed / 0 failed**.
- `cargo fmt --check` — clean. `cargo clippy --test spacing_parity` — 0
  замечаний в файле; pre-existing drift в других crates не тронут
  (prec-bsl-config/model.rs:89, prec-bsl-source:581, bsl_checkers.rs:344).

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
