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
- **Active Cycle:** Cross-Cutting Cycle 2 (Fresh pass from scratch)
- **Active Phase:** CC-06: API Surfaces, Transports & Error Hygiene
- **Completed Phases:** CC-01, CC-02, CC-03, CC-04, CC-05 (5/17 completed — 29.4%)
- **Active Branch:** `main`

---

## Cross-Cutting Audit Execution Protocol

For each cross-cutting phase, execute the strict closed-loop protocol:

1. **CodeGraph Discovery Pass (`codegraph_explore`):** Query the cross-module symbols, traits, and facades that define the capability. Inspect how callers across `apps/server`, `crates/modules/*`, `crates/libs/*`, and `crates/ui/*` consume the capability.
2. **Invariant & Anti-Pattern Map:** Verify code against [docs/standards/logic-hygiene-patterns.md](./logic-hygiene-patterns.md), [docs/standards/forbidden-actions.md](./forbidden-actions.md), and [docs/ai/KNOWN_PITFALLS.md](../ai/KNOWN_PITFALLS.md).
3. **Blast Radius Isolation:** Group all violations by root cause. If an invariant is broken across multiple modules, treat them as part of the unified cross-cutting remediation unit.
4. **Atomic Remediation:** Fix the underlying contract, types, and implementations simultaneously. Preserve compile-time correctness and module sovereign boundaries.
5. **Immediate Adjacent Re-Audit:** Verify upstream callers (controllers/handlers/resolvers) and downstream persistence/events.
6. **Scoped Verification:** Run `cargo check -p <crate>` and `cargo test -p <crate>` for all modified crates. Never claim test passes without execution.
7. **Commit & Ledger Update:** Record exact SHA, changes, and verification evidence in this ledger.

---

## Phase Backlog — Cross-Cutting Capability Sequence (17 Phases Total)

