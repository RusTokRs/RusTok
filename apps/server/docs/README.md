# Documentation: `apps/server`

Local documentation for the main RusToK backend host application. This file captures only the live composition/runtime contract; detailed runbooks, framework notes, and rollout plans live in specialized documents inside this folder and in the central docs.

## Purpose

`apps/server` is the main backend composition root. The application:

- assembles platform modules, shared foundation crates, and host-level capabilities into a single runtime;
- publishes HTTP, GraphQL, Leptos `#[server]`, health, metrics, and related control-plane surfaces;
- remains a thin transport/wiring layer where domain logic has already been extracted into module crates.

## Mandatory platform baseline

For `apps/server`, the mandatory baseline consists of two layers.

Platform `Core` modules:

- `rustok-modules`
- `rustok-auth`
- `rustok-cache`
- `rustok-channel`
- `rustok-email`
- `rustok-index`
- `rustok-outbox`
- `rustok-tenant`
- `rustok-rbac`

Shared foundation / support crates used by the backend platform:

- `rustok-core`
- `rustok-events`
- `rustok-telemetry`
- `rustok-api`
- `rustok-runtime`
- `rustok-web`
- `rustok-fba`
- `rustok-cli-core`

`rustok-modules` is the mandatory module-platform control plane. The server
registers it as a Core module but must not own artifact lifecycle, marketplace
policy or executor selection; sandboxed execution is delegated to the neutral
`rustok-sandbox` foundation.

This is a foundation catalog, not a requirement that the production server binary depends on
every listed crate. `apps/server` should depend only on the crates needed for its active
runtime boundary. For example, Axum response mapping belongs to `rustok-web`, typed runtime
helper extraction belongs to `rustok-runtime` when needed, FBA descriptors belong to
`rustok-fba`, and CLI provider contracts belong to `rustok-cli-core` but command adapters
must stay outside the production HTTP runtime.

`rustok-api` remains the stable contract crate. Executable runtime helpers, Axum boundary
helpers, FBA metadata and CLI provider contracts must live in the dedicated foundation
crates above instead of accumulating in `apps/server` or expanding `rustok-api`.

The tenant-toggle logic applies only to `Optional` modules. `Core` modules should not be treated as switchable by host configuration.

## Runtime surface

- `/api/graphql` and `/api/fn/*` are parallel transport layers; Leptos server functions do not replace the GraphQL API.
- `/api/graphql/schema.graphql` publishes the assembled SDL schema without tenant context for contract tooling; full introspection is exported via POST request to `/api/graphql`. Both snapshots are part of reference artifacts alongside OpenAPI JSON/YAML.
- Embedded UI is no longer considered an unconditional part of the backend binary: `rustok-admin` and `rustok-storefront` are linked only with compile-time feature flags `embed-admin` / `embed-storefront`, not merely by their presence in the workspace.
- Commerce OpenAPI/REST surface on `/admin/*` now includes the first post-order refund contract built on top of `payment-collections`; the host publishes these routes, but the refund lifecycle remains domain-owned in `rustok-payment` and `rustok-commerce`.
- Guest-cart HTTP capability parsing and response emission belong to
  `rustok-cart::guest_access_http`; the server only composes that owner adapter.
