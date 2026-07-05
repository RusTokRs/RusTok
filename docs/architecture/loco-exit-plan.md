---
id: doc://docs/architecture/loco-exit-plan.md
kind: implementation_plan
language: markdown
status: draft
---
# План ухода от Loco RS

Этот документ фиксирует единый repo-level план перехода `apps/server` с Loco RS на чистый Axum runtime и собственные CLI/maintenance entrypoints.

План заменяет прежнюю линию "интегрировать Loco глубже". Старые документы `apps/server/docs/loco-core-integration-plan.md` и `apps/server/docs/LOCO_FEATURE_SUPPORT.md` остаются историческим контекстом и inventory, но не являются целевым roadmap.

## Execution checkpoint

- Current phase: `phase_1_runtime_context_and_request_extractors`
- Last checkpoint: введён `rustok_api::HostRuntimeContext`, `apps/server` прокидывает его в Leptos `#[server]` functions; `rustok-index-admin` и `rustok-outbox-admin` больше не импортируют `loco_rs::app::AppContext`; `rustok-content-orchestration` больше не импортирует Loco и получает DB/event bus/GraphQL data handles явно; Alloy runtime core строится из явного `DatabaseConnection`, не импортирует Loco, регистрируется через `ServerRuntimeContext`, Alloy GraphQL читает `SharedAlloyRuntime` из schema data, а Alloy HTTP handlers используют `AlloyHttpRuntime`; `rustok-ai` больше не зависит от Loco и использует собственный `AiHostRuntime` для GraphQL/service/direct/MCP execution; `rustok-commerce` storefront checkout orchestration использует `StorefrontCheckoutRuntime` с явными DB/event bus handles, а shared/admin product, storefront product/catalog/order/cart/checkout, admin order/change/return, admin fulfillment, admin shipping и admin payment HTTP handlers используют `CommerceHttpRuntime`; остальные commerce transport adapters ещё Loco-boundary до следующих срезов; `rustok-blog` HTTP post/comment handlers используют узкий `BlogHttpRuntime`, `rustok-pages` HTTP handlers используют `PagesHttpRuntime`, `rustok-forum` REST handlers используют `ForumHttpRuntime`, `rustok-media` HTTP handlers используют `MediaHttpRuntime`, `rustok-workflow` HTTP/webhook handlers используют `WorkflowHttpRuntime`, а `rustok-seo` HTTP handlers используют `SeoHttpRuntime`; Loco `AppContext` в этих модулях остался только в controller state adapters до полного Axum route cutover; `scripts/verify/verify-loco-inventory.mjs` классифицирует оставшиеся Loco entrypoints; ADR `2026-07-02-axum-runtime-and-ops-cli-boundary` принят; `SettingsService`, event bus, event transport, module dispatcher/runtime extensions, email, app runtime helpers, rate-limit bootstrap, GraphQL schema service, worker lifecycle orchestration, auth lifecycle providers, build/release/runtime guardrail services переведены на `ServerRuntimeContext`; middleware, auth extractors, module guards и channel contracts используют собственные runtime contracts; весь `apps/server/src/graphql/**` не импортирует Loco, а query/mutation/subscription paths получают `ServerRuntimeContext`, `DatabaseConnection` и schema-owned handles через GraphQL data; OAuth REST endpoints и marketplace registry/governance REST surface используют `ServerAuthRuntime`/`ServerRuntimeContext` вместо Loco `AppContext`; server больше не реэкспортирует `rustok_outbox::loco` для transactional event publishing.
- Next step: выбрать следующий production slice для замены `loco_rs::app::AppContext` на `ServerRuntimeContext` или узкие typed contexts.
- Open blockers: нет для Phase 1 planning; перед Phase 4 потребуется targeted integration smoke для чистого Axum startup.
- Hand-off notes for next agent: не добавлять compatibility wrappers и dual execution paths; каждый cutover должен переводить все внутренние callers на целевой контракт и удалять заменённый Loco path в том же change set.
- Last updated at (UTC): 2026-07-02T00:00:00Z

## Цель

Целевое состояние:

