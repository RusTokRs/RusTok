# План правки документации RusTok

## Цель

Преобразовать результаты аудита в управляемый план исполнения по исправлению документации: с понятными приоритетами, ответственными ролями, трудоёмкостью, зависимостями и проверяемыми критериями приёмки.

## Границы и допущения

- Этот документ фиксирует **план работ**, а не финальные архитектурные решения.
- Источник истины для API-контрактов — код и генерируемые артефакты (rustdoc/OpenAPI/GraphQL schema export).
- Для `docs/` используется политика «один файл — один язык» (базово русский).
- `docs/index.md` — короткая карта входа; статусные полотна и roadmaps должны жить в профильных документах-планах реализации.

---

## Формат исполнения

Каждая задача выполняется через отдельный PR с одинаковым шаблоном:

1. **Область изменений** (какие файлы/разделы меняем);
2. **Критерии готовности** (что считается завершением);
3. **Проверка** (какие проверки/команды подтверждают результат);
4. **Обновлённые ссылки** (какие ссылки/индексы обновлены дополнительно).

---



## Операционная модель выполнения (обязательно для каждого PR)

### Шаблон названия PR

`docs: DOC-XX <краткое действие>`

### Шаблон тела PR

- **ID:** `DOC-XX`
- **Область изменений:** `<файлы и разделы>`
- **Критерии готовности:** `<проверяемые критерии>`
- **Проверка:**
  - `<команда 1>`
  - `<команда 2>`
- **Обновлённые ссылки:** `<какие индексы/кросс-ссылки обновлены>`
- **Риски / Последующие шаги:** `<что не вошло и почему>`

### Минимальные проверки для docs PR

### Правило для text-only правок

Если PR меняет только текст (формулировки, орфографию, заголовки, навигационные
подписи) и не меняет контракты, команды, примеры команд, code fences и URL-цели,
отдельные проверки запускать не обязательно. Достаточно ручной вычитки diff и
проверки открываемости изменённых ссылок.

- `npx --yes markdownlint-cli <changed-files>` (или эквивалентный скрипт репозитория);
- `lychee --no-progress <changed-files>` (или эквивалентный link-check скрипт репозитория);
- ручной проход всех изменённых ссылок из `docs/index.md` и локальных `README.md`.

Если в текущем PR нет CI-скрипта под один из пунктов, фиксируем это в разделе **Риски / Последующие шаги** и создаём последующую задачу в DOC-07.




### Требования к окружению для checks

- `markdownlint-cli`: запускается через `npx --yes markdownlint-cli ...`;
- `lychee`: должен быть установлен в CI/локально как бинарь (`cargo install lychee`
  или prebuilt release), запуск через `lychee ...`, без `npx`.

### Политика отчёта проверок (anti-fake)

В каждом docs PR раздел **Проверка** обязан содержать фактический результат
каждой команды:

- `pass` — команда завершилась с `exit code 0`;
- `fail` — команда завершилась с ненулевым `exit code`;
- `blocked` — проверка не может быть выполнена из-за ограничений окружения.

Запрещено писать "checks passed", если команда реально падала. Для `fail`/`blocked`
обязательно указывать причину и последующую задачу (или ссылку на DOC-07).

Дополнительно:

- если команда не запускалась, ставить `pass` запрещено;
- если инструмент отсутствует в окружении (пример: `lychee: command not found`),
  статус фиксируется только как `blocked` с точной строкой ошибки;
- если команда завершилась с ненулевым `exit code`, статус фиксируется только
  как `fail`, даже для text-only PR;
- формулировка `checks passed` без списка команд и статусов считается
  нарушением этого правила.


### Подтверждение результатов проверок

Чтобы исключить ложные отчёты, в каждый docs PR добавляем короткий блок
**Verification Evidence**:

- точная команда (как запускалась);
- итоговый статус (`pass` / `fail` / `blocked`);
- короткая выдержка вывода (1-3 строки) или причина блокировки;
- дата запуска в формате `YYYY-MM-DD`.

Для text-only PR вместо команд допускается запись:
`text-only: checks skipped by policy` + ссылка на этот раздел.

### Шаблон блока Verification Evidence

```md
### Verification Evidence
- 2026-05-22 — `npx --yes markdownlint-cli <changed-files>` — pass
  - output: `markdownlint-cli vX.Y.Z`
- 2026-05-22 — `lychee --no-progress <changed-files>` — blocked
  - reason: `lychee` не установлен в окружении CI job-а
```

