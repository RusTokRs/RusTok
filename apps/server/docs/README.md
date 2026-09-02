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

Effective module policy remains owner-resolved in `rustok-modules`. When a
request has a channel resolution, the server/channel adapter forwards a
validated `ModuleEffectivePolicyChannelInput` to
`EffectiveModulePolicyService::resolve_for_channel`; channel lookup and
channel-owned storage stay in `rustok-channel`.
Operational maintenance is forwarded through the same context facade as a
revisioned module-scoped or global snapshot; active maintenance denies serving
without changing tenant enablement rows.
Node readiness is forwarded as the same owner context and must observe the
base policy revision before the server accepts the final policy revision.
For dynamic artifact execution, every serving server additionally requires a
stable non-nil `RUSTOK_ARTIFACT_NODE_ID` UUID. The server passes the exact
already-resolved installation to `rustok-modules`' durable observed-assignment
gate before non-lifecycle CAS/sandbox execution. The gate compares payload
kind and admitted media type as well as the digests and revisions. A missing,
degraded, stale, or identity-mismatched assignment denies execution; the
sandbox worker receives only the scoped execution request and does not gain
database, module-policy, AI, or product access. The runtime uses a bounded
digest-keyed verified CAS cache (`RUSTOK_ARTIFACT_NODE_CACHE_MAX_BYTES`, default
64 MiB) and rehashes every hit before any sandbox request. The authenticated
agent transport is deployed independently through
`rustok-artifact-node-controller`; deployment topology and agent materialization
remain separate unfinished control-plane work.

## Runtime surface

- `/api/graphql` and `/api/fn/*` are parallel transport layers; Leptos server functions do not replace the GraphQL API.
- `/api/graphql/schema.graphql` publishes the assembled SDL schema without tenant context for contract tooling; full introspection is exported via POST request to `/api/graphql`. Both snapshots are part of reference artifacts alongside OpenAPI JSON/YAML.
- Embedded UI is no longer considered an unconditional part of the backend binary: `rustok-admin` and `rustok-storefront` are linked only with compile-time feature flags `embed-admin` / `embed-storefront`, not merely by their presence in the workspace.
- Commerce OpenAPI/REST surface on `/admin/*` now includes the first post-order refund contract built on top of `payment-collections`; the host publishes these routes, but the refund lifecycle remains domain-owned in `rustok-payment` and `rustok-commerce`.
- Guest-cart HTTP capability parsing and response emission belong to
  `rustok-cart::guest_access_http`; the server only composes that owner adapter.
- Commerce surface is no longer a compile-time baseline for any server build: `controllers::commerce`, commerce-specific error mapping, and the commerce fragment in OpenAPI live only with `mod-commerce`, so a reduced/headless host can build without the ecommerce transport layer.
- Content REST/OpenAPI surface for `blog`, `forum`, and `pages` is also no longer an unconditional part of the host binary: the corresponding server controllers and OpenAPI fragments are included only with `mod-blog`, `mod-forum`, and `mod-pages`, so a module-sliced build does not have to pull in other content transport dependencies.
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
- Build history and active-build GraphQL/native transports use the read-only
  host-composed `rustok_build::SharedBuildControl`; transports do not construct
  `BuildService` directly. The
  control returns typed framework-neutral `rustok-api` snapshots, so GraphQL
  only wraps canonical facts and does not map SeaORM models.
- Effective-module-policy snapshots carry the owner-produced tenant and exact
  policy-revision cache identity. Server consumers must match both fields;
  TTL or process generation alone never makes a cached authorization decision
  current.
- Production release admission, rollout, activation, recovery, and related
  events are owned by `rustok-modules`; build events contain build facts only.
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
- Per-registry marketplace freshness is projected through
  `SharedModuleMarketplaceCatalog`, not directly from the server health
  aggregate. The operator-only GraphQL/native contract exposes stable logical
  registry identity, typed status, last success, and consecutive failures and
  omits deployment endpoints and remote error content.
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
  does not replace a deployment profile or choose build artifacts. Artifact
  HTTP and command dispatch resolve the shared effective module policy before
  invoking a binding and fail closed for a disabled or denied module; they do
  not reconstruct tenant enablement from transport-local SQL.
