# План правки документации RusTok

## Цель

Преобразовать findings из аудита в управляемый execution-план по исправлению документации: с понятными приоритетами, ответственными ролями, трудоёмкостью, зависимостями и проверяемыми критериями приёмки.

## Границы и допущения

- Этот документ фиксирует **план работ**, а не финальные архитектурные решения.
- Источник истины для API-контрактов — код и generated artifacts (rustdoc/OpenAPI/GraphQL schema export).
- Для `docs/` используется политика «один файл — один язык» (базово русский).
- `docs/index.md` — короткая карта входа; статусные полотна и roadmaps должны жить в профильных implementation-plan документах.

---

## Формат исполнения

Каждая задача выполняется через отдельный PR с одинаковым шаблоном:

1. **Scope** (какие файлы/разделы меняем);
2. **Definition of Done** (что считается завершением);
3. **Verification** (какие проверки/команды подтверждают результат);
4. **Links updated** (какие ссылки/индексы обновлены дополнительно).

---



## Операционная модель выполнения (обязательно для каждого PR)

### Шаблон названия PR

`docs: DOC-XX <краткое действие>`

### Шаблон тела PR

- **ID:** `DOC-XX`
- **Scope:** `<файлы и разделы>`
- **Definition of Done:** `<проверяемые критерии>`
- **Verification:**
  - `<команда 1>`
  - `<команда 2>`
- **Links updated:** `<какие индексы/кросс-ссылки обновлены>`
- **Risks / Follow-ups:** `<что не вошло и почему>`

### Минимальные проверки для docs PR

- `pnpm markdownlint <changed-files>` (или эквивалентный скрипт репозитория);
- `pnpm link-check <changed-files>` (или эквивалентный скрипт репозитория);
- ручной проход всех изменённых ссылок из `docs/index.md` и локальных `README.md`.

Если в текущем PR нет CI-скрипта под один из пунктов, фиксируем это в разделе **Risks / Follow-ups** и создаём follow-up в DOC-07.


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
| DOC-09 | P2 | Generated reference pipeline (rustdoc/OpenAPI/GraphQL) | Platform + API owners | 20ч | DOC-03, DOC-07 | Артефакты публикуются в CI; diff контрактов контролируется |
| DOC-10 | P2 | Language/naming/review governance | Platform docs owner | 8ч | DOC-05, DOC-07 | Формализованы policy, checklist и процесс ownership-review |
| DOC-11 | P2 | Docs reviewer checklist + PR template upgrade | DevEx/CI owner | 4ч | DOC-07 | Чеклист обязателен и реально используется в docs PR |
| DOC-12 | P2 | Документирование code hotspots first | Module owners | 18ч | DOC-09 | Приоритетные узлы закрыты целевыми doc updates |

---

## Детализация задач P0

### DOC-01 — Санировать root docs

**Scope:** `README.md`, `README.ru.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, `docs/guides/quickstart.md`.

**Что делаем:**
- исправляем устаревшие URL/названия;
- приводим quickstart и toolchain к актуальным шагам;
- удаляем некорректные/битые фрагменты changelog;
- нормализуем стиль и повторяемость шагов входа.

**Verification:**
- markdownlint/link-check на изменённых файлах;
- ручной smoke-readme pass по шагам onboarding.

### DOC-02 — Truth table профилей запуска

**Scope:** `docs/guides/quickstart.md` + ссылки из root docs.

**Что делаем:**
- добавляем таблицу профилей (Docker, non-Docker, local SSR, Next-hosted);
- для каждого профиля фиксируем owner и canonical source;
- объясняем связь с `modules.toml`.

**Verification:**
- таблица проходит peer-review от DevEx и platform;
- нет конфликтов портов/ролей между quickstart и скриптами.

### DOC-03 — Drift cleanup manual API docs

**Scope:** `crates/rustok-workflow/CRATE_API.md` (+ cross-links).

**Что делаем:**
- убираем ручные списки сигнатур из markdown;
- оставляем curated overview: инварианты, границы, сценарии;
- добавляем ссылки на generated reference.

**Verification:**
- нет утверждений, противоречащих текущему коду;
- module owner подтверждает, что doc не дублирует rustdoc вручную.

### DOC-04 — Cleanup changelog

**Scope:** `CHANGELOG.md`.

**Что делаем:**
- приводим к фиксированному release-шаблону;
- убираем «спринтовый дневник» и stale refs;
- добавляем единые секции (Added/Changed/Fixed/Deprecated/Security).

**Verification:**
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

### DOC-09 — Generated reference pipeline

- rustdoc artifacts;
- OpenAPI export;
- GraphQL schema export;
- PR diff-check на контрактные изменения.

### DOC-10/11 — Governance

- reviewer checklist для docs PR;
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

## Трекер статуса (обновлять в каждом docs PR)

- [ ] DOC-01 Root docs sanitation
- [ ] DOC-02 Profiles truth table
- [ ] DOC-03 Workflow API doc drift cleanup
- [ ] DOC-04 Changelog normalization
- [ ] DOC-05 docs/index.md refactor
- [ ] DOC-06 Registry ↔ manifest sync
- [ ] DOC-07 Docs CI quality gates
- [ ] DOC-08 Executable examples hub
- [ ] DOC-09 Generated reference pipeline
- [ ] DOC-10 Language/naming governance
- [ ] DOC-11 Reviewer checklist + PR template
- [ ] DOC-12 Code hotspots documentation