1. `apps/server` запускается как чистое Axum-приложение без `loco_rs::cli`, `Hooks`, `AppContext`, `Routes`, Loco tasks и Loco initializers.
2. Runtime context принадлежит RusToK: typed DB handle, settings, shared runtime registry, event/outbox/cache/storage/email handles, shutdown handles и observability hooks доступны через собственные host contracts.
3. Operator CLI принадлежит RusToK, но живёт отдельно от server runtime: отдельный ops crate/binary вызывает typed Rust APIs для migrate, seed, install и maintenance flows; `apps/server` не зависит от этого CLI layer.
4. Модули и UI packages не импортируют `loco_rs`; они получают host data через `rustok-api`, module ports, GraphQL, REST или native `#[server]` context.
5. Workspace dependency `loco-rs` удалена из `Cargo.toml` и `Cargo.lock`.

## Не входит в план

- Замена Axum, SeaORM, async-graphql или Leptos.
- Удаление GraphQL при добавлении native `#[server]` paths.
- Перенос domain logic в `apps/server`.
- Введение второго временного runtime рядом с Loco на неопределённый срок.
- Сохранение Loco-compatible aliases ради внутренних callers.

## Что уже перенесено

| Область | Текущее состояние | Целевой owner |
|---|---|---|
| Auth/JWT/password/session domain | Реализовано в `rustok-auth` и server services; Loco JWT не является source of truth | `rustok-auth` + server auth adapter |
| RBAC | Runtime policy вынесен в `rustok-rbac`/shared contracts | `rustok-rbac`, `rustok-api`, `apps/server` enforcement |
| Cache | `rustok-cache` и tenant cache infra используются вместо Loco cache | `rustok-cache`, tenant middleware |
| Storage | `rustok-storage` + `rustok-media`; server bootstrap инициализирует `StorageService` | `rustok-storage`, `rustok-media`, server wiring |
| Email | `rustok-email` и server email service покрывают SMTP/templates; Loco provider ещё остаётся option | `rustok-email` + server adapter |
| Queue/event delivery | Transactional outbox и relay являются source of truth; Loco Queue не используется | `rustok-outbox`, `rustok-events`, server workers |
| WebSocket channels | Используется custom Axum WS path, не Loco channels | `apps/server` + channel/auth modules |
| Module-owned API composition | GraphQL/REST всё больше собираются через manifests и owner-owned roots | module crates + generated server composition |
| Leptos server-function context | Начат переход на `rustok_api::HostRuntimeContext`; `index/outbox` admin уже переведены | `rustok-api` + server context provider |
| Installer CLI | `rustok-server install ...` уже существует как собственный CLI slice; целевое состояние - перенос в отдельный ops binary | `rustok-installer` + ops CLI adapter |

## Что ещё держит Loco

| Остаток | Примеры текущих точек | Что заменить |
|---|---|---|
| Application bootstrap | `apps/server/src/main.rs`, `apps/server/src/app.rs`, `impl Hooks for App` | `serve()` на Axum `Router`, собственный lifecycle bootstrap |
| Loco `AppContext` | controllers, GraphQL data, middleware, tasks, services, tests, module UI adapters | `ServerRuntimeContext` / `HostRuntimeContext` / typed request extractors |
| Loco route wrappers | `loco_rs::controller::Routes`, `format`, `ErrorDetail`, `loco_rs::Result` | Axum `Router`, typed response/error mappers |
| Loco config | `loco_rs::config::Config`, `config/*.yaml` conventions | `RustokSettings` loader + explicit env/file contract |
| Loco tasks | `cargo loco task --name ...`, `loco_rs::task::{Task, Vars}` | отдельный `rustok-ops task <name>` / typed subcommands |
| Loco initializers | `loco_rs::app::Initializer` | explicit bootstrap phases with ordered init functions |
| Loco test helpers | `loco_rs::tests_cfg::app::get_app_context` | `rustok-test-utils` server context fixtures |
| Loco mailer option | `EmailProvider::Loco`, `ctx.mailer` | remove provider or replace with native SMTP/provider adapter |
| Outbox Loco adapter | `rustok-outbox/loco-adapter`, `rustok_outbox::loco` | host-neutral event bus factory |
| Docs/verification | Loco integration plans and Loco-specific guards | Axum/ops CLI cutover guards and archived Loco docs |

