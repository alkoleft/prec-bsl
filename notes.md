# Notes — issue #9 Task 4 (spec/доки) + итоги Tasks 0–3

План: `docs/superpowers/plans/2026-10-31-issue-9-operator-spacing-...md`
(в основном репо). Коммиты ветки: Task 0 — 970bdb4; Task 1 — f5b092e +
ревью-фикс 1fc43ba; Task 2 — 231bdfc + ревью-фикс 3b3204a; Task 3 — 99b894c +
ревью-фикс 3f723a0; **Task 4 — docs(spec)-коммит (этот)**.
## Task 4 — что сделано (только docs, код не тронут)

- `spec/reference-scenario-inventory.md` (Extension Scenarios, ~78–125):
  +2 записи (`ПробелыВокругОператоров`, `ЛишниеЗапятыеВКонце`) с отсылкой к
  контракт-секции parser-strategy; перечень diagnostics переписан —
  MissingSpace теперь «comma subset для ПробелыПослеЗапятых и binary-operator
  subset для ПробелыВокругОператоров» (старое «MissingSpace (comma subset)»
  убрано); ПробелыВКомментариях/ПробелыПослеЗапятых помечены как per-line.
- `spec/parser-strategy.md`: Where To Use It + обе строки сценариев (~69–70);
  новая контракт-секция «Operator spacing and trailing comma fixers contract»
  (строки ~602–720, перед Implementation Contract) по образцу
  canonical-spelling/LineLength: сканер (события/prev_code/depth,
  литералы/даты/`|`/`#`/BOM), span-модель (`apply_spans`, byte-спаны,
  идемпотентность структурная, EOL = пробел), семантика обоих фикссеров,
  Deliberate non-parity №1–6 (unary `[ ( , =`; `,`/`;`/keywords вне scope;
  сломанный код — лексический фикс там, где bsl-ls молчит из-за ParseError,
  gap-проверка защищает `#`-директивы; paren-group `А = (Б + В, )`;
  числовые экспоненты `1e-5`; `Возврат -1` → бинарный), Parity scope
  statement (числа: pre 104/2 → post 84/0, residue 81/2/1, механическая
  классификация по токену, 16/350 файлов, идемпотентный второй проход) и
  deferred-DRY пункт (per-line фикссеры сохраняют свои трекеры; новые
  span-фикссеры обязаны использовать единый сканер).
- `README.md` (секция «Поддержанные v1-сценарии», extension-подсписок):
  +2 пункта с кратким описанием поведения; CHANGELOG не тронут (конвенции
  нет).
- `spec/IMPLEMENTATION_TODO.md`: T51 (Milestone 12) → DONE 2026-10-31 с
  Resolution + Completion evidence (коммиты f5b092e, 1fc43ba, 231bdfc,
  3b3204a, 99b894c, 3f723a0 + этот), gates 481/0. NB: в ledger уже есть
  второй «T51. DONE» в Milestone 14 (LineLength, issue #4) — коллизия id
  существовала до этой задачи (обе записи с полным текстом; правку нумерации
  не делал — вне scope, отмечено здесь). Архив T51 в
  `spec/archive/completed-tasks-t1-t43-t47.md` НЕ переносил: стиль T50 —
  запись остаётся в активном файле с DONE-статусом (T52/T53 тоже остаются);
  архивирование — отдельный maintenance-шаг вне задачи.

## Проверки (Task 4, факт)

- `cargo fmt --check` — clean.
- `cargo test --workspace` — **481 passed / 0 failed** (parity skip без env,
  как в Task 3).
- `PREC_BSL_PARITY_CORPUS=/tmp/opencode/parity-yaxunit-9 cargo test --test
  spacing_parity -- --nocapture` — **ok 31.9 c**: pre 104/2 → post 84/0,
  residue unary 2 / comma 81 / keyword 1, fixers changed 16/350, второй
  проход идемпотентен (0 изменений), корпус восстановился сам (pristine).
  Этот прогон подтверждает числа, записанные в spec-текст Task 4.
- Клон `/tmp/opencode/parity-yaxunit-9` и jar `/tmp/opencode/bsl_ls.jar`
  остаются для повторных прогонов.

## Ключевые факты для последующих задач (Tasks 1–3, короткая выжимка)

- Сканирующая модель: единый `scan()` в `crates/prec-bsl-bsl/src/text_fixers.rs`
  (события Code/Comment, prev_code, depth), span-правки
  `InsertSpaceAt`/`DeleteRange`, `apply_spans` — один проход, байт-точность
  (CR-only, BOM, кириллица). Идемпотентность структурная.
- Unary-набор ровно `[ ( , =` + file start (`.` в baseline-наборе bsl-ls
  есть, но фактических случаев нет — правило синхронизировано с фикстером).
- Хвостовой регион: запятые+пробелы+`//`-комментарии до `)` на согласующей
  глубине; комментарии и пробел перед ними сохраняются; gap обязан быть
  чисто пробельным (`#`-директивы/висячие `|` в gap → регион не хвостовой).
- Паритет: bsl-ls 1.0.7, yaxunit d21ce6ff, jar sha256 в baseline; сравниваются
  только MissingSpace/ExtraCommas (known_noise: недетерминизм остальных
  кодов); fileinfo-URI ре-анкорится лексически+canonicalize (LSP character —
  UTF-16 code units).
- Риски вне задачи: pre-existing clippy drift в других crates
  (prec-bsl-config/model.rs:89, prec-bsl-source:581, bsl_checkers.rs:344)
  заблокирует `-D warnings` CI-gate всей ветки; T51-id-коллизия в ledger
  (см. выше); CHANGELOG-конвенции нет.
