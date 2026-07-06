---
id: doc://docs/modules/crates-registry.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Реестр crate-модулей `crates/rustok-*`

Документ фиксирует:

- зону ответственности crate-ов;
- их публичные точки входа;
- недопустимые обходы модульного слоя;
- различие между платформенными модулями, общими библиотеками и support/capability crate-ами.

## Важная граница

Этот документ описывает **все crate-ы**, а не только платформенные модули.

Правило терминов:

- платформенные модули получают статус только `Core` или `Optional` и определяются через `modules.toml`;
- crate — это техническая форма упаковки;
- рядом с module-crate-ами живут общие библиотеки и support/capability crate-ы.

Источником истины для живого контракта crate-уровня остаётся локальная документация самого компонента:

- root `README.md` in English;
- `docs/README.md` in English;
- `docs/implementation-plan.md` in English if the crate maintains a local development plan.

Этот реестр нужен как сводный слой: он фиксирует владение, публичные точки входа и запреты, но не заменяет локальные docs.

## Единый реестр

| Crate | Ответственность | Публичные точки входа | Нельзя делать |
|---|---|---|---|
| `rustok-core` | Общий foundation-слой платформы: модульная модель, типизированные примитивы, RBAC/security-контракты, validation helper-ы и базовые cross-module типы. | `RusToKModule`, `ModuleRegistry`, `Permission`, `Resource`, `Action`, `SecurityContext`, общие helper-типы из `lib.rs`. | Дублировать foundation-контракты в приложениях и модулях или тянуть сюда доменно-владеемую runtime-логику. |
| `rustok-events` | Каноническая import-поверхность для event-контрактов: `DomainEvent`, `EventEnvelope`, schema metadata и validation rules; `rustok-core::events` остаётся только compatibility re-export-путём. | `DomainEvent`, `EventEnvelope`, `EventSchema`, `FieldSchema`, `EVENT_SCHEMAS`, `ValidateEvent`. | Возвращать каноническое владение event-контрактами обратно в `rustok-core` или дублировать schema registry в crate-ах-потребителях. |
| `rustok-api` | Общий host/API-слой для transport-адаптеров: tenant/auth/request/channel contexts, GraphQL-хелперы, module-agnostic контракты UI-маршрутов и shared FFA UI input/query helpers. | `AuthContext`, `TenantContext`, `RequestContext`, `UiRouteContext`, `UiRouteQueryUpdate`, `normalize_ui_text`, `parse_ui_csv`, `PageInfo`, `PaginationInput`, `GraphQLError`, `scope_matches`. | Возвращать общие HTTP/GraphQL host-контракты обратно в `apps/server` или тянуть в `rustok-core` web/API-specific поверхность. |
| `rustok-auth` | **[ЗАМЕНЯЕТ loco auth]** `Core` module аутентификации: JWT (HS256 и RS256), Argon2 хеширование, refresh tokens, password reset/invite/verification tokens, auth lifecycle/OAuth runtimes и owner-owned auth/OAuth GraphQL. Bridge к loco — `apps/server/src/auth.rs`. | `AuthConfig`, `JwtAlgorithm`, `AuthLifecyclePort`, `AuthLifecycleRuntime`, `OAuthAdminPort`, `OAuthAdminRuntime`, `graphql::{AuthQuery, AuthMutation, OAuthQuery, OAuthMutation}`, JWT/credential helpers. | Использовать `loco_rs::prelude::auth::JWT` напрямую; держать auth/OAuth GraphQL resolver/DTO в `apps/server`; реализовывать JWT/хеширование вне этого crate. |
| `rustok-cache` | **[ЗАМЕНЯЕТ loco cache config]** `Core` module управления Redis-соединением: единая точка подключения, configurable Redis circuit breaker, in-memory fallback, instrumented `CacheBackend::stats()`, `CacheService::health()`. Доступен через `ctx.shared_store.get::<CacheService>()`. Redis URL: `settings.rustok.cache.redis_url` (YAML) > `RUSTOK_REDIS_URL` > `REDIS_URL`. | `CacheService`, `CacheService::from_url`, `CacheService::from_url_with_options`, `CacheBackendOptions`, `CacheHealthReport`, `CacheSettings`. | Читать `REDIS_URL` вручную в модулях; создавать `redis::Client` напрямую; использовать `ctx.config.cache`. |
| `rustok-email` | **[ЗАМЕНЯЕТ loco mailer как основной transport]** `Core` модуль email-рассылок: SMTP через lettre, Tera-шаблоны. Фабрика `email_service_from_ctx` в `apps/server/src/services/email.rs` выбирает провайдер (`smtp`\|`loco`\|`none`); SMTP кэшируется через `SharedSmtpEmailService`. Два публичных trait: `PasswordResetEmailSender` (узкий) и `TransactionalEmailSender` (общий, по template ID `"{module}/{action}"`). | `EmailService`, `PasswordResetEmailSender`, `TransactionalEmailSender`, `PasswordResetEmail`, `EmailTemplateProvider`, `RenderedEmail`, `SmtpEmailSender::with_provider`. | Вызывать `ctx.mailer` напрямую в обработчиках; создавать `AsyncSmtpTransport` вне email-сервиса; выносить email в отдельный платформенный модуль поверх crate. |
| `rustok-storage` | Общий storage abstraction-слой: `StorageBackend`, `StorageService`, path generation и backend boundary для file-oriented модулей. Инициализируется в `bootstrap_app_runtime`, доступен через `ctx.shared_store.get::<StorageService>()`. | `StorageService`, `StorageBackend`, `UploadedObject`, `LocalStorage`, `LocalStorageConfig`. | Создавать ad-hoc upload/storage backends в контроллерах или добавлять параллельные storage-path мимо этого crate. |
| `rustok-content` | Общие content-хелперы и port-based orchestration core для `blog` / `forum` / `comments` / `pages`; owner-owned content dashboard post analytics; не продуктовый CRUD transport-слой. | `ContentModule`, `ContentOrchestrationService`, `ContentOrchestrationBridge`, `load_post_stats_snapshot`, `ContentCountSnapshot`, `graphql::ContentQuery`, `graphql::{NodeLoader, NodeTranslationLoader, NodeBodyLoader}`, `locale::*`, helper-поверхность `services::NodeService`. | Возвращать product GraphQL/REST/admin/storefront-поверхности или content entity dataloaders в `apps/server`, держать SQL/DTO content analytics в `apps/server::RootQuery`, строить новые доменные модули поверх `NodeService` как основного хранилища или снова зашивать orchestration в общие `nodes`. |
| `rustok-content-orchestration` | Support crate для cross-module bridge implementation поверх `rustok-content` orchestration contracts; держит blog/forum/comments/taxonomy conversion internals вне `apps/server` и не импортирует Loco runtime types. | `build_content_orchestration_service`, `content_orchestration_from_shared`, `SharedContentOrchestrationService`, `graphql::ContentOrchestrationMutation`, реализация `ContentOrchestrationBridge` при включённых feature-срезах `mod-content`/`mod-blog`/`mod-forum`/`mod-comments`. | Возвращать bridge implementation, GraphQL conversion DTO/resolvers, прямые SQL/entity imports owner crates, Loco `AppContext` service locator или conversion business rules обратно в `apps/server`. |
| `rustok-cart` | Дефолтный cart-подмодуль семейства `ecommerce`: cart storage, line items, totals и lifecycle корзины. | `CartModule`, `CartService`, `dto::*`, `entities::*`. | Тянуть зависимость на `rustok-commerce` как на нижний общий слой или пришивать обязательные FK на product/order tables. |
| `rustok-customer` | Дефолтный storefront customer-подмодуль семейства `ecommerce`: отдельный customer profile, optional linkage на `user_id` и optional service-level bridge `customer -> user -> profile` для read enrichment без схлопывания доменов. | `CustomerModule`, `CustomerService`, `dto::*`, `entities::*`. | Схлопывать customer profile обратно в platform/admin user или тянуть зависимость на `rustok-commerce` как на нижний общий слой. |
| `rustok-profiles` | Универсальный публичный профиль пользователя поверх платформенного `users`: handle/display-name/visibility/public summary-контракт, batched author/member lookup, taxonomy-backed `profile_tags`, explicit backfill path и `profile.updated` event. | `ProfilesModule`, `ProfileService`, `ProfilesReader`, `ProfileSummaryLoader`, `graphql::*`, `dto::*`, `entities::*`. | Схлопывать `profiles` обратно в auth/user identity, в `rustok-customer` или в будущий seller-домен. |
| `rustok-commerce` | Корневой umbrella-модуль семейства `ecommerce`: orchestration, compatibility-фасад, legacy GraphQL/REST-адаптеры, store context/locale policy и верхняя transport/API-точка входа. Storefront checkout orchestration принимает host-neutral `StorefrontCheckoutRuntime`, shared/admin product, storefront product/catalog/order/cart/checkout, admin order/change/return, admin fulfillment, admin shipping и admin payment HTTP handlers принимают `CommerceHttpRuntime`; остальные Loco-boundary transport adapters режутся отдельными срезами. | `CommerceModule`, `CheckoutService`, `StorefrontCheckoutRuntime`, `CommerceHttpRuntime`, `StoreContextService`, `CatalogService`, `PricingService`, `InventoryService`, `graphql::*`, `controllers::*`. | Возвращать продуктовую/pricing/inventory/region бизнес-логику обратно в umbrella-crate, прокидывать Loco `AppContext` внутрь checkout orchestration/product/order/cart/checkout/change/return/fulfillment/shipping/payment handlers или реализовывать commerce transport/API поверх `apps/server` мимо crate. |
| `rustok-commerce-foundation` | Support crate семейства `ecommerce`, используемый только как зависимость: общие DTO, entities, error-поверхность и query/search helper-ы для split commerce crate-ов. | `dto::*`, `entities::*`, `CommerceError`, `CommerceResult`. | Делать его самостоятельным платформенным модулем или переносить в него orchestration/facade-логику устойчивых ограниченных контекстов. |
| `rustok-product` | Дефолтный catalog-подмодуль семейства `ecommerce`. | `ProductModule`, `CatalogService`. | Тянуть зависимость на `rustok-commerce` как на нижний общий слой. |
| `rustok-region` | Дефолтный region submodule семейства `ecommerce`: регионы, валюты, страны и tax policy. | `RegionModule`, `RegionService`, `dto::*`, `entities::*`. | Возвращать ownership таблицы `regions` в `rustok-pricing` или смешивать region lifecycle с umbrella orchestration. |
| `rustok-pricing` | Дефолтный pricing-подмодуль семейства `ecommerce`. | `PricingModule`, `PricingService`. | Тянуть зависимость на `rustok-commerce` как на нижний общий слой. |
| `rustok-inventory` | Дефолтный inventory-подмодуль семейства `ecommerce`. | `InventoryModule`, `InventoryService`. | Тянуть зависимость на `rustok-commerce` как на нижний общий слой. |
| `rustok-order` | Дефолтный order-подмодуль семейства `ecommerce`: storage, lifecycle, line item snapshots, order events и owner-owned dashboard order analytics. | `OrderModule`, `OrderService`, `load_order_stats_snapshot`, `OrderStatsSnapshot`, `dto::*`, `entities::*`. | Тянуть зависимость на `rustok-commerce` как на нижний общий слой, пришивать обязательные FK на product/catalog tables или держать SQL/DTO order analytics в `apps/server::RootQuery`. |
| `rustok-payment` | Дефолтный payment submodule семейства `ecommerce`: payment collections, payment attempts и lifecycle авторизации/капчура в built-in manual/default режиме. | `PaymentModule`, `PaymentService`, `dto::*`, `entities::*`. | Смешивать базовую payment domain model с provider-specific логикой вроде Stripe вместо отдельного следующего подмодуля. |
| `rustok-fulfillment` | Дефолтный fulfillment submodule семейства `ecommerce`: shipping options, fulfillment records и shipment lifecycle в built-in manual/default режиме. | `FulfillmentModule`, `FulfillmentService`, `dto::*`, `entities::*`. | Смешивать базовую shipping-модель с carrier/provider-specific логикой вместо отдельного следующего подмодуля. |
| `rustok-blog` | Blog-домен с собственным storage, comment backend через `rustok-comments` и author presentation через `rustok-profiles`. REST post/comment handlers используют узкий `BlogHttpRuntime`; Loco host state остаётся только в текущем route-state adapter до полного Axum cutover. | `BlogModule`, `PostService`, `CommentService`, `BlogHttpRuntime`, `graphql::*`, `controllers::*`. | Обходить blog-правила напрямую через `rustok-content` legacy helpers или SQL; прокидывать Loco `AppContext` обратно в post/comment handlers. |
| `rustok-forum` | Forum-домен и transport-адаптеры, включая author presentation через `rustok-profiles`. REST handlers используют узкий `ForumHttpRuntime`; Loco host state остаётся только в текущем route-state adapter до полного Axum cutover. | `ForumModule`, `TopicService`, `ReplyService`, `ForumHttpRuntime`, `graphql::*`, `controllers::*`. | Обходить forum-сервисы через server-only handlers или прокидывать Loco `AppContext` обратно в forum REST handlers. |
| `rustok-pages` | Pages/menus/blocks и transport-адаптеры. REST page/block handlers используют узкий `PagesHttpRuntime`; Loco host state остаётся только в текущем route-state adapter до полного Axum cutover. | `PagesModule`, `PageService`, `PagesHttpRuntime`, `graphql::*`, `controllers::*`. | Оставлять pages GraphQL/REST в `apps/server` или прокидывать Loco `AppContext` обратно в page/block handlers. |
| `rustok-seo` | Optional SEO module: explicit metadata overrides, canonical storefront read contract, manual redirects, sitemaps, robots, shared SEO capability contracts и cross-cutting admin infrastructure surface. HTTP handlers используют узкий `SeoHttpRuntime`; Loco host state остаётся только в текущем route-state adapter до полного Axum cutover. | `SeoModule`, `SeoService`, `SeoHttpRuntime`, `SeoQuery`, `SeoMutation`, `controllers::*`, `dto::*`. | Дублировать SEO source of truth в storefront host-ах, переносить canonical/redirect resolution в adapter-слой, делать host-local metadata precedence, прокидывать Loco `AppContext` обратно в SEO HTTP handlers или считать `rustok-seo-admin` долгосрочным owner-экраном для чужих entity editors. |
| `rustok-seo-render` | Support crate для Rust-host последней мили: рендерит `SeoPageContext` в SSR head HTML и сериализует typed robots directives без владения SEO runtime. | `render_head_html`, `robots_directives`. | Переносить сюда SEO storage/routing logic, tenant policy или снова собирать локальные Rust-host render helper-ы поверх того же SEO contract. |
| `rustok-seo-admin-support` | Support crate для owner-module admin SEO: reusable Leptos panels, form helpers и GraphQL transport вокруг shared `rustok-seo` capability contract. | `SeoEntityPanel`, `SeoCapabilityNotice`, `SeoEntityForm`, internal `transport::*`. | Превращать его в central SEO route, держать здесь runtime/storage policy или переносить ownership entity screens из `pages/product/blog/forum` обратно в `rustok-seo-admin`. |
| `rustok-workflow` | Workflow automation domain: triggers, steps, execution history, webhook ingress, admin UI и transport-адаптеры поверх платформенной event-инфраструктуры. HTTP/webhook handlers используют узкий `WorkflowHttpRuntime`; Loco host state остаётся только в текущем route-state adapter до полного Axum cutover. | `WorkflowModule`, `WorkflowService`, `WorkflowHttpRuntime`, `WorkflowEngine`, `graphql::*`, `controllers::*`. | Превращать workflow в отдельный event-transport, считать Alloy жёсткой зависимостью workflow-графа на уровне registry/runtime или прокидывать Loco `AppContext` обратно в workflow HTTP handlers. |
| `rustok-media` | Media lifecycle, storage-facing services и owner-owned transport-адаптеры, включая usage statistics. HTTP handlers используют узкий `MediaHttpRuntime`; Loco host state остаётся только в текущем route-state adapter до полного Axum cutover. | `MediaService`, `MediaHttpRuntime`, `load_media_usage_snapshot`, `graphql::{MediaQuery, MediaMutation, MediaUsageStats}`, `controllers::*`. | Держать media resolver/DTO, включая `mediaUsage`, или прямые media entity imports в `apps/server`; прокидывать Loco `AppContext` обратно в media HTTP handlers. |
| `alloy` | Capability-oriented модуль script/runtime: script storage, execution, scheduler, bridge helper-ы, GraphQL/HTTP-поверхности и hook-oriented integration-контракты. HTTP handlers используют узкий `AlloyHttpRuntime`; Loco host state остаётся только в текущем route-state adapter до полного Axum cutover. | `AlloyModule`, `create_default_engine`, `build_alloy_runtime`, `SharedAlloyRuntime`, `AlloyHttpRuntime`, `ScriptEngine`, `ScriptOrchestrator`, `Scheduler`, `ScriptRegistry`, `SeaOrmStorage`, `create_router`. | Выводить Alloy из `ModuleRegistry`, разносить script runtime по host-коду, прокидывать Loco `AppContext` обратно в Alloy HTTP handlers или превращать capability surface в server-only wiring без module contract. |
| `rustok-index` | Индексация и search-контракты. | `IndexModule`, `Indexer`, `LocaleIndexer`. | Строить ad-hoc индексацию мимо index-контрактов. |
| `rustok-search` | Search/read discovery модуль: search documents, query/runtime, analytics, GraphQL, admin/storefront search UI. | `SearchModule`, `PgSearchEngine`, `SearchQueryPort`, `SearchSuggestionPort`, `graphql::SearchQueryRoot`, `graphql::SearchMutationRoot`, `rustok-search-admin`, `rustok-search-storefront`. | Смешивать search с `rustok-index`, держать search GraphQL query/mutation/types в `apps/server` или переносить search query/runtime в host UI/app. |
| `rustok-rbac` | Контракты авторизации, tenant policy runtime и RBAC GraphQL role surface. | `RbacModule`, `PermissionResolver`, `PermissionAuthorizer`, `AuthzEngine`, `graphql::RbacQuery`, `graphql::RbacMutation`. | Возвращаться к hardcoded role checks в server-коде или держать RBAC GraphQL query/mutation/types в `apps/server`. |
| `rustok-tenant` | Tenant lifecycle и module enablement. | `TenantModule`, `TenantService`, tenant DTOs. | Менять tenant/module configuration напрямую в приложениях или SQL. |
| `rustok-outbox` | `Core` module transactional outbox и relay-контракты. **Не замена Loco Queue** — решает другую задачу: гарантирует атомарность между доменной операцией и публикацией события (запись в `sys_events` в одной DB-транзакции). Loco Queue (Sidekiq) — универсальный background job runner; для maintenance-задач используются loco Tasks. | `OutboxModule`, `TransactionalEventBus`, `OutboxRelay`, `OutboxTransport`. | Публиковать критичные межмодульные события мимо outbox; дублировать event delivery-path через Loco Queue. |
| `rustok-iggy` | Event streaming transport runtime. | `IggyTransport`, topology/DLQ/replay managers. | Писать parallel transport-runtime для тех же потоков в сервисах. |
| `rustok-iggy-connector` | Подключение к Iggy и message I/O abstractions. | `IggyConnector`, `MessageSubscriber`, connector configs. | Обходить connector-абстракцию прямыми ad-hoc подключениями. |
| `rustok-telemetry` | Общий observability-foundation слой: telemetry bootstrap, metrics/tracing wiring и общие instrumentation helper-ы для host/runtime-слоя. | `init`, `TelemetryConfig`, `render_metrics`, `current_trace_id`. | Настраивать разрозненные telemetry pipelines в разных модулях или тянуть сюда domain-specific observability logic. |
| `rustok-mcp` | Тонкая MCP adapter/server-поверхность поверх `rmcp`: typed tools, runtime binding, access policy, audit hooks, owner-owned management GraphQL и Alloy-related scaffold/review/apply vertical; persisted storage и DB-backed runtime bridges живут в `apps/server`. | `RusToKMcpServer`, `McpManagementPort`, `McpManagementRuntime`, `graphql::{McpQuery, McpMutation}`, `McpRuntimeBinding`, `McpAccessResolver`, `McpAuditSink`, `McpScaffoldDraftStore`, tool re-exports. | Держать MCP GraphQL resolver/DTO в `apps/server`; реализовывать отдельные MCP entrypoints, если сценарий уже покрывает `rustok-mcp`; дублировать upstream MCP/rmcp spec и security docs. |
| `rustok-ai` | Capability crate AI host/orchestrator-слоя: multiprovider-реестр (`OpenAI-compatible`, `Anthropic`, `Gemini`), `AiRouter`, task profiles, hybrid direct/MCP execution model, persisted control-plane service layer для provider/task/tool profiles, sessions/runs/traces/approvals, owner-owned GraphQL query/mutation/subscription surface, direct first-party verticals (`alloy_code`, `image_asset`, `product_copy`, `blog_draft`), bounded live streaming через `aiSessionEvents`, recent history и runtime observability. | `ModelProvider`, `OpenAiCompatibleProvider`, `AnthropicProvider`, `GeminiProvider`, `AiRouter`, `AiRuntime`, `AiHostRuntime`, `McpClientAdapter`, `DirectExecutionRegistry`, `AiManagementService`, `graphql::{AiQuery, AiMutation, AiSubscription}`, `AiGraphqlRoleSlugProviderHandle`. | Расширять `rustok-mcp` до model-хоста; размещать AI GraphQL resolver/DTO в `apps/server`; прятать AI-авторизацию за `MCP_MANAGE`; делать MCP обязательной внутренней шиной; обходить канонические domain services; дублировать AI business UI в host-приложениях вместо capability-owned пакетов; прокидывать Loco `AppContext` внутрь `rustok-ai`. |
| `flex` | Capability crate системы custom fields: attached/standalone-контракты, field definitions, registry/orchestration helper-ы и localized attached values; donor ownership остаётся у модулей-потребителей. При этом crate теперь формализован и как `capability_only` ghost module в `modules.toml`. | `FlexModule`, `CustomFieldsSchema`, standalone/attached-контракты, registry/orchestration helper-ы из `crates/flex`, module-local docs и plan. | Превращать `flex` в самостоятельный бизнес-модуль, забирать donor persistence себе, тянуть стандартные модули в зависимость от Flex как обязательного слоя или считать server-owned transport surfaces доказательством, что ownership donor contracts переехал в `flex`. |
| `rustok-test-utils` | Общий testing-support crate: database setup helper-ы, mock event bus/transport, fixtures и reusable test helper-ы для RusToK crates/apps. | `setup_test_db`, `MockEventBus`, `MockEventTransport`, `fixtures::*`, `helpers::*`. | Дублировать одни и те же fixtures и mocks локально в модулях вместо использования общего testing-слоя. |