## Целевой runtime contract

### Server bootstrap

Целевой entrypoint:

1. load settings;
2. connect DB;
3. optionally validate required schema/runtime preconditions;
4. build `ServerRuntimeContext`;
5. register module runtime extensions;
6. build event/outbox/cache/storage/email/telemetry runtimes;
7. compose Axum router;
8. start HTTP server and shutdown handling.

`apps/server/src/app.rs` должен перестать быть Loco hooks layer и стать обычным Axum composition module или быть разрезан на `bootstrap`, `router`, `lifecycle`.

### Runtime context

Минимальный собственный context:

- `db: DatabaseConnection`
- `settings: Arc<RustokSettings>`
- `registry: ModuleRegistry`
- `runtime_extensions: Arc<ModuleRuntimeExtensions>`
- `shared`: typed runtime store only if ownership is explicit
- event bus / event transport
- cache/storage/email services
- shutdown handles
- build/release/installer runtimes

Правило: module-owned UI и transport adapters не получают этот full context напрямую. Для них используются узкие contracts: `HostRuntimeContext`, `RequestContext`, `PortContext`, GraphQL data или module-owned facades.

### Operator CLI

CLI остаётся как внешний operator/dev interface для миграций, seed, install и maintenance flows. Он не является внутренним integration layer: бизнес-логика, module contracts и runtime wiring должны жить в typed Rust APIs, а команды только вызывают эти APIs и задают режим выполнения.

Целевой owner: отдельный crate/binary, например `crates/rustok-ops-cli` с бинарём `rustok-ops`. `apps/server` не должен зависеть от этого crate, а production HTTP binary не должен тащить maintenance command code в свою сборку.

Доменное ядро модуля не зависит от `clap`/stdout/exit-code contracts. Если модулю нужен maintenance flow, модуль может владеть отдельным `cli/` adapter package рядом с доменным кодом; этот adapter вызывает публичные typed APIs модуля и не участвует в server runtime сборке. Для внешних или переписанных модулей такой adapter может поставляться вместе с модулем или жить в integration layer, но registry подключает его одинаково.

#### Масштабируемая модель ops commands

`rustok-ops` не должен превращаться в каталог hardcoded команд для всех модулей. Целевая структура:

- `rustok-ops-api`: маленький stable contract для описания ops capabilities, аргументов, прав, dry-run режима, tenant scope и machine-readable результата.
- `rustok-ops-cli`: runner, parser, help/list/search UX, загрузка settings и построение ops runtime context.
- `rustok-ops-registry`: явный registry подключённых command providers для конкретной сборки/дистрибутива.
- `crates/rustok-<module>/cli`: module-local ops adapter package, который мапит ops command provider на typed API модуля.
- `integrations/<external-module>/cli`: adapter package для внешних модулей, если поставщик не привозит свой module-local CLI adapter.

Доменный crate не зависит от `rustok-ops-api`; зависимость направлена с `cli/` adapter package на доменный crate. Физически adapter лежит рядом с модулем, чтобы команды, scripts и maintenance-требуха не копились в центральном ops crate, но архитектурно это inbound adapter, а не domain core.

Имена команд namespace-based, без плоской глобальной свалки:

| Формат | Пример | Назначение |
|---|---|---|
| `rustok-ops <namespace> <command>` | `rustok-ops index rebuild` | module-owned maintenance |
| `rustok-ops core <command>` | `rustok-ops core migrate` | platform/core operations |
| `rustok-ops list --namespace index` | - | discoverability без огромного root help |

Подключение provider должно быть явным: через module manifest, feature/distribution manifest или generated registry, а не через runtime magic. Это сохраняет воспроизводимость сборки, позволяет иметь сколько угодно модулей без ручного central mapping и позволяет выпускать production server без ops layer, а ops binary - только с нужными command providers.

#### Distribution-aware builds

`rustok-ops` в целевом состоянии может стать не только maintenance runner, но и build/pack/install toolchain для платформы из выбранного набора модулей. Это нужно для сборок с самописными модулями других участников без жёстких правок центрального server crate.

Принцип:

