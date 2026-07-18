---
id: doc://docs/modules/crates-registry.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Crate Registry `crates/rustok-*`

This document captures:

- Responsibility zone of crates;
- Their public entry points;
- Impermissible bypasses of the module layer;
- The difference between platform modules, shared libraries and support/capability crates.

## Important Boundary

This document describes **all crates**, not just platform modules.

Terminology rule:

- Platform modules receive only `Core` or `Optional` status and are defined through `modules.toml`;
- A crate is a technical packaging form;
- Shared libraries and support/capability crates live next to module crates.

The source of truth for the live crate-level contract remains the local documentation of the component itself:

- Root `README.md` in English;
- `docs/README.md` in English;
- `docs/implementation-plan.md` in English if the crate maintains a local development plan.

This registry serves as a summary layer: it captures ownership, public entry points and prohibitions, but does not replace local docs.

## Unified Registry

| Crate | Responsibility | Public Entry Points | Must Not Do |
|---|---|---|---|
| `rustok-core` | Shared foundation layer of the platform: modular model, typed primitives, RBAC/security contracts, validation helpers and base cross-module types. | `RusToKModule`, `ModuleRegistry`, `Permission`, `Resource`, `Action`, `SecurityContext`, shared helper types from `lib.rs`. | Duplicate foundation contracts in applications and modules or pull domain-owned runtime logic here. |
| `rustok-events` | Canonical import surface for event contracts: `DomainEvent`, `EventEnvelope`, schema metadata and validation rules; `rustok-core::events` remains only a compatibility re-export path. | `DomainEvent`, `EventEnvelope`, `EventSchema`, `FieldSchema`, `EVENT_SCHEMAS`, `ValidateEvent`. | Return canonical event contract ownership back to `rustok-core` or duplicate schema registry in consumer crates. |
| `rustok-api` | Shared host/API layer for transport adapters: tenant/auth/request/channel contexts, GraphQL helpers, pagination and permission matching. UI route/query/input/i18n contracts live outside this crate. | `AuthContext`, `TenantContext`, `RequestContext`, `PageInfo`, `PaginationInput`, `GraphQLError`, `scope_matches`, `locale_tags_match`. | Return shared HTTP/GraphQL host contracts back to `apps/server`, pull web/API-specific surface into `rustok-core` or own UI route/query/input/i18n helpers. |
| `rustok-runtime` | Host runtime foundation helpers: typed shared-handle lookup, neutral runtime DB access helpers and runtime composition utilities. | `HostRuntimeContext`, `db_clone`, `require_shared`, `RuntimeHandleError`. | Own domain services, HTTP response mapping, CLI contracts, FBA metadata, UI transport or turn into a new service locator. |
| `rustok-sandbox` | Neutral execution foundation shared by Alloy and module artifacts: typed execution subjects, default-deny capability broker, resource policy, executor registry and audit observers. | `SandboxRuntime`, `SandboxExecutor`, `SandboxPolicy`, `CapabilityBroker`, `ExecutionObserver`, `RhaiEngine`. | Depend on Alloy, `rustok-modules`, server hosts or domain modules; own marketplace identity, source authoring, installation state or module governance. |
| `rustok-verification-transport` | Typed tonic gRPC framing and adapters for the module trust-verification port. | `GrpcTrustVerifier`, `VerificationGrpcService`, generated `VerificationService` protobuf contract. | Own admission policy, CAS, database transactions, verification credentials, or execute Cosign/SLSA/SBOM tools. |
| `rustok-verification-worker` | Isolated operational trust-verification worker for artifact admission. | `VerificationWorker`, `VerificationPolicy`, `VerificationGrpcService`. | Let the server or runtime execute verification tools, access installation storage/CAS, or make marketplace admission decisions. |
| `rustok-module-build-transport` | Typed mTLS tonic gRPC framing and adapters for the isolated module build-worker port. | `GrpcModuleBuildWorker`, `ModuleBuildGrpcService`, generated `ModuleBuildService` protobuf contract. | Own build policy, source/CAS access, database transactions, publication credentials, or run Cargo in a server/runtime process. |
| `rustok-module-build-worker` | Separate process boundary that invokes a fixed image-owned module-build job runner through the typed worker protocol. | Binary `rustok-module-build-worker`, `CommandBuildWorker`. | Access tenant DB/CAS, accept request-selected runner commands, inherit platform secrets, expose plaintext traffic, or run inside server/runtime. |
| `rustok-module-build-dispatcher` | Broker-neutral process-and-ack coordination for a queued build and the mTLS worker, including the independently deployable dedicated-Iggy delivery host. | `rustok-module-build-dispatcher` binary, `ModuleBuildDeliveryConsumer`, `IggyModuleBuildDeliverySource`, `ModuleBuildDeliverySource`, `ModuleBuildDelivery`. | Run Cargo, access CAS, share the worker process, consume global outbox rows directly, or acknowledge a failed delivery. |
| `rustok-worker-transport` | Shared mutually authenticated tonic listener configuration for isolated workers. | `MutualTlsListenerConfig`. | Own worker protocol, policy, build/verification execution, application DB access, or plaintext listener fallbacks. |
| `rustok-web` | Axum HTTP boundary helpers with RusToK-owned response and error mapping. | `HttpError`, `HttpResult`, `ErrorBody`, `json_response`. | Own domain errors or business rules, runtime composition, CLI contracts, FBA metadata or UI transport. |
| `rustok-fba` | Fluid Backend Architecture metadata contracts for provider/consumer registries and embedded/remote transport profile descriptions. | `BackendTopology`, `TransportProfile`, `CapabilityId`, `FbaCallContext`, `FbaProviderDescriptor`, `FbaConsumerDependency`. | Own transport implementations, gRPC/HTTP adapters, domain services, runtime composition, CLI contracts or duplicate `rustok-api::ports`. |
| `rustok-cli-core` | Core contracts for the RusToK platform CLI and module command providers; the user-facing runner is `rustok-cli`. | `CommandDescriptor`, `CommandRequest`, `CommandOutcome`, `CommandProvider`, asynchronous `CommandProvider::execute`, `CliCoreError`. | Depend on domain crates, collect all module commands centrally, enter the production server binary or own terminal/parser UX that belongs to the binary crate. |
| `rustok-cli-platform` | Platform-level CLI command provider crate for commands that are not owned by a domain module. | `PlatformCommandProvider`, `command_provider`, `core version`. | Depend on `apps/server`, `rustok-cli` or domain crates; become a dumping ground for module-specific maintenance commands. |
| `rustok-migrations` | Neutral platform schema composition: platform-owned SeaORM migrations plus module `MigrationSource` exports and validated cross-module ordering. | `Migrator`, `MigrationDescriptor`. | Depend on `apps/server`, place HTTP runtime composition in migration code or duplicate module migration sources. |
| `rustok-installer` | Neutral installation policy contracts: plans, versioned topology, preflight, receipts, seed workflow, executor ports, and distributed-role deployment hand-offs. | `InstallPlan`, `InstallTopology`, `InstallProfile`, `InstallApplyOptions`, `InstallApplyOutput`, `InstallDeploymentPort`, `execute_distributed_role_deployments`, `InstallRoleDeploymentRequest`, install state/receipt/preflight contracts. | Depend on `apps/server`, parse CLI arguments, compose Axum routes, run Cargo/deployment-provider commands, or duplicate install sequencing in a host. |
| `rustok-installer-cli` | Selected `rustok-cli` adapter for installer plan, preflight, apply, durable status and seed operations. | `command_provider`, `install plan`, `install preflight`, `install apply`, `install status`, `seed apply`. | Depend on `apps/server`, own installer policy, or duplicate shared executor and seed workflows. |
| `rustok-installer-persistence` | SeaORM adapter for installer database readiness, schema application, durable sessions, locks, receipts, reusable bootstrap writers, and standalone apply verification, using schema composed by `rustok-migrations`. | `SeaOrmInstallerPorts`, `SeaOrmInstallerBootstrapPorts`, `SeaOrmInstallerApplyPorts`, `InstallerPersistenceService`, `entities::{install_session, install_step_receipt}`. | Depend on `apps/server`, expose HTTP/CLI parsing, duplicate installer state contracts, or move SeaORM installer mappings back to an executable host. |
| `rustok-build` | Platform build/release capability: persistent build and release state contracts, queue/execution services, role-specific runtime intent, portable deployment settings/workspace paths, and typed host publication hand-off. | `build::{Entity, Model, BuildStatus, BuildStage, DeploymentProfile}`, `BuildRuntimeMode`, `RoleBuildPlan`, `DeploymentSettings`, `DeploymentBackend`, `DeploymentWorkspace`, `release::{Entity, Model, ReleaseStatus}`, `ReleasePublisherPort`. | Depend on `apps/server`, own HTTP/CLI parsing, run deployment commands in installer core, or place build/release persistence models back in the server host. |
| `rustok-cli-registry` | Selected distribution command provider registry for the platform CLI. It is generated from root `cli-registry.toml` and module `[provides.cli]` metadata and sits between module-local `cli/` adapters and the runner. | `SelectedDistributionRegistry`, `selected_distribution_registry`, generated `src/generated.rs`. | Depend on `apps/server`, `rustok-cli`, terminal parsing libraries or domain crates; implement real module command logic centrally. |
| `rustok-distribution` | Selected module-registry composition shared by executable hosts. It maps distribution feature flags to a single `ModuleRegistry` and canonical composition identity without owning routes or commands. | `build_registry`, `composition_identity`. | Depend on `apps/server`, own HTTP routes, own command providers or duplicate module lifecycle policy. |
| `rustok-cli` | User-facing platform CLI runner for maintenance, distribution and module command entrypoints. It consumes `rustok-cli-core` contracts through `rustok-cli-registry` and is separate from the production HTTP server. | Binary `rustok-cli`, `run_with_args`, `parse_command_args`, `CommandRegistry::execute`, `core version`, `collect_commands`, `render_command_list`, `render_command_list_json`. | Depend on `apps/server`, collect all module commands centrally, place module-specific business logic in the runner or become part of the HTTP server binary. |
| `rustok-ui-core` | Framework-agnostic FFA UI contracts shared by Leptos packages and future adapters: route context, route query updates/intents, admin query sanitization, UI input normalization and busy-key helpers. | `UiRouteContext`, `UiRouteQueryUpdate`, `UiRouteQueryIntent`, `UiRouteQueryWrite`, `AdminQueryKey`, `sanitize_admin_route_query`, `normalize_ui_text`, `parse_ui_csv`, `ui_busy_key*`. | Depend on Leptos/Dioxus/Next.js, own module-specific route keys, select locale from cookies/headers/query or contain transport execution. |
| `rustok-ui-transport` | Framework-agnostic FFA transport evidence and build-profile selected transport orchestration for module-owned UI facades: native server functions for Leptos monolith builds and GraphQL for headless/standalone-compatible builds. | `UiTransportPath`, `UiTransportError`, `UiTransportResult`, `execute_selected_transport`. | Execute HTTP/GraphQL requests, depend on Leptos/Dioxus/Next.js, own module-specific DTOs, replace owner-module transport facades or automatically fallback from monolith native execution to GraphQL. |
| `rustok-graphql` | Framework-agnostic GraphQL HTTP client contracts for UI and transport adapters. | `GraphqlRequest`, `GraphqlResponse`, `GraphqlHttpError`, `execute`, `persisted_query_extension`. | Depend on Leptos/Dioxus/Next.js, own schema/resolver contracts or contain module-specific query documents. |
| `rustok-graphql-leptos` | Leptos reactive hooks adapter for `rustok-graphql`. | `use_query`, `use_mutation`, `use_lazy_query`, `QueryResult`, `MutationResult`. | Own HTTP execution rules, replace module transport facades, or contain Dioxus integration. |
| `rustok-ui-i18n` | Framework-agnostic UI message catalog and key-resolution core for Leptos and future Dioxus adapters. | `UiMessageCatalog`, `UiTranslator`, `build_ui_message_catalog`, `resolve_ui_message`, `resolve_ui_message_or_fallback`. | Select locale from cookies/headers/query, depend on Leptos/Dioxus/Next.js/GraphQL or contain module-specific copy. |
| `rustok-ui-i18n-leptos` | Leptos adapter for `rustok-ui-i18n` static bundles and host-provided route locale context. | `LeptosUiMessages`. | Own message resolution rules, select locale from cookies/headers/query, or contain Dioxus integration. |
| `rustok-auth` | `Core` authentication module: JWT (HS256 and RS256), Argon2 hashing, refresh tokens, password reset/invite/verification tokens, auth lifecycle/OAuth runtimes and owner-owned auth/OAuth GraphQL. | `AuthConfig`, `JwtAlgorithm`, `AuthLifecyclePort`, `AuthLifecycleRuntime`, `OAuthAdminPort`, `OAuthAdminRuntime`, `graphql::{AuthQuery, AuthMutation, OAuthQuery, OAuthMutation}`, JWT/credential helpers. | Keep auth/OAuth GraphQL resolver/DTO in `apps/server`; implement JWT/hashing outside this crate. |
| `rustok-cache` | `Core` Redis connection management module: single connection point, configurable Redis circuit breaker, in-memory fallback, instrumented `CacheBackend::stats()`, and `CacheService::health()`. Redis URL: `settings.rustok.cache.redis_url` (YAML) > `RUSTOK_REDIS_URL` > `REDIS_URL`. | `CacheService`, `CacheService::from_url`, `CacheService::from_url_with_options`, `CacheBackendOptions`, `CacheHealthReport`, `CacheSettings`. | Read `REDIS_URL` manually in modules; create `redis::Client` directly; bypass typed runtime composition. |
| `rustok-email` | `Core` email delivery module: SMTP via lettre and Tera templates. Factory `email_service_from_ctx` in `apps/server/src/services/email.rs` selects configured SMTP or disabled delivery; SMTP is cached through `SharedSmtpEmailService`. Two public traits: `PasswordResetEmailSender` (narrow) and `TransactionalEmailSender` (general, by template ID `"{module}/{action}"`). | `EmailService`, `PasswordResetEmailSender`, `TransactionalEmailSender`, `PasswordResetEmail`, `EmailTemplateProvider`, `RenderedEmail`, `SmtpEmailSender::with_provider`. | Bypass owner-owned email services in handlers; create `AsyncSmtpTransport` outside the email service; extract email into a separate platform module over the crate. |
| `rustok-storage` | Shared storage abstraction layer: `StorageBackend`, `StorageService`, path generation and backend boundary for file-oriented modules. Initialized in `bootstrap_app_runtime`, available through `ctx.shared_store.get::<StorageService>()`. | `StorageService`, `StorageBackend`, `UploadedObject`, `LocalStorage`, `LocalStorageConfig`. | Create ad-hoc upload/storage backends in controllers or add parallel storage paths bypassing this crate. |
| `rustok-content` | Shared content helpers and port-based orchestration core for `blog` / `forum` / `comments` / `pages`; owner-owned content dashboard post analytics; not a product CRUD transport layer. | `ContentModule`, `ContentOrchestrationService`, `ContentOrchestrationBridge`, `load_post_stats_snapshot`, `ContentCountSnapshot`, `graphql::ContentQuery`, `graphql::{NodeLoader, NodeTranslationLoader, NodeBodyLoader}`, `locale::*`, helper surface `services::NodeService`. | Return product GraphQL/REST/admin/storefront surfaces or content entity dataloaders to `apps/server`, keep SQL/DTO content analytics in `apps/server::RootQuery`, build new domain modules on top of `NodeService` as primary storage or re-stitch orchestration into shared `nodes`. |
| `rustok-content-orchestration` | Support crate for cross-module bridge implementation over `rustok-content` orchestration contracts; holds blog/forum/comments/taxonomy conversion internals outside `apps/server` and uses explicit runtime types. | `build_content_orchestration_service`, `content_orchestration_from_shared`, `SharedContentOrchestrationService`, `graphql::ContentOrchestrationMutation`, implementation of `ContentOrchestrationBridge` with enabled feature slices `mod-content`/`mod-blog`/`mod-forum`/`mod-comments`. | Return bridge implementation, GraphQL conversion DTO/resolvers, direct SQL/entity imports owner crates, host-wide service locators or conversion business rules back to `apps/server`. |
| `rustok-cart` | Default cart submodule of the `ecommerce` family: cart storage, line items, totals and cart lifecycle. | `CartModule`, `CartService`, `dto::*`, `entities::*`. | Pull dependency on `rustok-commerce` as a lower shared layer or hard-wire mandatory FKs to product/order tables. |
| `rustok-customer` | Default storefront customer submodule of the `ecommerce` family: separate customer profile, optional linkage to `user_id` and optional service-level bridge `customer -> user -> profile` for read enrichment without collapsing domains. | `CustomerModule`, `CustomerService`, `dto::*`, `entities::*`. | Collapse customer profile back into platform/admin user or pull dependency on `rustok-commerce` as a lower shared layer. |
| `rustok-profiles` | Universal public user profile over platform `users`: handle/display-name/visibility/public summary contract, batched author/member lookup, taxonomy-backed `profile_tags`, explicit backfill path and `profile.updated` event. | `ProfilesModule`, `ProfileService`, `ProfilesReader`, `ProfileSummaryLoader`, `graphql::*`, `dto::*`, `entities::*`. | Collapse `profiles` back into auth/user identity, into `rustok-customer` or into the future seller domain. |
| `rustok-commerce` | Root umbrella module of the `ecommerce` family: orchestration, compatibility facade, GraphQL/REST adapters, store context/locale policy and top-level transport/API entry point. Storefront checkout orchestration accepts host-neutral `StorefrontCheckoutRuntime`, shared/admin product, storefront product/catalog/order/cart/checkout, admin order/change/return, admin fulfillment, admin shipping and admin payment HTTP handlers accept `CommerceHttpRuntime`. | `CommerceModule`, `CheckoutService`, `StorefrontCheckoutRuntime`, `CommerceHttpRuntime`, `StoreContextService`, `CatalogService`, `PricingService`, `InventoryService`, `graphql::*`, `controllers::*`. | Return product/pricing/inventory/region business logic back to the umbrella crate or implement commerce transport/API over `apps/server` outside the crate. |
| `rustok-commerce-foundation` | Support crate of the `ecommerce` family, used only as a dependency: shared DTO, entities, error surface and query/search helpers for split commerce crates. | `dto::*`, `entities::*`, `CommerceError`, `CommerceResult`. | Make it a standalone platform module or move orchestration/facade logic of stable bounded contexts into it. |
| `rustok-product` | Default catalog submodule of the `ecommerce` family. | `ProductModule`, `CatalogService`. | Pull dependency on `rustok-commerce` as a lower shared layer. |
| `rustok-region` | Default region submodule of the `ecommerce` family: regions, currencies, countries and tax policy. | `RegionModule`, `RegionService`, `dto::*`, `entities::*`. | Return ownership of the `regions` table to `rustok-pricing` or mix region lifecycle with umbrella orchestration. |
| `rustok-pricing` | Default pricing submodule of the `ecommerce` family. | `PricingModule`, `PricingService`. | Pull dependency on `rustok-commerce` as a lower shared layer. |
| `rustok-inventory` | Default inventory submodule of the `ecommerce` family. | `InventoryModule`, `InventoryService`. | Pull dependency on `rustok-commerce` as a lower shared layer. |
| `rustok-order` | Default order submodule of the `ecommerce` family: storage, lifecycle, line item snapshots, order events and owner-owned dashboard order analytics. | `OrderModule`, `OrderService`, `load_order_stats_snapshot`, `OrderStatsSnapshot`, `dto::*`, `entities::*`. | Pull dependency on `rustok-commerce` as a lower shared layer, hard-wire mandatory FKs to product/catalog tables or keep SQL/DTO order analytics in `apps/server::RootQuery`. |
| `rustok-payment` | Default payment submodule of the `ecommerce` family: payment collections, payment attempts and authorization/capture lifecycle in built-in manual/default mode. | `PaymentModule`, `PaymentService`, `dto::*`, `entities::*`. | Mix base payment domain model with provider-specific logic like Stripe instead of a separate next submodule. |
| `rustok-fulfillment` | Default fulfillment submodule of the `ecommerce` family: shipping options, fulfillment records and shipment lifecycle in built-in manual/default mode. | `FulfillmentModule`, `FulfillmentService`, `dto::*`, `entities::*`. | Mix base shipping model with carrier/provider-specific logic instead of a separate next submodule. |
| `rustok-blog` | Blog domain with its own storage, comment backend through `rustok-comments` and author presentation through `rustok-profiles`. REST post/comment handlers use narrow `BlogHttpRuntime` through module-owned Axum routes. | `BlogModule`, `PostService`, `CommentService`, `BlogHttpRuntime`, `graphql::*`, `controllers::*`. | Bypass blog rules directly through `rustok-content` legacy helpers or SQL; move post/comment handlers into `apps/server`. |
| `rustok-forum` | Forum domain and transport adapters, including author presentation through `rustok-profiles`. REST handlers use narrow `ForumHttpRuntime` through module-owned Axum routes. | `ForumModule`, `TopicService`, `ReplyService`, `ForumHttpRuntime`, `graphql::*`, `controllers::*`. | Bypass forum services through server-only handlers or move forum REST handlers into `apps/server`. |
| `rustok-pages` | Pages/menus/blocks and transport adapters. REST page/block handlers use narrow `PagesHttpRuntime` through module-owned Axum routes. | `PagesModule`, `PageService`, `PagesHttpRuntime`, `graphql::*`, `controllers::*`. | Leave pages GraphQL/REST in `apps/server`. |
| `rustok-seo` | Optional SEO module: explicit metadata overrides, canonical storefront read contract, manual redirects, sitemaps, robots, shared SEO capability contracts and cross-cutting admin infrastructure surface. HTTP handlers use narrow `SeoHttpRuntime` through module-owned Axum routes. | `SeoModule`, `SeoService`, `SeoHttpRuntime`, `SeoQuery`, `SeoMutation`, `controllers::*`, `dto::*`. | Duplicate SEO source of truth in storefront hosts, move canonical/redirect resolution to the adapter layer, make host-local metadata precedence, or consider `rustok-seo-admin` a long-term owner screen for other entity editors. |
| `rustok-seo-render` | Support crate for Rust-host last mile: renders `SeoPageContext` to SSR head HTML and serializes typed robots directives without owning SEO runtime. | `render_head_html`, `robots_directives`. | Move SEO storage/routing logic here, tenant policy or reassemble local Rust-host render helpers over the same SEO contract. |
| `rustok-seo-admin-support` | Support crate for owner-module admin SEO: reusable Leptos panels, form helpers and GraphQL transport around shared `rustok-seo` capability contract. | `SeoEntityPanel`, `SeoCapabilityNotice`, `SeoEntityForm`, internal `transport::*`. | Turn it into a central SEO route, keep runtime/storage policy here or move ownership of entity screens from `pages/product/blog/forum` back to `rustok-seo-admin`. |
| `rustok-workflow` | Workflow automation domain: triggers, steps, execution history, webhook ingress, admin UI and transport adapters over platform event infrastructure. HTTP/webhook handlers use narrow `WorkflowHttpRuntime` through module-owned Axum routes. | `WorkflowModule`, `WorkflowService`, `WorkflowHttpRuntime`, `WorkflowEngine`, `graphql::*`, `controllers::*`. | Turn workflow into a separate event transport or consider Alloy a hard dependency of the workflow graph at the registry/runtime level. |
| `rustok-media` | Media lifecycle, storage-facing services and owner-owned transport adapters, including usage statistics. HTTP handlers use narrow `MediaHttpRuntime` through module-owned Axum routes. | `MediaService`, `MediaHttpRuntime`, `load_media_usage_snapshot`, `graphql::{MediaQuery, MediaMutation, MediaUsageStats}`, `controllers::*`. | Keep media resolver/DTO, including `mediaUsage`, or direct media entity imports in `apps/server`. |
| `alloy` | Capability-oriented authoring and automation module: script/source storage, scheduler, Alloy-specific bridges, GraphQL/HTTP surfaces, draft review and hook-oriented integration contracts. Generic Rhai execution is provided by `rustok-sandbox`. HTTP handlers use narrow `AlloyHttpRuntime` through module-owned Axum routes. | `AlloyModule`, `create_default_engine`, `build_alloy_runtime`, `SharedAlloyRuntime`, `AlloyHttpRuntime`, `ScriptEngine`, `ScriptOrchestrator`, `Scheduler`, `ScriptRegistry`, `SeaOrmStorage`, `create_router`. | Own a parallel production sandbox, remove Alloy from `ModuleRegistry`, scatter authoring contracts across host code, or turn capability surface into server-only wiring without module contract. |
| `rustok-index` | Indexing and search contracts. | `IndexModule`, `Indexer`, `LocaleIndexer`. | Build ad-hoc indexing bypassing index contracts. |
| `rustok-search` | Search/read discovery module: search documents, query/runtime, analytics, GraphQL, admin/storefront search UI. | `SearchModule`, `PgSearchEngine`, `SearchQueryPort`, `SearchSuggestionPort`, `graphql::SearchQueryRoot`, `graphql::SearchMutationRoot`, `rustok-search-admin`, `rustok-search-storefront`. | Mix search with `rustok-index`, keep search GraphQL query/mutation/types in `apps/server` or move search query/runtime to host UI/app. |
| `rustok-rbac` | Authorization contracts, tenant policy runtime and RBAC GraphQL role surface. | `RbacModule`, `PermissionResolver`, `PermissionAuthorizer`, `AuthzEngine`, `graphql::RbacQuery`, `graphql::RbacMutation`. | Revert to hardcoded role checks in server code or keep RBAC GraphQL query/mutation/types in `apps/server`. |
| `rustok-tenant` | Tenant lifecycle and module enablement. | `TenantModule`, `TenantService`, tenant DTOs. | Change tenant/module configuration directly in applications or SQL. |
| `rustok-outbox` | `Core` module transactional outbox and relay contracts. It guarantees atomicity between a domain operation and event publication (writing to `sys_events` in one DB transaction); it is not a general background-job runner. | `OutboxModule`, `TransactionalEventBus`, `OutboxRelay`, `OutboxTransport`. | Publish critical cross-module events bypassing the outbox; duplicate event delivery paths through another background-job system. |
| `rustok-iggy` | Event streaming transport runtime. | `IggyTransport`, `PersistentConsumerGroup`, topology/DLQ/replay managers. | Write parallel transport runtime for the same streams in services. |
| `rustok-iggy-connector` | Iggy connection and message I/O abstractions. | `IggyConnector`, `MessageSubscriber`, `ConsumerCursor`, connector configs. | Bypass connector abstraction with direct ad-hoc connections. |
| `rustok-telemetry` | Shared observability foundation layer: telemetry bootstrap, metrics/tracing wiring and shared instrumentation helpers for the host/runtime layer. | `init`, `TelemetryConfig`, `render_metrics`, `current_trace_id`. | Set up disparate telemetry pipelines in different modules or pull domain-specific observability logic here. |
| `rustok-secrets` | Shared secret-reference foundation: tenant-authorized resolver registry, short-lived redacted cache, environment/mounted-file/Vault/Kubernetes/cloud secret resolvers and server-owned endpoint/identity policy. | `SecretRef`, `SecretResolverRegistry`, `SecretAccessPolicy`, `EnvResolver`, `MountedFileResolver`, `VaultResolver`, `KubernetesSecretResolver`, cloud resolver adapters. | Persist plaintext secrets in capability profiles, allow tenants to supply resolver endpoints or cloud identities, or duplicate resolver/cache policy in individual modules. |
| `rustok-mcp` | Thin MCP adapter/server surface over `rmcp`: typed tools, runtime binding, access policy, audit hooks, owner-owned management GraphQL and Alloy-related scaffold/review/apply vertical; persisted storage and DB-backed runtime bridges live in `apps/server`. | `RusToKMcpServer`, `McpManagementPort`, `McpManagementRuntime`, `graphql::{McpQuery, McpMutation}`, `McpRuntimeBinding`, `McpAccessResolver`, `McpAuditSink`, `McpScaffoldDraftStore`, tool re-exports. | Keep MCP GraphQL resolver/DTO in `apps/server`; implement separate MCP entrypoints if the scenario already covers `rustok-mcp`; duplicate upstream MCP/rmcp spec and security docs. |
| `rustok-ai` | Capability crate of the AI host/orchestrator layer: Rig 0.39 provider registry and engine, `AiRouter`, task profiles, policy-governed agent tool loop, persisted provider/task/tool profiles, sessions/runs/traces/approvals, owner-owned GraphQL query/mutation/subscription surface, direct first-party verticals (`alloy_code`, `image_asset`, `product_copy`, `blog_draft`), bounded live streaming through `aiSessionEvents`, embedding/rerank entrypoints and runtime observability. | `ProviderSlug`, `ProviderFeature`, `RigAgentDriver`, `InferenceEngine`, `AiRouter`, `AiHostRuntime`, `McpClientAdapter`, `DirectExecutionRegistry`, `AiManagementService`, `AiMigrationSource`, `graphql::{AiQuery, AiMutation, AiSubscription}`, `AiGraphqlRoleSlugProviderHandle`. | Expand `rustok-mcp` to a model host; place AI GraphQL resolver/DTO in `apps/server`; hide AI authorization behind `MCP_MANAGE`; make MCP a mandatory internal bus; bypass canonical domain services; duplicate AI business UI in host applications instead of capability-owned packages; pass host-wide context inside `rustok-ai`. |
| `rustok-ai-athanor` | First-party Athanor library adapter for the AI-owned RAG boundary; Basic RAG uses Athanor canonical snapshots and Tantivy search, while vector retrieval remains capability-gated. | `AthanorRagAdapter`, `AthanorRagConfig`, `ATHANOR_SOURCE_ID`. | Own a second storage/index implementation, expose SurrealDB/Tantivy handles through AI contracts, or enable vector retrieval before Athanor Phase 9. |
| `flex` | Capability crate of the custom fields system: attached/standalone contracts, field definitions, registry/orchestration helpers and localized attached values; donor ownership remains with consumer modules. The crate is now also formalized as a `capability_only` ghost module in `modules.toml`. | `FlexModule`, `CustomFieldsSchema`, standalone/attached contracts, registry/orchestration helpers from `crates/flex`, module-local docs and plan. | Turn `flex` into a standalone business module, take donor persistence ownership, pull standard modules into dependency on Flex as a mandatory layer or consider server-owned transport surfaces as proof that donor contract ownership moved to `flex`. |
| `rustok-test-utils` | Shared testing-support crate: database setup helpers, mock event bus/transport, fixtures and reusable test helpers for RusToK crates/apps. | `setup_test_db`, `MockEventBus`, `MockEventTransport`, `fixtures::*`, `helpers::*`. | Duplicate the same fixtures and mocks locally in modules instead of using the shared testing layer. |