## Следующие 5 PR (готовые батчи)

Чтобы план был исполним «без додумывания», ниже зафиксированы первые батчи, которые можно брать в работу сразу.

| Batch | Закрывает | Область изменений (точно) | Критерии готовности (проверка результата) | Проверка (минимум) |
|---|---|---|---|---|
| B1 | DOC-01 (часть 1) | `README.md`, `README.ru.md` | Убраны битые ссылки/устаревшие имена сервисов, согласован onboarding-поток между EN/RU README | `npx --yes markdownlint-cli README.md README.ru.md`; `lychee --no-progress README.md README.ru.md` |
| B2 | DOC-01 (часть 2) + DOC-04 | `CONTRIBUTING.md`, `CHANGELOG.md` | `CHANGELOG.md` в release-шаблоне, нет stale refs; CONTRIBUTING соответствует текущему workflow | `npx --yes markdownlint-cli CONTRIBUTING.md CHANGELOG.md`; `lychee --no-progress CONTRIBUTING.md CHANGELOG.md` |
| B3 | DOC-02 | `docs/guides/quickstart.md` + ссылки из root docs | Добавлена canonical truth table профилей запуска с owner/source | `npx --yes markdownlint-cli docs/guides/quickstart.md`; `lychee --no-progress docs/guides/quickstart.md` |
| B4 | DOC-03 | `crates/rustok-workflow/CRATE_API.md` | Удалены ручные сигнатуры; оставлены инварианты + ссылки на generated reference | `npx --yes markdownlint-cli crates/rustok-workflow/CRATE_API.md`; подтверждение владельца модуля |
| B5 | DOC-07 (начальный этап) | `.github`/CI docs jobs + `docs/verification/*` (если нужно) | PR падает на broken links/anchors/markdown errors | Технический PR-проверка с намеренно сломанной ссылкой (ожидаемый fail) |

> Рекомендация: вести batches последовательно B1 → B5; параллелить только B4 (независим от B1/B2/B3).

## Очередь работ (P0 → P2)

> Оценки — инженерные часы без очереди на ревью.

| ID | Приоритет | Задача | Ответственная роль | Оценка | Зависимости | Критерий приёмки |
|---|---|---|---|---:|---|---|
| DOC-01 | P0 | Санировать root docs (`README*`, `CONTRIBUTING`, `CHANGELOG`, quickstart) | Platform docs owner | 14ч | — | Нет mojibake и битых ссылок; команды старта/инструменты согласованы |
| DOC-02 | P0 | Единая таблица профилей запуска и портов | Platform + DevEx | 8ч | DOC-01 | Одна canonical truth table (`profile → host → ports → owner → source`) |
| DOC-03 | P0 | Убрать drift ручного API reference (старт с `rustok-workflow/CRATE_API.md`) | Module owner (`rustok-workflow`) | 12ч | — | Ручные сигнатуры убраны/минимизированы; явные ссылки на generated truth |
| DOC-04 | P0 | Нормализовать `CHANGELOG.md` до release-log формата | Platform docs owner | 6ч | DOC-01 | Структура пригодна для релизов; нет ссылок на отсутствующие файлы |
| DOC-05 | P1 | Сжать `docs/index.md` до короткой карты | Architecture docs owner | 10ч | DOC-01 | Index компактен и навигационный, без статусной хроники |
| DOC-06 | P1 | Синхронизировать `docs/modules/*` registry с `modules.toml` | Module platform owner | 10ч | DOC-02 | Нет «призрачных» модулей; capability/support помечены единообразно |
| DOC-07 | P1 | Ввести docs CI quality gates (lint/link/anchors/fences) | DevEx/CI owner | 12ч | DOC-01 | PR блокируется на broken links/anchors/markdown errors |
| DOC-08 | P1 | Централизовать executable examples + index | DevRel + module owners | 16ч | DOC-02, DOC-07 | Есть единый каталог примеров и smoke-валидация в CI |
| DOC-09 | P2 | Конвейер генерации reference-артефактов (rustdoc/OpenAPI/GraphQL) | Platform + API owners | 20ч | DOC-03, DOC-07 | Артефакты публикуются в CI; diff контрактов контролируется |
| DOC-10 | P2 | Language/naming/review governance | Platform docs owner | 8ч | DOC-05, DOC-07 | Формализованы policy, checklist и процесс ownership-review |
| DOC-11 | P2 | Docs-чеклист ревьюера + обновление PR-шаблона | DevEx/CI owner | 4ч | DOC-07 | Чеклист обязателен и реально используется в docs PR |
| DOC-12 | P2 | Документирование приоритетных hotspot-зон кода в первую очередь | Module owners | 18ч | DOC-09 | Приоритетные узлы закрыты целевыми doc updates |