- distribution manifest описывает набор core, internal и external modules, их features, migrations, seeds, runtime entrypoints и ops providers;
- generated registry создаётся из manifest/lockfile и фиксирует, какие runtime modules и CLI providers входят в конкретную сборку;
- server build получает только runtime parts выбранного дистрибутива;
- ops build получает только ops providers выбранного дистрибутива;
- external modules подключаются через опубликованный module manifest и optional `cli/` adapter package, а не через ручной код в центральном репозитории.

Возможные команды будущего toolchain:

| Команда | Назначение |
|---|---|
| `rustok-ops distro check` | проверить manifests, features, migrations, command namespaces |
| `rustok-ops distro generate` | сгенерировать runtime/ops registries для выбранной сборки |
| `rustok-ops distro build server` | собрать server binary без ops layer |
| `rustok-ops distro build ops` | собрать ops binary с выбранными providers |
| `rustok-ops distro pack` | подготовить installable artifact |

Целевые команды:

| Команда | Назначение | Заменяет |
|---|---|---|
| `rustok-server` / server binary | HTTP runtime only | `cargo loco start` / `loco_rs::cli` |
| `rustok-ops migrate up/down/status` | DB migrations | Loco migration wrapper |
| `rustok-ops seed <profile>` | seed profiles | Loco `seed` hook |
| `rustok-ops task cleanup ...` | cleanup maintenance | `cargo loco task --name cleanup` |
| `rustok-ops task rebuild ...` | rebuild/index maintenance | `cargo loco task --name rebuild` |
| `rustok-ops task db-baseline ...` | DB baseline report | `cargo loco task --name db_baseline` |
| `rustok-ops task media-cleanup ...` | storage/media cleanup | `cargo loco task --name media_cleanup` |
| `rustok-ops oauth create-app ...` | OAuth app bootstrap | `cargo loco task --name create_oauth_app` |
| `rustok-ops install ...` | install/preflight/apply | existing Rustok install CLI path |

## Фазы миграции

### Phase 0. Inventory и запрет нового Loco surface

- [x] Ввести `rustok_api::HostRuntimeContext`.
- [x] Перевести первые module UI adapters (`rustok-index-admin`, `rustok-outbox-admin`) с Loco context.
- [x] Добавить guardrail в `scripts/verify/verify-api-surface-contract.mjs` для этих adapters.
- [x] Добавить общий inventory script: все `loco_rs`, `loco-rs`, `cargo loco`, `rustok_outbox::loco` с категоризацией текущих host/runtime/task/test/module/docs/dependency точек.
- [x] Запретить новые `loco_rs` imports вне allowlist.
- [x] Зафиксировать ADR целевого Axum runtime и отдельного ops CLI layer.

Exit gate: inventory script проходит, allowlist зафиксирован, новые Loco imports без категории падают в CI.

### Phase 1. Собственный runtime context и request extractors