- `settings.rustok.runtime.background_workers` governs only maintenance workers on top of the already published HTTP/GraphQL surface. In `development.yaml`, for standalone admin debug, `workflow_cron_enabled` and `seo_bulk_enabled` are disabled so that cron/bulk loops do not saturate the local PostgreSQL pool; the production/default runtime keeps them enabled.
- The generic module-work host supplies artifact queues with an active-tenant
  enumerator from `rustok-tenant` and a server-composed CAS-backed Rhai/WASM
  executor before artifact registrations run. The runtime CAS is obtained from
  the same operation-scoped `ModuleControlPlane` as artifact capability,
  installation, audit, and sandbox-policy services, so the server does not
  create an independent artifact staging identity source. Executor registration
  records placement explicitly and rejects same-kind fallback registration.
  Artifact and Alloy Rhai now share one mTLS-authenticated
  `rustok-sandbox-worker` client after exact readiness and register only the
  `isolated_worker` placement; connection, attestation/cgroup readiness,
  protocol, cancellation, or worker failure has no in-process fallback.
  Wasmtime remains explicitly `in_process` under its accepted component-runtime
  threat model. The canonical Kubernetes renderer now supplies the
  digest-pinned gVisor/Kata, mTLS-probed, restricted-network worker profile;
  retained cluster enforcement and supervisor evidence remains open.
  The neutral Rhai bridge and WIT import both reach only `SandboxHost`;
  `platform.data`, `platform.data.objects`, `platform.secrets`, and
  `platform.mcp` are the composed sandbox capability routes. Object transfer is
  bounded and larger writes use resumable owner-owned uploads: every decoded
  base64 chunk is at most 44 KiB, durable private sessions verify ordering,
  final size, and SHA-256 before publication, and no call exposes a storage key,
  URL, bucket, or credential. The secret route returns only a logical reference
  and revision after an immediate exact installation, lifecycle, grant-revision,
  grant, and derived-scope recheck; secret values and resolver identities remain
  host-only. The MCP route maps only the stable `rustok` alias to the
  owner-defined read-only registry tools, derives an MCP service identity from
  the admitted artifact installation, reapplies the owner access policy, and
  requires redacted durable audit before invocation. It accepts no endpoint,
  token, credential, transport, or discovery input. All other unregistered
  capability names remain default-deny. Artifact HTTP is a separate host
  transport at `/api/artifacts/{installation_id}/{*path}` and artifact commands
  use `POST /api/artifacts/{installation_id}/commands/{binding_id}`. Host-rendered
  declarative actions/forms use only
  `POST /api/artifacts/{installation_id}/ui/contributions/{contribution_id}/execute`;
  that route resolves the contribution to its exact admitted Command binding
  before it joins the generic command execution path. Its companion
  `GET /api/artifacts/{installation_id}/ui/contributions` reads the effective
  locale only from server middleware, filters contributions by their dynamic
  RBAC permission, and returns only host-safe localized declarative metadata.
  Its response uses the shared `rustok_api::ArtifactUiContributionView` DTO,
  not a server-local descriptor projection. Headless GraphQL exposes the same
  projection through `artifactUiContributions(installationId)`; both
  transports share one server adapter, receive locale only from resolved
  request context, and omit unavailable exact-locale contributions. GraphQL
  `executeArtifactUiAction(installationId, contributionId, input,
  idempotencyKey)` is the corresponding headless action path. It resolves the
  contribution to its admitted Command binding before using the same effective
  policy, dynamic-RBAC, durable-idempotency, sandbox-dispatch, and audit path
  as REST; it cannot select a raw binding ID.
  Callers cannot select a locale; an unavailable exact locale omits the
  contribution without fallback. Its audit companion
  `GET /api/artifacts/{installation_id}/ui/contributions/{contribution_id}/audit`
  resolves and authorizes that same contribution before returning its redacted
  binding-specific evidence. Headless GraphQL exposes the same evidence through
  `artifactUiActionAudit(installationId, contributionId)` using that exact
  contribution-resolution and dynamic-RBAC path. Both audit transports project
  the one `rustok_api::ArtifactBindingExecutionAuditEntry` DTO and do not expose
  a raw binding selector. All five routes resolve only an active exact
  installation and check the binding's dynamic RBAC grant. The three execution
  routes enforce JSON and shared durable binding-idempotency limits through
  `rustok-modules` and call the shared sandbox executor rather than mounting
  artifact routers or injecting dynamic GraphQL fields; the audit route does
  not dispatch the artifact.
