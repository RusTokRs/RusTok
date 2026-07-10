# RusToK Server — Loco.rs Feature Support & Anti-Duplication Matrix

> Status: deprecated inventory. Active roadmap: [Loco RS Exit Plan](../../../docs/architecture/loco-exit-plan.md).

**Date:** 2026-02-18  
**Loco.rs Version:** `0.16` (workspace dependency)  
**Purpose:** preserve a complete overview of implemented server functionality (including auth and domain APIs), while explicitly establishing boundaries: where we use Loco, where we consciously use custom code.

---

## 1) Complete matrix: Loco capability vs RusToK implementation

| Capability area | Loco support | Currently implemented | Source of truth (target) | Duplication risk | Decision |
|---|---|---|---|---|---|
| Application hooks (`Hooks`) | ✅ | `boot`, `routes`, `after_routes`, `truncate`, `register_tasks`, `initializers`, `connect_workers`, `seed` | **Loco hooks** | Low | Keep on Loco |
| Application configuration | ✅ | `development.yaml`/`test.yaml`, `auth.jwt`, custom `settings.rustok.*` | **Loco config + typed project settings** | Low | Keep as is |
| REST/GraphQL routing | ✅ | `AppRoutes` + Axum layers, GraphQL endpoint | **Loco + project controllers** | Low | Keep as is |
| ORM/migrations/entities | ✅ (SeaORM stack) | migration crate + entities + models | **Loco/SeaORM stack** | Low | Keep as is |
| Auth framework primitives | ✅ (patterns/hooks) | JWT, refresh sessions, password reset tokens, RBAC domain wiring | **Project domain logic atop Loco runtime** | Medium | Don't duplicate Loco infra layer, but keep domain auth logic as custom |
| Tasks (`cargo loco task`) | ✅ | `CleanupTask` registered | **Loco Tasks** | Low | Keep on Loco |
| Initializers | ✅ | `TelemetryInitializer` via Loco API | **Loco Initializers** | Low | Keep on Loco |
| Mailer subsystem | ✅ | Provider-based server email service (`smtp|loco|none`) + templated auth mail | **Server email service + Loco Mailer adapter** | Low | Keep provider switch in server infra, without separate platform module |
| Workers/queue subsystem | ✅ | Currently custom event-driven outbox relay worker | **RusToK custom (consciously)** | Medium | Keep queues/workers as custom (don't duplicate Loco queue runtime) |
| Storage abstraction (uploads/assets) | ✅ | Shared storage contract via `rustok-storage` + `rustok-media`, initialized in server runtime | **`rustok-storage` service + server runtime wiring** | Low | Use shared storage contract and don't proliferate adhoc upload paths |
| Tenancy caching | N/A (project concern) | custom tenant cache + negative cache + invalidation + metrics | **RusToK custom** | Low | Keep as custom (platform-specific) |
| Event bus / outbox transport | N/A (project architecture) | memory/outbox/iggy transport + relay worker | **RusToK custom** | Low | Keep as custom |

## Governance register

The registry below is the mandatory entry point for architectural decisions on Loco-capabilities in `apps/server`.

| Capability | Runtime owner (current) | Source of truth (target) | ADR / reference (required) | Decision status | Next review date | Code points |
|---|---|---|---|---|---|---|
| Application hooks (`Hooks`) | `apps/server` + `loco_rs` runtime | Loco hooks contract + `apps/server/src/app.rs` as integration layer | `apps/server/docs/loco/README.md`; `DECISIONS/2026-02-19-core-server-module-bundles-routing.md` | Accepted | 2026-06-01 | `apps/server/src/app.rs` |
| Application configuration (`Config` + `settings.rustok.*`) | `apps/server` | Loco config (`config/*.yaml`) + typed settings in server | `apps/server/docs/loco/README.md`; `docs/architecture/overview.md` | Accepted | 2026-06-01 | `apps/server/src/common/settings.rs`; `apps/server/config/development.yaml`; `apps/server/config/test.yaml` |
| REST/GraphQL routing | `apps/server` | Loco `AppRoutes` + server controllers/graphql modules | `DECISIONS/2026-02-19-core-server-module-bundles-routing.md`; `docs/architecture/api.md` | Accepted | 2026-06-01 | `apps/server/src/app.rs`; `apps/server/src/controllers/mod.rs`; `apps/server/src/graphql/mod.rs` |
| ORM/migrations/entities | `apps/server` migration + SeaORM entities | SeaORM stack in server app + migration crate | `docs/architecture/database.md`; `apps/server/docs/README.md` | Accepted | 2026-06-01 | `apps/server/migration/src/lib.rs`; `apps/server/src/models/mod.rs` |
| Auth framework primitives (JWT/sessions/reset/RBAC wiring) | `apps/server` + `rustok-core` + `rustok-rbac` | Domain auth logic atop Loco runtime | `DECISIONS/2026-02-26-auth-lifecycle-unification-session-invalidation.md`; `DECISIONS/2026-03-05-rbac-relation-only-final-cutover-gate.md` | Accepted | 2026-05-20 | `apps/server/src/services/rbac_service.rs`; `apps/server/src/graphql/auth/mutation.rs`; `apps/server/src/controllers/auth.rs` |
| Tasks (`cargo loco task`) | `apps/server` via Loco task runtime | Loco tasks API with server task registration | `apps/server/docs/README.md`; `docs/guides/quickstart.md` | Accepted | 2026-06-01 | `apps/server/src/tasks/mod.rs`; `apps/server/src/tasks/cleanup.rs`; `apps/server/src/app.rs` |
| Initializers | `apps/server` startup composition | Default-superadmin setup is an explicit host bootstrap action over `ServerRuntimeContext`; remaining historical initializer references are inventory only | `apps/server/docs/README.md`; `docs/architecture/loco-exit-plan.md` | Accepted | 2026-07-10 | `apps/server/src/initializers/superadmin.rs`; `apps/server/src/app.rs` |
| Mailer subsystem | `apps/server` (`services/email.rs` + typed settings) | Server email service with `EmailProvider::{Smtp,Loco,None}` and Loco Mailer adapter | `apps/server/docs/loco/README.md`; `docs/architecture/api.md`; `apps/server/docs/README.md` | Accepted | 2026-06-01 | `apps/server/src/services/email.rs`; `apps/server/src/graphql/auth/mutation.rs`; `apps/server/src/common/settings.rs` |
| Workers/queue subsystem | `apps/server` + `rustok-outbox` | RusToK event-driven worker runtime (without Loco queue duplication) | `DECISIONS/2026-03-11-queue-runtime-source-of-truth-outbox.md`; `docs/architecture/event-flow-contract.md`; `docs/standards/transactional-outbox.md` | Accepted | 2026-05-01 | `apps/server/src/app.rs`; `apps/server/src/services/event_transport_factory.rs`; `crates/rustok-outbox/src/relay.rs` |
| Storage abstraction (uploads/assets) | `apps/server` + `rustok-storage` + `rustok-media` | Shared `StorageService` runtime + media module APIs; maintenance cleanup is provided by `rustok-media-cli` | `apps/server/docs/README.md`; `docs/architecture/modules.md`; `docs/modules/registry.md` | Accepted | 2026-06-01 | `apps/server/src/services/app_runtime.rs`; `apps/server/src/services/graphql_schema.rs`; `crates/rustok-media/cli/src/lib.rs` |
| Tenancy caching | `apps/server` + `rustok-core` cache backends | RusToK custom tenancy cache (`tenant.rs`) + shared cache backend contract | `crates/rustok-tenant/docs/README.md`; `docs/guides/observability-quickstart.md` | Accepted | 2026-05-01 | `apps/server/src/middleware/tenant.rs`; `apps/server/src/middleware/tenant_cache_v3.rs`; `crates/rustok-core/src/cache.rs` |
| Event bus / transport (`memory|outbox|iggy`) | `apps/server` + `rustok-events` + `rustok-outbox` | RusToK event transport contract + transactional outbox flow | `DECISIONS/2026-02-19-rustok-events-canonical-contract.md`; `crates/rustok-outbox/docs/README.md`; `apps/server/docs/event-transport.md` | Accepted | 2026-05-01 | `apps/server/src/services/event_transport_factory.rs`; `apps/server/src/services/build_request_events.rs`; `apps/server/src/workers/outbox_relay.rs` |

---

## 2) What is implemented in the server (complete functional slice)

### 2.1 Core Loco lifecycle & app bootstrap

Implemented in `impl Hooks for App`:
- `app_name`, `app_version`;
- `boot` on `create_app::<Self, Migrator>`;
- `routes` with registration of health/metrics/auth/graphql and domain controllers;
- `after_routes` with tenant middleware + runtime extensions;
- `truncate` (not a stub, but real table cleanup in dependency order);
- `register_tasks`;
- `initializers`;
- `connect_workers`;
- `seed`.

### 2.2 Configuration system

- Environment yaml configs (`development.yaml`, `test.yaml`).
- Loco `auth.jwt` configuration.
- Typed settings extension via `settings.rustok.*` (`tenant`, `search`, `features`, `rate_limit`, `events`, `email`).

### 2.3 Controllers & API surface

- REST controllers: health, metrics, auth, swagger, pages.
- Domain controllers: commerce, content, blog, forum.
- GraphQL endpoint + domain GraphQL modules (`auth`, `commerce`, `blog`, `forum`, `pages`, loaders, persisted queries).

### 2.4 Models / ORM / persistence

- SeaORM integration is active.
- Migration crate is connected.
- Core entities and models are used in auth/tenancy/domain flows.

### 2.5 Authentication & authorization (important: not removed)

Implemented and used:
- JWT access token + refresh token flow.
- Session management in DB (`sessions`).
- Password hashing (`argon2`) and verify.
- Password reset flow (forgot/reset mutations, reset token encoding/decoding, revoke sessions after reset).
- RBAC permissions/roles assignment via `RbacService` + `rustok-rbac`/domain entities.

### 2.6 Middleware / tenancy / rate-limit context

- Tenant resolution middleware (header/domain modes).
- Tenant identifier validation.
- Cache + negative cache for tenant resolution.
- Middleware layering via `after_routes`.
- Rate-limit settings are present in `settings`; actual behavior is tied to server middleware/services.

### 2.7 Background processing / events

- Outbox relay worker starts from `connect_workers`.
- Event runtime is created from transport configuration (`memory` / `outbox` / `iggy`).
- Event-driven approach remains the priority for queues and integrations.

### 2.8 Tasks & Initializers

- `cleanup` task is registered, supports `sessions`, `cache`, full cleanup.
- `TelemetryInitializer` connected via Loco initializer API.

### 2.9 Testing support

- Loco testing feature is included in server dev-dependencies.
- Set of unit/integration tests in server module is present (see `apps/server/tests` and inline tests in modules).

---

## 3) What exists in Loco but we have decided/should have differently

### 3.1 Mailer (server infra with provider switch)

**Currently:** server email path is centralized in `apps/server/src/services/email.rs` and supports `EmailProvider::{Smtp,Loco,None}`. Built-in auth emails are rendered via file templates, and `provider=loco` uses `ctx.mailer`.
**Decision:** Loco Mailer is already part of the live server-infra contract, but SMTP is retained as a compatibility/provider option, not as a separate parallel architectural layer.

**Boundary rule:** Mailer is not extracted into a separate platform module. This is an infrastructure responsibility of `apps/server` based on Loco API.

### 3.2 Workers/Queue (consciously custom — Loco Queue not connected)

**Currently:** `loco-rs` is connected without queue features (no `sidekiq` / `bg-redis` in features). `connect_workers` hook is empty — all background processes start as tokio tasks in `connect_runtime_workers`:

| Worker | Implementation | Type |
|--------|-----------|-----|
| Outbox relay | `OutboxRelayWorkerHandle` | tokio task, polling outbox |
| Build worker | `BuildWorkerHandle` | tokio task, polling DB |
| Module event dispatcher | `spawn_module_event_dispatcher` | tokio task |
| Workflow cron | `WorkflowCronScheduler` | tokio task (feature `mod-workflow`) |

`spawn_module_event_dispatcher` collects module-owned handlers from `ModuleRegistry`.
`WorkflowCronScheduler` remains a separate background runtime path and is not considered
part of this event-listener contract.

Loco Tasks (CLI): `cleanup`, `rebuild`, `db_baseline`, and `create_oauth_app` remain legacy inventory. Media cleanup has moved to `rustok-cli media cleanup` and no longer has a Loco task.

For slow operations on request path (e.g., SMTP in `forgot_password`) `tokio::spawn` is used — without Sidekiq.

**Decision:** Loco Queue (Sidekiq/Redis) is not needed:
- Outbox pattern is architecturally better for domain events — guarantees atomicity of write + publish.
- Build worker — rare internal operation, polling is sufficient.
- If push-based queue with retry is needed — extend outbox relay, don't connect Sidekiq.

Policy anchor: `DECISIONS/2026-03-11-queue-runtime-source-of-truth-outbox.md`.

### 3.3 Storage abstraction (shared library contract + server wiring)

**Currently:** shared storage contract is already extracted to `rustok-storage`, and server runtime initializes a single `StorageService`; domain media path is implemented via `rustok-media`.
**Decision:** source of truth for storage is now not adhoc controller code and not separate per-module upload flows, but shared storage service + media module APIs.

**Boundary rule:** Storage doesn't become `ModuleKind::Core` just for the sake of storing files. Shared storage-contract lives in library/runtime layer, and server is responsible for bootstrap and integration wiring.

---

## 4) Caching: current state (detailed)

### 4.1 Tenant cache (main path)

`middleware/tenant.rs` implements:
- versioned cache keys,
- positive cache + negative cache,
- anti-stampede request coalescing (`in_flight` + `Notify`),
- Redis pub/sub invalidation channel (`tenant.cache.invalidate`) when `redis-cache` is enabled,
- metrics (`hits/misses/negative/coalesced`).

### 4.2 Cache backends (shared infra)

`rustok-core` provides:
- `InMemoryCacheBackend` (Moka),
- `RedisCacheBackend` (feature-gated), including circuit breaker.

The server uses shared CacheBackend contract with backend selection by feature/runtime.

### 4.3 Cache observability

`/metrics` returns tenant cache metrics `rustok_tenant_cache_*` (hits, misses, entries, negative indicators).

### 4.4 Tenant cache v3

`tenant_cache_v3.rs` is present as an alternative implementation with circuit breaker + Moka model, but the main production path currently goes through `tenant.rs` infrastructure.

---

## 5) Practical anti-duplication rules

1. Before adding infra functionality, check if there is a mature implementation in Loco.
2. For conscious deviations, document rationale (as for queue/workers) in this document.
3. Don't maintain parallel production implementations of the same layer (Mailer/Storage/Queue) without a migration plan.
4. Any cache change must be accompanied by requirements for invalidation + metrics.
5. For new modules: use the established source of truth from the matrix in section 1.

---

## 6) Current operating contract for Mailer and Storage

### Mailer

- Runtime config lives in `settings.rustok.email.*`.
- Provider selection is explicit: `smtp | loco | none`.
- Built-in auth email rendering is template-based and stays in server infra.
- `provider=loco` uses `ctx.mailer`; `provider=smtp` remains a compatibility/provider path, not a second architecture owner.

### Storage

- Runtime config lives in `settings.rustok.storage.*` and is parsed into `rustok_storage::StorageConfig`.
- `StorageService` is initialized once during app bootstrap and stored in `AppContext.shared_store`.
- Media/domain workflows consume the shared storage contract through `rustok-media` and runtime wiring, not via controller-local backend setup.

### Anti-duplication invariant

- Don't add parallel mailer runtime outside `apps/server/src/services/email.rs`.
- Don't add adhoc upload/download backends bypassing `rustok-storage` / shared runtime wiring.
- Any new provider/driver must extend existing typed config and observability path, not introduce a separate source of truth.

## 7) Operational runbook (incidents / rollback)

- Incident/rollback runbook for phases 2–4: [`LOCO_FEATURE_SUPPORT.md#6-loco-mailer--storage-roadmap-release-phases`](./LOCO_FEATURE_SUPPORT.md#6-loco-mailer--storage-roadmap-release-phases).
- Mandatory procedure for gate metric alerts:
  1. Document incident with phase ID (`mailer-shadow`, `mailer-cutover`, `storage-cutover`).
  2. Enable rollback toggle (provider=`smtp` or legacy storage provider) in runtime config.
  3. Verify SLA restoration in 2 consecutive observation windows.
  4. Save post-incident summary and update this roadmap before re-rollout.

## 6.1 Established architectural rule (2026-03-11)

- Mailer and Storage are considered part of Loco-backed infrastructure in `apps/server`.
- For Mailer/Storage it is forbidden to create a separate platform module in `crates/rustok-*`.
- Domain modules use these capabilities through server-level adapters/policies.
- Decision details are documented in ADR: `DECISIONS/2026-03-11-loco-mailer-storage-as-server-infra.md`.

---

## 8) Sources

- `apps/server/src/app.rs`
- `apps/server/src/controllers/mod.rs`
- `apps/server/src/controllers/metrics.rs`
- `apps/server/src/graphql/mod.rs`
- `apps/server/src/graphql/auth/mutation.rs`
- `apps/server/src/services/email.rs`
- `apps/server/src/services/event_transport_factory.rs`
- `apps/server/src/tasks/mod.rs`
- `apps/server/src/tasks/cleanup.rs`
- `apps/server/src/initializers/mod.rs`
- `apps/server/src/initializers/telemetry.rs`
- `apps/server/src/middleware/tenant.rs`
- `apps/server/src/middleware/tenant_cache_v3.rs`
- `apps/server/src/common/settings.rs`
- `apps/server/config/development.yaml`
- `apps/server/config/test.yaml`
- `crates/rustok-core/src/cache.rs`
- `crates/rustok-core/src/context.rs`
- `apps/server/Cargo.toml`
- `Cargo.toml`