- [x] Ввести `ServerRuntimeContext` в `apps/server` или shared server-support crate.
- [x] Перевести middleware на собственный context вместо `loco_rs::app::AppContext`.
- [x] Перевести GraphQL implementation и controller handlers на собственный context и узкие runtime handles: `apps/server/src/graphql/**` и `controllers/graphql.rs` больше не импортируют Loco `AppContext`; Loco `Routes` остаётся routing adapter до Phase 2.
- [x] Перевести host-owned GraphQL query/mutation roots и settings/system/user fields/build subscription на neutral GraphQL data: build/marketplace paths используют `ServerRuntimeContext`, DB-only paths — schema-owned `DatabaseConnection`, settings mutation — schema-owned `TransactionalEventBus`; HTTP/WS controller больше не добавляет Loco host data в GraphQL requests.
- [x] Перевести первые shared-store services (`BuildEventHub`, `FieldDefinitionCache`, `MarketplaceCatalog`) на `ServerRuntimeContext`; GraphQL/channel/build boundaries пока строят context из текущего Loco host data.
- [x] Перевести `EventBus` и server-side `TransactionalEventBus` factory на `ServerRuntimeContext`; crate-local `rustok_outbox::loco` adapter и dependency feature остаются отдельным Phase 5 removal item.
- [x] Перевести runtime guardrail snapshot service, health readiness и metrics controllers на `ServerRuntimeContext` + узкий email runtime state.
- [x] Перевести RBAC consistency stats service и metrics caller на `ServerRuntimeContext`; legacy cleanup task пока остаётся task boundary adapter.
- [x] Перевести release deployment backend на `ServerRuntimeContext`; build worker lifecycle пока остаётся boundary adapter.
- [x] Перевести build executor service на `ServerRuntimeContext`; build worker и legacy rebuild task пока остаются boundary adapters.
- [x] Перевести event transport factory на `ServerRuntimeContext`; app runtime bootstrap пока остаётся boundary adapter.
- [x] Перевести spawn path module event dispatcher на `ServerRuntimeContext`; host-provider wiring пока остаётся boundary adapter из-за auth lifecycle provider.
- [x] Перевести email service factory/password reset URL на `ServerRuntimeContext`; Loco mailer остаётся явным boundary handle.
- [x] Перевести app runtime bootstrap helpers (`module_runtime_extensions_from_ctx`, storage, marketplace catalog, workflow cron shared setup) на `ServerRuntimeContext`; Loco-specific bootstrap остается boundary adapter.
- [x] Перевести rate-limit bootstrap и shared limiter registration на `ServerRuntimeContext`; Loco `AppContext` в `bootstrap_app_runtime` остаётся только для текущих Loco-boundary adapters.
- [x] Перевести GraphQL schema assembly, shared/cache helpers и media storage fallback на `ServerRuntimeContext`; Loco host data остаётся только выше, в текущем app bootstrap/controller boundary.
- [x] Убрать Loco `AppContext` из `rustok-content-orchestration`: host строит `SharedContentOrchestrationService` из явных DB/event bus handles, GraphQL получает его через schema data.
- [x] Перевести Alloy runtime bootstrap, GraphQL resolvers и HTTP handlers на явные runtime handles: host строит `SharedAlloyRuntime` через `alloy::build_alloy_runtime(DatabaseConnection)`, GraphQL читает этот handle из schema-owned data, REST handlers принимают `AlloyHttpRuntime`, а Loco остаётся только в текущем route-state adapter до Phase 2/5.
- [x] Убрать Loco `AppContext` из `rustok-ai`: GraphQL mutation, `AiManagementService`, direct execution handlers и in-process MCP adapter используют `AiHostRuntime` с явными DB/event bus/storage/Alloy/module-registry handles.
- [x] Убрать Loco `AppContext` из `rustok-commerce` storefront checkout runtime: owner storefront SSR adapters собирают `StorefrontCheckoutRuntime` из DB/event bus на своей boundary, а checkout orchestration API принимает только этот host-neutral contract.
- [x] Сузить первый `rustok-commerce` product HTTP slice: `controllers/products.rs` и `controllers/admin/products.rs` принимают `CommerceHttpRuntime` и не используют `rustok_outbox::loco`; остальные commerce admin/storefront adapters остаются следующими Loco-boundary срезами.
- [x] Сузить `rustok-commerce` storefront product/catalog HTTP slice: `controllers/store/products.rs` принимает `CommerceHttpRuntime` для product list/show, regions и shipping-options reads; файл больше не принимает Loco `AppContext` и не использует `rustok_outbox::loco`.
- [x] Сузить `rustok-commerce` storefront order HTTP slice: `controllers/store/orders.rs` принимает `CommerceHttpRuntime` для customer/order/return/refund/change reads и return creation; файл больше не принимает Loco `AppContext` и не использует `rustok_outbox::loco`.
- [x] Сузить `rustok-commerce` storefront cart HTTP slice: `controllers/store/carts.rs` принимает `CommerceHttpRuntime` для create/get/context/line-item routes и больше не принимает Loco `AppContext` или `rustok_outbox::loco`.
- [x] Сузить `rustok-commerce` storefront checkout HTTP slice: `controllers/store/checkout.rs` принимает `CommerceHttpRuntime` для payment-collection и complete checkout routes; `controllers/store/mod.rs` больше не держит `rustok_outbox::loco` helper wrappers.
- [x] Сузить `rustok-commerce` admin fulfillment HTTP slice: `controllers/admin/fulfillments.rs` принимает `CommerceHttpRuntime` для list/create/show/ship/deliver/reopen/reship/cancel и больше не принимает Loco `AppContext`.
- [x] Сузить `rustok-commerce` admin shipping HTTP slice: `controllers/admin/shipping.rs` принимает `CommerceHttpRuntime` для shipping profiles/options list/create/show/update/deactivate/reactivate и больше не принимает Loco `AppContext`.
- [x] Сузить `rustok-commerce` admin payment HTTP slice: `controllers/admin/payments.rs` принимает `CommerceHttpRuntime` для payment collections/refunds list/show/lifecycle routes и больше не принимает Loco `AppContext`.
- [x] Сузить `rustok-commerce` admin order HTTP slice: `controllers/admin/orders.rs` принимает `CommerceHttpRuntime` для order list/detail/lifecycle routes и больше не использует `rustok_outbox::loco`.
- [x] Сузить `rustok-commerce` admin order-change HTTP slice: `controllers/admin/changes.rs` принимает `CommerceHttpRuntime` для create/list/show/apply/cancel и больше не использует `rustok_outbox::loco`.
- [x] Сузить `rustok-commerce` admin return HTTP slice: `controllers/admin/returns.rs` принимает `CommerceHttpRuntime` для create/list/show/complete/cancel/decision routes и больше не использует `rustok_outbox::loco`.
- [x] Сузить `rustok-blog` HTTP controller state: post/comment handlers принимают `BlogHttpRuntime` и строят services из явных DB/event bus handles; текущий Loco `AppContext` остаётся только в `FromRef` state adapter до общего Axum route cutover.
- [x] Сузить `rustok-pages` HTTP controller state: page/block handlers принимают `PagesHttpRuntime` и не используют `rustok_outbox::loco`; текущий Loco `AppContext` остаётся только в `FromRef` state adapter до общего Axum route cutover.
- [x] Сузить `rustok-forum` REST controller state: category/topic/reply/user/widget handlers принимают `ForumHttpRuntime`, topic/reply services получают event bus без `rustok_outbox::loco`, а текущий Loco `AppContext` остаётся только в `FromRef` state adapter до общего Axum route cutover.
- [x] Сузить `rustok-media` HTTP controller state: upload/list/get/delete/translation handlers принимают `MediaHttpRuntime` с явными DB/storage handles; текущий Loco `AppContext` остаётся только в `FromRef` state adapter до общего Axum route cutover.
- [x] Сузить `rustok-workflow` HTTP controller state: workflow/step/execution/webhook handlers принимают `WorkflowHttpRuntime` с явным DB handle; текущий Loco `AppContext` остаётся только в `FromRef` state adapter до общего Axum route cutover.
- [x] Сузить `rustok-seo` HTTP controller state: REST handlers принимают `SeoHttpRuntime` с явными DB/event bus/runtime extensions handles и не используют `rustok_outbox::loco`; текущий Loco `AppContext` остаётся только в `FromRef` state adapter до общего Axum route cutover.
- [x] Перевести runtime worker lifecycle orchestration на `ServerRuntimeContext` для settings/shared-store/event-runtime lookup; worker loops пока остаются boundary adapters там, где им нужен `AppContext`.
- [x] Перевести DB-only paths auth lifecycle provider (`list_sessions`, `update_profile`, `change_password`, `logout`, session revoke, accept invite user create) на `ServerRuntimeContext`.
- [x] Перевести config-aware auth lifecycle provider paths (`login`, `register`, `refresh`, `reset_password`) на `ServerRuntimeContext` + явный `AuthConfig`; сборка `AuthConfig` пока остаётся boundary adapter к текущему Loco config.
- [x] Удалить полный Loco `AppContext` из `ServerAuthLifecycleProvider` и runtime extension assembly; bootstrap передаёт явные `ServerRuntimeContext`, `AuthConfig` и узкий mailer handle.
- [x] Перевести REST auth controller на `ServerAuthRuntime`/`ServerEmailRuntime`, runtime/config entrypoints и удалить superseded Loco `AppContext` entrypoints из `AuthLifecycleService`.
- [x] Перевести `tenant`, `channel` и `locale` middleware на `ServerRuntimeContext`; router/health/metrics остаются boundary adapters.
- [x] Перевести `auth_context`, `CurrentUser`/`OptionalCurrentUser` и RBAC permission extractor macro на узкий `ServerAuthRuntime`; Loco `AppContext` остаётся только boundary source для сборки этого runtime в текущем host.
- [x] Перевести module guard и server channel contract на `ServerRuntimeContext`; Loco context больше не является публичным request/channel contract внутри server.
- [x] Перевести GraphQL и users controller handlers на Axum substate (`ServerRuntimeContext`/`ServerAuthRuntime`); Loco `Routes`, response format и error contracts остаются Phase 2 routing cutover.
- [x] Перевести metrics handler и helper pipeline на `ServerRuntimeContext` + узкий `ServerEmailRuntime`; non-clone worker handles читаются через scoped runtime API без утечки Loco `SharedStore`.
- [x] Перевести health readiness/runtime handlers и dependency checks на `ServerRuntimeContext` + `ServerEmailRuntime`; response formatting остаётся отдельным Phase 2 item.
- [x] Перевести channel и standalone Flex REST handlers на `ServerRuntimeContext`; Flex controller tests также используют neutral runtime fixture вместо test-only Loco `AppContext`.
- [x] Перевести OAuth metadata handler на `ServerAuthRuntime`; discovery metadata больше не читает auth config из Loco host state.
- [x] Перевести OAuth REST token, authorize/consent, browser-session и revoke handlers на `ServerAuthRuntime`/`ServerRuntimeContext`; Loco `Routes` остаётся только routing adapter до Phase 2.
- [x] Перевести marketplace registry/governance REST handlers на `ServerRuntimeContext`; artifact storage и remote executor settings читаются через neutral runtime state.
- [x] Перевести swagger, installer status/receipts, admin DLQ, MCP management/remote tools и build WebSocket handlers на `ServerRuntimeContext`; Loco `Routes` остаётся только routing adapter до Phase 2.
- [ ] Перевести Leptos server functions на `HostRuntimeContext`/typed narrow contexts.
- [ ] Перевести module/capability crates, где `loco_rs::app::AppContext` сейчас используется как service locator.