## RBAC-контракт runtime-реестра

Для модулей, которые реально регистрируются в `apps/server/src/modules/mod.rs`, канонический
RBAC-контракт задаётся тремя источниками:

- `RusToKModule::permissions()`;
- `RusToKModule::dependencies()`;
- root `README.md` с `## Purpose`, `## Responsibilities`, `## Entry points`, `## Interactions`
  и ссылкой на `docs/README.md`.

Текущее владение RBAC-поверхностью:

- `rustok-auth` -> `users:*`
- `rustok-tenant` -> `tenants:*`, `modules:*`
- `rustok-rbac` -> `settings:*`, `logs:*`
- `rustok-content` -> orchestration permissions (`forum_topics:*`, `blog_posts:*` для conversion flows)
- `rustok-customer` -> `customers:*`
- `rustok-profiles` -> `profiles:*`
- `rustok-region` -> `regions:*`
- `rustok-order` -> `orders:*`
- `rustok-payment` -> `payments:*`
- `rustok-fulfillment` -> `fulfillments:*`
- `rustok-commerce` -> commerce resources
- `rustok-blog` -> `blog_posts:*`
- `rustok-forum` -> `forum_categories:*`, `forum_topics:*`, `forum_replies:*`
- `rustok-pages` -> `pages:*`
- `rustok-workflow` -> `workflows:*`, `workflow_executions:*`

