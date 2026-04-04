# Реестр crate-модулей `crates/rustok-*`

Документ фиксирует:

- зону ответственности crate'ов;
- их публичные entry points;
- недопустимые обходы модульного слоя;
- различие между platform modules, shared libraries и support/capability crates.

## Важная граница

Этот документ описывает **все crate'ы**, а не только platform modules.

Правило терминов:

- platform modules получают статус только `Core` или `Optional` и определяются через `modules.toml`;
- crate — это техническая форма упаковки;
- рядом с module-crates живут shared libraries и support/capability crates.

## Единый реестр

| Crate | Ответственность | Публичные entry points | Нельзя делать |
|---|---|---|---|
| `rustok-core` | Базовые платформенные контракты: модульная модель, события, RBAC-примитивы, shared types. | `RusToKModule`, `ModuleRegistry`, `Permission`, `Resource`, `Action`, `DomainEvent`, `EventEnvelope`. | Дублировать core-контракты в приложениях и модулях. |
| `rustok-api` | Общий web/API слой для transport-адаптеров: tenant/auth/request context, GraphQL helper'ы, extractor-friendly types. | `AuthContext`, `TenantContext`, `RequestContext`, `PageInfo`, `PaginationInput`, `GraphQLError`, `scope_matches`. | Возвращать общие HTTP/GraphQL helper'ы обратно в `apps/server` или в `rustok-core`. |
| `rustok-auth` | **[ЗАМЕНЯЕТ loco auth]** `Core` module аутентификации: JWT (HS256 и RS256), Argon2 хеширование, refresh tokens, password reset/invite/verification tokens. Bridge к loco — `apps/server/src/auth.rs`. Алгоритм: `AuthConfig::algorithm: JwtAlgorithm` (HS256 по умолчанию); RS256 — `with_rs256(private_pem, public_pem)`. | `AuthConfig`, `JwtAlgorithm`, `encode_access_token`, `decode_access_token`, `hash_password`, `verify_password`, `generate_refresh_token`. | Использовать `loco_rs::prelude::auth::JWT` напрямую; реализовывать JWT/хеширование вне этого crate. |
| `rustok-cache` | **[ЗАМЕНЯЕТ loco cache config]** `Core` module управления Redis-соединением: единая точка подключения, in-memory fallback, `CacheService::health()`. Доступен через `ctx.shared_store.get::<CacheService>()`. Redis URL: `settings.rustok.cache.redis_url` (YAML) > `RUSTOK_REDIS_URL` > `REDIS_URL`. | `CacheService`, `CacheService::from_url`, `CacheHealthReport`, `CacheSettings`. | Читать `REDIS_URL` вручную в модулях; создавать `redis::Client` напрямую; использовать `ctx.config.cache`. |
| `rustok-email` | **[ЗАМЕНЯЕТ loco mailer как primary transport]** `Core` module email-рассылок: SMTP через lettre, Tera-шаблоны. Фабрика `email_service_from_ctx` в `apps/server/src/services/email.rs` выбирает провайдер (`smtp`\|`loco`\|`none`); SMTP кэшируется через `SharedSmtpEmailService`. Два публичных trait: `PasswordResetEmailSender` (узкий) и `TransactionalEmailSender` (общий, по template ID `"{module}/{action}"`). | `EmailService`, `PasswordResetEmailSender`, `TransactionalEmailSender`, `PasswordResetEmail`, `EmailTemplateProvider`, `RenderedEmail`, `SmtpEmailSender::with_provider`. | Вызывать `ctx.mailer` напрямую в обработчиках; создавать `AsyncSmtpTransport` вне email-сервиса; выносить email в отдельный platform module поверх crate. |
| `rustok-storage` | **[ЗАМЕНЯЕТ loco storage abstraction]** Infrastructure crate: `StorageBackend` trait, `LocalStorage`, `StorageService`. Инициализируется в `bootstrap_app_runtime`, доступен через `ctx.shared_store.get::<StorageService>()`. S3 задекларирован в features, не реализован. | `StorageService`, `StorageBackend`, `UploadedObject`, `LocalStorageConfig`. | Создавать adhoc upload backends в контроллерах; добавлять параллельные storage paths мимо этого crate. |
| `rustok-content` | Shared content helpers и port-based orchestration core для `blog` / `forum` / `comments` / `pages`; не product transport layer. | `ContentModule`, `ContentOrchestrationService`, `ContentOrchestrationBridge`, `locale::*`, helper surface `services::NodeService`. | Возвращать product GraphQL/REST/admin/storefront surfaces в `rustok-content`, строить новые доменные модули поверх `NodeService` как primary storage или снова зашивать orchestration в shared `nodes`. |
| `rustok-cart` | Дефолтный cart submodule семейства `ecommerce`: cart storage, line items, totals и lifecycle корзины. | `CartModule`, `CartService`, `dto::*`, `entities::*`. | Тянуть зависимость на `rustok-commerce` как на нижний shared layer или пришивать обязательные FK на product/order tables. |
| `rustok-customer` | Дефолтный storefront customer submodule семейства `ecommerce`: отдельный customer profile, optional linkage на `user_id` и optional service-level bridge `customer -> user -> profile` для read enrichment без схлопывания доменов. | `CustomerModule`, `CustomerService`, `dto::*`, `entities::*`. | Схлопывать customer profile обратно в platform/admin user или тянуть зависимость на `rustok-commerce` как на нижний shared layer. |
| `rustok-profiles` | Универсальный публичный профиль пользователя поверх platform `users`: handle/display-name/visibility/public summary contract для forum/blog/groups/social/commerce, включая batched author/member lookup без внутреннего N+1, host-level `ProfileSummaryLoader` для GraphQL runtime, explicit backfill path для missing profiles и outbox-событие `profile.updated` после profile writes. | `ProfilesModule`, `ProfileService`, `ProfilesReader`, `ProfileSummaryLoader`, `graphql::*`, `dto::*`, `entities::*`. | Схлопывать `profiles` обратно в auth/user identity, в `rustok-customer` или в будущий seller-domain. |
| `rustok-commerce` | Root umbrella module семейства `ecommerce`: orchestration, compatibility facade, legacy GraphQL/REST adapters, store context/locale policy и верхний transport/API entry point. | `CommerceModule`, `CheckoutService`, `StoreContextService`, `CatalogService`, `PricingService`, `InventoryService`, `graphql::*`, `controllers::*`. | Возвращать продуктовую/pricing/inventory/region бизнес-логику обратно в umbrella-crate или реализовывать commerce transport/API поверх `apps/server` мимо crate. |
| `rustok-commerce-foundation` | Internal support crate семейства `ecommerce`: shared DTO/entities/errors/search helpers. | `dto::*`, `entities::*`, `CommerceError`, `CommerceResult`. | Делать его самостоятельным platform module или переносить в него orchestration/facade-логику. |
| `rustok-product` | Дефолтный catalog submodule семейства `ecommerce`. | `ProductModule`, `CatalogService`. | Тянуть зависимость на `rustok-commerce` как на нижний shared layer. |
| `rustok-region` | Дефолтный region submodule семейства `ecommerce`: регионы, валюты, страны и tax policy. | `RegionModule`, `RegionService`, `dto::*`, `entities::*`. | Возвращать ownership таблицы `regions` в `rustok-pricing` или смешивать region lifecycle с umbrella orchestration. |
| `rustok-pricing` | Дефолтный pricing submodule семейства `ecommerce`. | `PricingModule`, `PricingService`. | Тянуть зависимость на `rustok-commerce` как на нижний shared layer. |
| `rustok-inventory` | Дефолтный inventory submodule семейства `ecommerce`. | `InventoryModule`, `InventoryService`. | Тянуть зависимость на `rustok-commerce` как на нижний shared layer. |
| `rustok-order` | Дефолтный order submodule семейства `ecommerce`: storage, lifecycle, line item snapshots, order events. | `OrderModule`, `OrderService`, `dto::*`, `entities::*`. | Тянуть зависимость на `rustok-commerce` как на нижний shared layer или пришивать обязательные FK на product/catalog tables. |
| `rustok-payment` | Дефолтный payment submodule семейства `ecommerce`: payment collections, payment attempts и lifecycle авторизации/капчура в built-in manual/default режиме. | `PaymentModule`, `PaymentService`, `dto::*`, `entities::*`. | Смешивать базовую payment domain model с provider-specific логикой вроде Stripe вместо отдельного следующего подмодуля. |
| `rustok-fulfillment` | Дефолтный fulfillment submodule семейства `ecommerce`: shipping options, fulfillment records и shipment lifecycle в built-in manual/default режиме. | `FulfillmentModule`, `FulfillmentService`, `dto::*`, `entities::*`. | Смешивать базовую shipping-модель с carrier/provider-specific логикой вместо отдельного следующего подмодуля. |
| `rustok-blog` | Blog-домен с собственным storage, comment backend через `rustok-comments` и author presentation через `rustok-profiles`. | `BlogModule`, `PostService`, `CommentService`, `graphql::*`, `controllers::*`. | Обходить blog-правила напрямую через `rustok-content` legacy helpers или SQL. |
| `rustok-forum` | Forum-домен и transport-адаптеры, включая author presentation через `rustok-profiles`. | `ForumModule`, `TopicService`, `ReplyService`, `graphql::*`, `controllers::*`. | Обходить forum-сервисы через server-only handlers. |
| `rustok-pages` | Pages/menus/blocks и transport-адаптеры. | `PagesModule`, `PageService`, `graphql::*`, `controllers::*`. | Оставлять pages GraphQL/REST в `apps/server`. |
| `rustok-workflow` | Workflow automation, execution history, webhook ingress и transport-адаптеры. | `WorkflowModule`, `WorkflowService`, `WorkflowEngine`, `graphql::*`, `controllers::*`. | Считать Alloy runtime-зависимостью workflow registry-графа. |
| `rustok-media` | Media lifecycle, storage-facing services и transport-адаптеры. | `MediaService`, `graphql::*`, `controllers::*`. | Держать media transport/API слой в `apps/server`. |
| `alloy` | Module-agnostic Alloy runtime/engine capability: script storage, execution, scheduler, migrations и REST handler core. | `create_default_engine`, `ScriptEngine`, `ScriptOrchestrator`, `Scheduler`, `ScriptRegistry`, `SeaOrmStorage`, `create_router`. | Регистрировать Alloy как runtime-модуль или навешивать на него tenant module lifecycle. |
| `rustok-index` | Индексация и search contracts. | `IndexModule`, `Indexer`, `LocaleIndexer`. | Строить ad-hoc индексацию мимо index contracts. |
| `rustok-rbac` | Authorization contracts и Casbin-backed runtime. | `RbacModule`, `PermissionResolver`, `PermissionAuthorizer`, `AuthzEngine`. | Возвращаться к hardcoded role checks в server-коде. |
| `rustok-tenant` | Tenant lifecycle и module enablement. | `TenantModule`, `TenantService`, tenant DTOs. | Менять tenant/module configuration напрямую в приложениях или SQL. |
| `rustok-outbox` | `Core` module transactional outbox и relay contracts. **Не замена Loco Queue** — решает другую задачу: гарантирует атомарность между доменной операцией и публикацией события (запись в `sys_events` в одной DB-транзакции). Loco Queue (Sidekiq) — generic background job runner; для maintenance-задач используются loco Tasks. | `OutboxModule`, `TransactionalEventBus`, `OutboxRelay`, `OutboxTransport`. | Публиковать критичные межмодульные события мимо outbox; дублировать event delivery-path через Loco Queue. |
| `rustok-iggy` | Event streaming transport runtime. | `IggyTransport`, topology/DLQ/replay managers. | Писать parallel transport-runtime для тех же потоков в сервисах. |
| `rustok-iggy-connector` | Подключение к Iggy и message I/O abstractions. | `IggyConnector`, `MessageSubscriber`, connector configs. | Обходить connector-абстракцию прямыми ad-hoc подключениями. |
| `rustok-telemetry` | Observability bootstrap. | `init`, `TelemetryConfig`, `render_metrics`, `current_trace_id`. | Настраивать разрозненные telemetry pipelines в разных модулях. |
| `rustok-mcp` | MCP adapter/server tool surface, Alloy-related integrations, identity/policy foundation, session-start runtime binding hooks, pluggable scaffold draft store и первый real Alloy scaffold flow через официальный SDK `rmcp`; persisted management layer, scaffold draft control plane и DB-backed runtime bridge живут в `apps/server`. | `RusToKMcpServer`, `McpServerConfig`, `McpSessionContext`, `McpRuntimeBinding`, `McpAccessResolver`, `McpAuditSink`, `McpScaffoldDraftStore`, `ScaffoldModuleRequest`, `StageModuleScaffoldResponse`, `ReviewModuleScaffoldRequest`, `ApplyModuleScaffoldRequest`, `serve_stdio`, tool re-exports. | Реализовывать отдельные MCP entrypoints в приложениях, если сценарий уже покрывает `rustok-mcp`; дублировать upstream spec/SDK docs в локальных файлах; считать текущий draft-store contract финальной заменой remote MCP bootstrap и полного codegen/publish pipeline. |
| `rustok-ai` | Capability crate AI host/orchestrator слоя: multiprovider registry (`OpenAI-compatible`, `Anthropic`, `Gemini`), `AiRouter`, task profiles, hybrid direct/MCP execution model, persisted control-plane service layer для provider/task/tool profiles, sessions/runs/traces/approvals, direct first-party verticals (`alloy_code`, `image_asset`, `product_copy`, `blog_draft`), bounded live streaming через `aiSessionEvents` с native text/tool-call streaming для всех текущих provider families, bounded recent stream-event history через `aiRecentRunStreamEvents`, bounded recent run history через `aiRecentRuns`, а также bounded runtime observability snapshot + Prometheus module/span telemetry для router/run outcomes. | `ModelProvider`, `OpenAiCompatibleProvider`, `AnthropicProvider`, `GeminiProvider`, `AiRouter`, `AiRuntime`, `McpClientAdapter`, `DirectExecutionRegistry`, `ExecutionMode`, `ExecutionOverride`, `TaskProfile`, `ToolTrace`, `AiManagementService`. | Расширять `rustok-mcp` до model host; прятать AI authorization за `MCP_MANAGE` вместо RBAC-first permissions; делать MCP обязательной внутренней шиной для first-party AI workflows; обходить канонические domain services (`Alloy`, `MediaService`, `CatalogService`, `PostService`) в direct verticals; дублировать AI business UI в `apps/admin` или `apps/next-admin` вместо capability-owned пакетов. |

## Runtime registry RBAC contract

Для модулей, которые реально регистрируются в `apps/server/src/modules/mod.rs`, канонический
RBAC-контракт задаётся тремя источниками:

- `RusToKModule::permissions()`;
- `RusToKModule::dependencies()`;
- `README.md` -> `## Interactions`.

Текущее владение RBAC surface:

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

Alloy остаётся capability-слоем с permission surface `scripts:*`, но не входит в runtime registry.



