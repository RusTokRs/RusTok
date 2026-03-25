# Документация Leptos Admin

Локальная документация для `apps/admin`.

## Текущий runtime contract

- UI/state: `leptos`, `leptos_router`, `Resource`, actions.
- GraphQL transport: `crates/leptos-graphql`.
- Build progress: `/modules` использует `buildProgress` через `/api/graphql/ws`, polling остаётся только fallback-механизмом.
- `/modules` detail panel теперь умеет рендерить schema-driven tenant settings form из `[settings]` в `rustok-module.toml`: scalar-поля (`string` / `integer` / `number` / `boolean`) идут typed controls, `object` / `array` получают top-level structured editor и deep nested editor с create-actions, key rename и array reorder на каждом object/array уровне, а `json` / `any` редактируются как per-field JSON editors с inline summary и helper-actions (`Format JSON`, `Reset`, `Add property/item`); raw JSON fallback остаётся только для модулей без schema.
- FSD-структура остаётся канонической: `app/`, `pages/`, `widgets/`, `features/`, `entities/`, `shared/`.
- Tailwind/shadcn миграция завершена: новые экраны используют семантические CSS-переменные и общие UI-примитивы.

## Generated module UI wiring

- `apps/admin/build.rs` теперь читает `modules.toml` и модульные `rustok-module.toml`, а затем генерирует manifest-driven wiring в `OUT_DIR`.
- Текущий convention-based contract для publishable Leptos admin UI: `[provides.admin_ui].leptos_crate` плюс экспорт корневого компонента `<PascalSlug>Admin`.
- Host регистрирует три generic surface-а без знания о конкретном модуле: `AdminSlot::DashboardSection`, `AdminSlot::NavItem` и `AdminPageRegistration`.
- Для module-owned admin pages используется единый host route `/modules/:module_slug` и его nested-вариант `/modules/:module_slug/*module_path`: `ModuleAdminPage` резолвит модуль по generated registry, прокидывает generic `UiRouteContext` и рендерит root component, если модуль включён у tenant.
- Header shell теперь также использует `rustok-search` как host-level capability: глобальный поиск по админке идёт через GraphQL `adminGlobalSearch`, показывает быстрые результаты прямо в shell и умеет передавать пользователя в полный search control plane.
- `[provides.admin_ui]` может дополнительно задать `route_segment`, `nav_label` и `[[provides.admin_ui.pages]]` для manifest-driven secondary nav; если optional поля не указаны, host берёт `module.slug` и `module.name`.
- Референсные publishable admin packages в workspace сейчас: `rustok-blog-admin`, `rustok-content-admin`, `rustok-forum-admin`, `rustok-workflow-admin` и `rustok-pages-admin`.
- `rustok-pages-admin` остаётся первым честным working exemplar для базового page CRUD.
- `rustok-blog-admin` теперь служит вторым рабочим эталоном для обычного контентного CRUD: пакет сам делает list/create/edit/update/publish/archive/delete через модульный GraphQL, без blog-specific логики в `apps/admin`.
- `rustok-content-admin` теперь служит третьим рабочим эталоном для core module-owned CRUD: пакет сам резолвит `currentTenant`, затем делает node list/create/edit/update/publish/archive/restore/delete через `rustok-content`, не вынося content-specific UI в `apps/admin`.
- `rustok-forum-admin` теперь служит отдельным working exemplar для admin-only forum surface: пакет рендерит categories/topics/replies preview и делает category/topic CRUD через `rustok-forum` REST contract, а host остаётся generic.

## Ограничения

- Nested contract сейчас остаётся intentionally thin: host знает только wildcard route, `UiRouteContext` и manifest-driven secondary nav, а само ветвление по subpath остаётся внутри module package.
- `workflow` уже использует этот contract для `/modules/workflow/templates`, но detail/edit flow пока ещё живёт на legacy-маршрутах `/workflows/*`.
- Для внешних crate-ов вне текущего workspace всё ещё нужен более явный entry-point contract, чем текущие naming conventions.

## Связанные документы

- [План реализации](./implementation-plan.md)
- [Контракты UI API](../../../UI/docs/api-contracts.md)
- [Каталог UI-компонентов Rust](../../../docs/UI/rust-ui-component-catalog.md)
- [Карта документации](../../../docs/index.md)