Exit gate: module-owned crates и UI packages не импортируют `loco_rs::app::AppContext`; Loco context остаётся только в server bootstrap/tests allowlist.

### Phase 2. Axum routing и errors без Loco controller API

- [ ] Заменить `loco_rs::controller::Routes` на Axum `Router` в server controllers.
- [ ] Ввести единый `AppError` / response mapper без `loco_rs::Error`.
- [ ] Перевести `crate::error::{Error, Result}` off `pub use loco_rs`.
- [ ] Перевести health/metrics/graphql/auth/controllers на Axum response contracts.
- [ ] Обновить OpenAPI/export reference gates.

Exit gate: production HTTP/GraphQL routes собираются без `loco_rs::controller::*`.

### Phase 3. Отдельный ops CLI для tasks, seeds, migrations

- [ ] Оставить server binary ответственным только за HTTP runtime startup/shutdown.
- [ ] Ввести `rustok-ops-api` для stable ops capability/provider contracts.
- [ ] Ввести отдельный ops crate/binary (`crates/rustok-ops-cli`, bin `rustok-ops`) для maintenance entrypoints.
- [ ] Ввести явный ops registry, который агрегирует command providers выбранного дистрибутива из module manifests/generated registry.
- [ ] Вынести module-specific commands в module-local `cli/` adapter packages, а не в domain core и не в центральный ops crate.
- [ ] Перенести `cleanup`, `rebuild`, `profiles_backfill`, `db_baseline`, `media_cleanup`, `create_oauth_app` на typed ops subcommands.
- [ ] Перенести seed profiles на `rustok-ops seed`.
- [ ] Перенести migration command wrappers на `rustok-ops migrate ...` поверх `Migrator`.
- [ ] Спроектировать follow-up для distribution-aware builds: module manifests, generated runtime/ops registries, external module packaging.
- [ ] Обновить docs/guides/scripts, убрать `cargo loco task` из active instructions.