- Commerce surface is no longer a compile-time baseline for any server build: `controllers::commerce`, commerce-specific error mapping, and the commerce fragment in OpenAPI live only with `mod-commerce`, so a reduced/headless host can build without the ecommerce transport layer.
- Content REST/OpenAPI surface for `blog`, `forum`, and `pages` is also no longer an unconditional part of the host binary: the corresponding server controllers and OpenAPI fragments are included only with `mod-blog`, `mod-forum`, and `mod-pages`, so a module-sliced build does not have to pull in other content transport dependencies.
- Maintenance binary `migrate_legacy_richtext` belongs to the content storage migration path and is built only with `mod-content`; headless server profiles without the content module should not link this tool.
- `flex` attached field-definition and standalone schemas/entries GraphQL are published via `/api/graphql`, while standalone REST remains at `/api/v1/flex/schemas*`; this is a live tenant-scoped surface with separate `flex_schemas:*` and `flex_entries:*` permission gates. GraphQL query/mutation roots, runtime handle, and DTO belong to `flex::graphql`; roots are included in the schema via `[provides.graphql]` manifest codegen, and the server builder registers only `FlexGraphqlRuntime` on top of concrete `FlexStandaloneSeaOrmService`, `FieldDefRegistry`, DB handle, and cache adapter. REST request/response DTO, command mapping, and view mapping belong to `flex::rest`; attached field-definition row-to-core/view/command mapping, create guardrails, persisted JSON shape helpers, persisted type-name normalization, lifecycle events, and cache invalidation event taxonomy belong to `flex::registry`; the server is the Axum/SeaORM adapter.
- Health/observability surface is published via `/health*` and `/metrics`.
- Module/runtime wiring relies on `modules.toml`, `rustok-module.toml`, and generated host integration.
- Optional module REST/GraphQL surfaces are mounted only from owner-owned crate entrypoints,
  declared in `rustok-module.toml` (`provides.http`, `provides.graphql`) and `modules.toml`.
  OpenAPI fragments for optional modules also live in owner crates and are merged by the server
  as ready-made documents, without enumerating module-owned handlers/DTO in `apps/server`.
  `apps/server/src/controllers/<module>` and `apps/server/src/graphql/<module>` are not
  valid composition points for optional modules; the source guard `module_surface_boundary_guard`
  prevents the return of server-owned shims.
- Shared content canonical query and cross-module conversion mutations also arrive as ready-made
  GraphQL roots from `rustok-content` and `rustok-content-orchestration`; the host does not own their resolver/DTO.
- `rustok-content-orchestration` is registered by the host as `SharedContentOrchestrationService`,
  constructed from explicit DB and `TransactionalEventBus`; conversion GraphQL resolvers read this handle from schema data,
  not through framework-owned application context.
- Auth lifecycle and OAuth GraphQL query/mutation/types belong to `rustok-auth`; auth, OAuth and users REST request/response DTOs and OpenAPI schema derives also belong to `rustok-auth::rest`. The server implements only `AuthLifecyclePort`/`OAuthAdminPort` on top of persisted lifecycle/OAuth/email services, registers the corresponding runtimes in shared runtime extensions, and keeps auth/OAuth/users HTTP controllers as route/extractor/response adapters that re-export or import owner DTOs for Swagger and route compatibility. `AuthLifecycleService` accepts only `ServerRuntimeContext` and an explicit `AuthConfig`. `ServerAuthLifecycleProvider` receives explicit `ServerRuntimeContext`, `AuthConfig`, and mailer handle; `CurrentUser`/`OptionalCurrentUser`, `auth_context` middleware, and RBAC permission extractors use a narrow `ServerAuthRuntime`.
- AI GraphQL/service/direct execution receives `rustok_ai::AiHostRuntime` as schema-owned data: the host passes explicit DB, transactional event bus, module registry, storage, Alloy runtime handles, and owner-composed read ports such as checkout status and product catalog projection. The Leptos admin adapter remains a host-boundary point that assembles this runtime from neutral host state.
- MCP GraphQL query/mutation/types and REST/control-plane DTOs belong to `rustok-mcp`; the server implements `McpManagementPort` on top of persisted `McpManagementService`, registers `McpManagementRuntime`, and keeps HTTP controllers as Axum adapters that import owner DTOs and actor parsing.
- Content GraphQL dataloaders for `nodes`, `node_translations`, and `bodies` live in
  `rustok-content`; `apps/server` only registers owner-owned loader types in the schema builder.
- System GraphQL may publish media usage, but reads it through `rustok-media::load_media_usage_snapshot`,
  without directly importing module-owned media entities.
- Settings and system GraphQL resolvers receive `ServerRuntimeContext`, DB, and schema-owned
  `TransactionalEventBus` through GraphQL data. They do not extract or adapt a framework-global
  application context.
- App runtime rate-limit bootstrap and shared limiter registration use `ServerRuntimeContext`;
- `host::run` owns YAML configuration loading, database connection and graceful shutdown for the pure Axum executable; `services::server_bootstrap` owns startup validation, default-superadmin initialization, runtime/worker setup and router composition.
  Alloy runtime bootstrap also registers `SharedAlloyRuntime` via `ServerRuntimeContext` from an explicit DB handle,
  and Alloy GraphQL receives this runtime as schema-owned data without a framework-global context.
