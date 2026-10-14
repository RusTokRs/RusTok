# Cross-Cutting Architectural Review & Remediation Ledger (CCAR)

This file is the canonical living review ledger for **horizontal, cross-cutting architectural audits** across the RusToK platform.
Unlike the vertical, module-by-module audit ([continuous-review-ledger.md](./continuous-review-ledger.md)), this ledger tracks platform-wide invariants that cut across all crates, modules, transports, and layers using `codegraph` for cross-boundary call flow exploration.

> [!IMPORTANT]
> **Mandatory CodeGraph Prerequisite:**
> This horizontal review plan traces call flows and blast radius across 80+ crates and is **designed exclusively for environments with CodeGraph support (`codegraph_explore` / CodeGraph CLI)**.
> When CodeGraph is unavailable (e.g. CI or non-indexed environments), agents MUST NOT run this horizontal plan and must instead use the sequential, module-by-module audit in [continuous-review-ledger.md](./continuous-review-ledger.md).

---

## Canonical Triggers

To instruct any AI agent (Antigravity, Claude, Codex) in an active CodeGraph environment to execute the next pending phase of this cross-cutting audit, use either canonical instruction:

```text
реализуй план кроссфункционала
```
*(или: `реализуй план сквозного аудита` / in English: `execute cross-cutting audit plan`)*

When triggered, the agent:
1. Reads this ledger and identifies the active phase (`CC-XX`).
2. Uses `codegraph_explore` (or CodeGraph CLI) with the target query symbols to trace the end-to-end call graph from entrypoint to persistence across all modules.
3. Maps discovered invariant violations, antipatterns, and defects across the entire repository.
4. Atomically remediates root causes in affected crates.
5. Runs scoped checks (`cargo check -p <crate>`, `cargo test -p <crate>`, `python scripts/verify/verify-remediation-gate.py`).
6. Marks the sub-items and phase complete in this ledger and records integration evidence.

---

## Current Cycle Status

- **Status:** ACTIVE
- **Active Cycle:** Cross-Cutting Cycle 1 (CC-1)
- **Active Phase:** CC-02 — Fluid Frontend Architecture (FFA) & UI Adapter Purity
- **Current main SHA:** `f50846812`
- **Active Branch:** `main`

---

## Cross-Cutting Audit Execution Protocol

For each cross-cutting phase, execute the strict closed-loop protocol:

1. **CodeGraph Discovery Pass (`codegraph_explore`):** Query the cross-module symbols, traits, and facades that define the capability. Inspect how callers across `apps/server`, `crates/modules/*`, `crates/libs/*`, and `crates/ui/*` consume the capability.
2. **Invariant & Anti-Pattern Map:** Verify code against [docs/standards/patterns-vs-antipatterns.md](./patterns-vs-antipatterns.md), [docs/standards/forbidden-actions.md](./forbidden-actions.md), and [docs/ai/KNOWN_PITFALLS.md](../ai/KNOWN_PITFALLS.md).
3. **Blast Radius Isolation:** Group all violations by root cause. If an invariant is broken across 5 modules, treat them as part of the unified cross-cutting remediation unit.
4. **Atomic Remediation:** Fix the underlying contract, types, and implementations simultaneously. Preserve compile-time correctness and module sovereign boundaries.
5. **Immediate Adjacent Re-Audit:** Verify upstream callers (controllers/handlers/resolvers) and downstream persistence/events.
6. **Scoped Verification:** Run `cargo check -p <crate>` and `cargo test -p <crate>` for all modified crates. Never claim test passes without execution.
7. **Commit & Ledger Update:** Record exact SHA, changes, and verification evidence in this ledger.

---

## Phase Backlog — Cross-Cutting Capability Sequence