- `development.yaml` keeps `database.max_connections: 30` because heavy admin bootstrap routes like AI control plane resolve several GraphQL root fields in parallel. This is a local debug guardrail for both admin panels, not a new production contract.
- Routed artifact HTTP and UI-command bindings accept an optional non-nil UUID `Idempotency-Key` only when their admitted binding declares an idempotency policy. The server creates the tenant-matched `ModuleCommandContext`; the owner receipt binds actor, trace, correlation, request digest, and UUID key before it can replay a stored binding response.
- For registry/governance surfaces the server remains the canonical validator of lifecycle policy, `reason` / `reason_code` contract and allowed action set; thin clients may do preflight but do not define policy locally.
- Registry release, publication, and validation adapters obtain their shared transactional governance aggregate through `ModuleControlPlane`; host authorization and artifact storage remain adapter concerns. A live validation enqueue requires a non-nil UUID `Idempotency-Key`: the server derives one platform-scoped `ModuleCommandContext`, and the owner replays only an exact request revision, actor, trace, correlation, principal, and rejected-retry-policy match from its durable enqueue receipt. A remote validation claim returns the owner-issued `requestRevision`; a terminal runner request must return it as `expectedRequestRevision`, so claim and terminal stage transitions use the same request CAS. Heartbeats only renew an already-issued operational lease.
- For control-plane composition install/uninstall/upgrade server uses a single orchestration path: full manifest validation, CAS-update `platform_state`, owner-operation receipt completion, and build enqueue are executed atomically within one transaction boundary. The authenticated transport supplies a positive expected revision and UUID idempotency key; the owner admits the immutable command before manifest adaptation and exact retries replay the original build. `platform_state.manifest_hash` is the SHA-256 canonical composition snapshot digest, `manifest_ref` is always `platform_state:<revision>`, and the build's `manifest_hash` is the distinct immutable execution-request identity over the snapshot, composition digest, deployment profile, and execution plan.
- Server module-control-plane adapters construct `ModuleControlPlane` once per scoped operation and obtain composition, lifecycle, installation, or sandbox-policy services only through that owner facade. Static tenant enablement, normalized settings, post-hook retry, and compensation share the owner-owned `module_static_tenant_lifecycle` aggregate: its inherited/default revision is zero, each authenticated command carries a tenant-matched `ModuleCommandContext` plus its aggregate revision, and its execution claim fails concurrent work closed. The lifecycle journal and settings receipt retain the context actor, trace, correlation, and idempotency evidence; changed-context replay fails closed. The settings writer completes its exact owner-operation receipt in the same transaction as settings persistence, revision advancement, and claim release; direct model writes, server-built lifecycle state, and native settings write fallbacks are not production contracts. GraphQL derives tenant, actor, trace, and `modules:manage`, requires a non-nil UUID plus non-negative expected revision, and returns the owner-issued next revision. `module_operations` retains hook lifecycle status as `validated/running/committed/failed`; only pre-validation failures create no journal row. GraphQL maps canonical lifecycle/recovery facts to transport errors (`BAD_USER_INPUT`, `IDEMPOTENCY_CONFLICT`, `REVISION_CONFLICT`, `MODULE_HOOK_FAILED`, `INTERNAL_ERROR`), with idempotency conflict non-retryable; admin/SSR clients must not remap these fields.
- GraphQL `tenantModules` reads bounded explicit override/settings snapshots
  through the module owner facade. It does not query `tenant_modules` or infer
  effective availability; enabled/denied state remains owned by
  `ModuleEffectivePolicy`.
- GraphQL `artifactTenantLifecycle` and `setArtifactTenantEnabled` expose the
  owner-issued intent state for one admitted Optional artifact. The read
  returns the exact next CAS precondition (one for inherited enabled intent);
  the mutation derives tenant, actor, and `modules:manage` authority from the
  authenticated context and requires a non-nil installation UUID, positive
  expected revision, non-empty reason, and idempotency UUID. It delegates
  directly to the owner lifecycle transaction, so audit/outbox facts and exact
  replay remain owner-owned and raw admission or storage errors never cross
  GraphQL.
