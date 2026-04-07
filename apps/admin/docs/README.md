# Документация Leptos Admin

Локальная документация для `apps/admin`.

## Текущий runtime contract

- Инвариант: GraphQL и native Leptos `#[server]` calls поддерживаются параллельно; добавление server functions не является заменой GraphQL path.
- UI/state: `leptos`, `leptos_router`, `Resource`, actions.
- GraphQL transport: `crates/leptos-graphql`.
- Data layer поддерживает две реализации одновременно: direct GraphQL HTTP и Leptos `#[server]` path `/api/fn/admin/graphql`.
- Server-fn path сейчас может делегировать в существующий GraphQL transport, но это не отменяет requirement сохранить прямой GraphQL-клиент в приложении.
- Уже заведён native read-path поверх `#[server]` для части admin surface: `roles`, `cache`, `moduleRegistry`, `installedModules`, `enabled_modules`, `tenantModules`, `marketplace`, `marketplaceModule`, `activeBuild`, `activeRelease`, `buildHistory`, `dashboardStats`, `recentActivity`, `userDetails`, `users`, `eventsStatus`, `eventSettings`, `emailSettings`, `oauthApps`, `workflows`, `workflow`, `workflowExecutions`, `workflowTemplates`, `workflowVersions`.
- Для `roles`, `cache`, `moduleRegistry`, `installedModules`, `enabled_modules`, `tenantModules`, `marketplace`, `marketplaceModule`, `activeBuild`, `activeRelease`, `buildHistory`, `dashboardStats`, `recentActivity`, `userDetails`, `users`, `eventsStatus`, `eventSettings`, `emailSettings`, `oauthApps`, `workflows`, `workflow`, `workflowExecutions`, `workflowTemplates` и `workflowVersions` host сначала пытается native `#[server]` path, а затем откатывается к GraphQL, если native path недоступен.
- Для `workflow`-домена тот же dual-path уже распространяется и на часть write-side: `createWorkflow`, `deleteWorkflow`, `activateWorkflow`, `pauseWorkflow`, `addWorkflowStep`, `deleteWorkflowStep`, `createWorkflowFromTemplate`, `restoreWorkflowVersion`.
- Core-wave для host/admin теперь дополнительно покрывает `auth` и `search`: `login`, `register`, `reset`, `profile.updateProfile`, `security.changePassword`, `adminGlobalSearch`, а также surfaces из `rustok-search-admin`.
- Для `auth` и `search` host и module-owned UI идут по модели native `#[server]` first с GraphQL fallback; для `channel` module-owned admin UI идёт по модели native `#[server]` first с REST fallback, при этом `/api/channels/*` не удаляется.
- Для core-модулей без собственного UI раньше добавлены новые module-owned overview surfaces: `rustok-index-admin`, `rustok-outbox-admin`, `rustok-tenant-admin`, `rustok-rbac-admin`. Они идут по native `#[server]` path и не заменяют существующие host-side flows.
- Optional-wave теперь тоже публикуется через manifest-driven module-owned UI: `rustok-media-admin` и `rustok-comments-admin`.
- Для `rustok-media-admin` действует native `#[server]` first с GraphQL fallback для `library/detail/translations/delete/usage` и с сохранённым REST-first upload path через `/api/media`.
- Для `rustok-comments-admin` зафиксировано исключение: moderation UI работает через native `#[server]` path без GraphQL/REST fallback, потому что legacy transport surface у `comments` не существовал; существующая интеграция `blog -> rustok-comments` при этом не меняется.
- `rustok-content` остаётся shared helper/orchestration boundary без отдельного operator-facing UI, а commerce split crates в этой волне не выносятся из `rustok-commerce-admin`.
- Для `modules` write-side native-first сейчас покрывает `toggleModule`, `updateModuleSettings`, `installModule`, `uninstallModule`, `upgradeModule` и `rollbackBuild`; GraphQL path при этом не удаляется и остаётся fallback-веткой для всех этих операций.
- `marketplace` и `marketplaceModule` в native path собираются из runtime `modules.toml` плюс локальных `rustok-module.toml`/`Cargo.toml`; это additive слой поверх существующего GraphQL resolver, а не его замена. Поверх ownership/trust/visual metadata этот path теперь также публикует machine-readable surface contract `hasAdminUi` / `hasStorefrontUi` / `uiClassification`, причём для no-UI модулей туда теперь тоже попадает explicit `capability_only` / `future_ui`, если это зафиксировано в manifest. Для `marketplaceModule` registry lifecycle теперь отдаёт machine-readable `moderationPolicy` даже без persisted publish/release history, поэтому `/modules` и native SSR больше не должны догадываться о live publish policy только по `ownership`.
- `updateModuleSettings` в native path валидирует JSON по runtime `[settings]` schema из `rustok-module.toml` и только затем пишет в `tenant_modules`; при любой ошибке native path GraphQL mutation остаётся fallback-веткой.
- `installModule` / `uninstallModule` / `upgradeModule` в native path теперь правят runtime `modules.toml`, затем ставят queued build напрямую в `builds` с совместимым `execution_plan`; при ошибке enqueue локальный manifest rollback'ается, а GraphQL mutation остаётся fallback-веткой.
- `rollbackBuild` в native path делает прямой DB-side release switch (`active -> rolled_back`, `previous -> active`) и затем возвращает восстановленный build; GraphQL mutation остаётся fallback-веткой на случай расхождения runtime contract.
- `apps/admin` поддерживает feature-профили `csr`, `hydrate`, `ssr`, однако фактический runtime path всё ещё остаётся CSR-first; прямой SSR/service-layer путь для admin требует отдельного выноса части backend-логики из `apps/server`.
- `/modules` использует `buildProgress` через `/api/graphql/ws`; polling остаётся только fallback-механизмом.
- `/modules` detail panel умеет рендерить schema-driven tenant settings form из `[settings]` в `rustok-module.toml`, включая `select` для scalar-полей с declarative `options`; для complex-полей панель показывает manifest-driven shape hints через `object_keys` и `item_type`, а recursive `shape` metadata (`properties` / `items`) теперь используется и в top-level editors, и в deep nested JSON tree для schema-driven add actions, schema-locked object keys и nested scalar controls, которые уважают child `type`, `options`, `min` и `max`. Nested `object` / `array` children теперь редактируются inline прямо внутри structured editors, а отдельная lower nested panel остаётся в основном для `json` / `any`. Отдельно панель показывает operator-facing metadata readiness по `description` / visuals / publisher / release trail для registry flow, включая latest non-yanked published version/date вместо слепого “есть какие-то versions”, machine-readable surface metadata (`UI class`, `Admin UI`, `Storefront UI`) из catalog contract, а также V2 publish lifecycle state: canonical `moderationPolicy` даже для модулей без persisted registry history, latest request status/actor/requested publisher/timestamps, latest persisted release state, persisted owner binding с `bound_by`/`updated_at`, surfaced validation/rejection details, policy-aware moderation summary, actionable next-step hints, copyable operator commands (`publish`, `owner-transfer`, `yank`, `stage-run`) с inline `Copy` CTA, state-aware live API actions (`status`, `validate`, `approve`, `reject`, `owner-transfer`, `yank`) с подсказкой по allowed actor/current authority, явной маркировкой `read-only` vs `write-path`, минимальными request-body hints для mutating actions, header hints для `x-rustok-actor` / `x-rustok-publisher`, `xtask` live-mode snippets и Windows-friendly `curl.exe` snippets, а также recent governance audit trail по publish/upload/validate/approve/reject/owner-transfer/yank/owner-binding событиям.
- `/modules` catalog filters теперь покрывают `search`, `category`, `tag`, `source`, `trustLevel`, compatibility и install-state; `marketplace` read-path поддерживает `tag` и в native `#[server]` ветке, и в GraphQL fallback, так что operator UI реально использует тот же provider-side narrowing, что и registry V1 catalog contract.
- FSD-структура остаётся канонической: `app/`, `pages/`, `widgets/`, `features/`, `entities/`, `shared/`.
- Tailwind/shadcn миграция завершена: новые экраны используют семантические CSS-переменные и общие UI-примитивы.

