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
