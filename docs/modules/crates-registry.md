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

- root `README.md` на английском;
- `docs/README.md` на русском;
- `docs/implementation-plan.md` на русском, если crate ведёт локальный план развития.

Этот реестр нужен как сводный слой: он фиксирует владение, публичные точки входа и запреты, но не заменяет локальные docs.

## Единый реестр

| Crate | Ответственность | Публичные точки входа | Нельзя делать |
|---|---|---|---|
| `rustok-core` | Общий foundation-слой платформы: модульная модель, типизированные примитивы, RBAC/security-контракты, validation helper-ы и базовые cross-module типы. | `RusToKModule`, `ModuleRegistry`, `Permission`, `Resource`, `Action`, `SecurityContext`, общие helper-типы из `lib.rs`. | Дублировать foundation-контракты в приложениях и модулях или тянуть сюда доменно-владеемую runtime-логику. |
| `rustok-events` | Каноническая import-поверхность для event-контрактов: `DomainEvent`, `EventEnvelope`, schema metadata и validation rules; `rustok-core::events` остаётся только compatibility re-export-путём. | `DomainEvent`, `EventEnvelope`, `EventSchema`, `FieldSchema`, `EVENT_SCHEMAS`, `ValidateEvent`. | Возвращать каноническое владение event-контрактами обратно в `rustok-core` или дублировать schema registry в crate-ах-потребителях. |
| `rustok-api` | Общий host/API-слой для transport-адаптеров: tenant/auth/request/channel contexts, GraphQL-хелперы и module-agnostic контракты UI-маршрутов. | `AuthContext`, `TenantContext`, `RequestContext`, `UiRouteContext`, `PageInfo`, `PaginationInput`, `GraphQLError`, `scope_matches`. | Возвращать общие HTTP/GraphQL host-контракты обратно в `apps/server` или тянуть в `rustok-core` web/API-specific поверхность. |
| `rustok-auth` | **[ЗАМЕНЯЕТ loco auth]** `Core` module аутентификации: JWT (HS256 и RS256), Argon2 хеширование, refresh tokens, password reset/invite/verification tokens. Bridge к loco — `apps/server/src/auth.rs`. Алгоритм: `AuthConfig::algorithm: JwtAlgorithm` (HS256 по умолчанию); RS256 — `with_rs256(private_pem, public_pem)`. | `AuthConfig`, `JwtAlgorithm`, `encode_access_token`, `decode_access_token`, `hash_password`, `verify_password`, `generate_refresh_token`. | Использовать `loco_rs::prelude::auth::JWT` напрямую; реализовывать JWT/хеширование вне этого crate. |
| `rustok-cache` | **[ЗАМЕНЯЕТ loco cache config]** `Core` module управления Redis-соединением: единая точка подключения, in-memory fallback, `CacheService::health()`. Доступен через `ctx.shared_store.get::<CacheService>()`. Redis URL: `settings.rustok.cache.redis_url` (YAML) > `RUSTOK_REDIS_URL` > `REDIS_URL`. | `CacheService`, `CacheService::from_url`, `CacheHealthReport`, `CacheSettings`. | Читать `REDIS_URL` вручную в модулях; создавать `redis::Client` напрямую; использовать `ctx.config.cache`. |
| `rustok-email` | **[ЗАМЕНЯЕТ loco mailer как основной transport]** `Core` модуль email-рассылок: SMTP через lettre, Tera-шаблоны. Фабрика `email_service_from_ctx` в `apps/server/src/services/email.rs` выбирает провайдер (`smtp`\|`loco`\|`none`); SMTP кэшируется через `SharedSmtpEmailService`. Два публичных trait: `PasswordResetEmailSender` (узкий) и `TransactionalEmailSender` (общий, по template ID `"{module}/{action}"`). | `EmailService`, `PasswordResetEmailSender`, `TransactionalEmailSender`, `PasswordResetEmail`, `EmailTemplateProvider`, `RenderedEmail`, `SmtpEmailSender::with_provider`. | Вызывать `ctx.mailer` напрямую в обработчиках; создавать `AsyncSmtpTransport` вне email-сервиса; выносить email в отдельный платформенный модуль поверх crate. |
| `rustok-storage` | Общий storage abstraction-слой: `StorageBackend`, `StorageService`, path generation и backend boundary для file-oriented модулей. Инициализируется в `bootstrap_app_runtime`, доступен через `ctx.shared_store.get::<StorageService>()`. | `StorageService`, `StorageBackend`, `UploadedObject`, `LocalStorage`, `LocalStorageConfig`. | Создавать ad-hoc upload/storage backends в контроллерах или добавлять параллельные storage-path мимо этого crate. |
| `rustok-content` | Общие content-хелперы и port-based orchestration core для `blog` / `forum` / `comments` / `pages`; не продуктовый transport-слой. | `ContentModule`, `ContentOrchestrationService`, `ContentOrchestrationBridge`, `locale::*`, helper-поверхность `services::NodeService`. | Возвращать product GraphQL/REST/admin/storefront-поверхности в `rustok-content`, строить новые доменные модули поверх `NodeService` как основного хранилища или снова зашивать orchestration в общие `nodes`. |
| `rustok-cart` | Дефолтный cart-подмодуль семейства `ecommerce`: cart storage, line items, totals и lifecycle корзины. | `CartModule`, `CartService`, `dto::*`, `entities::*`. | Тянуть зависимость на `rustok-commerce` как на нижний общий слой или пришивать обязательные FK на product/order tables. |
| `rustok-customer` | Дефолтный storefront customer-подмодуль семейства `ecommerce`: отдельный customer profile, optional linkage на `user_id` и optional service-level bridge `customer -> user -> profile` для read enrichment без схлопывания доменов. | `CustomerModule`, `CustomerService`, `dto::*`, `entities::*`. | Схлопывать customer profile обратно в platform/admin user или тянуть зависимость на `rustok-commerce` как на нижний общий слой. |
| `rustok-profiles` | Универсальный публичный профиль пользователя поверх платформенного `users`: handle/display-name/visibility/public summary-контракт, batched author/member lookup, taxonomy-backed `profile_tags`, explicit backfill path и `profile.updated` event. | `ProfilesModule`, `ProfileService`, `ProfilesReader`, `ProfileSummaryLoader`, `graphql::*`, `dto::*`, `entities::*`. | Схлопывать `profiles` обратно в auth/user identity, в `rustok-customer` или в будущий seller-домен. |
| `rustok-commerce` | Корневой umbrella-модуль семейства `ecommerce`: orchestration, compatibility-фасад, legacy GraphQL/REST-адаптеры, store context/locale policy и верхняя transport/API-точка входа. | `CommerceModule`, `CheckoutService`, `StoreContextService`, `CatalogService`, `PricingService`, `InventoryService`, `graphql::*`, `controllers::*`. | Возвращать продуктовую/pricing/inventory/region бизнес-логику обратно в umbrella-crate или реализовывать commerce transport/API поверх `apps/server` мимо crate. |
| `rustok-commerce-foundation` | Support crate семейства `ecommerce`, используемый только как зависимость: общие DTO, entities, error-поверхность и query/search helper-ы для split commerce crate-ов. | `dto::*`, `entities::*`, `CommerceError`, `CommerceResult`. | Делать его самостоятельным платформенным модулем или переносить в него orchestration/facade-логику устойчивых ограниченных контекстов. |
| `rustok-product` | Дефолтный catalog-подмодуль семейства `ecommerce`. | `ProductModule`, `CatalogService`. | Тянуть зависимость на `rustok-commerce` как на нижний общий слой. |
| `rustok-region` | Дефолтный region submodule семейства `ecommerce`: регионы, валюты, страны и tax policy. | `RegionModule`, `RegionService`, `dto::*`, `entities::*`. | Возвращать ownership таблицы `regions` в `rustok-pricing` или смешивать region lifecycle с umbrella orchestration. |
| `rustok-pricing` | Дефолтный pricing-подмодуль семейства `ecommerce`. | `PricingModule`, `PricingService`. | Тянуть зависимость на `rustok-commerce` как на нижний общий слой. |
| `rustok-inventory` | Дефолтный inventory-подмодуль семейства `ecommerce`. | `InventoryModule`, `InventoryService`. | Тянуть зависимость на `rustok-commerce` как на нижний общий слой. |
| `rustok-order` | Дефолтный order-подмодуль семейства `ecommerce`: storage, lifecycle, line item snapshots, order events. | `OrderModule`, `OrderService`, `dto::*`, `entities::*`. | Тянуть зависимость на `rustok-commerce` как на нижний общий слой или пришивать обязательные FK на product/catalog tables. |
| `rustok-payment` | Дефолтный payment submodule семейства `ecommerce`: payment collections, payment attempts и lifecycle авторизации/капчура в built-in manual/default режиме. | `PaymentModule`, `PaymentService`, `dto::*`, `entities::*`. | Смешивать базовую payment domain model с provider-specific логикой вроде Stripe вместо отдельного следующего подмодуля. |
| `rustok-fulfillment` | Дефолтный fulfillment submodule семейства `ecommerce`: shipping options, fulfillment records и shipment lifecycle в built-in manual/default режиме. | `FulfillmentModule`, `FulfillmentService`, `dto::*`, `entities::*`. | Смешивать базовую shipping-модель с carrier/provider-specific логикой вместо отдельного следующего подмодуля. |
| `rustok-blog` | Blog-домен с собственным storage, comment backend через `rustok-comments` и author presentation через `rustok-profiles`. | `BlogModule`, `PostService`, `CommentService`, `graphql::*`, `controllers::*`. | Обходить blog-правила напрямую через `rustok-content` legacy helpers или SQL. |
| `rustok-forum` | Forum-домен и transport-адаптеры, включая author presentation через `rustok-profiles`. | `ForumModule`, `TopicService`, `ReplyService`, `graphql::*`, `controllers::*`. | Обходить forum-сервисы через server-only handlers. |
| `rustok-pages` | Pages/menus/blocks и transport-адаптеры. | `PagesModule`, `PageService`, `graphql::*`, `controllers::*`. | Оставлять pages GraphQL/REST в `apps/server`. |
| `rustok-seo` | Optional SEO module: explicit metadata overrides, canonical storefront read contract, manual redirects, sitemaps, robots, shared SEO capability contracts и cross-cutting admin infrastructure surface. | `SeoModule`, `SeoService`, `SeoQuery`, `SeoMutation`, `controllers::*`, `dto::*`. | Дублировать SEO source of truth в storefront host-ах, переносить canonical/redirect resolution в adapter-слой, делать host-local metadata precedence или считать `rustok-seo-admin` долгосрочным owner-экраном для чужих entity editors. |
| `rustok-seo-render` | Support crate для Rust-host последней мили: рендерит `SeoPageContext` в SSR head HTML и сериализует typed robots directives без владения SEO runtime. | `render_head_html`, `robots_directives`. | Переносить сюда SEO storage/routing logic, tenant policy или снова собирать локальные Rust-host render helper-ы поверх того же SEO contract. |
| `rustok-seo-admin-support` | Support crate для owner-module admin SEO: reusable Leptos panels, form helpers и GraphQL transport вокруг shared `rustok-seo` capability contract. | `SeoEntityPanel`, `SeoCapabilityNotice`, `SeoEntityForm`, `api::*`. | Превращать его в central SEO route, держать здесь runtime/storage policy или переносить ownership entity screens из `pages/product/blog/forum` обратно в `rustok-seo-admin`. |
| `rustok-workflow` | Workflow automation domain: triggers, steps, execution history, webhook ingress, admin UI и transport-адаптеры поверх платформенной event-инфраструктуры. | `WorkflowModule`, `WorkflowService`, `WorkflowEngine`, `graphql::*`, `controllers::*`. | Превращать workflow в отдельный event-transport или считать Alloy жёсткой зависимостью workflow-графа на уровне registry/runtime. |
| `rustok-media` | Media lifecycle, storage-facing services и transport-адаптеры. | `MediaService`, `graphql::*`, `controllers::*`. | Держать media transport/API слой в `apps/server`. |
| `alloy` | Capability-oriented модуль script/runtime: script storage, execution, scheduler, bridge helper-ы, GraphQL/HTTP-поверхности и hook-oriented integration-контракты. | `AlloyModule`, `create_default_engine`, `ScriptEngine`, `ScriptOrchestrator`, `Scheduler`, `ScriptRegistry`, `SeaOrmStorage`, `create_router`. | Выводить Alloy из `ModuleRegistry`, разносить script runtime по host-коду или превращать capability surface в server-only wiring без module contract. |
| `rustok-index` | Индексация и search-контракты. | `IndexModule`, `Indexer`, `LocaleIndexer`. | Строить ad-hoc индексацию мимо index-контрактов. |
| `rustok-rbac` | Контракты авторизации и Casbin-backed runtime. | `RbacModule`, `PermissionResolver`, `PermissionAuthorizer`, `AuthzEngine`. | Возвращаться к hardcoded role checks в server-коде. |
| `rustok-tenant` | Tenant lifecycle и module enablement. | `TenantModule`, `TenantService`, tenant DTOs. | Менять tenant/module configuration напрямую в приложениях или SQL. |
| `rustok-outbox` | `Core` module transactional outbox и relay-контракты. **Не замена Loco Queue** — решает другую задачу: гарантирует атомарность между доменной операцией и публикацией события (запись в `sys_events` в одной DB-транзакции). Loco Queue (Sidekiq) — универсальный background job runner; для maintenance-задач используются loco Tasks. | `OutboxModule`, `TransactionalEventBus`, `OutboxRelay`, `OutboxTransport`. | Публиковать критичные межмодульные события мимо outbox; дублировать event delivery-path через Loco Queue. |
| `rustok-iggy` | Event streaming transport runtime. | `IggyTransport`, topology/DLQ/replay managers. | Писать parallel transport-runtime для тех же потоков в сервисах. |
| `rustok-iggy-connector` | Подключение к Iggy и message I/O abstractions. | `IggyConnector`, `MessageSubscriber`, connector configs. | Обходить connector-абстракцию прямыми ad-hoc подключениями. |
| `rustok-telemetry` | Общий observability-foundation слой: telemetry bootstrap, metrics/tracing wiring и общие instrumentation helper-ы для host/runtime-слоя. | `init`, `TelemetryConfig`, `render_metrics`, `current_trace_id`. | Настраивать разрозненные telemetry pipelines в разных модулях или тянуть сюда domain-specific observability logic. |
| `rustok-mcp` | Тонкая MCP adapter/server-поверхность поверх `rmcp`: typed tools, runtime binding, access policy, audit hooks и Alloy-related scaffold/review/apply vertical; persisted management/control plane и DB-backed runtime bridges живут в `apps/server`. | `RusToKMcpServer`, `McpServerConfig`, `McpSessionContext`, `McpRuntimeBinding`, `McpAccessResolver`, `McpAuditSink`, `McpScaffoldDraftStore`, `ScaffoldModuleRequest`, `StageModuleScaffoldResponse`, `ReviewModuleScaffoldRequest`, `ApplyModuleScaffoldRequest`, `serve_stdio`, tool re-exports. | Реализовывать отдельные MCP entrypoints в приложениях, если сценарий уже покрывает `rustok-mcp`; дублировать upstream MCP/rmcp spec и security docs в локальных файлах; считать текущий draft-store-контракт финальной заменой remote MCP bootstrap и полного codegen/publish pipeline. |
| `rustok-ai` | Capability crate AI host/orchestrator-слоя: multiprovider-реестр (`OpenAI-compatible`, `Anthropic`, `Gemini`), `AiRouter`, task profiles, hybrid direct/MCP execution model, persisted control-plane service layer для provider/task/tool profiles, sessions/runs/traces/approvals, direct first-party verticals (`alloy_code`, `image_asset`, `product_copy`, `blog_draft`), bounded live streaming через `aiSessionEvents` с native text/tool-call streaming для всех текущих семейств provider-ов, bounded recent stream-event history через `aiRecentRunStreamEvents`, bounded recent run history через `aiRecentRuns`, а также bounded runtime observability snapshot + Prometheus module/span telemetry для router/run outcomes. | `ModelProvider`, `OpenAiCompatibleProvider`, `AnthropicProvider`, `GeminiProvider`, `AiRouter`, `AiRuntime`, `McpClientAdapter`, `DirectExecutionRegistry`, `ExecutionMode`, `ExecutionOverride`, `TaskProfile`, `ToolTrace`, `AiManagementService`. | Расширять `rustok-mcp` до model-хоста; прятать AI-авторизацию за `MCP_MANAGE` вместо RBAC-first permission-модели; делать MCP обязательной внутренней шиной для собственных AI-workflow; обходить канонические domain services (`Alloy`, `MediaService`, `CatalogService`, `PostService`) в direct verticals; дублировать AI business UI в `apps/admin` или `apps/next-admin` вместо capability-owned пакетов. |
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

## Связанные документы

- [Обзор модульной платформы](./overview.md)
- [Реестр модулей и приложений](./registry.md)
- [Индекс документации по модулям](./_index.md)
- [Контракт `rustok-module.toml`](./manifest.md)
- [Шаблон документации модуля](../templates/module_contract.md)