- GraphQL `activateTenantArtifact`, `deactivateTenantArtifact`,
  `uninstallTenantArtifact`, and `rollbackTenantArtifact` expose the matching
  owner lifecycle commands only for the authenticated tenant. Their scope is
  constructed from `TenantContext`, never accepted from the client; actor and
  `modules:manage` are derived by the same boundary. Rollback permits no
  target-installation selector: the owner can select only its retained direct
  predecessor after re-evaluating the supplied target capability-grant
  revision and migration rollback mode. There is deliberately no platform
  lifecycle GraphQL endpoint: a tenant-derived permission cannot authorize a
  platform-wide operation, and the transport remains fail-closed until a
  separate platform operator authority is defined. The shared GraphQL module
  authorization extension classifies the lifecycle snapshot as `modules:read`
  and every lifecycle mutation as `modules:manage` before resolver execution;
  resolver-level owner checks remain a second boundary. The same pre-resolver
  guard classifies the immutable composition snapshot as read and the
  operational marketplace registry freshness projection as manage, matching
  their owner-resolver checks. `enabledModules`, the tenant availability list
  consumed by admin navigation, is also `modules:read`-gated at both layers.
- GraphQL build history and active-build reads use the host-composed read-only
  `rustok-build::SharedBuildControl`. Pagination limits are enforced by the
  owner, and the transport does not import build persistence entities.
- Native admin and GraphQL marketplace reads share the host-composed
  `SharedModuleMarketplaceCatalog`. The server adapter combines active local
  composition with configured remote registry providers, applies durable
  governance projection once, and attaches the owner-derived registry
  lifecycle snapshot for detail reads. Transport adapters perform DTO mapping
  only and do not scan the workspace or recreate governance policy. A
  production remote registry set is configured as a JSON array of unique,
  canonical stable identities and HTTPS endpoints. Endpoints may contain no
  embedded credentials, query, or fragment, and identity is independent from
  endpoint location. List reads may retain the local catalog during a remote
  outage, while per-provider health becomes explicitly degraded in
  `/health/ready`; remote detail transport/contract failures fail closed while
  an explicit remote `404` remains not-found. Slug collisions between
  non-local providers fail instead of selecting one by order, while the local
  manifest remains the explicit authority for a matching compiled module.
  Active registry-governed versions receive their immutable installable
  artifact contract only from the module owner projection. The adapter fails
  the catalog read when an active release lacks that contract; it never
  reconstructs registry, descriptor, source-lineage, or trust evidence from a
  checksum or server ORM row.