---

## Детализация задач P0

### DOC-01 — Санировать root docs

**Область изменений:** `README.md`, `README.ru.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, `docs/guides/quickstart.md`.

**Что делаем:**
- исправляем устаревшие URL/названия;
- приводим quickstart и toolchain к актуальным шагам;
- удаляем некорректные/битые фрагменты changelog;
- нормализуем стиль и повторяемость шагов входа.

**Проверка:**
- markdownlint/link-check на изменённых файлах;
- ручной smoke-readme pass по шагам onboarding.

### DOC-02 — Truth table профилей запуска

**Область изменений:** `docs/guides/quickstart.md` + ссылки из root docs.

**Что делаем:**
- добавляем таблицу профилей (Docker, non-Docker, local SSR, Next-hosted);
- для каждого профиля фиксируем owner и canonical source;
- объясняем связь с `modules.toml`.

**Проверка:**
- таблица проходит peer-review от DevEx и platform;
- нет конфликтов портов/ролей между quickstart и скриптами.

### DOC-03 — Drift cleanup manual API docs

**Область изменений:** `crates/rustok-workflow/CRATE_API.md` (+ cross-links).

**Что делаем:**
- убираем ручные списки сигнатур из markdown;
- оставляем curated overview: инварианты, границы, сценарии;
- добавляем ссылки на generated reference.

**Проверка:**
- нет утверждений, противоречащих текущему коду;
- module owner подтверждает, что doc не дублирует rustdoc вручную.

### DOC-04 — Cleanup changelog

**Область изменений:** `CHANGELOG.md`.

**Что делаем:**
- приводим к фиксированному release-шаблону;
- убираем «спринтовый дневник» и stale refs;
- добавляем единые секции (Added/Changed/Fixed/Deprecated/Security).

**Проверка:**
- changelog читается как release history;
- нет ссылок на отсутствующие документы.

---

## Детализация задач P1

### DOC-05 — Рефакторинг `docs/index.md`

**Результат:** короткая карта входа (navigation-first), ссылки только на актуальные разделы и правила выбора дальнейшей документации.

### DOC-06 — Синхронизация реестров модулей

**Результат:** `docs/modules/registry.md`, `_index.md`, `UI_PACKAGES_INDEX.md` синхронизированы с `modules.toml`, включая capability/support-метки.

### DOC-07 — Docs quality gates в CI

**Минимальный набор:**
- markdownlint;
- link/anchor checker;
- fenced code block validation;
- docs build/smoke-джоб.

### DOC-08 — Централизация примеров

**Результат:** единая точка discoverability (`docs/examples/` или `examples/`) + smoke-команды для критичных сценариев.

---

## Детализация задач P2

### DOC-09 — Конвейер генерации reference-артефактов

- rustdoc artifacts;
- OpenAPI export;
- GraphQL schema export;
- PR diff-check на контрактные изменения.

### DOC-10/11 — Governance

- чеклист ревьюера для docs PR;
- обновлённый PR template;
- ownership-правила для архитектурных/контрактных разделов.

### DOC-12 — Hotspots

- приоритизация по impact/risk;
- целевые обновления сначала в зонах наибольшего drift.

---


## DOC-12: приоритетные hotspot-зоны (first pass)

Ниже — стартовый список зон с максимальным риском документационного drift,
которые должны обновляться в первую очередь при изменениях кода.

| Приоритет | Зона | Основные файлы/контракты | Почему это hotspot | Владелец | Минимум для PR |
|---|---|---|---|---|---|
| H1 | Runtime composition и module manifest | `modules.toml`, `docs/modules/registry.md`, `docs/index.md` | Любой drift ломает навигацию и контракт module ownership | Platform foundation + module platform owner | синхронизация registry/index + ссылка на changed manifest entries |
| H2 | Installer / bootstrap flow | `docs/guides/quickstart.md`, `apps/server/docs/*`, `DECISIONS/2026-04-26-hybrid-installer-architecture.md` | Быстро устаревает из-за изменений install/preflight/apply цепочки | Platform foundation | обновить runbook + preflight/apply примеры + environment constraints |
| H3 | Admin/storefront host topology | `CONTRIBUTING.md`, `docs/guides/quickstart.md`, `docs/UI/*` | Частые изменения портов/host wiring и transport contracts | Frontend owners | подтвердить profile matrix + host start commands + transport notes |
| H4 | Workflow/Public API contracts | `crates/rustok-workflow/CRATE_API.md`, `apps/server/src/modules/workflow.rs` | Высокий риск ручного дублирования сигнатур | Workflow owner | обновить curated overview/invariants без ручных сигнатур |
| H5 | Release and compatibility communication | `CHANGELOG.md`, `README.md`, `README.ru.md` | Дрейф релизных заметок и root onboarding влияет на весь репозиторий | Platform docs owner | release-log sections + кросс-ссылки на canonical docs |

### Правило DOC-12 для каждого PR в hotspot-зоне

Если PR затрагивает hotspot-зону, в описании PR обязательно добавить блок:

- `Hotspot:` `H1..H5`
- `Doc contracts updated:` список файлов
- `Residual drift risk:` что осталось вне scope (если есть)

## Рекомендуемая дорожная карта (4 недели)

- **Неделя 1:** DOC-01, DOC-02, DOC-03, DOC-04.
- **Неделя 2:** DOC-05, DOC-06, старт DOC-07.
- **Неделя 3:** завершение DOC-07, DOC-08, старт DOC-09.
- **Неделя 4:** DOC-09, DOC-10, DOC-11, DOC-12.

## Минимальный спринт (если ограничены ресурсами)

Сделать только:
1. DOC-01;
2. DOC-02;
3. DOC-03;
4. DOC-07.

Это даёт максимальное снижение риска документационного drift уже в первом цикле.

---



## Исполняемый backlog на ближайший цикл (операционализация)

Чтобы «продолжить реализацию» без дополнительного планирования, фиксируем
следующий исполняемый набор задач поверх batches B1–B5.

### Batch card template (копировать в каждый PR)

```md
### Batch Card
- Batch: `B?`
- Закрывает: `DOC-..`
- Scope: `<список файлов>`
- Depends on: `<batch/ID или none>`
- Done when:
  - [ ] критерий 1
  - [ ] критерий 2
- Verification Evidence:
  - YYYY-MM-DD — `<command>` — pass/fail/blocked
```

### Следующие батчи после B1–B5

| Batch | Закрывает | Область изменений (точно) | Exit criteria |
|---|---|---|---|
| B6 | DOC-05 | `docs/index.md` | Индекс сокращён до navigation-first карты без статусной хроники и с валидными кросс-ссылками |
| B7 | DOC-06 | `docs/modules/registry.md`, `docs/modules/_index.md`, `docs/modules/UI_PACKAGES_INDEX.md`, `modules.toml` (только сверка) | Все module entries синхронизированы, capability/support метки выровнены |
| B8 | DOC-07 (завершение) | `.github/workflows/*docs*` и/или существующие CI docs jobs, `docs/verification/*` | CI действительно блокирует PR на markdown/link/anchor ошибки |
| B9 | DOC-11 | `.github/pull_request_template.md`, `docs/guides/*` (если нужно) | PR template содержит обязательный docs checklist и Verification Evidence |
| B10 | DOC-12 (first pass) | hotspot H1–H3 документы из этого плана | Для каждой hotspot-зоны есть owner, scope и минимальный PR contract |

### B6 kickoff package (готово к исполнению)

Чтобы следующий PR мог сразу стартовать DOC-05, фиксируем заранее:

- **Batch:** `B6`
- **Закрывает:** `DOC-05`
- **Scope (strict):**
  - `docs/index.md`
- **Out of scope (для этого батча):**
  - изменение содержимого профильных документов;
  - перенос ADR/verification-истории в другие файлы;
  - правки `docs/modules/*` (это зона B7).

#### B6: ожидаемый diff-pattern

1. Сократить вводный раздел `docs/index.md` до navigation-first правил (без исторических статусных вставок).
2. Оставить только актуальные «точки входа» по разделам платформы.
3. Убрать дублирующиеся/шумовые пояснения, которые не влияют на маршрутизацию читателя.
4. Проверить, что ссылки на `docs/modules/registry.md` и `DECISIONS/README.md` остаются доступными.

#### B6: Verification plan (шаблон для исполнения)

```md
### Verification Evidence
- YYYY-MM-DD — `npx --yes markdownlint-cli docs/index.md` — pass/fail/blocked
- YYYY-MM-DD — `lychee --no-progress docs/index.md` — pass/fail/blocked
- YYYY-MM-DD — `sed -n '1,220p' docs/index.md` — pass
```

### Матрица исполнения B6–B10 (owner + handoff)

| Batch | Owner role | Reviewer role | Handoff artifact | Blockers to check before start |
|---|---|---|---|---|
| B6 | Architecture docs owner | Platform docs owner | PR + обновлённый `docs/index.md` diff-summary | Нет параллельного PR, меняющего карту `docs/index.md` |
| B7 | Module platform owner | Platform foundation | Сверка `registry/_index/UI_PACKAGES_INDEX` ↔ `modules.toml` | Зафиксирован текущий `modules.toml` baseline в PR description |
| B8 | DevEx/CI owner | Platform docs owner | CI job logs + intentional fail evidence | Есть отдельная тест-ветка/PR для негативной проверки |
| B9 | DevEx/CI owner | Architecture docs owner | Обновлённый PR template + пример заполнения | Шаблон не конфликтует с обязательными разделами репозитория |
| B10 | Module owners (H1–H3) | Platform docs owner | Hotspot blocks в PR (`Hotspot`, `Doc contracts updated`, `Residual drift risk`) | Для каждого hotspot назначен конкретный владелец |

Перед стартом batch добавлять короткий preflight-чеклист в PR:

- [ ] Подтверждён owner и reviewer для batch;
- [ ] Проверено отсутствие конфликтующего PR по тому же scope;
- [ ] Scope ограничен файлами batch либо явно расширен в `Risks`;
- [ ] Проверки и Verification Evidence запланированы заранее.

### Протокол запуска следующего batch (DoR/DoD-lite)

Перед стартом любого batch исполнитель обязан зафиксировать в PR:

1. `Batch` и `DOC-ID` из таблицы B6–B10 (без переименований);
2. точный `Scope` (только файлы из batch, расширение scope — отдельным пунктом в Risks);
3. `Verification Evidence` с фактическим статусом каждой проверки;
4. `Residual drift risk` — что сознательно оставлено вне PR.

Batch считается закрытым только если одновременно выполнены условия:

- выполнены все `Done when` критерии из batch card;
- трекер обновлён в формате `[x] ... (Batch: B?, PR: #..., merged: YYYY-MM-DD)`;
- в `docs/index.md` (или профильном индексе) добавлены/проверены кросс-ссылки, если scope затрагивал навигацию.

### Правило обновления статусов

- В каждом docs PR обновляется только соответствующий пункт в «Трекере статуса».
- Статус `[~]` разрешён только при наличии открытого PR-номера.
- Статус `[x]` ставится только после merge в default branch с датой merge.
- Если задача декомпозирована на batches, в пункте указывается последний активный batch.

Формат:
- `- [~] DOC-06 ... (Batch: B7, PR: #1234)`
- `- [x] DOC-06 ... (Batch: B7, PR: #1234, merged: YYYY-MM-DD)`

### Журнал исполнения батчей (обновлять по факту PR)

Этот блок нужен, чтобы план был не только «что делать», но и «что уже сделано».
Запись добавляется только после открытия PR (для `[~]`) или после merge (для `[x]`).

| Date | Batch | DOC | Status | PR | Notes |
|---|---|---|---|---|---|
| YYYY-MM-DD | B6 | DOC-05 | `[~]`/`[x]` | `#1234` | Кратко: что закрыто / что осталось |
| 2026-05-22 | B6 | DOC-05 | `[x]` | `commit: 1d087a3` | `docs/index.md` переведён в navigation-first, убрана статусная хроника |
| 2026-05-22 | B7 | DOC-06 | `[x]` | `commit: c9a22f1` | Реестры синхронизированы с `modules.toml` |
| 2026-05-22 | B8 | DOC-07 | `[x]` | `commit: 1bf7ead` | Зафиксирован baseline quality-gates в verification-планах |

Пример реальной записи после merge:

| 2026-05-22 | B6 | DOC-05 | `[x]` | `#1234` | `docs/index.md` переведён в navigation-first, убрана статусная хроника |

Правила заполнения:

- Запрещены записи вида `PR: pending`.
- Запрещены фиктивные номера PR (`#NNNN`, `#0000`, `#TBD` и т.п.).
- Если статус `[~]`, поле `PR` обязательно содержит реальный номер PR.
- Если статус `[x]`, в `Notes` указывается дата merge и что именно вошло в scope.
- Одна строка журнала = один batch-PR.

## Правила ведения статусов

- `[ ]` — не начато;
- `[~]` — в работе (есть открытый PR);
- `[x]` — завершено и вмержено в default branch;
- у каждого пункта в PR обязательно указывать ссылку на закрывающий PR в комментарии к пункту.



Формат отметки в трекере:
- `- [~] DOC-01 ... (PR: #1234)`
- `- [x] DOC-01 ... (PR: #1234, merged: YYYY-MM-DD)`

## Трекер статуса (обновлять в каждом docs PR)

> Правило консистентности: `pending` не может использоваться со статусом `[~]`.
> Если открытого PR ещё нет, используем только `[ ]`.

- [x] DOC-01 Root docs sanitation (commit: aa36e6f, merged: 2026-05-22)
- [x] DOC-02 Profiles truth table (commit: 0f61cec, merged: 2026-05-22)
- [x] DOC-03 Workflow API doc drift cleanup (commit: 1baf08e, merged: 2026-05-22)
- [x] DOC-04 Changelog normalization (commit: 265b332, merged: 2026-05-22)
- [x] DOC-05 docs/index.md refactor (commit: 1d087a3, merged: 2026-05-22)
- [x] DOC-06 Registry ↔ manifest sync (commit: c9a22f1, merged: 2026-05-22)
- [x] DOC-07 Docs CI quality gates (commit: 1bf7ead, merged: 2026-05-22)
- [x] DOC-08 Executable examples hub (commit: 0f35991, merged: 2026-05-22)
- [ ] DOC-09 Конвейер генерации reference-артефактов
- [ ] DOC-10 Language/naming governance
- [ ] DOC-11 Reviewer checklist + PR template
- [ ] DOC-12 Code hotspots documentation

### Процедура синхронизации трекера с уже влитыми PR

Если изменения уже вмержены, но статус в трекере ещё `[ ]`, обновление делаем
отдельным docs PR по шагам:

1. найти merge commit и номер PR в `git log --oneline`;
2. обновить соответствующий пункт трекера на `[x]` c `(PR: #..., merged: YYYY-MM-DD)`;
3. добавить строку в «Журнал исполнения батчей» с тем же PR и кратким scope;
4. в Verification Evidence приложить команды, которыми подтверждён merge-след.

Если номер PR недоступен (например, squash/rebase без явной ссылки в истории),
допускается временно использовать `merge commit` как идентификатор в формате:

- `(commit: <short-sha>, merged: YYYY-MM-DD)`.

После восстановления номера PR запись должна быть обновлена до canonical
формы с `PR: #...`.

### Политика формулировок в разделе Testing (для docs PR)

Чтобы исключить противоречия между фактом и описанием:

- если команды запускались, запрещено писать `No automated tests were executed`;
- если проверки не запускались по policy, писать только
  `text-only: checks skipped by policy` и причину;
- если часть проверок запускалась, а часть нет, это фиксируется построчно
  с отдельным статусом `pass` / `fail` / `blocked` для каждой команды.

### Политика согласованности Testing ↔ Verification Evidence

Разделы **Testing** и **Verification Evidence** в одном PR обязаны быть
строго согласованы:

- нельзя заявлять в Testing, что проверка `pass`, если в Verification Evidence
  у той же команды указан `fail` или `blocked`;
- для каждой команды из Testing должна быть парная строка в Verification
  Evidence с той же командой и тем же статусом;
- если инструмент отсутствует (`command not found`), в обоих разделах статус
  обязан быть `blocked`, без формулировок «passed in CI/local».

### Обязательная дата запуска проверок

Во всех строках `Verification Evidence` дата запуска обязательна и указывается
в формате `YYYY-MM-DD` (например, `2026-05-22`).

Запрещены:

- относительные формулировки (`today`, `yesterday`, `сегодня`, `вчера`);
- неполные даты (`22-05-2026`, `2026/05/22`);
- строки без даты.

### Правило для text-only: единая формулировка

Для docs PR, где проверки не запускались по policy, используется только одна
формулировка в разделе **Testing**:

- `text-only: checks skipped by policy`.

Дописывать одновременно `No automated tests were executed` запрещено,
поскольку это создаёт дублирующее и противоречивое описание статуса проверок.

### Шаблон причины для статуса blocked

Чтобы `blocked`-отчёты были единообразными, используем формат:

- `reason: <точная stderr/stdout строка или ограничение окружения>`.

Примеры корректных формулировок:

- `reason: /bin/bash: lychee: command not found`;
- `reason: network disabled in CI job`;
- `reason: required secret is not available for fork PR`.

### Запрет на ретроактивное изменение статусов без основания

Статусы в `Testing`, `Verification Evidence`, трекере и журнале batch-исполнения
нельзя менять задним числом без явного основания.

Если статус исправляется постфактум, в PR обязательно добавляется:

- `Correction note:` что именно было изменено;
- `Why corrected:` причина исправления (например, неверно записанный exit code);
- `Evidence:` команда/лог, подтверждающий корректный статус.

### Минимальный audit-bundle для каждого docs PR

Чтобы любой reviewer мог проверить отчёт без дополнительного запроса логов,
каждый docs PR должен содержать минимальный audit-bundle:

- список команд из раздела **Testing**;
- зеркальный список в **Verification Evidence** с датой и статусом;
- ссылку/указание на изменённые файлы (scope);
- при `fail`/`blocked` — `reason` и следующий шаг.

### Чеклист самопроверки автора перед публикацией docs PR

Перед отправкой PR автор обязан подтвердить:

- [ ] статус каждой команды в **Testing** совпадает со статусом в
      **Verification Evidence**;
- [ ] у каждой строки в **Verification Evidence** есть дата `YYYY-MM-DD`;
- [ ] для каждого `blocked` указан `reason: ...` в стандартном формате;
- [ ] отсутствуют фиктивные PR-идентификаторы (`#NNNN`, `#TBD`, `pending`);
- [ ] scope в PR совпадает с фактически изменёнными файлами.

### Чеклист reviewer-а для валидации отчёта

Reviewer перед approve проверяет:

- [ ] команды из **Testing** дословно присутствуют в **Verification Evidence**;
- [ ] статусы команд совпадают между двумя разделами;
- [ ] для каждого `blocked` есть `reason: ...` и указанный следующий шаг;
- [ ] даты в Evidence указаны в формате `YYYY-MM-DD`;
- [ ] нет формулировок, противоречащих policy
      (`No automated tests were executed` при фактически запущенных командах).

### Правило закрытия review-замечаний по отчётности

Если в PR был review-комментарий по разделам **Testing** / **Verification Evidence**,
автор при исправлении обязан явно указать:

- `Resolved review note:` кратко, какое замечание закрыто;
- `What changed:` какие строки/статусы исправлены;
- `Re-check command:` команда, которой подтверждена корректность после правки.

## Продолжение реализации: executable backlog для DOC-09..DOC-12

Чтобы закрыть оставшийся хвост плана без повторного перепланирования,
фиксируем готовые к запуску батчи для задач DOC-09..DOC-12.

### Следующие батчи после B10

| Batch | Закрывает | Область изменений (точно) | Exit criteria |
|---|---|---|---|
| B11 | DOC-09 (phase 1) | `xtask/`, `scripts/`, `docs/verification/*` | Добавлены команды генерации rustdoc/OpenAPI/GraphQL артефактов и описан единый pipeline запуска |
| B12 | DOC-09 (phase 2) | `.github/workflows/*`, `docs/verification/*` | CI публикует reference-артефакты и показывает diff контрактов в PR |
| B13 | DOC-10 + DOC-11 | `.github/pull_request_template.md`, `docs/guides/*`, `docs/standards/*` | Формализованы governance-правила и reviewer checklist; шаблон PR требует Verification Evidence |
| B14 | DOC-12 | hotspot-документы H1..H5 (`docs/modules/*`, `docs/guides/*`, `docs/UI/*`, `README*`) | Для каждой hotspot-зоны есть актуализированный doc-contract и зафиксирован residual drift risk |

### Batch cards (готовые заготовки)

```md
### Batch Card
- Batch: `B11`
- Закрывает: `DOC-09 (phase 1)`
- Scope:
  - `xtask/*`
  - `scripts/*`
  - `docs/verification/*`
- Depends on: `B8`
- Done when:
  - [ ] есть воспроизводимый локальный запуск генерации rustdoc/OpenAPI/GraphQL
  - [ ] команды и output-пути описаны в `docs/verification/*`
```

```md
### Batch Card
- Batch: `B12`
- Закрывает: `DOC-09 (phase 2)`
- Scope:
  - `.github/workflows/*`
  - `docs/verification/*`
- Depends on: `B11`
- Done when:
  - [ ] reference-артефакты публикуются CI job-ом
  - [ ] PR показывает contract diff (или explicit no-diff)
```

```md
### Batch Card
- Batch: `B13`
- Закрывает: `DOC-10`, `DOC-11`
- Scope:
  - `.github/pull_request_template.md`
  - `docs/guides/*`
  - `docs/standards/*`
- Depends on: `B8`
- Done when:
  - [ ] governance policy формализован без конфликтов с AGENTS
  - [ ] docs reviewer checklist обязателен в PR template
```

```md
### Batch Card
- Batch: `B14`
- Закрывает: `DOC-12`
- Scope:
  - hotspot H1..H5 документы из этого плана
- Depends on: `B11`, `B13`
- Done when:
  - [ ] по каждой hotspot-зоне есть owner + обновлённые doc-contracts
  - [ ] для каждой зоны зафиксирован residual drift risk
```

### Трекер статуса (рабочий порядок закрытия хвоста)

- [ ] DOC-09 Конвейер генерации reference-артефактов (Batch: B11 → B12)
- [ ] DOC-10 Language/naming governance (Batch: B13)
- [ ] DOC-11 Reviewer checklist + PR template (Batch: B13)
- [ ] DOC-12 Code hotspots documentation (Batch: B14)

### Verification policy для B11..B14

Для батчей B11..B14 раздел **Verification Evidence** обязателен и должен
минимум включать:

- дату запуска (`YYYY-MM-DD`);
- точную команду;
- статус (`pass`/`fail`/`blocked`);
- строку `reason: ...` для каждого `fail`/`blocked`;
- ссылку на изменённые файлы в scope батча.

### Runbook запуска B11..B14 (без дополнительного планирования)

Ниже — минимальный protocol, чтобы каждый следующий PR стартовал одинаково и
без расхождений в отчётности.

1. Выбрать ровно один batch (`B11`/`B12`/`B13`/`B14`) и зафиксировать его в PR
   заголовке по шаблону `docs: DOC-XX ...`.
2. До первого коммита вставить `Batch Card` в описание PR и проверить scope по
   `git diff --name-only`.
3. Выполнить проверки из раздела ниже и перенести статусы в
   `Verification Evidence` без переформулировок.
4. Обновить «Трекер статуса» только для соответствующего DOC-пункта.

### Минимальные команды проверки по batch

| Batch | Команды для Verification Evidence | Ожидаемый baseline |
|---|---|---|
| B11 | `cargo xtask --help`; `rg -n "rustdoc|openapi|graphql" xtask scripts docs/verification` | Команды генерации/экспорта найдены и задокументированы |
| B12 | `rg -n "markdown|link|docs|artifact|upload" .github/workflows`; `sed -n '1,260p' docs/verification/README.md` | CI-конвейер и публикация артефактов явно описаны |
| B13 | `sed -n '1,260p' .github/pull_request_template.md`; `rg -n "checklist|Verification Evidence|owner" docs/guides docs/standards` | PR template и governance-checklist синхронизированы |
| B14 | `rg -n "Hotspot|Residual drift risk|Doc contracts updated" docs`; `rg -n "H1|H2|H3|H4|H5" docs/research/fix\ docs.md` | Для hotspot-зон есть обязательные блоки и owner-контекст |

### Definition of Done для DOC-09..DOC-12 (строгая фиксация)

- `DOC-09` закрывается только после merge двух батчей (`B11` и `B12`) и
  наличия evidence по локальному запуску + CI публикации артефактов.
- `DOC-10` и `DOC-11` закрываются одновременно одним батчем (`B13`), так как
  governance-policy и PR-template проверяются как единый контракт.
- `DOC-12` закрывается только после `B14` и подтверждения, что для H1..H5
  добавлены `Hotspot`, `Doc contracts updated`, `Residual drift risk`.

### Anti-overlap правило для параллельной работы

Чтобы избежать конфликтов между авторами, при запуске batch применяется
фиксированная ownership-матрица:

- `B11`: владелец `xtask/` + `scripts/` + `docs/verification/*`;
- `B12`: владелец `.github/workflows/*` + `docs/verification/*`;
- `B13`: владелец `.github/pull_request_template.md` + governance docs;
- `B14`: владельцы hotspot-зон H1..H5 по module registry.

Если два batch пересекаются по одному файлу, более поздний batch стартует
только после merge более раннего по зависимости (`Depends on`).
