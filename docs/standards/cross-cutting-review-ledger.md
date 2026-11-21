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
- **Active Phase:** CC-12: Internationalization (i18n), Localization & Translation Storage
- **Completed Phases:** CC-01, CC-02, CC-03, CC-04, CC-05, CC-06, CC-07, CC-08, CC-09, CC-10, CC-11 (11/17 completed — 64.7%)
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
- [x] **[CC-06: API Surfaces, Transports & Error Hygiene](#cc-06-api-surfaces-transports--error-hygiene)** (5/5 completed)
- [x] **[CC-07: Multi-Tenancy & Effective Context Isolation](#cc-07-multi-tenancy--effective-context-isolation)** (4/4 completed)
- [x] **[CC-08: Database Transactions, Concurrency & Data Integrity](#cc-08-database-transactions-concurrency--data-integrity)** (4/4 completed)
- [x] **[CC-09: Background Workers, Schedulers & Async Runtime Safety](#cc-09-background-workers-schedulers--async-runtime-safety)** (4/4 completed)
- [x] **[CC-10: Observability, Distributed Tracing & Telemetry](#cc-10-observability-distributed-tracing--telemetry)** (4/4 completed)
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

- **Audit Outcome:** COMPLETED (5/5 sub-items verified & remediated)

#### Sub-Audit Checklist
- [x] **CC-06.01 — Error Envelope Consistency**
  - Audited `apps/server/src/error.rs`, `crates/libs/rustok-web/src/lib.rs`, and `crates/libs/rustok-api/src/graphql/errors.rs`.
  - Verified that Axum `Error::into_response` maps all domain and transport errors to canonical RFC 7807 compatible `ErrorBody` envelopes (`code`, `message`, `retryable`), ensuring database errors (`sea_orm::DbErr`) and internal crashes are strictly sanitized as `"internal_error"` without leaking connection strings, table names, SQL queries, or backtraces.
  - Verified `port_error_to_http_error` in `rustok-web`: infrastructure errors (`Unavailable`, `Timeout`, `InvariantViolation`) receive standardized non-leaking messages while domain errors preserve typed codes and retry advice.
  - Verified GraphQL error handling: resolvers use `rustok_api::graphql::errors::GraphQLError` (`UNAUTHENTICATED`, `PERMISSION_DENIED`, `INTERNAL_ERROR`, `BAD_USER_INPUT`, `NOT_FOUND`, `CONFLICT`) and redact backend diagnostics in DataLoader failures (`tenant_name_loader_error`).
- [x] **CC-06.02 — DTO vs SeaORM Entity Boundary**
  - Audited controllers in `apps/server/src/controllers/*` and GraphQL resolvers in `crates/modules/*/src/graphql/`.
  - Confirmed zero leakage of raw SeaORM `Model` or `ActiveModel` across API boundaries.
  - Controllers return typed response DTOs (`*Response`), and GraphQL resolvers return strongly typed GraphQL objects (`Gql*`, domain output types), keeping the persistence schema decoupled from public API contracts.
- [x] **CC-06.03 — GraphQL Dataloader & N+1 Elimination**
  - Audited DataLoader implementations: `TenantNameLoader` in `apps/server/src/graphql/loaders.rs`, `ProfileSummaryLoader` in `crates/modules/rustok-profiles/src/loader.rs`, and `ProfileSummaryAudiencePolicy` in `apps/server/src/graphql/profile_summary_policy.rs`.
  - Verified request-scope caching: `DataLoader` is created per-request in `prepare_request` extension, bound to the authenticated `ProfileAccessAudience`.
  - Verified strict tenant and locale isolation: `ProfileSummaryLoaderKey` batches queries grouped strictly by `(tenant_id, requested_locale, tenant_default_locale)`, preventing cross-tenant or cross-locale data leakage during batch resolution.
  - Confirmed N+1 query elimination in `rustok-forum` resolvers (`member_card_query.rs`, `mutation.rs`, `query_runtime.rs`) and profile queries.
- [x] **CC-06.04 — List Pagination & Limit Clamping**
  - Audited pagination across server endpoints (`admin_events.rs`, `mcp.rs`, `flex.rs`, `channel.rs`, `oauth.rs`).
  - Verified bounds enforcement: list endpoints enforce bounded limits using `.clamp(1, max)` or helper `clamp_limit(limit, default, max)` (e.g. DLQ events clamped `1..=200` with default 100; MCP audit events clamped `1..=200` with default 100; MCP scaffold drafts clamped `1..=100` with default 50).
  - Verified database-level pagination in module queries (e.g. `blog_tag_usage.paginate(&txn, per_page)` using `Ord::max(filter.page, 1)` and saturating arithmetic).
- [x] **CC-06.05 — MCP Tool Schema Validation**
  - Audited MCP tool routing and execution in `crates/modules/rustok-mcp/src/server.rs`, `crates/modules/rustok-mcp/src/tools.rs`, and `apps/server/src/controllers/mcp.rs`.
  - Verified JSON Schema derivation via `schemars::schema_for!` for all tools in `list_tools`.
  - Verified payload validation: tool inputs are strictly validated against strongly typed request structs (`ModuleLookupRequest`, `ModuleQueryRequest`, `ScaffoldModuleRequest`, `ReviewModuleScaffoldRequest`, `ApplyModuleScaffoldRequest`, `AlloyPublishedReleaseImportRequest`) before execution, returning typed `invalid_params` / `invalid_arguments` error envelopes on malformed inputs.
  - Reconciled stale file paths in `verify-pricing-admin-graphql-error-safety.mjs` and `verify-commerce-graphql-query-error-boundary.mjs` reflecting module refactoring.
  - Added comprehensive verification in `scripts/verify/verify-api-transport-error-hygiene.mjs` (PASSED). Verified scoped compilation with `cargo check -p rustok-api -p rustok-web -p rustok-mcp` (PASSED, 0 errors, 0 warnings).

---

### CC-07: Multi-Tenancy & Effective Context Isolation

- **Canonical Documents:** [crates/modules/rustok-tenant/docs/README.md](../../crates/modules/rustok-tenant/docs/README.md).
- **Primary Owner Crates:** `rustok-tenant`, `rustok-channel`, `apps/server/src/middleware/tenant.rs`.
- **CodeGraph Target:** `TenantContext resolve_tenant Column::TenantId ensure_tenant_access`

- **Audit Outcome:** COMPLETED (4/4 sub-items verified & remediated)

#### Sub-Audit Checklist
- [x] **CC-07.01 — Universal Query Filter Invariant**
  - Audited SeaORM entity queries and repositories across all domain modules (`rustok-tenant`, `rustok-channel`, `rustok-blog`, `rustok-forum`, `rustok-comments`, `rustok-commerce`, `rustok-product`, `rustok-content`, `rustok-profiles`, `rustok-rbac`, `alloy`, `flex`).
  - Confirmed that all tenant-scoped database lookups, updates, and deletes enforce `.filter(Column::TenantId.eq(tenant_id))` or scoped repository methods (`scoped_by_id`, `ensure_script_scope`, `find_scoped`), preventing cross-tenant data access.
- [x] **CC-07.02 — Request Host Smuggling & Boundary Defense**
  - Audited `apps/server/src/middleware/tenant_resolution.rs`: verified rejection of duplicate `Host` headers (`ConflictingHeaderValues`) to eliminate HTTP smuggling and cache poisoning vectors.
  - Verified effective host extraction using peer IP and trusted reverse-proxy settings (`extract_effective_host`), rejecting spoofed `X-Forwarded-Host` from untrusted clients.
  - Verified host validation with port stripping via `TenantIdentifierValidator::validate_host`.
  - Verified subdomain resolution against configured `base_domains`: rejects naked base domain (`BaseDomainRequiresTenantSlug`), rejects nested subdomains (`NestedSubdomain`), and rejects unmatched hosts (`NoBaseDomainMatch`).
  - Verified dual-header correlation: `validate_resolved_slug` strictly rejects mismatched assertions (`ConflictingTenantAssertions`).
- [x] **CC-07.03 — Cross-Tenant Relation Verification**
  - Audited cross-entity relation creation in `rustok-channel`, `rustok-comments`, `rustok-product`, `rustok-rbac`.
  - Confirmed `RequestFacts` in `rustok-channel/src/resolution.rs` rejects nil tenant ID and validates `detail.channel.tenant_id == facts.tenant_id` for header lookups.
  - Confirmed parent comment lookups in `rustok-comments/src/services.rs` verify `find_comment_in_tx(txn, tenant_id, parent_id)` and enforce `parent.thread_id == thread.id`.
- [x] **CC-07.04 — Context Propagation**
  - Audited end-to-end tenant context flow: `apps/server/src/middleware/tenant.rs` loads validated context and injects `TenantContextExtension(context)` into Axum request extensions.
  - Verified `CurrentTenant` extractor in `apps/server/src/extractors/tenant.rs` extracting context fail-closed (404 on missing).
  - Verified GraphQL propagation: `graphql_handler` injects `tenant_ctx` into `schema.execute` data; WebSocket init validates slug via `resolve_tenant_context_by_slug` and checks channel tenant (`ensure_ws_channel_tenant`).
  - Verified outbox events: all event envelopes enforce non-nil `tenant_id` via `EventValidationError::NilUuid("tenant_id")`.
  - Verified with 10 tenant verification scripts including newly added `verify-tenant-context-isolation.mjs` (PASSED). Scoped compilation `cargo check -p rustok-tenant -p rustok-channel` clean (0 errors, 0 warnings).

---

### CC-08: Database Transactions, Concurrency & Data Integrity

- **Canonical Documents:** [docs/architecture/database.md](../architecture/database.md), [AGENTS.md](../../AGENTS.md) §7.
- **Primary Owner Crates:** `migration/`, `crates/modules/*/migration/`, SeaORM entities.
- **CodeGraph Target:** `DatabaseConnection::begin TransactionTrait find_by_id insert update`

- **Audit Outcome:** COMPLETED (4/4 sub-items verified & remediated)

#### Sub-Audit Checklist
- [x] **CC-08.01 — Atomic Multi-Table Mutations**
  - Audited multi-table mutations and transactional outbox event coupling across domain modules:
    - In `rustok-blog` (`crates/modules/rustok-blog/src/services/post/commands.rs`), `update_post` atomically updates post metadata, route slug claims (`claim_post_route_in_tx`), translations (`upsert_translation_in_tx`), channel visibility (`replace_channel_visibility_in_tx`), taxonomy tags (`sync_post_tags_in_tx`), and outbox event emissions (`self.event_bus.publish_in_tx(&txn, ...)`) within a single transaction committed via `txn.commit().await?`.
    - In `rustok-comments` (`crates/modules/rustok-comments/src/services.rs`), comment creation, status mutations, and comment deletion atomically couple comment state changes with thread counter adjustments (`update_thread_counters_in_tx`) and outbox events (`publish_comment_created_in_tx`, `publish_comment_status_changed_in_tx`) in the same database transaction.
    - In `rustok-tenant` (`crates/modules/rustok-tenant/src/services/tenant_service.rs`), `create_tenant` atomically inserts the tenant record, initial tenant locale record, and publishes `DomainEvent::TenantCreated` in a single transaction before committing.
    - In `rustok-forum` (`crates/modules/rustok-forum/src/services/topic_merge.rs`), `merge_topics` executes all merge reconciliations (subscriptions, tags, votes, read state, audience) in a unified transaction.
- [x] **CC-08.02 — TOCTOU Race Condition Guards**
  - Verified optimistic concurrency control (CAS) in `rustok-blog`: updates filter by `.filter(blog_post::Column::Version.eq(version))` and explicitly check `result.rows_affected != 1` to detect concurrent modifications and return `BlogError::conflict`.
  - Verified row-level locking (`.lock_exclusive()` / `LockType::Update`) in SeaORM queries:
    - `rustok-comments`: comment updates and thread lookups lock rows via `.lock_exclusive()` to prevent concurrent conflicting writes and race conditions.
    - `rustok-outbox`: PostgreSQL polling in `crates/modules/rustok-outbox/src/relay.rs` utilizes `.lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)`, preventing lock contention between competing worker processes and eliminating head-of-line blocking.
    - `flex`: `crates/modules/flex/src/attached_translation_guard.rs` enforces `lock_attached_translation_schema_in_tx` using `SELECT ... FOR UPDATE` (or SQLite UPDATE no-op) on the durable cache generation row before reading definitions, ensuring schema snapshot integrity against concurrent updates or phantom inserts.
    - `rustok-forum`: `crates/modules/rustok-forum/src/services/reply.rs` uses `.lock_exclusive()` on PostgreSQL.
- [x] **CC-08.03 — Soft Delete & Archive Discipline**
  - Verified soft-delete invariant: entities with `deleted_at` (such as `comments`) strictly enforce `.filter(comment::Column::DeletedAt.is_null())` on active lookups.
  - Verified that soft deletions (`delete_comment_in_tx`) atomically mark `active.deleted_at = Set(Some(...))` and adjust parent thread counters (`update_thread_counters_in_tx(txn, &thread, -1, None)`) in the same transaction.
  - Reinstated the canonical `forum-moderation-revision-migration-contract.md` handoff in `crates/modules/rustok-forum/docs/` and indexed it in `crates/modules/rustok-forum/docs/README.md`, maintaining trigger parity and backfill fencing contracts.
- [x] **CC-08.04 — Non-Blocking Transaction Scope**
  - Verified that expensive CPU operations, password hashing, and external calls are executed strictly outside database transactions:
    - `rustok-auth`: Argon2id password hashing (`hash_password`) in `bootstrap.rs` and `cli/src/lib.rs` executes prior to opening database transactions, preventing connection pool exhaustion during key derivation.
    - `rustok-blog`: schedule validation (`validate_schedule_for_update`) and clock sampling (`chrono::Utc::now()`) run prior to acquiring database transactions (`self.db.begin()`).
  - Added automated gate `scripts/verify/verify-db-transactions-concurrency-guards.mjs` (PASSED). Verified with migration and index storage scripts (PASSED) and scoped check `cargo check -p rustok-blog -p rustok-comments -p rustok-forum` (PASSED, 0 errors, 0 warnings).

---

### CC-09: Background Workers, Schedulers & Async Runtime Safety

- **Canonical Documents:** [docs/guides/runtime-guardrails.md](../guides/runtime-guardrails.md), [AGENTS.md](../../AGENTS.md) §10, §14.
- **Primary Owner Crates:** `crates/workers/*`, `rustok-workflow`, `rustok-sandbox`, `apps/server/src/services/app_runtime.rs`.
- **CodeGraph Target:** `spawn_worker StopHandle AppRuntime shutdown_runtime_workers tokio::fs`

- **Audit Outcome:** COMPLETED (4/4 sub-items verified & remediated)

#### Sub-Audit Checklist
- [x] **CC-09.01 — Non-Blocking Async Invariant**
  - Audited standard library blocking I/O across async worker loops and request flows: confirmed zero naked blocking filesystem calls (`std::fs`) in async execution paths.
  - Verified uniform usage of `tokio::fs` for all asynchronous filesystem operations across services and workers (`apps/server/src/host.rs`, `crates/modules/rustok-modules/src/source_object.rs`, `crates/workers/rustok-module-build-worker/src/runner.rs`, `crates/utils/rustok-storage/src/runtime.rs`).
  - Verified `tokio::time::sleep` is used exclusively for asynchronous timer intervals and backoffs; enforced by the platform linter in `crates/utils/rustok-doctor/src/hygiene/mod.rs`.
- [x] **CC-09.02 — Graceful Worker Drain**
  - Audited server lifecycle coordination in `apps/server/src/services/app_lifecycle.rs`: `shutdown_runtime_workers` coordinates a 5-second graceful drain using `StopHandle` and explicitly awaits individual worker handle shutdowns (`OutboxRelayWorkerHandle`, `OutboxRetentionWorkerHandle`, `AuthRetentionWorkerHandle`, `RemoteExecutorReaperHandle`, `SeoBulkWorkerHandle`, `ModuleWorkSchedulerHandle`, `WorkflowCronSchedulerHandle`, `ModuleTransitionWatchdogHandle`, `ModuleEventDispatcherHandle`, `ModuleArtifactMaintenanceHandle`).
  - Audited standalone gRPC worker daemons in `crates/workers/*` (`rustok-artifact-node-agent`, `rustok-artifact-node-controller`, `rustok-artifact-node-reconciler`, `rustok-module-build-worker`, `rustok-verification-worker`): all workers bind `.serve_with_shutdown(listener.address, shutdown_signal())` powered by `rustok-worker-transport::shutdown_signal()`, allowing in-flight requests to complete before process exit.
- [x] **CC-09.03 — Concurrency Limiting**
  - Audited background queues, async dispatchers, and external calls:
    - `rustok-core/src/events/handler.rs`: event dispatcher bounds listener concurrency and queue depth using `Arc<Semaphore>`.
    - `rustok-core/src/resilience/bulkhead.rs`: Bulkhead pattern bounds concurrent execution via `Arc<Semaphore>` based on `max_concurrent_calls`.
    - `rustok-worker-transport/src/lib.rs`: Worker admission controller enforces `concurrency_limit` via `Semaphore` permits (`WorkerPermit`).
    - `rustok-cache/src/refresh.rs`: cache stampede protection bounds concurrent background refreshes via `Semaphore`.
    - `rustok-media/src/image.rs`: image resizing and transcoding bounds concurrency via `Semaphore(max_concurrency)`.
    - `apps/server/src/services/comments_provider_runtime_base.rs`: TCP connection pool bounded via `Semaphore(config.max_connections)`.
- [x] **CC-09.04 — Sandbox Isolation**
  - Audited Rhai guest execution in `crates/workers/rustok-sandbox/`:
    - `SandboxLimits`: strictly bounds execution with defaults `wall_clock_ms: 100`, `instruction_budget: 50_000`, `max_memory_bytes: 64MB`, `max_output_bytes: 1MB`, `max_concurrency: 1`.
    - Rhai engine in `crates/workers/rustok-sandbox/src/rhai.rs`: explicitly sets `set_max_operations`, `set_max_call_levels(16)`, `set_max_string_size`, `set_max_array_size`, `set_max_map_size`, and on-progress wall-clock elapsed check.
    - Zero host filesystem or network capability grants by default; all guest operations execute on memory JSON/Dynamic records with typed error mapping (`EvalAltResult::ErrorTooManyOperations` -> `SandboxError::LimitExceeded`).
  - Added automated gate `scripts/verify/verify-workers-schedulers-runtime-safety.mjs` (PASSED). Verified with scoped compilation `cargo check -p rustok-sandbox -p rustok-worker-transport -p rustok-module-build-worker` (PASSED, 0 errors, 0 warnings).

---

### CC-10: Observability, Distributed Tracing & Telemetry

- **Canonical Documents:** [docs/standards/logging.md](./logging.md), [docs/standards/opentelemetry-integration.md](./opentelemetry-integration.md), [docs/standards/distributed-tracing.md](./distributed-tracing.md).
- **Primary Owner Crates:** `rustok-telemetry`, `apps/server/src/controllers/metrics.rs`, `apps/server/src/controllers/health.rs`.
- **CodeGraph Target:** `init_telemetry register_metrics tracing::instrument health_check`

- **Audit Outcome:** COMPLETED (4/4 sub-items verified & remediated)

#### Sub-Audit Checklist
- [x] **CC-10.01 — Ad-Hoc Print Elimination**
  - Audited production crates and runtime services across the workspace for naked `println!`, `eprintln!`, and `dbg!` calls.
  - Remediated naked `eprintln!` in `crates/workers/rustok-artifact-node-agent/src/agent.rs` (replaced with structured `tracing::error!(%error, "artifact node agent iteration failed")` and added `tracing.workspace = true` to its manifest).
  - Remediated naked `eprintln!` in `crates/modules/flex/src/bin/service.rs` (replaced with `tracing::error!(%error, "Flex shutdown signal unavailable")`).
  - Commented out lingering debug `eprintln!("Column Deserialize Error: {:?}", e)` in `crates/utils/serde-columnar-vendored/src/column/serde_impl.rs` across all 5 column visitor decoders.
  - Replaced print example in `crates/libs/rustok-telemetry/src/otel.rs` doc comment with `tracing::info!`.
- [x] **CC-10.02 — PII & Secret Redaction in Spans**
  - Audited `#[instrument]` across domain modules and host services against [docs/standards/logging.md](./logging.md).
  - Remediated sensitive and user-authored parameter leakage into distributed traces:
    - In `apps/server/src/services/auth_lifecycle.rs`: added `email`, `name`, and `prepared` (carrying `password_hash`) to `skip(...)` across `create_user_runtime`, `create_user_db`, `create_user_in_tx`, `publish_user_event_in_tx`, and `update_profile_runtime`, preventing passwords, hashes, and PII from entering span attributes.
    - In `crates/modules/rustok-newsletter/src/services/subscriber.rs`: removed unhashed raw `email = %input.email` from span fields in `SubscriberService::subscribe`, replacing with bounded `subscriber_id`.
    - In `crates/modules/rustok-newsletter/src/services/campaign.rs`: removed user-authored `title = %input.title` from span fields in `CampaignService::create`, replacing with bounded `campaign_id`.
    - In `apps/server/src/services/oauth_app.rs`: removed user-authored `name = %input.name` from span fields in `OAuthAppService::create_app`, replacing with bounded `client_id`.
    - In `crates/modules/rustok-workflow/src/services/workflow_service.rs`: removed user-authored `name = %input.name` from span fields in `WorkflowService::create`, replacing with bounded `workflow_id`.
- [x] **CC-10.03 — Telemetry Lifecycle Safety**
  - Discovered and eliminated a critical process startup crash hazard in `crates/libs/rustok-telemetry/src/lib.rs` and `otel.rs`: `rustok_telemetry::init` previously attempted `tokio::task::block_in_place(|| Handle::current().block_on(...))` to initialize OpenTelemetry. When invoked prior to the Tokio runtime entry in `apps/server/src/main.rs`, setting `OTEL_ENABLED=true` panicked with "no reactor running".
  - Refactored OpenTelemetry layer construction into synchronous `build_otel_layer(&config)`, allowing safe initialization without requiring an active Tokio runtime context, while preserving graceful fallback to structured logging when exporter creation fails.
- [x] **CC-10.04 — Health & Readiness Probes**
  - Audited health endpoints in `apps/server/src/controllers/health.rs`:
    - `/health`: lightweight process status check returning version.
    - `/health/live`: K8s liveness probe returning 200 OK.
    - `/health/ready`: K8s readiness probe evaluating critical vs non-critical dependencies.
    - Concurrency and starvation guards: probe stream uses `.buffer_unordered(READINESS_CONCURRENCY)` to bound in-flight health checks; aggregate probe deadline enforced via `READINESS_TIMEOUT`.
    - Circuit breaker pattern: guarded dependency checks track failure streaks (`on_check_failure`) and trip open for a cooldown window (`circuit_open_for`), preventing cascaded failures and connection exhaustion against failing dependencies.
    - Criticality mapping: failure of a `Critical` dependency yields 503 (`SERVICE_UNAVAILABLE`), while failure of a `NonCritical` dependency yields 200 (`OK`) with `status: "degraded"` and degraded reasons.
    - Remediated `HealthRegistry::check_all` in `crates/libs/rustok-core/src/health/mod.rs` so that an unhealthy non-critical check sets status to `Degraded` instead of being incorrectly ignored as `Healthy`.
  - Added automated gate `scripts/verify/verify-observability-telemetry-health.mjs` (PASSED). Verified scoped compilation and test suites: `cargo test -p rustok-core --lib`, `cargo test -p rustok-telemetry --lib`, `cargo check -p rustok-artifact-node-agent -p rustok-newsletter -p rustok-workflow -p flex` (PASSED, 0 errors, 0 warnings).

---

### CC-11: Dynamic Settings, Tenant Configuration & Secrets

- **Canonical Documents:** [docs/architecture/settings.md](../architecture/settings.md), [DECISIONS/2026-09-22-platform-module-settings-architecture.md](../../DECISIONS/2026-09-22-platform-module-settings-architecture.md).
- **Primary Owner Crates:** `apps/server/src/services/settings_service.rs`, `crates/modules/*/settings`, `crates/utils/rustok-secrets`.
- **CodeGraph Target:** `SettingsService load_tenant_settings Zeroize`

- **Audit Outcome:** COMPLETED (4/4 sub-items verified & remediated)

#### Sub-Audit Checklist
- [x] **CC-11.01 — In-Memory Secret Zeroization & Redaction**
  - Audited `crates/utils/rustok-secrets`: verified `secrecy::SecretString` with automatic `ZeroizeOnDrop` in RAM and redaction on Debug/Display.
  - Enhanced `redact_secrets` in `apps/server/src/services/settings_service.rs` to redact OAuth secrets (`client_secret`, `clientSecret`, `secret`) across root-level, provider map, and provider array structures in addition to email and cache secrets.
  - Enhanced `preserve_oauth_secret_fields` and `preserve_email_secret_fields` to recognize placeholder masks (`"[REDACTED]"`, `"********"`, `""`) and clear masks to empty strings when no previous secret exists, preventing placeholder masks from ever being persisted as live credentials. Added support for both object/map and array provider representations in OAuth preservation.
- [x] **CC-11.02 — Dynamic Settings Schema Validation**
  - Audited all categories in `category::ALL` (`general`, `email`, `rate_limit`, `features`, `i18n`, `oauth`, `cache`).
  - Implemented typed validator structs implementing `SettingsValidator`:
    - `FeaturesSettingsValidator`: strictly validates boolean flags (supporting both snake_case and camelCase) and rejects unknown flags.
    - `GeneralSettingsValidator`: validates bounds on site name (1..=255 chars), description (<= 2000 chars), absolute URL parsing via `url::Url` (enforcing http/https and host presence), and boolean maintenance mode.
    - `I18nSettingsValidator`: validates BCP 47 alphanumeric subtags (1..=8 chars per tag component), rejects malformed tags, enforces non-empty `locales`, and validates `default_locale` inclusion in `locales`.
    - `OAuthSettingsValidator`: validates provider structures (map and array format) and enforces non-empty provider identifiers.
  - Registered all 7 category validators in `ValidatorRegistry::default()`.
  - Updated `ValidatorRegistry::validate` to fail closed (returning `Err`) if an unregistered category is evaluated instead of silently passing through.
- [x] **CC-11.03 — Hierarchy & Fallback Resolution**
  - Audited 3-level fallback hierarchy (1. DB tenant row in `platform_settings`, 2. deployment YAML `settings.rustok.<category>`, 3. `{}` compiled defaults).
  - Verified bounded CAS concurrency control (`update_if_current` verifying `expected_schema_version`, optimistic locking query, atomic version increment, and transactional outbox emission `DomainEvent::PlatformSettingsChanged` within the same transaction).
- [x] **CC-11.04 — Environment Variable Containment**
  - Audited domain modules for direct `env::var` usage; verified that per-tenant settings never bypass dynamic database resolution and `SettingsService`.
  - Verified that environment variables are strictly contained to process bootstrap and deployment configuration.
  - Added automated gate `scripts/verify/verify-dynamic-settings-secrets.mjs` (PASSED). Verified scoped compilation and tests: `cargo test -p rustok-secrets --lib`, `cargo check -p rustok-search -p rustok-modules -p rustok-seo` (PASSED, 0 errors, 0 warnings).

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