- Effective module availability now uses the facade's `EffectivePolicyService`, which reads tenant overrides and applies Core/default policy inside `rustok-modules`. The service returns the owner decision with its deterministic revision, typed facts, and denial reasons; server guards, GraphQL, and installer adapters consume only its enabled projection.
- Static module manifests and catalog entries are parsed by the server host, but `rustok-modules` owns module metadata, resolved static-catalog topology (defaults, dependencies, conflicts, and version compatibility), static manifest-versus-registry comparison, settings-schema, marketplace metadata, static UI classification, UI i18n semantics, static HTTP provider exclusivity, crate-local runtime binding normalization, deployment build-surface semantics, and settings normalization before lifecycle persistence with the effective-enablement and Core invariants; server filesystem, registry-fact extraction, and ORM code are adapters only.
- For post-hook failure recovery/compensation a separate runbook `module-lifecycle-retry-compensation-runbook.md` is used; committed tenant state is not rolled back automatically. The server verifies tenant scope and derives the complete authenticated `ModuleCommandContext`, then delegates retry/compensation policy, state assembly, dispatch, and journaling to `ModuleLifecycleDbWriter` as separate owner lifecycle operations. `retryFailedModuleOperationPostHook` and `compensateFailedModuleOperation` require non-nil UUID `idempotencyKey` values; the owner journals the context tenant-scoped and replays only the same actor/trace/correlation/idempotency evidence without redispatching its hook, while a changed context returns `IDEMPOTENCY_CONFLICT`.
- Registry metadata now follows the common multilingual storage contract: publish/release base rows hold language-agnostic state and `default_locale`, while display metadata (`name`, `description`) live in `registry_*_translations`.
- Registry publish-request translations are written only through the owner create/publication transactions; the server no longer retains a parallel translation upsert helper.
- Registry audit payload no longer holds historical runtime fallback: `registry_governance_events.details` is normalized to a typed shape (`stage_key`, nested `owner_transition`, structured principal objects), and the controller maps lifecycle failures from typed `RegistryGovernanceError`, not from substring matching.
- Final registry publication at `POST /v2/catalog/publish/{request_id}/approve` requires a non-nil UUID `Idempotency-Key` header when `dry_run=false`. The session-backed transport derives one platform-scoped `ModuleCommandContext`; the owner binds its actor UUID to the structured approval principal and persists actor, trace, correlation, idempotency, and approval facts with the resulting release. It replays only an identical retry and rejects changed context, changed approval data, or a published row without a receipt.
- Live registry review transitions at `POST /v2/catalog/publish/{request_id}/reject`, `request-changes`, `hold`, and `resume` likewise require a non-nil `Idempotency-Key`. They derive a platform-scoped context from the session and telemetry, use one immutable owner receipt ledger keyed within the publish request, and replay only the same operation kind, revision, actor, trace, correlation, reason, and reason code. Dry-run previews remain non-mutating and do not reserve an operation key.
- Live registry release yanking likewise requires a non-nil `Idempotency-Key`. The session and telemetry derive the global-registry command context; the owner locks the exact release and records actor, trace, correlation, principal, privilege, reason, and reason code in an immutable receipt. Only the exact retry succeeds after a yank; changed input fails closed.
- Live registry owner transfer likewise requires a non-nil `Idempotency-Key`. The owner locks the slug binding, records the previous and new owner with actor, trace, correlation, privilege, and reason facts, and accepts only an identical retry.
- Live registry publish-request creation requires a non-nil `Idempotency-Key`. The request ID remains the deterministic business-command identity, while a separate immutable create receipt binds the authenticated actor, trace, correlation, principal, and privilege facts for exact replay.
- Live registry artifact upload requires a non-nil `Idempotency-Key` before the host stores bytes. The platform-scoped context accompanies both owner slot authorization and the durable attach; the attach transaction locks the request and records metadata, storage result, actor, trace, correlation, and privilege in one immutable receipt. Only that exact retry returns the committed attachment result.
- `GET /v2/catalog/publish/{request_id}` remains a machine-readable operator status contract: without bearer auth it returns a status-driven superset `governanceActions`, and with a session-backed user bearer it scopes request-level actions to those actually allowed for that principal.
- Registry artifacts are never read or written through server-local filesystem paths: persisted state stores only `artifact_storage_key`, owner services call the shared `ObjectStore` directly, and `GET /v2/catalog/publish/{request_id}/artifact/download` uses native signing with stream fallback. Validation is selected by immutable artifact origin: platform-built and external-prebuilt uploads retain the bounded metadata-bundle contract, while `alloy_authored` accepts only the bounded canonical Rhai workspace media type. Final Alloy staging still requires that upload checksum to equal the reviewed source-revision digest.
- `POST /v2/catalog/publish/{request_id}/author-signature` accepts an authenticated platform-scoped command context plus immutable signature reference, SHA-256, signer, policy revision, and body idempotency key. It never accepts the signed artifact digest: `rustok-modules` locks the publish request, derives its current attached-artifact checksum, persists the signature digest in the immutable evidence fact verified at final publication, and records an exact-replay receipt over the context and signature facts before returning the owner status snapshot.
- Alloy release staging is host-composed on both transports: `POST /api/alloy/scripts/{id}/releases/stage` and the manifest-owned GraphQL `stageRelease` mutation require the authenticated tenant's current script revision, `scripts:manage` plus `modules:manage`, reviewed source digest, publish request ID, and idempotency key. They delegate all marketplace writes to `rustok-modules`; final promotion remains the registry governance approval operation, and publish-status `nextStep` now points Alloy-authored requests to this staging boundary instead of implying direct approval.
- Alloy marketplace import is host-composed on authenticated HTTP `POST /api/alloy/releases/import`, GraphQL `importPublishedRelease`, and remote MCP `alloy_import_published_release` on `/api/mcp/runtime/tools/call` and `/api/mcp/runtime/tools/stream`. The server derives tenant and actor identity, requires `scripts.manage` plus `modules.manage`, and injects an owner-backed source provider that accepts only the exact active publication projection and verified digest-pinned CAS workspace. The adapter cannot read catalog DTOs, upload objects, or mutable OCI tags. Generic stdio MCP does not advertise this tool because it lacks the server's durable tenant-bound composition. Imported-draft preview and workspace-test execution resolve the immutable parent reference through the owner to the exact active tenant installation and policy on every run; inactive, disabled, stale, mismatched, or missing parent state fails closed without a default policy fallback.
- Marketplace catalog transport exposes only the current `GET /catalog` and
  `GET /catalog/{slug}` contracts. The server router and outbound registry
  clients do not probe or serve version-suffixed compatibility paths.
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
- MCP remote bootstrap surface `POST /api/mcp/runtime/bootstrap` performs a server-owned token-to-runtime-binding handshake for non-stdio transport: accepts Bearer/plaintext MCP token, returns tenant/client/token binding and effective access context, updates last-used timestamps and writes an audit event `remote_session_bootstrapped` with correlation id. Remote tool transport is complemented by `POST /api/mcp/runtime/tools/call` for JSON invocations and `POST /api/mcp/runtime/tools/stream` for SSE invocations of core registry tools (`mcp_health`, `mcp_whoami`, `list_modules`, `query_modules`, `module_exists`, `module_details`), Alloy scaffold draft tools (`alloy_scaffold_module`, `alloy_review_module_scaffold`, `alloy_apply_module_scaffold`), remote-only Alloy script authoring tools, and source-free deleted-evidence retention tools. Script authoring and retention authority resolve the durable binding, require `scripts.manage`, verify identity-to-binding tenant equality, derive actor provenance, and construct the same `SharedAlloyRuntime::scoped` owner service used by HTTP/GraphQL. They return source-redacted authoring evidence or source-free retention state, and generic stdio/in-process MCP cannot advertise those owner-bound names. Scaffold tools use the server-owned persisted draft store, so stage/review/apply go through `mcp_scaffold_drafts`, tenant/client binding, and audit surface, rather than process-local memory of the MCP runtime.
- The hybrid product installer is introduced through `rustok-installer` and
  `rustok-installer-persistence`. The latter owns the shared SeaORM database,
  durable-state, and bootstrap-writer adapter. `/api/install/*` delegates plan,
  receipt, preflight and execution semantics to these shared crates; the web wizard
  must not become a separate bootstrap implementation. The server binary has
  no `install` parser. `rustok-installer-cli` owns `install plan`, `install
  preflight`, `install apply`, `install status`, and `seed apply`; apply uses
  the shared executor-port extraction rather than server code.
