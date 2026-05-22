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


## Правила ведения статусов

- `[ ]` — не начато;
- `[~]` — в работе (есть открытый PR);
- `[x]` — завершено и вмержено в default branch;
- у каждого пункта в PR обязательно указывать ссылку на закрывающий PR в комментарии к пункту.



Формат отметки в трекере:
- `- [~] DOC-01 ... (PR: #1234)`
- `- [x] DOC-01 ... (PR: #1234, merged: YYYY-MM-DD)`

## Трекер статуса (обновлять в каждом docs PR)

- [ ] DOC-01 Root docs sanitation
- [~] DOC-02 Profiles truth table (PR: pending)
- [ ] DOC-03 Workflow API doc drift cleanup
- [ ] DOC-04 Changelog normalization
- [ ] DOC-05 docs/index.md refactor
- [ ] DOC-06 Registry ↔ manifest sync
- [ ] DOC-07 Docs CI quality gates
- [ ] DOC-08 Executable examples hub
- [ ] DOC-09 Конвейер генерации reference-артефактов
- [ ] DOC-10 Language/naming governance
- [ ] DOC-11 Reviewer checklist + PR template
- [ ] DOC-12 Code hotspots documentation
