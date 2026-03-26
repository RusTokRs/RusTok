# Документация Leptos Admin

Локальная документация для `apps/admin`.

## Текущий runtime contract

- UI/state: `leptos`, `leptos_router`, `Resource`, actions.
- GraphQL transport: `crates/leptos-graphql`.
- `/modules` использует `buildProgress` через `/api/graphql/ws`; polling остаётся только fallback-механизмом.
- `/modules` detail panel умеет рендерить schema-driven tenant settings form из `[settings]` в `rustok-module.toml`.
- FSD-структура остаётся канонической: `app/`, `pages/`, `widgets/`, `features/`, `entities/`, `shared/`.
- Tailwind/shadcn миграция завершена: новые экраны используют семантические CSS-переменные и общие UI-примитивы.

## Generated module UI wiring

- `apps/admin/build.rs` читает `modules.toml` и модульные `rustok-module.toml`, затем генерирует manifest-driven wiring в `OUT_DIR`.
- Текущий contract для publishable Leptos admin UI: `[provides.admin_ui].leptos_crate` плюс экспорт корневого компонента `<PascalSlug>Admin`.
- Host регистрирует generic surfaces без знания о конкретном модуле: `AdminSlot::DashboardSection`, `AdminSlot::NavItem`, `AdminPageRegistration`.
- Для module-owned admin pages используется host route `/modules/:module_slug` и nested-вариант `/modules/:module_slug/*module_path`.
- Header shell использует `rustok-search` как host-level capability: глобальный поиск идёт через GraphQL `adminGlobalSearch` и умеет передавать пользователя в полный search control plane.
- `[provides.admin_ui]` может задавать `route_segment`, `nav_label` и `[[provides.admin_ui.pages]]` для manifest-driven secondary nav.
- `build.rs` также публикует список `Core`-модулей с UI, поэтому такие surfaces монтируются в host всегда и не зависят от tenant module toggles.

## Правило ownership UI

- Если модуль поставляет UI для админки, этот UI живёт рядом с модулем и остаётся module-owned независимо от `Core`/`Optional`.
- `apps/admin` выступает host/composition root и не переносит модульный business UI в свой код.
- Core-модули с UI подчиняются тому же правилу, что и optional-модули: наличие UI не делает host владельцем модульной поверхности.

## Рабочие exemplar-ы

- `rustok-pages-admin` — базовый page CRUD.
- `rustok-blog-admin` — content CRUD без blog-specific логики в host.
- `rustok-content-admin` — core module-owned CRUD без переноса content-specific UI в `apps/admin`.
- `rustok-commerce-admin` — commerce catalog CRUD без переноса catalog-specific UI в host.
- `rustok-forum-admin` — admin-only forum surface с category/topic CRUD через модульный REST contract.

## Ограничения

- Nested contract пока intentionally thin: host знает только wildcard route, `UiRouteContext` и manifest-driven secondary nav; само ветвление по subpath остаётся внутри module package.
- `workflow` уже использует этот contract для `/modules/workflow/templates`, но часть detail/edit flow пока живёт на legacy-маршрутах `/workflows/*`.
- Для внешних crates вне текущего workspace всё ещё нужен более явный entry-point contract, чем текущие naming conventions.

## Связанные документы

- [План реализации](./implementation-plan.md)
- [Контракты UI API](../../../UI/docs/api-contracts.md)
- [Каталог UI-компонентов Rust](../../../docs/UI/rust-ui-component-catalog.md)
- [Карта документации](../../../docs/index.md)