Alloy остаётся capability-oriented слоем с permission-поверхностью `scripts:*`,
но при этом входит в runtime-реестр как обычный optional модуль.

`flex` теперь тоже входит в runtime-реестр как capability-only ghost module и
держит permission-поверхность `flex_schemas:*` / `flex_entries:*`, не забирая
себе donor persistence ownership.

## Регламент актуализации

При изменении владения, точек входа, runtime-границ или anti-pattern rules у crate:

1. Сначала обновляется локальный `README.md` и `docs/README.md` соответствующего компонента.
2. Затем синхронизируется эта таблица-реестр.
3. Если crate становится платформенным модулем или перестаёт им быть, одновременно обновляются `modules.toml`, `rustok-module.toml`, [контракт manifest](./manifest.md) и [центральный реестр](./registry.md).


### Правило для implementation plans

Если добавляется новый crate (module/support/capability) с локальным `docs/implementation-plan.md`,
его нужно сразу добавить в `docs/modules/implementation-plans-registry.md` (`Global board`, уникальный `Plan ID`).

Если crate удаляется или переименовывается, строку в `Global board` нужно удалить или обновить в тот же цикл.

## Связанные документы

- [Обзор модульной платформы](./overview.md)
- [Реестр модулей и приложений](./registry.md)
- [Индекс документации по модулям](./_index.md)
- [Контракт `rustok-module.toml`](./manifest.md)
- [Шаблон документации модуля](../templates/module_contract.md)


## Скрипты модулей и библиотек

- Для каждого crate поддерживается локальная папка `scripts/` для crate-specific automation (verify/migration/generation/maintenance).
- Корневой `scripts/` используется только для общих orchestration сценариев уровня платформы.