## Generated module UI wiring

- `apps/admin/build.rs` читает `modules.toml` и модульные `rustok-module.toml`, затем генерирует manifest-driven wiring в `OUT_DIR`.
- Этот же build-time/native manifest layer теперь публикует runtime metadata не только для ownership/trust, но и для surface audit: `ownership`, `trust_level`, `recommended_admin_surfaces`, `showcase_admin_surfaces`, `hasAdminUi`, `hasStorefrontUi`, `uiClassification`, чтобы Leptos `#[server]` не зависел от GraphQL resolver-слоя для этих полей. Для no-UI path modules `uiClassification` теперь может быть не только derived `no_ui`, но и explicit `capability_only` / `future_ui`.
- `modules` read-side теперь split по источникам: `moduleRegistry` использует generated runtime metadata + `ModuleRegistry`, а `installedModules` читает live `modules.toml` через минимальный SSR manifest loader; GraphQL при этом остаётся fallback-веткой и не удаляется.
- Текущий contract для publishable Leptos admin UI: `[provides.admin_ui].leptos_crate` плюс экспорт корневого компонента `<PascalSlug>Admin`.
- Наличие `crates/<module>/admin/Cargo.toml` само по себе больше не считается интеграцией: build-time codegen теперь падает, если sub-crate существует без `[provides.admin_ui].leptos_crate`, или если manifest объявляет admin UI без реального `admin/Cargo.toml`.
- Host регистрирует generic surfaces без знания о конкретном модуле: `AdminSlot::DashboardSection`, `AdminSlot::NavItem`, `AdminPageRegistration`.
- Для module-owned admin pages используется host route `/modules/:module_slug` и nested-вариант `/modules/:module_slug/*module_path`.
- Header shell использует `rustok-search` как host-level capability: глобальный поиск теперь сначала идёт через native `#[server]` path `admin/global-search`, а затем откатывается к GraphQL `adminGlobalSearch`, если native path недоступен.
- `[provides.admin_ui]` может задавать `route_segment`, `nav_label` и `[[provides.admin_ui.pages]]` для manifest-driven secondary nav.
- Host admin теперь прокидывает effective locale в `UiRouteContext`, поэтому module-owned Leptos admin packages должны брать язык UI из `UiRouteContext.locale`, а не из локальных fallback-цепочек.
- Любая альтернативная locale-цепочка внутри admin package запрещена: пакет не должен заново читать query/header/cookie и не должен выбирать свой собственный fallback order поверх host contract.
- Package-owned bundle contract для Leptos admin surfaces фиксируется через `[provides.admin_ui.i18n]` и `admin/locales/*.json`; core surfaces `workflow`, `rbac`, `tenant`, `index`, `outbox`, `channel`, а также module-owned content/moderation surfaces `pages`, `comments`, `forum`, `search` и `commerce` уже живут на этом contract.
- Capability-owned surface `rustok-ai-admin` использует тот же host locale contract и те же package-owned `admin/locales/*.json`, но остаётся вне manifest-driven module registry, потому что это capability crate, а не `rustok-module.toml` package.
- `build.rs` также публикует список `Core`-модулей с UI, поэтому такие surfaces монтируются в host всегда и не зависят от tenant module toggles.