- [x] **[CC-01: Fluid Backend Architecture (FBA) & Module Sovereignty](#cc-01-fluid-backend-architecture-fba--module-sovereignty)** (6/6 completed)
- [x] **[CC-02: Fluid Frontend Architecture (FFA) & UI Adapter Purity](#cc-02-fluid-frontend-architecture-ffa--ui-adapter-purity)** (5/5 completed)
- [x] **[CC-03: RBAC, Auth & Security Enforcement](#cc-03-rbac-auth--security-enforcement)** (8/8 completed)
- [x] **[CC-04: Caching, Invalidation & Tenant Cache Isolation](#cc-04-caching-invalidation--tenant-cache-isolation)** (4/4 completed)
- [x] **[CC-05: Event Bus, Transactional Outbox & Async Messaging](#cc-05-event-bus-transactional-outbox--async-messaging)** (4/4 completed)
- [ ] **[CC-06: API Surfaces, Transports & Error Hygiene](#cc-06-api-surfaces-transports--error-hygiene)** (0/5 completed)
- [ ] **[CC-07: Multi-Tenancy & Effective Context Isolation](#cc-07-multi-tenancy--effective-context-isolation)** (0/4 completed)
- [ ] **[CC-08: Database Transactions, Concurrency & Data Integrity](#cc-08-database-transactions-concurrency--data-integrity)** (0/4 completed)
- [ ] **[CC-09: Background Workers, Schedulers & Async Runtime Safety](#cc-09-background-workers-schedulers--async-runtime-safety)** (0/4 completed)
- [ ] **[CC-10: Observability, Distributed Tracing & Telemetry](#cc-10-observability-distributed-tracing--telemetry)** (0/4 completed)
- [ ] **[CC-11: Dynamic Settings, Tenant Configuration & Secrets](#cc-11-dynamic-settings-tenant-configuration--secrets)** (0/4 completed)
- [ ] **[CC-12: Internationalization (i18n), Localization & Translation Storage](#cc-12-internationalization-i18n-localization--translation-storage)** (0/4 completed)
- [ ] **[CC-13: Documentation, ADRs, Manifests & Verification Gates](#cc-13-documentation-adrs-manifests--verification-gates)** (0/4 completed)
- [ ] **[CC-14: Dynamic Extensibility, Flex Fields & Runtime Scripting](#cc-14-dynamic-extensibility-flex-fields--runtime-scripting)** (0/4 completed)
- [ ] **[CC-15: Object Storage, Media Assets & Direct-Upload Lifecycle](#cc-15-object-storage-media-assets--direct-upload-lifecycle)** (0/4 completed)
- [ ] **[CC-16: CQRS Read Projections, Search & Index Synchronization](#cc-16-cqrs-read-projections-search--index-synchronization)** (0/4 completed)
- [ ] **[CC-17: Ingress Sanitization, Input Validation & SSRF/XSS Defense](#cc-17-ingress-sanitization-input-validation--ssrfxss-defense)** (0/4 completed)

---

### CC-01: Fluid Backend Architecture (FBA) & Module Sovereignty

- **Canonical Documents:** [docs/backend/module-backend-architecture.md](../backend/module-backend-architecture.md), [docs/backend/module-backend-implementation.md](../backend/module-backend-implementation.md), [docs/glossary.md](../glossary.md#fba-fluid-backend-architecture), [scripts/architecture_rules.toml](../../scripts/architecture_rules.toml).
- **Primary Owner Crates:** `crates/libs/rustok-fba`, `crates/libs/rustok-api`, `crates/libs/rustok-runtime`, `crates/utils/rustok-cli-core`.
- **CodeGraph Target:** `FbaProviderDescriptor FbaConsumerDependency PortContext PortError rustok_fba HostRuntimeContext`
- **Audit Outcome:** COMPLETED (6/6 sub-items verified & remediated)

#### Sub-Audit Checklist
- [x] **CC-01.01 — Module Physical Shape & Hexagonal Separation**
  - Verified hexagonal structure: `src/domain` pure policies, `src/entities` private persistence models, `src/services` application logic, `src/graphql` and `src/controllers` thin adapters.
  - Remediated `rustok-blog` canonical reference layout: relocated `normalization.rs` from root `src/` to `src/domain/normalization.rs`; modularized `src/services/post/tests.rs` (72KB) into modular slice files in `src/services/post/tests/mod.rs` (< 22KB each); verified with `node scripts/verify/verify-module-source-layout.mjs`.
- [x] **CC-01.02 — Inter-Module Dependency Sovereignty**
  - Verified dependency edges across all 80+ workspace crates using CodeGraph and Cargo graph analysis.
  - Confirmed module sovereignty rules from `scripts/architecture_rules.toml`: domain crates do not import foreign internal persistence entities directly; inter-module dependencies strictly respect published interfaces.
- [x] **CC-01.03 — Port & Seam Purity**
  - Traced port signatures and implementations across `crates/modules/*/src/ports` and `<owner>-ports`.
  - Discovered and eliminated raw SQL queries leaking foreign `taxonomy_*` queries from `NoopBlogTaxonomyPort`; ensured `NoopBlogTaxonomyPort` provides clean in-memory fallback without database coupling per ADR `2026-11-11-blog-taxonomy-sovereignty-and-port-decoupling.md`.
  - Relocated co-located transaction participant implementation to `src/services/taxonomy.rs`, keeping `src/ports/` persistence-neutral and re-exporting the contract.
- [x] **CC-01.04 — FBA Registry & Capability Profiles**
  - Verified `rustok-fba` descriptors (`CapabilityId`, `FbaProviderDescriptor`, `FbaConsumerDependency`, `BackendTopology`, `TransportProfile`) and validation logic.
  - Verified published RPC contracts in `-ports` packages (`flex-ports`, `rustok-product-ports`, `rustok-channel-ports`, `rustok-media-ports`).
- [x] **CC-01.05 — Host Minimalism (`apps/server`)**
  - Traced `apps/server` composition boundaries: Axum routing, middleware, runtime composition, and module mounting.
  - Verified that route handlers remain thin composition glue delegating to module-owned services and ports.
- [x] **CC-01.06 — CLI Package Decoupling**
  - Verified asynchronous CLI provider/outcome contracts in `crates/utils/rustok-cli-core`.
  - Confirmed domain module cores do not depend on `clap`, stdout, process exit semantics, or runtime CLI harnesses; operations adapters remain segregated in `cli/` packages.

---

### CC-02: Fluid Frontend Architecture (FFA) & UI Adapter Purity

- **Canonical Documents:** [docs/UI/module-package-architecture.md](../UI/module-package-architecture.md), [docs/UI/graphql-architecture.md](../UI/graphql-architecture.md), [AGENTS.md](../../AGENTS.md) §5.
- **Primary Owner Crates:** `crates/modules/*/admin`, `crates/modules/*/storefront`, `crates/ui/*`, `apps/admin`, `apps/storefront`, `apps/next-admin`, `apps/next-frontend`, `apps/next-admin/packages/*`, `apps/next-frontend/packages/*`.
- **CodeGraph Target:** `leptos_dom view rsx TransportFacade ViewModel rustok_ui`

- **Audit Outcome:** COMPLETED (5/5 sub-items verified & remediated)

#### Sub-Audit Checklist
- [x] **CC-02.01 — Zero Framework in Core**
  - Audited all `core/`, `model.rs`, and view-model boundaries across module UI crates (`crates/modules/*/admin`, `crates/modules/*/storefront`).
  - Verified with `node scripts/verify/verify-ffa-ui-boundary-sweep.mjs`: confirmed zero `leptos::*`, `axum::*`, `#[component]`, `#[server]`, or framework imports exist in core models.
- [x] **CC-02.02 — Zero Domain Logic in UI Adapters**
  - Verified that Leptos components (`src/ui/`) and Next.js packages (`apps/next-admin/packages/*`, `apps/next-frontend/packages/*`) act strictly as thin view/binding adapters that consume view-models, actions, and formatting functions from `core/`.
- [x] **CC-02.03 — Transport Facade Boundary**
  - Audited UI transport facades across module UI packages.
  - Reconciled stale paths in `scripts/verify/verify-frontend-host-ffa-contract.mjs` (workflow host components and pages relocated to `crates/modules/rustok-workflow/admin`; cache page removed and dynamically mounted; search package API checked). Verified with `node scripts/verify/verify-frontend-host-ffa-contract.mjs` (PASSED).
  - Documented tracked transport parity gaps for the 8 scaffolding modules (`billing`, `calendar`, `calls`, `deal`, `finance`, `page_builder`, `party`, `tasks`) where UI packages are currently in-progress. Verified with `node scripts/verify/verify-ffa-ui-transport-profile-sweep.mjs` (PASSED).
- [x] **CC-02.04 — Design Token & UI Primitive Reuse**
  - Verified design token compliance across `UI/tokens/base.css`, `tokens.json`, and shared primitives (`rustok-grid`, button variants, badge, card, input, switch, avatar, separator).
  - Verified with `node scripts/verify/verify-ui-parity.mjs` (PASSED: all UI workbench recipes and tokens across Web, Next.js, Desktop, and Mobile kit).
- [x] **CC-02.05 — Dual-Stack UI & Admin Synchronization (Leptos ↔ Next.js Parity)**
  - Verified dual-stack route and grid parity across 10 module twins and 70+ UI packages.
  - Verified with `node scripts/verify/verify-ui-grid-parity.mjs` (PASSED: 10 module twins, 212 Rust items, 211 TypeScript exports, 146 reviewed pairs, 32 justified one-sided symbols).
  - Verified UI client dependency boundaries (`verify-ui-client-dependency-boundary.mjs`), i18n keys across 73 packages (`verify-ui-i18n-keys.mjs`), and i18n parity across 77 catalogs (`verify-ui-i18n-parity.mjs`).

---

### CC-03: RBAC, Auth & Security Enforcement

- **Canonical Documents:** [crates/modules/rustok-rbac/docs/README.md](../../crates/modules/rustok-rbac/docs/README.md), [crates/modules/rustok-auth/docs/README.md](../../crates/modules/rustok-auth/docs/README.md), [docs/standards/security.md](./security.md).
- **Primary Owner Crates:** `rustok-rbac`, `rustok-auth`, `rustok-api`, `apps/server/src/middleware/auth_context.rs`, `apps/server/src/services/rbac_*.rs`.
- **CodeGraph Target:** `RequirePermission check_permission RbacService AuthContext PermissionKey`

- **Audit Outcome:** COMPLETED (8/8 sub-items verified & remediated)

#### Sub-Audit Checklist
- [x] **CC-03.01 — Controller & Resolver Permission Coverage**
  - Audited Axum route controllers (`apps/server/src/controllers/`) and GraphQL resolvers (`apps/server/src/graphql/`).
  - Standardized diagnostic and system health endpoints in `apps/server/src/graphql/system.rs`: `system_health` and `events_status` strictly enforce `require_host_authority(ctx, HostAuthority::Read)?;` while `cache_health` enforces `require_cache_read_authority(ctx)?` preserving shared tenant settings admin (`SETTINGS_READ`) and host operator (`HostAuthority::Read`) access per ADR/commit `3822922795`.
  - Reconciled explicit scope binding in `apps/server/src/controllers/artifact_permissions.rs`, ensuring `permission_key: input.permission_key` is passed cleanly into the domain command without redundant heap allocations. Verified with `verify-host-global-authority-boundary.mjs` and `verify-rbac-artifact-permission-outbox.mjs` (PASSED).
- [x] **CC-03.02 — Tenant Isolation in Security Context & Authoritative Queries**
  - Audited tenant context isolation across middleware and transports.
  - Enforced strict tenant match in `crates/modules/rustok-auth/admin/src/transport/native_server_adapter.rs` (`require_auth_admin_tenant_scope`), rejecting mismatched or nil tenant IDs before RBAC admission.
  - Eliminated dangerous host authority context leakage into GraphQL WebSockets in `apps/server/src/controllers/graphql.rs` (`build_ws_connection_data`), guaranteeing WebSockets remain strictly fail-closed and tenant-scoped. Verified with `verify-auth-admin-tenant-scope.mjs`, `verify-rbac-admin-tenant-scope.mjs`, and `verify-host-global-authority-boundary.mjs` (PASSED).
- [x] **CC-03.03 — RBAC Cache Invalidation & Canonical API Seams**
  - Audited durable invalidation generation state in `crates/modules/rustok-rbac/src/invalidation_generation.rs`, `crates/libs/rustok-telemetry/src/rbac_invalidation_metrics.rs`, and watchdog recovery in `apps/server/src/services/rbac_invalidation_generation.rs`.
  - Verified bounded Prometheus metrics (`rustok_rbac_invalidation_*`) without high-cardinality labels, and signed generation lag computation. Verified with `verify-rbac-policy-incident-trace.mjs` and `verify-rbac-fba.mjs` (PASSED).
- [x] **CC-03.04 — Password & Token Security**
  - Verified Argon2id password hashing and constant-time token digest comparison (`Sha256::digest(token.as_bytes())`, `presented_hash.ct_eq(&credential.token_sha256)`) in `apps/server/src/host_authority.rs`.
  - Enforced opaque credential header stripping (`headers.remove(HOST_AUTHORITY_TOKEN_HEADER)`) prior to dispatch. Verified with `verify-host-global-authority-boundary.mjs` (PASSED).
- [x] **CC-03.05 — FFA Purity & i18n in RBAC UI**
  - Audited RBAC admin packages in `crates/modules/rustok-rbac/admin/` and `apps/next-admin/packages/rbac/`.
  - Verified with `verify-rbac-admin-boundary.mjs` (PASSED) and `verify-auth-ui-boundary.mjs` (PASSED).
- [x] **CC-03.06 — Built-in Role Resource Parity & Completeness**
  - Audited built-in tenant role hierarchy (`SuperAdmin`, `Admin`, `Member`, `Guest`), owner role mutation policy, authority continuity, and system role repair in `crates/modules/rustok-rbac/src/role_mutation.rs` and `repair.rs`.
  - Verified with `verify-rbac-owner-role-mutation-contract.mjs` (PASSED).
- [x] **CC-03.07 — GraphQL Mutation Race Elimination & Role Invalidation**
  - Verified atomic transactional execution in `RbacArtifactPermissionAssignmentService::assign` coupling role state updates with outbox events in a single database transaction.
  - Verified with `verify-rbac-artifact-permission-outbox.mjs` (PASSED).
- [x] **CC-03.08 — Platform Permission Catalog Exhaustiveness**
  - Audited `BuiltinTenantRbacCatalog` and `RbacArtifactPermissionCatalog` (two-phase release admission and scoped projection).
  - Verified 6-migration append-only sequence (`m20260714_900001` through `m20260914_000001_localized_presentations`).
  - Verified with `verify-rbac-artifact-permission-tenant-integrity.mjs` (PASSED).

---

### CC-04: Caching, Invalidation & Tenant Cache Isolation

- **Canonical Documents:** [crates/modules/rustok-cache/README.md](../../crates/modules/rustok-cache/README.md), [tenant module documentation](../../crates/modules/rustok-tenant/docs/README.md).
- **Primary Owner Crates:** `rustok-cache`, `rustok-tenant`, `apps/server/src/services/settings_service.rs`.
- **CodeGraph Target:** `CacheService get_cached set_cached invalidate CacheKey TieredCache`

- **Audit Outcome:** COMPLETED (4/4 sub-items verified & remediated)

#### Sub-Audit Checklist
- [x] **CC-04.01 — Tenant Scoping in Cache Keys**
  - Audited canonical `CacheKeyBuilder` and cache key builders across `rustok-cache`, `apps/server/src/middleware/tenant.rs`, `channel.rs`, `locale.rs`, `services/field_definition_cache.rs`, `rbac_runtime.rs`, and `rustok-seo`.
  - Confirmed that all tenant-sensitive cache instances strictly isolate keys by `tenant_id` and maintain independent per-tenant version counters (`token(tenant_id)`), preventing cross-tenant data leaks.
- [x] **CC-04.02 — Mutation Path Invalidation**
  - Audited mutation paths across tenant, channel, field definitions, and RBAC roles.
  - Verified atomic event interceptor `TenantCacheGenerationTransport` coupling tenant lifecycle events with generation bumps and durable versioned invalidation records (`DurableCacheInvalidationRecord`) prior to downstream delivery.
  - Verified `field_definition_cache_reconciliation` and `channel_cache_invalidation` for supervised background state reconciliation and gap recovery.
- [x] **CC-04.03 — Stampede & Dogpile Prevention**
  - Audited `CacheLoadCoordinator` (`load_or_fill`, `load_or_fill_with_policy`) in `crates/modules/rustok-cache/src/service.rs`: verified per-key/backend request coalescing, bounded in-flight gates, and cancellation-safe lease releases.
  - Verified deterministic TTL jitter in `load_or_fill_with_policy` and `NegativeCachePolicy`, bounded stale-while-revalidate via `CacheRefreshCoordinator` using atomic CAS, and token-safe distributed leases with Lua release scripts.
  - Verified database-backed hot misses in middleware (`locale.rs`, `channel.rs`, `redirects.rs`) use Moka `.try_get_with(`.
- [x] **CC-04.04 — Negative Caching & Multi-Node Sync**
  - Audited `NegativeCachePolicy`: verified strict non-zero schema version, non-zero bounded TTL, and explicit typed negative reasons (`store_negative`), ensuring transient outages or network errors are never cached as negative domain results.
  - Audited multi-node state reconciliation in `crates/modules/rustok-cache/src/fallback.rs`: verified that `reconcile_local_state` re-confirms primary health, replays all retained pending invalidations, drops unacknowledged degraded writes, and maintains fail-closed state until complete replay succeeds.
  - Verified that all 5 cache backend wrappers (`GenerationAwareCacheBackend`, `GenerationRecoveryHealthBackend`, `ObservedCacheBackend`, `DegradationAwareFallbackBackend`, `InstrumentedCacheBackend`) properly forward `reconcile_local_state`.
  - Reconciled single-owner invariant test in `apps/server/tests/cache_architecture_guard.rs` (checking `bounded_invalidation.rs` for `pub fn acknowledge_recovery(`). Verified all 34 architectural guard invariants via `verify-cache-architecture-guards.mjs` (PASSED). Verified `rustok-cache` and `rustok-tenant` compile with 0 errors/warnings (`cargo check -p rustok-cache -p rustok-tenant`).

---

### CC-05: Event Bus, Transactional Outbox & Async Messaging

- **Canonical Documents:** [docs/references/outbox/README.md](../references/outbox/README.md), [docs/architecture/event-flow-contract.md](../architecture/event-flow-contract.md), [docs/standards/transactional-outbox.md](./transactional-outbox.md).
- **Primary Owner Crates:** `rustok-events`, `rustok-outbox`, `rustok-events-module`, `rustok-iggy`.
- **CodeGraph Target:** `publish_in_tx TransactionalEventBus Outbox sys_events publish`

- **Audit Outcome:** COMPLETED (4/4 sub-items verified & remediated)

#### Sub-Audit Checklist
- [x] **CC-05.01 — Atomic `publish_in_tx` Invariant**
  - Audited all domain mutation operations across all modules (`rustok-blog`, `rustok-comments`, `rustok-commerce`, `rustok-content`, `rustok-forum`, `rustok-product`, `rustok-profiles`, `rustok-seo`, `rustok-tenant`, `rustok-rbac`).
  - Confirmed that domain state mutations and event writes to the outbox (`sys_events`) are strictly bound within the same database transaction via `TransactionalEventBus::publish_in_tx` / `publish_root_in_tx` before `txn.commit()`.
  - Zero post-commit event publishing calls exist for critical domain events.
- [x] **CC-05.02 — Event Payload Integrity & PII Boundary**
  - Audited `EventContract` and `ContractEventPayload` in `crates/libs/rustok-events/src/contract.rs`.
  - Confirmed that event contracts are strictly sealed (`sealed::Sealed`) and implement `ValidateEvent`, preventing unregistered event types or dynamic string names from being dispatched.
  - Verified that sensitive credentials, unhashed passwords, raw tokens, and secret parameters are excluded from event payloads.
- [x] **CC-05.03 — Consumer Idempotency**
  - Audited `OutboxRelay` (`crates/modules/rustok-outbox/src/relay.rs`): verified batch claiming via PostgreSQL `FOR UPDATE SKIP LOCKED`, lease-based concurrency controls (`claimed_by`, `claimed_at`, `claim_ttl`), and status transitions (`Pending` -> `Dispatched`).
  - Audited `decode_envelope`: verified strict validation ensuring row metadata (`event_type`, `schema_version`) matches the deserialized envelope before dispatch.
  - Audited consumers across modules: confirmed projections and indexes update state idempotently based on authoritative entity IDs, versions, and receipts (`ContractEventWriteOnceError`).
- [x] **CC-05.04 — Outbox Retention & DLQ Pipelines**
  - Audited `OutboxRetention` (`crates/modules/rustok-outbox/src/retention.rs`): verified that only successfully dispatched rows (`Status::Dispatched`) with non-null `dispatched_at` older than the retention window are pruned in bounded batches (`DEFAULT_OUTBOX_RETENTION_BATCH_SIZE = 500`). `Pending` and `DeadLetter` rows are never pruned.
  - Audited DLQ tenant scoping and RBAC in `apps/server/src/controllers/admin_events.rs`: verified that DLQ replay requires `logs:manage` (`Permission::new(Resource::Logs, Action::Manage)`), DLQ queries reject client-supplied `tenant_id` and strictly scope by authenticated `CurrentTenant(tenant).id`, and replay uses `sys_event_tenant_condition` to fail closed as 404 for mismatched tenants.
  - Reconciled checkpoint references in `crates/modules/rustok-iggy/contracts/evidence/dlq-duplicate-alert-*.json` and verify scripts, aligning with canonical consolidated docs.
  - Verified with all 8 core outbox & event verify scripts (PASSED) and scoped check `cargo check -p rustok-events -p rustok-outbox -p rustok-outbox-storage -p rustok-iggy` (PASSED, 0 errors, 0 warnings).

---

### CC-06: API Surfaces, Transports & Error Hygiene

- **Canonical Documents:** [docs/architecture/api.md](../architecture/api.md), [docs/architecture/dataloader.md](../architecture/dataloader.md), [docs/standards/errors.md](./errors.md).
- **Primary Owner Crates:** `apps/server/src/controllers/*`, `apps/server/src/routes.rs`, `rustok-api`, `rustok-web`, `rustok-mcp`.
- **CodeGraph Target:** `build_application_router HttpError ErrorBody MergedObject McpRouter Dataloader`

#### Sub-Audit Checklist
- [ ] **CC-06.01 — Error Envelope Consistency**
- [ ] **CC-06.02 — DTO vs SeaORM Entity Boundary**
- [ ] **CC-06.03 — GraphQL Dataloader & N+1 Elimination**
- [ ] **CC-06.04 — List Pagination & Limit Clamping**
- [ ] **CC-06.05 — MCP Tool Schema Validation**

---

### CC-07: Multi-Tenancy & Effective Context Isolation

- **Canonical Documents:** [crates/modules/rustok-tenant/docs/README.md](../../crates/modules/rustok-tenant/docs/README.md).
- **Primary Owner Crates:** `rustok-tenant`, `rustok-channel`, `apps/server/src/middleware/tenant.rs`.
- **CodeGraph Target:** `TenantContext resolve_tenant Column::TenantId ensure_tenant_access`

#### Sub-Audit Checklist
- [ ] **CC-07.01 — Universal Query Filter Invariant**
- [ ] **CC-07.02 — Request Host Smuggling & Boundary Defense**
- [ ] **CC-07.03 — Cross-Tenant Relation Verification**
- [ ] **CC-07.04 — Context Propagation**

---

### CC-08: Database Transactions, Concurrency & Data Integrity

- **Canonical Documents:** [docs/architecture/database.md](../architecture/database.md), [AGENTS.md](../../AGENTS.md) §7.
- **Primary Owner Crates:** `migration/`, `crates/modules/*/migration/`, SeaORM entities.
- **CodeGraph Target:** `DatabaseConnection::begin TransactionTrait find_by_id insert update`

#### Sub-Audit Checklist
- [ ] **CC-08.01 — Atomic Multi-Table Mutations**
- [ ] **CC-08.02 — TOCTOU Race Condition Guards**
- [ ] **CC-08.03 — Soft Delete & Archive Discipline**
- [ ] **CC-08.04 — Non-Blocking Transaction Scope**

---

### CC-09: Background Workers, Schedulers & Async Runtime Safety

- **Canonical Documents:** [docs/guides/runtime-guardrails.md](../guides/runtime-guardrails.md), [AGENTS.md](../../AGENTS.md) §10, §14.
- **Primary Owner Crates:** `crates/workers/*`, `rustok-workflow`, `rustok-sandbox`, `apps/server/src/services/app_runtime.rs`.
- **CodeGraph Target:** `spawn_worker StopHandle AppRuntime shutdown_runtime_workers tokio::fs`

#### Sub-Audit Checklist
- [ ] **CC-09.01 — Non-Blocking Async Invariant**
- [ ] **CC-09.02 — Graceful Worker Drain**
- [ ] **CC-09.03 — Concurrency Limiting**
- [ ] **CC-09.04 — Sandbox Isolation**

---

### CC-10: Observability, Distributed Tracing & Telemetry

- **Canonical Documents:** [docs/standards/logging.md](./logging.md), [docs/standards/opentelemetry-integration.md](./opentelemetry-integration.md), [docs/standards/distributed-tracing.md](./distributed-tracing.md).
- **Primary Owner Crates:** `rustok-telemetry`, `apps/server/src/controllers/metrics.rs`, `apps/server/src/controllers/health.rs`.
- **CodeGraph Target:** `init_telemetry register_metrics tracing::instrument health_check`

#### Sub-Audit Checklist
- [ ] **CC-10.01 — Ad-Hoc Print Elimination**
- [ ] **CC-10.02 — PII & Secret Redaction in Spans**
- [ ] **CC-10.03 — Telemetry Lifecycle Safety**
- [ ] **CC-10.04 — Health & Readiness Probes**

---

### CC-11: Dynamic Settings, Tenant Configuration & Secrets

- **Canonical Documents:** [docs/architecture/settings.md](../architecture/settings.md), [DECISIONS/2026-09-22-platform-module-settings-architecture.md](../../DECISIONS/2026-09-22-platform-module-settings-architecture.md).
- **Primary Owner Crates:** `apps/server/src/services/settings_service.rs`, `crates/modules/*/settings`.
- **CodeGraph Target:** `SettingsService load_tenant_settings Zeroize`

#### Sub-Audit Checklist
- [ ] **CC-11.01 — In-Memory Secret Zeroization & Redaction**
- [ ] **CC-11.02 — Dynamic Settings Schema Validation**
- [ ] **CC-11.03 — Hierarchy & Fallback Resolution**
- [ ] **CC-11.04 — Environment Variable Containment**

---

### CC-12: Internationalization (i18n), Localization & Translation Storage

- **Canonical Documents:** [docs/architecture/i18n.md](../architecture/i18n.md), [docs/architecture/database.md](../architecture/database.md).
- **Primary Owner Crates:** `rustok-translation`, `rustok-translation-targets`, `rustok_flutter_i18n`, `apps/server/src/middleware/locale.rs`.
- **CodeGraph Target:** `constrain_locale_to_tenant TranslationService _translations`

#### Sub-Audit Checklist
- [ ] **CC-12.01 — Relational Translation Schema**
- [ ] **CC-12.02 — Fallback Negotiation Chain**
- [ ] **CC-12.03 — User-Facing String Decoupling**
- [ ] **CC-12.04 — Fluent & ICU Format Safety**

---

### CC-13: Documentation, ADRs, Manifests & Verification Gates

- **Canonical Documents:** [docs/index.md](../index.md), [DECISIONS/README.md](../../DECISIONS/README.md), [AGENTS.md](../../AGENTS.md) §1, §2.
- **Primary Owner Crates:** Platform documentation, `xtask/`, `scripts/verify/*`.
- **CodeGraph Target:** `build_registry RusToKModule modules.toml rustok-module.toml`

#### Sub-Audit Checklist
- [ ] **CC-13.01 — Registry vs Manifest Parity**
- [ ] **CC-13.02 — Active ADR Alignment**
- [ ] **CC-13.03 — Reference Schema Parity**
- [ ] **CC-13.04 — Documentation Integrity**

---

### CC-14: Dynamic Extensibility, Flex Fields & Runtime Scripting

- **Canonical Documents:** [crates/modules/flex/README.md](../../crates/modules/flex/README.md), [crates/modules/alloy/README.md](../../crates/modules/alloy/README.md), [DECISIONS/2026-07-11-neutral-sandbox-foundation.md](../../DECISIONS/2026-07-11-neutral-sandbox-foundation.md).
- **Primary Owner Crates:** `flex`, `alloy`, `crates/workers/rustok-sandbox`.
- **CodeGraph Target:** `FieldDefinitionService FieldDefRegistry flex_attached_localized_values RhaiEngine AlloyScript`

#### Sub-Audit Checklist
- [ ] **CC-14.01 — Unified Donor Opt-in**
- [ ] **CC-14.02 — Attached Multilingual Separation**
- [ ] **CC-14.03 — Fail-Closed Data Classification & AI Export**
- [ ] **CC-14.04 — Rhai/Alloy Sandbox Safety**

---

### CC-15: Object Storage, Media Assets & Direct-Upload Lifecycle

- **Canonical Documents:** [crates/utils/rustok-storage/README.md](../../crates/utils/rustok-storage/README.md), [crates/modules/rustok-media/docs/README.md](../../crates/modules/rustok-media/docs/README.md), [DECISIONS/2026-07-22-direct-object-store-runtime-owner-local-lifecycle.md](../../DECISIONS/2026-07-22-direct-object-store-runtime-owner-local-lifecycle.md).
- **Primary Owner Crates:** `rustok-storage`, `rustok-media`, `crates/modules/rustok-media-transport`.
- **CodeGraph Target:** `ObjectStorage StorageBackend generate_presigned_url MediaService CanonicalObjectKey`

#### Sub-Audit Checklist
- [ ] **CC-15.01 — Tenant-Scoped Object Keys**
- [ ] **CC-15.02 — Path Traversal Prevention**
- [ ] **CC-15.03 — Presigned Upload Expiration & Verification**
- [ ] **CC-15.04 — Media Extraction SSRF Guard**

---

### CC-16: CQRS Read Projections, Search & Index Synchronization

- **Canonical Documents:** [crates/modules/rustok-index/docs/README.md](../../crates/modules/rustok-index/docs/README.md), [crates/modules/rustok-search/README.md](../../crates/modules/rustok-search/README.md), [DECISIONS/2026-07-24-index-storage-layout.md](../../DECISIONS/2026-07-24-index-storage-layout.md).
- **Primary Owner Crates:** `rustok-index`, `rustok-search`.
- **CodeGraph Target:** `IndexService SearchService index_products index_content ReindexHandler`

#### Sub-Audit Checklist
- [ ] **CC-16.01 — CQRS-Lite Separation**
- [ ] **CC-16.02 — Event-Driven Reindexing Consistency**
- [ ] **CC-16.03 — Search Tenant Scoping**
- [ ] **CC-16.04 — Reindex Idempotency & Batching**

---

### CC-17: Ingress Sanitization, Input Validation & SSRF/XSS Defense

- **Canonical Documents:** [docs/guides/input-validation.md](../guides/input-validation.md), [docs/standards/security.md](./security.md).
- **Primary Owner Crates:** `rustok-web`, `packages/richtext`, all DTO modules.
- **CodeGraph Target:** `validator::Validate sanitize_html UrlAllowlist SecurityHeaders`

#### Sub-Audit Checklist
- [ ] **CC-17.01 — Universal DTO Ingress Validation**
- [ ] **CC-17.02 — HTML & Richtext Sanitization & Strict CSP**
- [ ] **CC-17.03 — Outbound SSRF Verification**
- [ ] **CC-17.04 — Path-Based Rate Limiting**
