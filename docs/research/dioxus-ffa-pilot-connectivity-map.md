# FFA/Dioxus Pilot Connectivity Map (Phase A baseline)

Документ фиксирует исполнение шагов **A1 (выбор пилотов)** и **A2 (карта связности)**
из `docs/research/dioxus-ffa-ui-migration-plan.md`.

## A1. Выбранные пилоты

### Pilot 1 (средняя сложность): `rustok-pages`

Причины выбора:
- ограниченный storefront read/edit surface;
- меньшее число cross-module зависимостей по сравнению с commerce/search;
- хороший кандидат для первого выделения `core -> transport -> ui/leptos`.

### Pilot 2 (высокая сложность): `rustok-search`

Причины выбора:
- выраженная search-state логика (query/filter/pagination/sort);
- чувствительность к route/query parity и locale/tenant context;
- наличие fallback ветвей и runtime-ветвления для SSR/GraphQL path.

## A2. Карта связности

## `rustok-pages`

### Leptos-specific точки
- `#[component]` поверхности storefront/admin;
- router/query hooks и навигационные binding-слои;
- reactive state и signal-производные для page selection/render.

### Transport binding точки
- native `#[server]` handlers для SSR/hydrate path;
- GraphQL fallback через module-owned API adapters;
- `cfg(feature = "ssr")` ветвление для runtime split.

### Риски смешения слоёв
- прямые вызовы transport из UI-компонентов;
- смешение view-state и domain mapping в Leptos hooks;
- дублирование error mapping между native и GraphQL path.

## `rustok-search`

### Leptos-specific точки
- `#[component]` search surfaces и filters UI;
- routing/query state binding (включая URL-driven selection state);
- reactive derived state для paging/sorting/empty/error views.

### Transport binding точки
- native `#[server]` read/search path в SSR/hydrate;
- GraphQL fallback adapters для headless/CSR-compatible flow;
- runtime условные ветви для fallback/degredation режима.

### Риски смешения слоёв
- связывание transport payload формата с UI-моделью напрямую;
- неявные policy/validation checks в UI слое;
- расхождение query normalization между native и GraphQL path.

## Phase A deliverables status

- [x] A1 pilot selection зафиксирован.
- [x] A2 connectivity map зафиксирован.
- [ ] A3 contract freeze evidence полностью приложен в рамках первой migration-задачи по коду.

## Следующий шаг (one-task-per-iteration)

Следующая итерация: для `rustok-pages` выделить **один** целевой use-case и провести
структурный срез `core/transport/ui` без изменения продуктового dual-path контракта,
с обязательным evidence по checklist:
`docs/verification/ffa-ui-parity-checklist.md`.

## Связанные документы

- `docs/research/dioxus-ffa-ui-migration-plan.md`
- `docs/verification/ffa-ui-parity-checklist.md`
- `docs/UI/graphql-architecture.md`


## Статус выполнения по модулям (Phase B tracking)

- [x] `rustok-pages` — выполнен первый slice декомпозиции: в `storefront` выделен `core` слой
  для selected-page presentation logic; Leptos UI делегирует эту логику в `core`.
- [ ] `rustok-search` — в очереди на первый code slice Phase B.

### Что уже сделано в `rustok-pages`

- добавлен `crates/rustok-pages/storefront/src/core.rs`;
- `SelectedPageCard` в `crates/rustok-pages/storefront/src/lib.rs` переведён на `core::*` функции;
- dual-path transport контракт (`native #[server]` + GraphQL fallback) не менялся.


### Перепроверка после выполненного (double-check)

- [x] Проход №1 (code/docs consistency):
  - `rustok-pages/storefront` фактически использует выделенный `core` слой для selected-page logic;
  - dual-path transport (`native #[server]` + GraphQL fallback) сохранён без удаления fallback surface.
- [x] Проход №2 (устранение устаревших формулировок):
  - в текущих central docs для этого шага не осталось формулировок, противоречащих `core`-срезу в `rustok-pages`.

### Следующий модуль (новая итерация)

- [ ] Стартуем `rustok-search` как следующий pilot slice Phase B.
- [ ] Цель итерации: выделить первый `core` use-case в `crates/rustok-search/storefront`
      без изменения продуктового transport-контракта.


### Scope matrix для `rustok-search` (чтобы ничего не пропустить)

- [x] `crates/rustok-search/storefront` (Leptos storefront UI package)
  - [x] выделен первый `core` use-case (query/filter input normalization: `parse_csv`, `optional_text`);
  - [x] выбранный use-case вынесен в `storefront/src/core.rs` и используется UI-слоем.
- [x] `crates/rustok-search/admin` (Leptos admin UI package)
  - [x] impact того же use-case проверен;
  - [x] тот же `core`-подход применён в `admin/src/core.rs` без расхождения контракта.
- [ ] Headless parity (Next/mobile/external)
  - [ ] подтверждено, что GraphQL fallback path не деградировал;
  - [ ] route/query/i18n contract не получил drift относительно host expectations.

### Evidence чек перед закрытием итерации `rustok-search`

- [ ] `cargo xtask module validate search` (запланировано после завершения следующего slice с transport assertions)
- [ ] `cargo xtask module test search` (запланировано после завершения следующего slice с transport assertions)
- [ ] docs double-check pass #1 (code/docs consistency)
- [ ] docs double-check pass #2 (cleanup stale wording)


### Выполнено в текущей итерации (`rustok-search`, slice #1)

- добавлены `crates/rustok-search/storefront/src/core.rs` и `crates/rustok-search/admin/src/core.rs`;
- в storefront/admin UI удалены локальные дубли `parse_csv`/`optional_text` и подключены `core::*`;
- dual-path transport (`native #[server]` + GraphQL fallback) не изменялся.

- `rustok-search` slice #2: facet name normalization вынесена в core для storefront/admin (`facet_display_name`).
- `rustok-search` slice #3: facet bucket label formatting вынесен в core (`facet_bucket_label`) для storefront/admin.
- `rustok-search` slice #4: snippet fallback rendering вынесен в core (`snippet_or_fallback`) для storefront/admin.
- `rustok-search` slice #5: score label normalization вынесена в core (`score_label`) для storefront/admin.
- `rustok-search` slice #6: entity/source/status labels вынесены в core для storefront/admin.
- `rustok-search` slice #7: score template value extraction переведён на core helper (`score_value`) без string hacks в UI.