## Правило ownership UI

- Если модуль поставляет UI для админки, этот UI живёт рядом с модулем и остаётся module-owned независимо от `Core`/`Optional`.
- `apps/admin` выступает host/composition root и не переносит модульный business UI в свой код.
- Core-модули с UI подчиняются тому же правилу, что и optional-модули: наличие UI не делает host владельцем модульной поверхности.

## Рабочие exemplar-ы

- `rustok-pages-admin` — базовый page CRUD.
- `rustok-blog-admin` — content CRUD без blog-specific логики в host.
- `rustok-commerce-admin` — commerce catalog CRUD плюс typed shipping-profile registry и shipping-option compatibility/lifecycle management без переноса commerce-specific UI в host; product и shipping-option editors используют registry-backed selectors вместо ручного ввода slug'ов.
- `rustok-search-admin` — nested control-plane exemplar с manifest-driven secondary nav (`playground`, `engines`, `dictionaries`, `analytics`) и native-first `#[server]` data-layer для bootstrap, preview, diagnostics, dictionaries, analytics и rebuild/settings write-path.
- `rustok-forum-admin` — admin-only forum surface с category/topic CRUD через модульный REST contract.
- `rustok-channel-admin` — core-module admin slice с nested pages (`targets`, `apps`) через тот же manifest-driven contract; data path теперь native-first через `#[server]`, но legacy REST fallback сохраняется параллельно.
- `rustok-index-admin` / `rustok-outbox-admin` / `rustok-tenant-admin` / `rustok-rbac-admin` — core-module overview slices, которые закрывают отсутствовавшие module-owned Leptos surfaces без дублирования существующих host CRUD flows.
- `rustok-media-admin` — optional module library slice с native-first `#[server]` data-layer, GraphQL fallback для read/delete/translation/usage и отдельным REST upload path.
- `rustok-comments-admin` — optional module moderation slice с thread list/detail и статусными действиями через native `#[server]` поверх `CommentsService`, без отдельного legacy fallback слоя.