- The HTTP adapter binds one portable instance root before preflight. Production
  requires host-selected `RUSTOK_INSTANCE_ROOT`; local storage and filesystem
  release materialization derive `storage` and `releases/platform/sha256`
  beneath it through `rustok-runtime::InstanceLayout`.
- For distributed plans the HTTP adapter discards any client-supplied bundle
  identity. It accepts the locally configured
  `RUSTOK_INSTALL_DISTRIBUTION_RELEASE_ID`, resolves the exact preparation,
  bundle-root, and role-set identity from the current admitted
  `rustok-modules` ledger. A fresh target instead configures
  `RUSTOK_INSTALL_BASE_DISTRIBUTION_RECEIPT` and
  `RUSTOK_INSTALL_BASE_DISTRIBUTION_PUBLIC_KEY`; the host strictly verifies the
  bounded signed receipt and executable composition before mutation. The two
  authority sources are mutually exclusive, and all client-supplied bundle or
  receipt values are discarded. The old
  per-role `rustok-build` activation adapter has been removed. Distributed
  apply remains unavailable until the single owner-controlled rollout adapter
  and verified fresh-bootstrap owner-ledger import are composed.
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

## Shared richtext frame assets

When the server hosts the Leptos admin static fallback, `/richtext/frame` and
`/richtext/frame/<hashed-asset>` are immutable capability assets copied from
`@rustok/richtext`. The security middleware recognizes this narrow path and
uses the frame CSP, `X-Frame-Options: SAMEORIGIN`, `no-referrer`, and disabled
browser capabilities. It does not create a second editor bundle or pass auth,
tenant, locale, or document data to the frame URL.

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