- [x] **[CC-01: Fluid Backend Architecture (FBA) & Module Sovereignty](#cc-01-fluid-backend-architecture-fba--module-sovereignty)**
- [x] **[CC-02: Fluid Frontend Architecture (FFA) & UI Adapter Purity](#cc-02-fluid-frontend-architecture-ffa--ui-adapter-purity)**
- [x] **[CC-03: RBAC, Auth & Security Enforcement](#cc-03-rbac-auth--security-enforcement)**
- [x] **[CC-04: Caching, Invalidation & Tenant Cache Isolation](#cc-04-caching-invalidation--tenant-cache-isolation)**
- [x] **[CC-05: Event Bus, Transactional Outbox & Async Messaging](#cc-05-event-bus-transactional-outbox--async-messaging)**
- [x] **[CC-06: API Surfaces, Transports & Error Hygiene](#cc-06-api-surfaces-transports--error-hygiene)**
- [x] **[CC-07: Multi-Tenancy & Effective Context Isolation](#cc-07-multi-tenancy--effective-context-isolation)**
- [x] **[CC-08: Database Transactions, Concurrency & Data Integrity](#cc-08-database-transactions-concurrency--data-integrity)**
- [x] **[CC-09: Background Workers, Schedulers & Async Runtime Safety](#cc-09-background-workers-schedulers--async-runtime-safety)**
- [ ] **[CC-10: Observability, Distributed Tracing & Telemetry](#cc-10-observability-distributed-tracing--telemetry)**
- [ ] **[CC-11: Dynamic Settings, Tenant Configuration & Secrets](#cc-11-dynamic-settings-tenant-configuration--secrets)**
- [ ] **[CC-12: Internationalization (i18n), Localization & Translation Storage](#cc-12-internationalization-i18n-localization--translation-storage)**
- [ ] **[CC-13: Documentation, ADRs, Manifests & Verification Gates](#cc-13-documentation-adrs-manifests--verification-gates)**
- [ ] **[CC-14: Dynamic Extensibility, Flex Fields & Runtime Scripting](#cc-14-dynamic-extensibility-flex-fields--runtime-scripting)**
- [ ] **[CC-15: Object Storage, Media Assets & Direct-Upload Lifecycle](#cc-15-object-storage-media-assets--direct-upload-lifecycle)**
- [ ] **[CC-16: CQRS Read Projections, Search & Index Synchronization](#cc-16-cqrs-read-projections-search--index-synchronization)**
- [ ] **[CC-17: Ingress Sanitization, Input Validation & SSRF/XSS Defense](#cc-17-ingress-sanitization-input-validation--ssrfxss-defense)**

---

### CC-01: Fluid Backend Architecture (FBA) & Module Sovereignty

- **Canonical Documents:** [docs/backend/module-backend-architecture.md](../backend/module-backend-architecture.md), [docs/backend/module-backend-implementation.md](../backend/module-backend-implementation.md), [docs/glossary.md](../glossary.md#fba-fluid-backend-architecture), [scripts/architecture_rules.toml](../../scripts/architecture_rules.toml).
- **Primary Owner Crates:** `crates/libs/rustok-fba`, `crates/libs/rustok-api`, `crates/libs/rustok-runtime`, `crates/utils/rustok-cli-core`.
- **CodeGraph Target:** `FbaProviderDescriptor FbaConsumerDependency PortContext PortError rustok_fba HostRuntimeContext`

#### Sub-Audit Checklist
- [x] **CC-01.01 — Module Physical Shape & Hexagonal Separation:** Verified that no executable `.rs` code is hidden in `contracts/` or `docs/`. Executed `node scripts/verify/verify-module-source-layout.mjs` — canonical module source layout verification passed.
- [x] **CC-01.02 — Inter-Module Dependency Sovereignty:** Executed `python scripts/architecture_dependency_guard.py` — passed with 0 violations. Dependency and import boundaries between modules are respected.
- [x] **CC-01.03 — Port & Seam Purity:** Discovered direct cross-module entity access in `rustok-workflow/src/controllers/webhook.rs` directly querying `rustok_tenant::entities::tenant`. Remediated by replacing with public `TenantService::get_tenant_by_slug(&tenant_slug)`, mapping `TenantError::NotFound` to HTTP 404, and added 2 unit tests. Verified `cargo test -p rustok-workflow` passed.
- [x] **CC-01.04 — FBA Registry & Capability Profiles:** Verified `rustok-fba` capability definitions (`BackendTopology`, `TransportProfile`, `CapabilityId`, `FbaProviderDescriptor`, `FbaConsumerDependency`). Embedded-first in-process execution is maintained across platform modules without uncontrolled remote fallbacks.
- [x] **CC-01.05 — Host Minimalism (`apps/server`):** Verified `apps/server` acts purely as composition root and host. Contains 0 `clap` CLI dependencies and delegates module business rules to owner crates.
- [x] **CC-01.06 — CLI Package Decoupling:** Verified that module maintenance commands reside in isolated `cli/` packages (`rustok-auth/cli`, `rustok-media/cli`, `rustok-modules/cli`, `rustok-profiles/cli`, `rustok-rbac/cli`) over `rustok-cli-core`. All domain crates have 0 `clap` dependencies.

---

### CC-02: Fluid Frontend Architecture (FFA) & UI Adapter Purity

- **Canonical Documents:** [docs/UI/module-package-architecture.md](../UI/module-package-architecture.md), [docs/UI/graphql-architecture.md](../UI/graphql-architecture.md), [AGENTS.md](../../AGENTS.md) §5.
- **Primary Owner Crates:** `crates/modules/*/admin`, `crates/modules/*/storefront`, `crates/ui/*`, `apps/admin`, `apps/storefront`.
- **CodeGraph Target:** `leptos_dom view rsx TransportFacade ViewModel rustok_ui`

#### Sub-Audit Checklist
- [x] **CC-02.01 — Zero Framework in Core:** Verified every `crates/modules/*/core` crate contains strictly zero imports of `leptos::*`, `dioxus::*`, or other UI frameworks. All models are pure Rust data types. Automated enforcement confirmed via `node scripts/verify/verify-ffa-ui-boundary-sweep.mjs` and codebase-wide regex grep.
- [x] **CC-02.02 — Zero Domain Logic in UI Adapters:** Verified `crates/modules/*/admin/src` and `storefront/src` do not perform business calculations, price/discount math, status validations, or command construction inside templates (`view!`, `rsx!`). View-models and calculation logic live strictly in framework-agnostic `core/` view-models.
- [x] **CC-02.03 — Transport Facade Boundary:** Ensured UI adapters invoke backend capabilities strictly through module-owned `transport/` facades, never calling raw fetch or ad-hoc API endpoints directly. Verified zero direct calls to `reqwest` or raw `api::*` from UI templates. Executed `node scripts/verify/verify-ffa-ui-transport-profile-sweep.mjs` — passed.
- [x] **CC-02.04 — Design Token & UI Primitive Reuse:** Verified module UIs consume canonical visual primitives (`rustok-ui`, `rustok-forms`, `rustok-grid`, `rustok-ui-i18n`) rather than implementing ad-hoc form engines or custom CSS styling. Synchronized documentation and registry status for `rustok-accounting`, `rustok-docs`, and `docs/modules/registry.md`. Executed `node scripts/verify/verify-ffa-ui-boundary-sweep.mjs` and `node scripts/verify/verify-ffa-ui-migration-contract.mjs` — all checks passed cleanly.

---

### CC-03: RBAC, Auth & Security Enforcement

- **Canonical Documents:** [crates/modules/rustok-rbac/docs/README.md](../../crates/modules/rustok-rbac/docs/README.md), [crates/modules/rustok-auth/docs/README.md](../../crates/modules/rustok-auth/docs/README.md), [docs/standards/security.md](./security.md).
- **Primary Owner Crates:** `rustok-rbac`, `rustok-auth`, `rustok-api`, `apps/server/src/middleware/auth_context.rs`, `apps/server/src/services/rbac_*.rs`.
- **CodeGraph Target:** `RequirePermission check_permission RbacService AuthContext PermissionKey`

#### Sub-Audit Checklist
- [x] **CC-03.01 — Controller & Resolver Permission Coverage:** Audited REST endpoints in `apps/server/src/controllers/*` and GraphQL operations in `apps/server/src/graphql/*`. Verified permission guards (`has_effective_permission`, type-safe extractors `Require*`, `require_graphql_auth`) are enforced across users, channels, flex, and GraphQL endpoints. Public mutations are restricted and validated.
- [x] **CC-03.02 — Tenant Isolation in Security Context:** Verified `SecurityContext::get_scope()` and `try_from_port_context` strictly require and enforce tenant scope and principal claims. Cross-tenant relations and unauthorized privilege escalations fail closed. Verified SQLite and PostgreSQL tenant integrity invariants.
- [x] **CC-03.03 — RBAC Cache Invalidation:** Verified that role and permission mutations (`replace_user_role_committed`, `user_admin.rs`, `rbac_repair.rs`) commit within transaction, reserve invalidation generation, invalidate local caches via `invalidate_user_rbac_caches`, and broadcast multi-node invalidation via `publish_user_rbac_invalidation`.
- [x] **CC-03.04 — Password & Token Security:** Verified Argon2id password hashing parameters with unique random salts via OsRng, 256-bit entropy refresh tokens, strict JWT token validation (RFC 7519, `leeway = 0`, `validate_exp = true`, `set_issuer`, `set_audience`, fail-closed algorithm matching), and transactional revocation of all active sessions upon password changes. Scoped checks `cargo test -p rustok-rbac` (93 passed) and `cargo test -p rustok-auth` (46 passed) verified 100% clean.

---

### CC-04: Caching, Invalidation & Tenant Cache Isolation

- **Canonical Documents:** [crates/modules/rustok-cache/README.md](../../crates/modules/rustok-cache/README.md), [docs/standards/patterns-vs-antipatterns.md](./patterns-vs-antipatterns.md) §6.
- **Primary Owner Crates:** `rustok-cache`, `rustok-tenant`, `apps/server/src/services/settings_service.rs`.
- **CodeGraph Target:** `CacheService get_cached set_cached invalidate CacheKey TieredCache`

#### Sub-Audit Checklist
- [x] **CC-04.01 — Tenant Scoping in Cache Keys:** Verified `CacheKeyBuilder::new` requires `tenant_or_global` as part of the immutable 6-tuple prefix (`service`, `environment`, `tenant_or_global`, `domain`, `schema_version`, `resource`). Dynamic identities are tagged and length-bounded, preventing cross-tenant key pollution.
- [x] **CC-04.02 — Mutation Path Invalidation:** Audited write/mutation paths (Product, Price, Category, User, Order, Settings). Verified that mutations execute deterministic invalidations locally and broadcast via `publish_invalidation` / Redis pub/sub.
- [x] **CC-04.03 — Stampede & Dogpile Prevention:** Verified singleflight / request coalescing active via `CacheLoadCoordinator` in `load_or_fill`. The first caller for a cache miss fills the key while concurrent callers await the shared in-flight load lease, eliminating cache dogpiles on cold start.
- [x] **CC-04.04 — Negative Caching & Multi-Node Sync:** Verified negative caching (`TENANT_NEGATIVE_CACHE_TTL = 60s`, `CHANNEL_NEGATIVE_CACHE_TTL = 10s`) via `rustok-cache::negative` to protect against DoS on non-existent tenants/resources. Verified atomic invalidation of negative cache entries on tenant creation, and multi-node sync through `CacheInvalidationService`. Scoped checks `cargo test -p rustok-cache` verified 190 passed tests with 0 failures.

---

### CC-05: Event Bus, Transactional Outbox & Async Messaging

- **Canonical Documents:** [docs/references/outbox/README.md](../references/outbox/README.md), [docs/architecture/event-flow-contract.md](../architecture/event-flow-contract.md), [docs/standards/transactional-outbox.md](./transactional-outbox.md).
- **Primary Owner Crates:** `rustok-events`, `rustok-outbox`, `rustok-events-module`, `rustok-iggy`.
- **CodeGraph Target:** `publish_in_tx TransactionalEventBus Outbox sys_events publish`

#### Sub-Audit Checklist
- [x] **CC-05.01 — Atomic `publish_in_tx` Invariant:** Verified all domain mutations write domain events inside the active database transaction (`publish_root_in_tx`, `publish_contract_in_tx`, `publish_in_tx`). Transaction rollback rolls back outbox events atomically.
- [x] **CC-05.02 — Event Payload Integrity & PII Boundary:** Verified `EventEnvelope` and `ContractEventEnvelope` enforce non-empty metadata (`id` generated via ULID, `event_type`, `schema_version`, `correlation_id`, `tenant_id`, `trace_id` from OpenTelemetry, `timestamp`). Enforces strict version checks (`schema_version == schema.version`) and PII-free serialization.
- [x] **CC-05.03 — Consumer Idempotency:** Verified durable owner-scoped idempotency receipts in `owner_operation_receipts` via `rustok-outbox::idempotency` (`Admission::Run(Lease)`, `Admission::Replay(Value)`, `Admission::ReplayError(PortError)`). Duplicate commands replay stored outcome without duplicating side-effects.
- [x] **CC-05.04 — Outbox Retention & DLQ Pipelines:** Verified `OutboxRelay` claims batches with TTL leases (`claim_batch`), enforces exponential backoff retry policies (`backoff_base` to `backoff_max`), quarantines unprocessable payloads and exhausted retries to DLQ (`SysEventStatus::Failed`), and prunes delivered rows via `OutboxRetentionWorker`. Scoped checks `cargo test -p rustok-events` (114 passed) and `cargo test -p rustok-outbox` (41 passed) verified 100% clean.

---

### CC-06: API Surfaces, Transports & Error Hygiene

- **Canonical Documents:** [docs/architecture/api.md](../architecture/api.md), [docs/architecture/dataloader.md](../architecture/dataloader.md), [docs/standards/errors.md](./errors.md).
- **Primary Owner Crates:** `apps/server/src/controllers/*`, `apps/server/src/routes.rs`, `rustok-api`, `rustok-web`, `rustok-mcp`.
- **CodeGraph Target:** `build_application_router HttpError ErrorBody MergedObject McpRouter Dataloader`

#### Sub-Audit Checklist
- [x] **CC-06.01 — Error Envelope Consistency:** Verified all Axum controllers and routes return structured JSON `ErrorBody` responses (`apps/server/src/error.rs`, `apps/server/src/routes.rs`). Zero plain-text 500 responses or raw database SQL error leaks (mapped to `"internal_error"`).
- [x] **CC-06.02 — DTO vs SeaORM Entity Boundary:** Verified controllers and resolvers do not expose raw SeaORM `Entity`/`Model` instances directly to API clients. Enforced typed DTO transformations across all modules.
- [x] **CC-06.03 — GraphQL Dataloader & N+1 Elimination:** Verified `DataLoader` batching across nested relations (`TenantNameLoader`, `ProfileSummaryLoader`) in `apps/server/src/graphql/schema.rs`.
- [x] **CC-06.04 — List Pagination & Limit Clamping:** Verified pagination standardization with `limit` clamped to maximum 100 in `rustok-api/src/graphql/common.rs` and REST controllers.
- [x] **CC-06.05 — MCP Tool Schema Validation:** Verified `rustok-mcp` tools enforce typed JSON-RPC schema validation and structured tool execution responses (`rustok-mcp/src/tools.rs`). Scoped tests passed (34/34).

---

### CC-07: Multi-Tenancy & Effective Context Isolation

- **Canonical Documents:** [crates/modules/rustok-tenant/docs/README.md](../../crates/modules/rustok-tenant/docs/README.md), [docs/standards/patterns-vs-antipatterns.md](./patterns-vs-antipatterns.md) §3.1, §6.
- **Primary Owner Crates:** `rustok-tenant`, `rustok-channel`, `apps/server/src/middleware/tenant.rs`.
- **CodeGraph Target:** `TenantContext resolve_tenant Column::TenantId ensure_tenant_access`

#### Sub-Audit Checklist
- [x] **CC-07.01 — Universal Query Filter Invariant:** Verified that domain table SeaORM queries (`find()`, `update_many()`, `delete_many()`) enforce `.filter(Column::TenantId.eq(tenant_id))` or tenant-scoped composite key filters across modules (`rustok-tenant`, `rustok-channel`, `rustok-auth`, `rustok-commerce`).
- [x] **CC-07.02 — Request Host Smuggling & Boundary Defense:** Verified middleware defense against duplicate Host/Forwarded headers, trailing dots, authority port extraction, and lowercase subdomain normalization in `apps/server/src/middleware/tenant_resolution.rs`.
- [x] **CC-07.03 — Cross-Tenant Relation Verification:** Verified cross-tenant relation validation across modules (`rustok-commerce`, `flex`, `rustok-fulfillment`), ensuring associated entities match active tenant and non-nil tenant UUIDs. Enforced cross-tenant policy channel update denial in `rustok-channel`.
- [x] **CC-07.04 — Context Propagation:** Verified explicit propagation of `TenantContext`, `ChannelContext`, and `LocaleContext` through Axum request extensions (`TenantContextExtension`, `ChannelContextExtension`, `LocaleContextExtension`) without manual header re-parsing. Scoped tests passed: `rustok-tenant` (24/24), `rustok-channel` (35/35).

---

### CC-08: Database Transactions, Concurrency & Data Integrity

- **Canonical Documents:** [docs/architecture/database.md](../architecture/database.md), [AGENTS.md](../../AGENTS.md) §7.
- **Primary Owner Crates:** `migration/`, `crates/modules/*/migration/`, SeaORM entities.
- **CodeGraph Target:** `DatabaseConnection::begin TransactionTrait find_by_id insert update`

#### Sub-Audit Checklist
- [x] **CC-08.01 — Atomic Multi-Table Mutations:** Audited write operations touching multiple rows or relations (`TenantService::create_tenant`, `ActiveModel::insert` in `oauth_apps.rs`, `OrderFieldService`). Guaranteed that mutations and transactional outbox publishes are strictly enclosed within `db.begin()` transactions with atomic commit.
- [x] **CC-08.02 — TOCTOU Race Condition Guards:** Verified compare-and-swap (CAS) via `expected_revision` across modules (`alloy`, `rustok-tenant`, `rustok-groups`, `rustok-commerce`, `rustok-sandbox`), unique database constraints, and row-level locking (`LockType::Update, LockBehavior::SkipLocked` in `rustok-outbox`).
- [x] **CC-08.03 — Soft Delete & Archive Discipline:** Verified business aggregates utilize explicit lifecycle states (`ProductStatus::Archived`, `PostStatus::Archived`, `TopicStatus`, `is_active` soft-delete toggles, and tombstoned deletion receipts) rather than destructive physical deletes.
- [x] **CC-08.04 — Non-Blocking Transaction Scope:** Verified that transactions do not hold database connections/locks across external network HTTP calls, timers, or async message broker awaits. Event publishing is deferred to local transactional outbox tables inside transactions. Scoped tests passed: `rustok-tenant` migration contract (3/3), `rustok-outbox` (41/41).

---

### CC-09: Background Workers, Schedulers & Async Runtime Safety

- **Canonical Documents:** [docs/guides/runtime-guardrails.md](../guides/runtime-guardrails.md), [AGENTS.md](../../AGENTS.md) §10, §14.
- **Primary Owner Crates:** `crates/workers/*`, `rustok-workflow`, `rustok-sandbox`, `apps/server/src/services/app_runtime.rs`.
- **CodeGraph Target:** `spawn_worker StopHandle AppRuntime shutdown_runtime_workers tokio::fs`

#### Sub-Audit Checklist
- [x] **CC-09.01 — Non-Blocking Async Invariant:** Verified 0 blocking calls (`std::thread::sleep`, `std::fs::*`) in production async code (grep sweep confirmed only test/benchmark blocks). All async runtime operations use `tokio::time::sleep`, `tokio::fs`, or `tokio::task::spawn_blocking`.
- [x] **CC-09.02 — Graceful Worker Drain:** Verified `StopHandle` broadcast channels and `shutdown_runtime_workers` in `apps/server/src/services/app_lifecycle.rs`, concurrently joining and draining all registered worker handles (relay, retention, reaper, seo, module work, workflow, watchdog) with bounded timeouts.
- [x] **CC-09.03 — Concurrency Limiting:** Verified explicit `tokio::sync::Semaphore` concurrency boundaries across workers, image processing (`rustok-media`), cache refreshing (`rustok-cache`), OCI registry transfers (`rustok-modules`), and bulkhead execution (`rustok-core`).
- [x] **CC-09.04 — Sandbox Isolation:** Verified resource quotas, execution timeouts, instruction counters, memory limits (64MB), and capability rate limiting in `rustok-sandbox` (`SandboxLimits`, `SandboxPolicy`). Scoped tests passed: `rustok-sandbox` (35/35).

---

### CC-10: Observability, Distributed Tracing & Telemetry

- **Canonical Documents:** [docs/standards/logging.md](./logging.md), [docs/standards/opentelemetry-integration.md](./opentelemetry-integration.md), [docs/standards/distributed-tracing.md](./distributed-tracing.md).
- **Primary Owner Crates:** `rustok-telemetry`, `apps/server/src/controllers/metrics.rs`, `apps/server/src/controllers/health.rs`.
- **CodeGraph Target:** `init_telemetry register_metrics tracing::instrument health_check`

#### Sub-Audit Checklist
- [ ] **CC-10.01 — Ad-Hoc Print Elimination:** Remove all instances of `println!`, `eprintln!`, and `dbg!` in production code. Replace with platform tracing macros.
- [ ] **CC-10.02 — PII & Secret Redaction in Spans:** Verify that `#[instrument]` macros use `skip(...)` on arguments containing passwords, tokens, API keys, or customer PII.
- [ ] **CC-10.03 — Telemetry Lifecycle Safety:** Verify that OpenTelemetry / Tracing subscriber is initialized exactly once at process startup without memory leaks or duplicate registries.
- [ ] **CC-10.04 — Health & Readiness Probes:** Verify `/health/live` and `/health/ready` probe contracts, ensuring degraded/unhealthy subsystem states map accurately to HTTP status codes.

---

### CC-11: Dynamic Settings, Tenant Configuration & Secrets

- **Canonical Documents:** [docs/architecture/settings.md](../architecture/settings.md), [DECISIONS/2026-09-22-platform-module-settings-architecture.md](../../DECISIONS/2026-09-22-platform-module-settings-architecture.md).
- **Primary Owner Crates:** `apps/server/src/services/settings_service.rs`, `crates/modules/*/settings`.
- **CodeGraph Target:** `SettingsService load_tenant_settings Zeroize`

#### Sub-Audit Checklist
- [ ] **CC-11.01 — In-Memory Secret Zeroization:** Verify that credentials, encryption keys, and tokens use `zeroize::Zeroize` to scrub memory when dropped.
- [ ] **CC-11.02 — Dynamic Settings Schema Validation:** Verify that JSON documents representing tenant module settings are validated against typed JSON schemas before persistence.
- [ ] **CC-11.03 — Hierarchy & Fallback Resolution:** Verify fallback resolution order: tenant explicit settings -> channel overrides -> module default settings.
- [ ] **CC-11.04 — Environment Variable Containment:** Ensure direct reads of environment variables (`std::env::var`) are restricted to host configuration crates, not spread across domain crates.

---

### CC-12: Internationalization (i18n), Localization & Translation Storage

- **Canonical Documents:** [docs/architecture/i18n.md](../architecture/i18n.md), [docs/architecture/database.md](../architecture/database.md).
- **Primary Owner Crates:** `rustok-translation`, `rustok-translation-targets`, `rustok_flutter_i18n`, `apps/server/src/middleware/locale.rs`.
- **CodeGraph Target:** `constrain_locale_to_tenant TranslationService _translations`

#### Sub-Audit Checklist
- [ ] **CC-12.01 — Relational Translation Schema:** Verify entity translations adhere to the canonical `base + *_translations` table pattern with `VARCHAR(32)` locale columns.
- [ ] **CC-12.02 — Fallback Negotiation Chain:** Verify the locale resolution chain: `requested locale -> tenant default locale -> platform default fallback`.
- [ ] **CC-12.03 — User-Facing String Decoupling:** Verify backend services return localized error codes / translation keys rather than hardcoded English strings.
- [ ] **CC-12.04 — Fluent & ICU Format Safety:** Audit Fluent message formatting and pluralization for safe argument substitution without panic risks.

---

### CC-13: Documentation, ADRs, Manifests & Verification Gates

- **Canonical Documents:** [docs/index.md](../index.md), [DECISIONS/README.md](../../DECISIONS/README.md), [AGENTS.md](../../AGENTS.md) §1, §2.
- **Primary Owner Crates:** Platform documentation, `xtask/`, `scripts/verify/*`.
- **CodeGraph Target:** `build_registry RusToKModule modules.toml rustok-module.toml`

#### Sub-Audit Checklist
- [ ] **CC-13.01 — Registry vs Manifest Parity:** Verify that all modules declared in `modules.toml` match `build_registry()` in `apps/server` and their respective `rustok-module.toml`.
- [ ] **CC-13.02 — Active ADR Alignment:** Verify that code implementation reflects accepted ADRs in `DECISIONS/` and that obsolete architecture documents are marked deprecated or updated.
- [ ] **CC-13.03 — Reference Schema Parity:** Run `scripts/verify/verify-reference-artifacts.mjs` to ensure committed OpenAPI JSON and GraphQL SDL artifacts match live route definitions.
- [ ] **CC-13.04 — Documentation Integrity:** Verify that links in `docs/index.md` are valid and that every module contains a synchronized `README.md`.

---

### CC-14: Dynamic Extensibility, Flex Fields & Runtime Scripting

- **Canonical Documents:** [crates/modules/flex/README.md](../../crates/modules/flex/README.md), [crates/modules/alloy/README.md](../../crates/modules/alloy/README.md), [DECISIONS/2026-07-11-neutral-sandbox-foundation.md](../../DECISIONS/2026-07-11-neutral-sandbox-foundation.md).
- **Primary Owner Crates:** `flex`, `alloy`, `crates/workers/rustok-sandbox`.
- **CodeGraph Target:** `FieldDefinitionService FieldDefRegistry flex_attached_localized_values RhaiEngine AlloyScript`

#### Sub-Audit Checklist
- [ ] **CC-14.01 — Unified Donor Opt-in:** Verify that all attached donors (`user`, `product`, `order`, `topic`, `taxonomy.category`) use the canonical Flex engine (`flex`) rather than implementing ad-hoc custom-fields or raw JSON metadata parsing.
- [ ] **CC-14.02 — Attached Multilingual Separation:** Ensure attached localized field values are stored in `flex_attached_localized_values` rather than inline locale-aware JSON blobs inside base entity rows.
- [ ] **CC-14.03 — Fail-Closed Data Classification & AI Export:** Verify that `flex_attached_field_policies` enforces fail-closed privacy: default to `tenant_private` with `ai_export_allowed = false`, and prohibit secret or immutable fields from being exported to AI contexts.
- [ ] **CC-14.04 — Rhai/Alloy Sandbox Safety:** Verify that Rhai execution in `alloy` and `rustok-sandbox` enforces instruction counters, maximum call stack depth, and execution timeouts to prevent DoS via infinite loops.

---

### CC-15: Object Storage, Media Assets & Direct-Upload Lifecycle

- **Canonical Documents:** [crates/utils/rustok-storage/README.md](../../crates/utils/rustok-storage/README.md), [crates/modules/rustok-media/docs/README.md](../../crates/modules/rustok-media/docs/README.md), [DECISIONS/2026-07-22-direct-object-store-runtime-owner-local-lifecycle.md](../../DECISIONS/2026-07-22-direct-object-store-runtime-owner-local-lifecycle.md).
- **Primary Owner Crates:** `rustok-storage`, `rustok-media`, `crates/modules/rustok-media-transport`.
- **CodeGraph Target:** `ObjectStorage StorageBackend generate_presigned_url MediaService CanonicalObjectKey`

#### Sub-Audit Checklist
- [ ] **CC-15.01 — Tenant-Scoped Object Keys:** Verify that canonical storage keys are strictly formatted as `<tenant_id>/<category>/<file_uuid>` to prevent cross-tenant asset enumeration and collision.
- [ ] **CC-15.02 — Path Traversal Prevention:** Verify sanitization on all file names and storage paths in local storage mode, strictly blocking directory traversal (`../`, absolute paths, null bytes).
- [ ] **CC-15.03 — Presigned Upload Expiration & Verification:** Ensure direct upload handles issue short-lived signed URLs and verify that file size, MIME type, and magic bytes match allowed constraints upon upload completion.
- [ ] **CC-15.04 — Media Extraction SSRF Guard:** Audit remote media imports / URL extraction (`UrlAllowlist`) to ensure agents reject private IPs (RFC 1918), localhost (`127.0.0.1`), and cloud metadata IP (`169.254.169.254`).

---

### CC-16: CQRS Read Projections, Search & Index Synchronization

- **Canonical Documents:** [crates/modules/rustok-index/docs/README.md](../../crates/modules/rustok-index/docs/README.md), [crates/modules/rustok-search/README.md](../../crates/modules/rustok-search/README.md), [DECISIONS/2026-07-24-index-storage-layout.md](../../DECISIONS/2026-07-24-index-storage-layout.md).
- **Primary Owner Crates:** `rustok-index`, `rustok-search`.
- **CodeGraph Target:** `IndexService SearchService index_products index_content ReindexHandler`

#### Sub-Audit Checklist
- [ ] **CC-16.01 — CQRS-Lite Separation:** Verify that storefront and public catalog queries read from denormalized `index_*` JSONB tables or search indexes, avoiding multi-table joins on write-side normalized tables.
- [ ] **CC-16.02 — Event-Driven Reindexing Consistency:** Verify that all entity mutations (create, update, status change, delete) emit events that deterministically trigger index projection updates.
- [ ] **CC-16.03 — Search Tenant Scoping:** Verify that search queries (`rustok-search`) enforce tenant, channel, and locale boundaries, preventing search results from leaking across tenants.
- [ ] **CC-16.04 — Reindex Idempotency & Batching:** Audit batch reindexing operations for chunking, error tolerance, and idempotency upon retry.

---

### CC-17: Ingress Sanitization, Input Validation & SSRF/XSS Defense

- **Canonical Documents:** [docs/guides/input-validation.md](../guides/input-validation.md), [docs/standards/security.md](./security.md), [docs/standards/patterns-vs-antipatterns.md](./patterns-vs-antipatterns.md) §7.5, §11.
- **Primary Owner Crates:** `rustok-web`, `packages/richtext`, all DTO modules.
- **CodeGraph Target:** `validator::Validate sanitize_html UrlAllowlist SecurityHeaders`

#### Sub-Audit Checklist
- [ ] **CC-17.01 — Universal DTO Ingress Validation:** Verify that every public REST request and GraphQL input DTO implements `validator::Validate` and calls `.validate()?` before handing data to services.
- [ ] **CC-17.02 — HTML & Richtext Sanitization:** Ensure user-submitted HTML / Markdown (e.g. blog posts, forum replies, reviews, product descriptions) passes through strict HTML sanitization (`ammonia` / `sanitize-html`) before database persistence to prevent Stored XSS.
- [ ] **CC-17.03 — Outbound SSRF Verification:** Verify that any outgoing HTTP client call (webhooks, remote media downloads, OIDC metadata fetching) uses an SSRF-safe client that rejects private/loopback/link-local IP addresses.
- [ ] **CC-17.04 — Path-Based Rate Limiting:** Verify that sensitive ingress paths (`/auth/login`, `/auth/register`, `/auth/forgot-password`, search, upload) are guarded by sliding-window rate limiters.