## Ограничения

- Nested contract пока intentionally thin: host знает только wildcard route, `UiRouteContext` и manifest-driven secondary nav; само ветвление по subpath остаётся внутри module package.
- `workflow` уже использует этот contract для `/modules/workflow/templates`, но часть detail/edit flow пока живёт на legacy-маршрутах `/workflows/*`.
- Для внешних crates вне текущего workspace всё ещё нужен более явный entry-point contract, чем текущие naming conventions.

## Связанные документы

- [План реализации](./implementation-plan.md)
- [Контракты UI API](../../../UI/docs/api-contracts.md)
- [Каталог UI-компонентов Rust](../../../docs/UI/rust-ui-component-catalog.md)
- [Карта документации](../../../docs/index.md)
## Актуализация `2026-04-04`

- `/modules` теперь умеет не только показывать V2 publish/governance lifecycle, но и запускать интерактивные операторские действия поверх уже существующих registry endpoints.
- В detail panel добавлены локальные формы и кнопки для `validate`, `approve`, `reject`, `owner-transfer` и `yank`.
- UI поддерживает `dry-run` toggle, ввод `actor` / `publisher` / `reason` / `new owner actor`, принудительное обновление detail state и отображение `warnings` / `errors` из `RegistryMutationResponse`.
- Для live destructive actions (`reject`, `owner-transfer`, `yank`) detail panel теперь требует отдельное подтверждение, тогда как `dry-run` по этим действиям остаётся быстрым preview path без дополнительного confirm-step.
- В `Publish lifecycle` теперь есть отдельная `Validation summary`: counts по `warnings/errors`, последний validation event из audit trail и раздельный вывод warnings/errors/rejection reason, чтобы оператор видел не только общий статус, но и фактический результат последней проверки.
- Та же `Validation summary` теперь различает автоматический `validation_failed` и ручной `governance reject`, а для статуса `approved` отдельно показывает `Ready for review`, чтобы успешная валидация не смешивалась с финальной публикацией.
- `Publish lifecycle` теперь также показывает отдельный `Follow-up gates` блок по `compile_smoke`, `targeted_tests` и `security_policy_review`, чтобы оператор видел внешние async/manual gates отдельно от базовой artifact/manifest validation; для всех трёх stages панель теперь подсказывает `cargo xtask module stage-run ...`, но уже не через локальные stage-key эвристики, а через canonical lifecycle metadata `executionMode` / `runnable` / `requiresManualConfirmation` из server contract. Для `security_policy_review` этот path по-прежнему требует explicit `--confirm-manual-review` и остаётся operator-assisted review, а не полностью автоматическим security scanner. `validationStages` теперь также несут canonical `reason_code` contract: `allowedTerminalReasonCodes`, `suggestedPassReasonCode`, `suggestedFailureReasonCode` и `suggestedBlockedReasonCode`. `/modules` показывает этот policy layer прямо в stage cards и hints, а live terminal stage updates (`passed` / `failed` / `blocked`) валидируются уже по stage-aware subset, а не только по общему global allowlist.