- User complex fields and build progress subscription use schema-owned `DatabaseConnection`
  directly and do not depend on a framework-global host context.
- Host-owned `RootQuery` also does not extract a framework-global context: DB-only read paths use
  schema-owned `DatabaseConnection`, and marketplace/cache paths use `ServerRuntimeContext`.
- All of `apps/server/src/graphql/**`, including `RootMutation`, RBAC writer, and search rate limiter,
  accepts explicit runtime dependencies. `services/graphql_schema.rs` also accepts only
  `ServerRuntimeContext`.
- Server event runtime builds the regular and transactional event bus from `ServerRuntimeContext`; `rustok-outbox`
  exposes no framework-specific composition adapter.
- GraphQL HTTP and WebSocket handlers extract `ServerRuntimeContext`/`ServerAuthRuntime` as Axum
  substate and do not pass framework context into request/connection data.
- Users REST handlers also extract `ServerRuntimeContext` and use `rustok_web::json_response`
  for JSON response formatting.
- Metrics handler and the entire metrics helper pipeline use `ServerRuntimeContext`; the mailer
  state is passed via a separate `ServerEmailRuntime`, and worker handles are read through a scoped shared API.
- Health readiness/runtime handlers use the same runtime contracts for DB, settings, cache,
  event transport, rate limits, worker lifecycle, and email backend state; JSON response formatting
  goes through `rustok_web::json_response`.
- Channel and standalone Flex REST handlers receive `ServerRuntimeContext`; channel JSON response
  formatting goes through `rustok_web::json_response`, and Flex controller tests assemble the same
  neutral runtime fixture.
- Auth REST handlers extract a narrow `ServerAuthRuntime`; password reset and verification endpoints
  additionally receive `ServerEmailRuntime`. The controller receives explicit config, DB, and mailer
  dependencies, and JSON response formatting goes through `rustok_web::json_response`.
- Module guard and server channel contract are typed through `ServerRuntimeContext`; no
  framework-global application context is a request/channel contract for server-owned runtime paths.
- OAuth discovery metadata also uses `ServerAuthRuntime` as the single source of auth config.
- OAuth REST token, authorize/consent, browser-session, and revoke handlers extract `ServerAuthRuntime`
  or `ServerRuntimeContext`; host-wide context does not participate in OAuth request state.
- Marketplace registry/governance REST handlers extract `ServerRuntimeContext`; catalog projection,
  artifact storage and remote executor policy are read through DB/settings/shared handles neutral runtime.
- Swagger document filtering, installer persistence reads, admin DLQ, MCP management/remote tools
  and build WebSocket extract `ServerRuntimeContext`; DB/shared runtime semantics do not depend on a
  framework-global context. Optional module HTTP composition recognizes manifest-declared Axum routers:
  `rustok-blog` is merged from its `HostRuntimeContext` entrypoint.
- Channel runtime surface remains a thin transport around `rustok-channel`: `/api/channels/*` already covers bootstrap, channel CRUD-lite, policy-set/rule authoring endpoints and request-level `resolution_trace` diagnostics, while the resolution pipeline, REST/control-plane DTOs and rule-payload mapping helpers live in the module. Request-level `tenant`, `channel` and `locale` middleware receive `ServerRuntimeContext`, and auth context receives `ServerAuthRuntime`.
- Module-owned event listeners are assembled from `ModuleRegistry` into a common `EventDispatcher`; `apps/server` no longer holds separate host-owned index/search/workflow listener paths.
- Server migrator is the backend composition root for module-owned schema: content-family modules (`blog`, `pages`, `comments`) and search must connect here via `crates/rustok-*/src/migrations`, otherwise external Next/Leptos admin surfaces get a working route shell without the needed tables.
- Product/search title filtering helpers are not server-owned services: product translation
  search predicates stay in the owner/foundation commerce search contract, and
  `apps/server` must not reintroduce `services::product_search`.
- `apps/server` can run as a `full`, `registry_only`, `api`, `admin_ssr`,
  `storefront_ssr`, or `worker` host. API and SSR modes skip background
  workers; the worker mode completes normal runtime bootstrap and starts those
  workers while mounting only health and metrics HTTP surfaces. `host_mode`
  does not replace a deployment profile or choose build artifacts.
