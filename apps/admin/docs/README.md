# Документация Leptos Admin

Локальная документация для `apps/admin`.

## Текущий runtime contract

- Инвариант: GraphQL и native Leptos `#[server]` calls поддерживаются параллельно; добавление server functions не является заменой GraphQL path.
- UI/state: `leptos`, `leptos_router`, `Resource`, actions.
- GraphQL transport: `crates/leptos-graphql`.
- Data layer поддерживает две реализации одновременно: direct GraphQL HTTP и Leptos `#[server]` path `/api/fn/admin/graphql`.
- Server-fn path сейчас может делегировать в существующий GraphQL transport, но это не отменяет requirement сохранить прямой GraphQL-клиент в приложении.
- Уже заведён native read-path поверх `#[server]` для части admin surface: `roles`, `cache`, `moduleRegistry`, `installedModules`, `enabled_modules`, `tenantModules`, `activeBuild`, `activeRelease`, `buildHistory`, `dashboardStats`, `recentActivity`, `userDetails`, `users`, `eventsStatus`, `eventSettings`, `emailSettings`, `oauthApps`, `workflows`, `workflow`, `workflowExecutions`, `workflowTemplates`, `workflowVersions`.
- Для `roles`, `cache`, `moduleRegistry`, `installedModules`, `enabled_modules`, `tenantModules`, `activeBuild`, `activeRelease`, `buildHistory`, `dashboardStats`, `recentActivity`, `userDetails`, `users`, `eventsStatus`, `eventSettings`, `emailSettings`, `oauthApps`, `workflows`, `workflow`, `workflowExecutions`, `workflowTemplates` и `workflowVersions` host сначала пытается native `#[server]` path, а затем откатывается к GraphQL, если native path недоступен.
- Для `workflow`-домена тот же dual-path уже распространяется и на часть write-side: `createWorkflow`, `deleteWorkflow`, `activateWorkflow`, `pauseWorkflow`, `addWorkflowStep`, `deleteWorkflowStep`, `createWorkflowFromTemplate`, `restoreWorkflowVersion`.
- `apps/admin` поддерживает feature-профили `csr`, `hydrate`, `ssr`, однако фактический runtime path всё ещё остаётся CSR-first; прямой SSR/service-layer путь для admin требует отдельного выноса части backend-логики из `apps/server`.
- `/modules` использует `buildProgress` через `/api/graphql/ws`; polling остаётся только fallback-механизмом.
- `/modules` detail panel умеет рендерить schema-driven tenant settings form из `[settings]` в `rustok-module.toml`, включая `select` для scalar-полей с declarative `options`, и показывает operator-facing metadata readiness по `description` / visuals / publisher / publish signals для registry flow.
- FSD-структура остаётся канонической: `app/`, `pages/`, `widgets/`, `features/`, `entities/`, `shared/`.
- Tailwind/shadcn миграция завершена: новые экраны используют семантические CSS-переменные и общие UI-примитивы.

## Generated module UI wiring

- `apps/admin/build.rs` читает `modules.toml` и модульные `rustok-module.toml`, затем генерирует manifest-driven wiring в `OUT_DIR`.
- Этот же build-time codegen теперь публикует runtime metadata (`ownership`, `trust_level`, `recommended_admin_surfaces`, `showcase_admin_surfaces`) для native `moduleRegistry` read-path, чтобы Leptos `#[server]` не зависел от GraphQL resolver-слоя для этих полей.
- `modules` read-side теперь split по источникам: `moduleRegistry` использует generated runtime metadata + `ModuleRegistry`, а `installedModules` читает live `modules.toml` через минимальный SSR manifest loader; GraphQL при этом остаётся fallback-веткой и не удаляется.
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
- `rustok-commerce-admin` — commerce catalog CRUD без переноса catalog-specific UI в host.
- `rustok-search-admin` — nested control-plane exemplar с manifest-driven secondary nav (`playground`, `engines`, `dictionaries`, `analytics`) без ручного router wiring в host.
- `rustok-forum-admin` — admin-only forum surface с category/topic CRUD через модульный REST contract.
- `rustok-channel-admin` — core-module admin slice с nested pages (`targets`, `apps`) через тот же manifest-driven contract.

## Ограничения

- Nested contract пока intentionally thin: host знает только wildcard route, `UiRouteContext` и manifest-driven secondary nav; само ветвление по subpath остаётся внутри module package.
- `workflow` уже использует этот contract для `/modules/workflow/templates`, но часть detail/edit flow пока живёт на legacy-маршрутах `/workflows/*`.
- Для внешних crates вне текущего workspace всё ещё нужен более явный entry-point contract, чем текущие naming conventions.

## Связанные документы

- [План реализации](./implementation-plan.md)
- [Контракты UI API](../../../UI/docs/api-contracts.md)
- [Каталог UI-компонентов Rust](../../../docs/UI/rust-ui-component-catalog.md)
- [Карта документации](../../../docs/index.md)