## RBAC Contract of the Runtime Registry

For modules that actually register in `apps/server/src/modules/mod.rs`, the canonical
RBAC contract is defined by three sources:

- `RusToKModule::permissions()`;
- `RusToKModule::dependencies()`;
- Root `README.md` with `## Purpose`, `## Responsibilities`, `## Entry points`, `## Interactions`
  and a link to `docs/README.md`.

Current RBAC surface ownership:

- `rustok-auth` -> `users:*`
- `rustok-tenant` -> `tenants:*`, `modules:*`
- `rustok-rbac` -> `settings:*`, `logs:*`
- `rustok-content` -> orchestration permissions (`forum_topics:*`, `blog_posts:*` for conversion flows)
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

Alloy remains a capability-oriented layer with `scripts:*` permission surface,
but is included in the runtime registry as a regular optional module.

`flex` is now also included in the runtime registry as a capability-only ghost module and
holds `flex_schemas:*` / `flex_entries:*` permission surface, without taking
donor persistence ownership.

## Update Procedure

When changing ownership, entry points, runtime boundaries or anti-pattern rules of a crate:

1. First update the local `README.md` and `docs/README.md` of the corresponding component.
2. Then synchronize this registry table.
3. If a crate becomes or ceases to be a platform module, simultaneously update `modules.toml`, `rustok-module.toml`, [manifest contract](./manifest.md) and [central registry](./registry.md).


### Rule for Implementation Plans

If a new crate (module/support/capability) is added with a local `docs/implementation-plan.md`,
it must be immediately added to `docs/modules/implementation-plans-registry.md` (`Global board`, unique `Plan ID`).

If a crate is deleted or renamed, the row in `Global board` must be deleted or updated in the same cycle.

## Related Documents

- [Module Platform Overview](./overview.md)
- [Module and Application Registry](./registry.md)
- [Module Documentation Index](./_index.md)
- [`rustok-module.toml` Contract](./manifest.md)
- [Module Documentation Template](../templates/module_contract.md)


## Module and Library Scripts

- Each crate maintains a local `scripts/` folder for crate-specific automation (verify/migration/generation/maintenance).
- Root `scripts/` is used only for shared platform-level orchestration scenarios.