- `settings.rustok.runtime.background_workers` governs only maintenance workers on top of the already published HTTP/GraphQL surface. In `development.yaml`, for standalone admin debug, `workflow_cron_enabled` and `seo_bulk_enabled` are disabled so that cron/bulk loops do not saturate the local PostgreSQL pool; the production/default runtime keeps them enabled.
- The generic module-work host supplies artifact queues with an active-tenant enumerator from `rustok-tenant` and a server-composed CAS-backed Rhai/WASM executor before artifact registrations run. The runtime CAS is obtained from the same operation-scoped `ModuleControlPlane` as artifact capability, installation, audit, and sandbox-policy services, so the server does not create an independent artifact staging identity source. Its neutral Rhai bridge and WIT import both reach only `SandboxHost`; `platform.data` and `platform.data.objects` are the composed sandbox capability routes. The latter has bounded single-call object transfer and resumable owner-owned uploads: every decoded base64 chunk is at most 44 KiB, durable private sessions verify ordering, final size, and SHA-256 before publication, and no call exposes a storage key, URL, bucket, or credential. Secret, MCP, and all other capability names remain default-deny until their deployment adapters are explicitly wired. Artifact HTTP is a separate host transport at `/api/artifacts/{installation_id}/{*path}` and artifact commands use `POST /api/artifacts/{installation_id}/commands/{binding_id}`. Both resolve only an active exact installation, check the binding's dynamic RBAC grant, enforce JSON and shared durable binding-idempotency limits through `rustok-modules`, and call the shared sandbox executor rather than mounting artifact routers or injecting dynamic GraphQL fields.
- `development.yaml` keeps `database.max_connections: 30` because heavy admin bootstrap routes like AI control plane resolve several GraphQL root fields in parallel. This is a local debug guardrail for both admin panels, not a new production contract.
- For registry/governance surfaces the server remains the canonical validator of lifecycle policy, `reason` / `reason_code` contract and allowed action set; thin clients may do preflight but do not define policy locally.
- Registry release, publication, and validation adapters obtain their shared transactional governance aggregate through `ModuleControlPlane`; host authorization and artifact storage remain adapter concerns.
- For control-plane composition install/uninstall/upgrade server uses a single orchestration path: manifest validation, CAS-update `platform_state` and enqueue build are executed atomically within one transaction boundary. `manifest_ref` for build is always formed as `platform_state:<revision>`, and `manifest_hash` is computed as SHA-256 canonical JSON snapshot.
- Server module-control-plane adapters construct `ModuleControlPlane` once per scoped operation and obtain composition, lifecycle, installation, or sandbox-policy services only through that owner facade. Tenant module enable/disable and settings persistence therefore load only active distribution defaults and a host-resolved settings schema before invoking its lifecycle writer and rehydrating an ORM view; bypass model-level toggles, server-built policy/catalog/dispatcher state, and server-owned lifecycle writes are not production contracts. `module_operations` records lifecycle status in the module-owned typed model `validated/running/committed/failed`, and pre-validation errors/no-op transitions must not create extraneous journal rows. GraphQL maps canonical lifecycle/recovery facts to transport errors (`BAD_USER_INPUT`, `MODULE_HOOK_FAILED`, `INTERNAL_ERROR`); admin/SSR clients must not remap these fields.
- Effective module availability now uses the facade's `EffectivePolicyService`, which reads tenant overrides and applies Core/default policy inside `rustok-modules`; server guards, GraphQL, and installer adapters consume only the resulting set.
- Static module manifests and catalog entries are parsed by the server host, but `rustok-modules` owns module metadata, resolved static-catalog topology (defaults, dependencies, conflicts, and version compatibility), static manifest-versus-registry comparison, settings-schema, marketplace metadata, static UI classification, UI i18n semantics, static HTTP provider exclusivity, crate-local runtime binding normalization, deployment build-surface semantics, and settings normalization before lifecycle persistence with the effective-enablement and Core invariants; server filesystem, registry-fact extraction, and ORM code are adapters only.
- For post-hook failure recovery/compensation a separate runbook `module-lifecycle-retry-compensation-runbook.md` is used; committed tenant state is not rolled back automatically. The server verifies tenant scope, then delegates retry/compensation policy, state assembly, dispatch, and journaling to `ModuleLifecycleDbWriter` as separate owner lifecycle operations. `retryFailedModuleOperationPostHook` and `compensateFailedModuleOperation` require non-nil UUID `idempotencyKey` values; the owner journals each key tenant-scoped and replays the same attempt without redispatching its hook, while a conflicting key reuse returns `IDEMPOTENCY_CONFLICT`.
- Registry metadata now follows the common multilingual storage contract: publish/release base rows hold language-agnostic state and `default_locale`, while display metadata (`name`, `description`) live in `registry_*_translations`.
- Registry publish-request translations are written only through the owner create/publication transactions; the server no longer retains a parallel translation upsert helper.
- Registry audit payload no longer holds historical runtime fallback: `registry_governance_events.details` is normalized to a typed shape (`stage_key`, nested `owner_transition`, structured principal objects), and the controller maps lifecycle failures from typed `RegistryGovernanceError`, not from substring matching.
- Final registry publication at `POST /v2/catalog/publish/{request_id}/approve` requires a non-nil UUID `Idempotency-Key` header when `dry_run=false`. The owner stores the exact approval command fingerprint with the resulting release, replays only an identical retry, and rejects key reuse with different approval data or legacy published rows without a fingerprint.
- `GET /v2/catalog/publish/{request_id}` remains a machine-readable operator status contract: without bearer auth it returns a status-driven superset `governanceActions`, and with a session-backed user bearer it scopes request-level actions to those actually allowed for that principal.
- Registry artifacts are no longer read or written via a direct filesystem path inside the governance service: persisted state stores only `artifact_storage_key`, upload/validation go through `StorageService`, and `GET /v2/catalog/publish/{request_id}/artifact/download` already works as a storage-backed private download route with presign-or-stream fallback. Validation is selected by immutable artifact origin: platform-built and external-prebuilt uploads retain the bounded metadata-bundle contract, while `alloy_authored` accepts only the bounded canonical Rhai workspace media type. Final Alloy staging still requires that upload checksum to equal the reviewed source-revision digest.
- Alloy release staging is host-composed on both transports: `POST /api/alloy/scripts/{id}/releases/stage` and the manifest-owned GraphQL `stageRelease` mutation require the authenticated tenant's current script revision, `scripts:manage` plus `modules:manage`, reviewed source digest, publish request ID, and idempotency key. They delegate all marketplace writes to `rustok-modules`; final promotion remains the registry governance approval operation, and publish-status `nextStep` now points Alloy-authored requests to this staging boundary instead of implying direct approval.
- Marketplace catalog transport is versioned only at `GET /v1/catalog` and
  `GET /v1/catalog/{slug}`. The server router and both outbound registry clients
  do not probe or serve the removed unversioned `/catalog` compatibility paths.
  Catalog generation fails closed when the active platform composition is
  invalid; it never substitutes a builtin manifest as a legacy fallback. The
  bounded registry client disables redirects and fails startup if its bounded
  client cannot be constructed; it never falls back to an unbounded default
  client.