Exit gate: все maintenance flows запускаются через `rustok-ops ...`; server binary не зависит от ops CLI crate; module commands обнаруживаются через registry, а не через ручную центральную свалку; Loco tasks не зарегистрированы.

### Phase 4. Bootstrap, initializers, workers, shutdown

- [ ] Разобрать `impl Hooks for App` на явные функции bootstrap/lifecycle.
- [ ] Заменить Loco initializers на ordered bootstrap phases.
- [ ] Перевести worker startup/shutdown на собственный lifecycle manager.
- [ ] Перевести tests с `loco_rs::tests_cfg` на `rustok-test-utils`.
- [ ] Удалить зависимость от `loco_rs::cli`.

Exit gate: server binary стартует чистым Axum runtime в targeted integration smoke.

### Phase 5. Последние adapters и dependency removal

- [ ] Удалить `EmailProvider::Loco` или перевести его на native provider без `ctx.mailer`.
- [ ] Удалить `rustok-outbox/loco-adapter` и `rustok_outbox::loco`.
- [ ] Удалить `loco-rs` из workspace dependencies и всех package manifests.
- [ ] Обновить `Cargo.lock`.
- [ ] Архивировать Loco reference docs, scripts и CI freshness checks.

Exit gate: `rg "loco_rs|loco-rs|cargo loco|rustok_outbox::loco"` не находит active code/config paths; допускаются только archived docs с ссылкой на этот план.