- The repo-side surface for the current `module-system` is considered closed for the purpose of Admin-driven install/uninstall/upgrade/deploy with progress feedback; ongoing work is limited to targeted verification and docs/audit, while rollout of `modules.rustok.dev` remains an external infra task.
- GraphQL control-plane surface publishes a read/write contract for lifecycle recovery: `moduleOperationRecoveryPlan` and `failedModuleOperationRecoveryPlans` return tenant-scoped retryability/action metadata from `module_operations`, and `retryFailedModuleOperationPostHook` / `compensateFailedModuleOperation` perform recovery only via `ModuleLifecycleService` and `modules:manage`, without raw SQL/bypass rollback.
- GraphQL auth surface `me.permissions` returns a request-scoped RBAC snapshot for headless/mobile UI gating; this does not replace server-side permission enforcement on mutations/queries.
- `PUT` and `DELETE /api/rbac/artifact-permissions/roles/{role_id}` are the
  host transport for RBAC-owned explicit artifact permission grants. They
  require `modules:manage`, derive tenant and actor identities from trusted
  request context, and accept only an exact admitted installation, registered
  permission key, and idempotency key. The routes never write static
  `role_permissions` and do not auto-grant anything at installation time.
- MCP remote bootstrap surface `POST /api/mcp/runtime/bootstrap` performs a server-owned token-to-runtime-binding handshake for non-stdio transport: accepts Bearer/plaintext MCP token, returns tenant/client/token binding and effective access context, updates last-used timestamps and writes an audit event `remote_session_bootstrapped` with correlation id. Remote tool transport is complemented by `POST /api/mcp/runtime/tools/call` for JSON invocations and `POST /api/mcp/runtime/tools/stream` for SSE invocations of core registry tools (`mcp_health`, `mcp_whoami`, `list_modules`, `query_modules`, `module_exists`, `module_details`) and Alloy scaffold draft tools (`alloy_scaffold_module`, `alloy_review_module_scaffold`, `alloy_apply_module_scaffold`) with the same persisted token binding, policy enforcement, and audit trail. Scaffold tools in remote transport use the server-owned persisted draft store, so stage/review/apply go through `mcp_scaffold_drafts`, tenant/client binding, and audit surface, rather than process-local memory of the MCP runtime.
- The hybrid product installer is introduced through `rustok-installer` and
  `rustok-installer-persistence`. The latter owns the shared SeaORM database,
  durable-state, and bootstrap-writer adapter. `/api/install/*` delegates plan,
  receipt, preflight and execution semantics to these shared crates; the web wizard
  must not become a separate bootstrap implementation. The server binary has
  no `install` parser. `rustok-installer-cli` owns `install plan`, `install
  preflight`, `install apply`, `install status`, and `seed apply`; apply uses
  the shared executor-port extraction rather than server code.
- When `rustok.build.enabled=true`, the HTTP installer also composes the
  server-owned distributed deployment adapter. It derives a role-specific
  `RoleBuildPlan`, executes/publishes the release, requires it to become
  active, and persists a deployment receipt before installer verification.
- The HTTP adapter for the Leptos wizard is available as a thin surface on top of the same
  pipeline: `GET /api/install/status`, `POST /api/install/plan`,
  `POST /api/install/preflight`, `POST /api/install/apply`,
  `GET /api/install/jobs/{job_id}`, and
  `GET /api/install/sessions/{session_id}/receipts`. HTTP `apply` starts a
  background job and returns `202 Accepted` with `job_id`; the wizard must poll
  the job status and read persisted receipts for progress UI. Mutating HTTP install
  requests support a setup-token guard via
  `RUSTOK_INSTALL_SETUP_TOKEN` and header `x-rustok-setup-token` or
  `Authorization: Bearer <token>`; production HTTP apply without a setup token
  is rejected. `/api/install/*` intentionally bypasses the tenant resolution middleware,
  because the first install runs before a tenant context is created. The CLI remains
  the canonical automation path.
- The tenant middleware resolution contract is fixed by integration tests in
  `apps/server/tests/tenant_resolver_invariants_test.rs`: the active tenant
  is resolved via `header`, `host`, and `subdomain`; a disabled tenant consistently
  returns `403`; a missing tenant returns `404`.
- The provisioning/deprovisioning path must trigger cache invalidation
  (`invalidate_tenant_cache_by_uuid/slug/host`) after create/update/deactivate/
  domain-change operations: the positive cache lives for `TENANT_CACHE_TTL=300s`,
  the negative cache miss lives for `TENANT_NEGATIVE_CACHE_TTL=60s`, so without
  invalidation stale resolver state is acceptable only within these TTLs.
  The regression matrix additionally captures lifecycle scenarios: stale positive
  cache after deactivate/update, negative cache after create-like flow, host
  cache after domain-change, and UUID invalidation.

## Responsibility boundaries

`apps/server` is responsible for:

- transport adapters, middleware, request/runtime context, and host wiring;
- the overall GraphQL schema surface and Leptos server-function entrypoints;
- composition of owner-owned AI GraphQL roots from `rustok-ai` and the narrow RBAC persistence adapter
  `AiGraphqlRoleSlugProvider`; AI resolver/DTO surface is not placed in `apps/server`;
- composition of `rustok-media::MediaQuery`, including owner-owned `mediaUsage`; media resolver/DTO
  are not placed in server `SystemQuery`;