## Verification gates

Минимальные gates для каждого PR в этом roadmap:

- `cargo fmt --check`
- targeted `cargo check` по затронутым crates
- `node scripts/verify/verify-api-surface-contract.mjs`
- `node scripts/verify/verify-loco-inventory.mjs`
- `cargo check -p rustok-server --no-default-features` для server/runtime changes
- `cargo xtask module validate <slug>` для затронутых modules

Финальный gate:

```bash
rg "loco_rs|loco-rs|cargo loco|rustok_outbox::loco" apps crates scripts Cargo.toml Cargo.lock
cargo check -p rustok-server --no-default-features
cargo check -p rustok-server
node scripts/verify/verify-api-surface-contract.mjs
```

В финальном состоянии первый `rg` должен возвращать только archived/deprecated docs, если поиск намеренно включает `docs/`.

## Documentation tasks

- [x] Добавить этот центральный план в `docs/index.md`.
- [x] Обновить `apps/server/docs/README.md`, чтобы Loco docs были историческим контекстом, а не active target.
- [x] Пометить `apps/server/docs/loco-core-integration-plan.md` как deprecated.
- [x] Пометить `apps/server/docs/LOCO_FEATURE_SUPPORT.md` как deprecated inventory.
- [x] Обновить `docs/AI_CONTEXT.md` после Phase 0 inventory, чтобы агенты больше не следовали старым Loco правилам.
- [x] Обновить `docs/ai/KNOWN_PITFALLS.md` после появления Axum/ops CLI guardrails.
- [x] Добавить ADR для Axum runtime и отдельного ops CLI cutover до Phase 1 code migration.

## Definition of done

План завершён, когда:

1. `loco-rs` отсутствует в workspace dependencies и lockfile.
2. `apps/server` стартует и тестируется как чистый Axum runtime без maintenance CLI code в production binary.
3. Все module-owned UI/server adapters используют `rustok-api`/module contracts, а не Loco context.
4. GraphQL, REST, Leptos `#[server]`, health, metrics, installer и отдельный maintenance ops CLI имеют verification evidence.
5. Старые Loco документы явно помечены deprecated/archived и указывают на этот план.
## Принцип отбора Loco conventions

Переход не означает слепое копирование Loco API или обязательное отличие от Loco во всех местах. Если существующее соглашение уже удобно для RusToK, совпадает с нашей моделью эксплуатации и не тащит `loco_rs` как runtime/dependency owner, оно может быть сохранено как собственный RusToK contract.

Правило отбора:

- сохраняем формат, naming или workflow, если они полезны проекту и становятся документированным контрактом RusToK;
- меняем или удаляем поведение, если оно нужно только потому, что так устроен Loco;
- не вводим совместимость ради совместимости: после cutover внутренние callers должны зависеть от RusToK-owned contracts, даже если внешний вид этих contracts похож на прежний Loco format.