- composition of dashboard order statistics via `rustok-order::load_order_stats_snapshot` when
  `mod-order` is enabled; SQL and DTO for order analytics belong to `rustok-order`, not
  `apps/server::RootQuery`;
- composition of dashboard post statistics via `rustok-content::load_post_stats_snapshot` when
  `mod-content` is enabled; SQL and DTO for content analytics belong to `rustok-content`,
  not `apps/server::RootQuery`;
- host-level user dashboard statistics and recent user activity via
  `services::dashboard_user_activity`; `RootQuery` only maps service DTO into GraphQL DTO and does not
  contain SQL/read-model logic for these dashboard widgets;
- manifest-driven composition of owner-owned `flex::graphql::FlexQuery` / `flex::graphql::FlexMutation` and
  registration of the concrete persistence adapter in `FlexGraphqlRuntime`; standalone Flex
  and attached field-definition resolver/DTO/error/RBAC/event mapping is not placed in `apps/server`;
- Axum REST handler for standalone Flex, which uses owner-owned `flex::rest`
  request/response DTO, request-to-command mapping, and view mapping; the server does not own Flex REST contract types;
- SeaORM adapter for standalone Flex, which stores persisted schema rows,
  but performs fields_config parsing/schema build/serialization, localized key derivation, row-to-view mapping, normalize/defaults/strip/validate, shared/localized split, read resolution, and PATCH merge via owner-owned helpers from `flex::standalone`;
- field-definition registry bootstrap, which registers donor persistence adapters for `user`/`order`/`product`/`topic`,
  but delegates row-to-core `FieldDefinition` mapping, `FieldDefinitionView` shape mapping, command-to-adapter-input conversions, persisted JSON shape helpers, and lifecycle policy/event construction to owner-owned `flex::registry`;
- bootstrap of the common module-owned event runtime via `ModuleRegistry` and `EventDispatcher`;
- health/runtime guardrails, build/release orchestration, and operator control-plane endpoints;
- installer HTTP/CLI adapters on top of `rustok-installer`, install locks, and
  persisted installer session receipts;
- RBAC enforcement, auth/session integration, and host-level observability.

`apps/server` must not:

- duplicate module-owned domain services, storage, and permission logic;
- replace module interaction contracts with its own ad hoc conventions;
- turn a cron, relay worker, or maintenance task into a pseudo-`event_listener` bypassing the module runtime contract;
- break the dual-path contract between GraphQL and `#[server]` when adding a new internal path.

## Health and runtime guardrails

- [health.md](./health.md) is the canonical document for readiness, runtime guardrails, `registry_only` smoke, and rollout evidence.
- `apps/server` must explicitly distinguish between `DeploymentProfile` and `settings.rustok.runtime.host_mode`.
- For reduced hosts, the health/runtime surface must describe the actually deployed runtime, not the full monolith by default.

## Verification

Minimum local verification path for changes in `apps/server`:

- targeted `cargo check` and `cargo test` on affected crates and transport slices;
- for build/profile wiring changes, separately verify at least one reduced build without embedded UI and one module-sliced profile such as `mod-commerce`-only or no-commerce content host, so that the server binary does not pull in extraneous surface dependencies;
- `cargo xtask module validate <slug>` for modules whose host wiring or manifest contract has changed;
- targeted contract checks for GraphQL, REST, server functions, and health/runtime surface;
- separate check of health/runtime paths if deployment profile, `host_mode`, or remote executor/runtime guardrails are affected.
- export API contracts via `node scripts/verify/export-reference-artifacts.mjs artifacts/reference`; the Bash wrapper `scripts/verify/export-reference-artifacts.sh` is intended for CI and Unix environments.

## Related documents

- [Health and runtime guardrails](./health.md)
- [Backend module guides](../../../docs/backend/README.md)
- [Library stack](./library-stack.md)
- [Axum runtime and operations CLI boundary](../../../DECISIONS/2026-07-02-axum-runtime-and-ops-cli-boundary.md)
- [Event transport contract](./event-transport.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Manifest layer contracts](../../../docs/modules/manifest.md)
- [Runbook retry/compensation lifecycle hook failures](./module-lifecycle-retry-compensation-runbook.md)
- [Documentation map](../../../docs/index.md)
