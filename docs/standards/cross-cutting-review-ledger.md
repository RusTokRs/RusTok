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
- **Active Cycle:** Cross-Cutting Cycle 1
- **Active Phase:** CC-12: Internationalization (i18n), Localization & Translation Storage
- **Completed Phases:** CC-01 (FBA & Module Sovereignty), CC-02 (FFA & UI Adapter Purity), CC-03 (RBAC, Auth & Security Enforcement), CC-04 (Caching, Invalidation & Tenant Cache Isolation), CC-05 (Event Bus, Transactional Outbox & Async Messaging), CC-06 (API Surfaces, Transports & Error Hygiene), CC-07 (Multi-Tenancy & Effective Context Isolation), CC-08 (Database Transactions, Concurrency & Data Integrity), CC-09 (Background Workers, Schedulers & Async Runtime Safety), CC-10 (Observability, Distributed Tracing & Telemetry), CC-11 (Dynamic Settings, Tenant Configuration & Secrets) (11/17 completed)
- **Active Branch:** `main`

---

## Cross-Cutting Audit Execution Protocol

For each cross-cutting phase, execute the strict closed-loop protocol:

1. **CodeGraph Discovery Pass (`codegraph_explore`):** Query the cross-module symbols, traits, and facades that define the capability. Inspect how callers across `apps/server`, `crates/modules/*`, `crates/libs/*`, and `crates/ui/*` consume the capability.
2. **Invariant & Anti-Pattern Map:** Verify code against [docs/standards/patterns-vs-antipatterns.md](./patterns-vs-antipatterns.md), [docs/standards/forbidden-actions.md](./forbidden-actions.md), and [docs/ai/KNOWN_PITFALLS.md](../ai/KNOWN_PITFALLS.md).
3. **Blast Radius Isolation:** Group all violations by root cause. If an invariant is broken across multiple modules, treat them as part of the unified cross-cutting remediation unit.
4. **Atomic Remediation:** Fix the underlying contract, types, and implementations simultaneously. Preserve compile-time correctness and module sovereign boundaries.
5. **Immediate Adjacent Re-Audit:** Verify upstream callers (controllers/handlers/resolvers) and downstream persistence/events.
6. **Scoped Verification:** Run `cargo check -p <crate>` and `cargo test -p <crate>` for all modified crates. Never claim test passes without execution.
7. **Commit & Ledger Update:** Record exact SHA, changes, and verification evidence in this ledger.

---

## Phase Backlog — Cross-Cutting Capability Sequence (17 Phases Total)

- [x] **[CC-01: Fluid Backend Architecture (FBA) & Module Sovereignty](#cc-01-fluid-backend-architecture-fba--module-sovereignty)** (6/6 completed — 100% PASS)
- [x] **[CC-02: Fluid Frontend Architecture (FFA) & UI Adapter Purity](#cc-02-fluid-frontend-architecture-ffa--ui-adapter-purity)** (5/5 completed — 100% PASS)
- [x] **[CC-03: RBAC, Auth & Security Enforcement](#cc-03-rbac-auth--security-enforcement)** (8/8 completed — 100% PASS)
- [x] **[CC-04: Caching, Invalidation & Tenant Cache Isolation](#cc-04-caching-invalidation--tenant-cache-isolation)** (4/4 completed — 100% PASS)
- [x] **[CC-05: Event Bus, Transactional Outbox & Async Messaging](#cc-05-event-bus-transactional-outbox--async-messaging)** (4/4 completed — 100% PASS)
- [x] **[CC-06: API Surfaces, Transports & Error Hygiene](#cc-06-api-surfaces-transports--error-hygiene)** (5/5 completed — 100% PASS)
- [x] **[CC-07: Multi-Tenancy & Effective Context Isolation](#cc-07-multi-tenancy--effective-context-isolation)** (4/4 completed — 100% PASS)
- [x] **[CC-08: Database Transactions, Concurrency & Data Integrity](#cc-08-database-transactions-concurrency--data-integrity)** (4/4 completed — 100% PASS)
- [x] **[CC-09: Background Workers, Schedulers & Async Runtime Safety](#cc-09-background-workers-schedulers--async-runtime-safety)** (4/4 completed — 100% PASS)
- [x] **[CC-10: Observability, Distributed Tracing & Telemetry](#cc-10-observability-distributed-tracing--telemetry)** (4/4 completed — 100% PASS)
- [x] **[CC-11: Dynamic Settings, Tenant Configuration & Secrets](#cc-11-dynamic-settings-tenant-configuration--secrets)** (4/4 completed — 100% PASS)
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

#### Sub-Audit Checklist
- [x] **CC-01.01 — Module Physical Shape & Hexagonal Separation:** Verified no executable `.rs` code is hidden in `contracts/` or `docs/`. Ran `node scripts/verify/verify-module-source-layout.mjs` (PASSED for reference modules).
- [x] **CC-01.02 — Inter-Module Dependency Sovereignty:** Ran `python scripts/architecture_dependency_guard.py` (PASSED); enforced module import boundaries with zero private cross-module coupling.
- [x] **CC-01.03 — Port & Seam Purity:** Verified cross-module interactions communicate strictly through owner-defined port traits (`RbacPermissionDecisionPort`, `PaymentGatewayPort`, etc.) and event bus; verified `cargo test -p rustok-api --lib ports` (8/8 passed).
- [x] **CC-01.04 — FBA Registry & Capability Profiles:** Verified `rustok-fba` capability definitions (`BackendTopology`, `TransportProfile`, `CapabilityId`, `FbaProviderDescriptor`, `FbaConsumerDependency`) and embedded-first execution; verified `cargo test -p rustok-fba` (5/5 passed).
- [x] **CC-01.05 — Host Minimalism (`apps/server`):** Verified `apps/server` acts purely as composition root and host with zero leaked module business rules; verified contract tests (6/6 passed) and scoped build (0 errors).
- [x] **CC-01.06 — CLI Package Decoupling:** Verified module maintenance commands reside in isolated `cli/` packages over `rustok-cli-core`; verified `cargo test -p rustok-cli-core` (6/6 passed).

---

### CC-02: Fluid Frontend Architecture (FFA) & UI Adapter Purity

- **Canonical Documents:** [docs/UI/module-package-architecture.md](../UI/module-package-architecture.md), [docs/UI/graphql-architecture.md](../UI/graphql-architecture.md), [AGENTS.md](../../AGENTS.md) §5.
- **Primary Owner Crates:** `crates/modules/*/admin`, `crates/modules/*/storefront`, `crates/ui/*`, `apps/admin`, `apps/storefront`, `apps/next-admin`, `apps/next-frontend`, `apps/next-admin/packages/*`, `apps/next-frontend/packages/*`.
- **CodeGraph Target:** `leptos_dom view rsx TransportFacade ViewModel rustok_ui`

#### Sub-Audit Checklist
- [x] **CC-02.01 — Zero Framework in Core:** Verified all 51 module admin/storefront `core/` source files contain strictly zero imports of `leptos::*` or `dioxus::*`; pure Rust data types verified by unit tests.
- [x] **CC-02.02 — Zero Domain Logic in UI Adapters:** Verified admin and storefront UI adapters contain zero business calculations or raw command construction; command preparation and validation delegated to `core/`.
- [x] **CC-02.03 — Transport Facade Boundary:** Ran `node scripts/verify/verify-ui-client-dependency-boundary.mjs` (PASSED for all 70 UI packages); UI adapters access backend capabilities strictly via typed transport facades.
- [x] **CC-02.04 — Design Token & UI Primitive Reuse:** Ran `node scripts/verify/verify-ui-parity.mjs` (PASSED for tokens, recipes, and cross-framework components) and `node scripts/verify/verify-ui-grid-parity.mjs` (PASSED: 10 module twins).
- [x] **CC-02.05 — Dual-Stack UI & Admin Synchronization (Leptos ↔ Next.js Parity):** Ran `node scripts/verify/verify-ui-i18n-parity.mjs` (PASSED across 76 catalog directories); verified route availability, workspace parity, and schema alignment across all 34 client packages.

---

### CC-03: RBAC, Auth & Security Enforcement

- **Canonical Documents:** [crates/modules/rustok-rbac/docs/README.md](../../crates/modules/rustok-rbac/docs/README.md), [crates/modules/rustok-auth/docs/README.md](../../crates/modules/rustok-auth/docs/README.md), [docs/standards/security.md](./security.md).
- **Primary Owner Crates:** `rustok-rbac`, `rustok-auth`, `rustok-api`, `apps/server/src/middleware/auth_context.rs`, `apps/server/src/services/rbac_*.rs`.
- **CodeGraph Target:** `RequirePermission check_permission RbacService AuthContext PermissionKey`

#### Sub-Audit Checklist
- [x] **CC-03.01 — Controller & Resolver Permission Coverage:** Verified REST controllers in `apps/server/src/controllers/*` and GraphQL resolvers bind permission checks; OpenAPI DTO derives strictly separated into `rustok-auth::rest` via `node scripts/verify/verify-auth-admin-boundary.mjs` and `node scripts/verify/verify-rbac-admin-boundary.mjs` (PASSED).
- [x] **CC-03.02 — Tenant Isolation in Security Context & Authoritative Queries:** Verified explicit principal kind typing and tenant enforcement without metadata fallback via `node scripts/verify/verify-rbac-explicit-principal-kind.mjs`, `node scripts/verify/verify-auth-admin-tenant-scope.mjs`, and `node scripts/verify/verify-rbac-admin-tenant-scope.mjs` (PASSED).
- [x] **CC-03.03 — RBAC Cache Invalidation & Canonical API Seams:** Role and permission mutations commit atomically within transaction boundaries and trigger user-scoped cache invalidation; verified `node scripts/verify/verify-rbac-owner-role-mutation-contract.mjs` (PASSED) and `node scripts/verify/verify-server-platform-settings-tenant-scope.mjs` (PASSED).
- [x] **CC-03.04 — Password & Token Security:** Verified Argon2id password hashing parameters, unique salts, 256-bit entropy refresh tokens, RFC 7519 JWT validation, and token revocation via unit tests (`cargo test -p rustok-auth --lib` — 74/74 PASSED).
- [x] **CC-03.05 — FFA Purity & i18n in RBAC UI:** Enforced framework-free view models in `crates/modules/rustok-rbac/admin/src/core/` and Fluent message resolution (`cargo test -p rustok-rbac-admin --lib` — 8/8 PASSED).
- [x] **CC-03.06 — Built-in Role Resource Parity & Completeness:** Audited system role definitions across `SUPER_ADMIN_PERMISSIONS`, `ADMIN_PERMISSIONS`, `MANAGER_PERMISSIONS`, and `CUSTOMER_PERMISSIONS` in `rustok-core::rbac` (`cargo test -p rustok-core --lib rbac::tests` — 19/19 PASSED).
- [x] **CC-03.07 — GraphQL Mutation Race Elimination & Role Invalidation:** Replaced direct `user_roles::Entity` SeaORM model coupling in `apps/server/src/graphql/rbac_runtime.rs` with portable backend-aware statement queries and deterministic user invalidation (`cargo check -p rustok-server --lib --no-default-features` — 0 errors).
- [x] **CC-03.08 — Platform Permission Catalog Exhaustiveness:** Verified resource and permission catalog parity across `rustok-core::rbac`, `rustok-api::Resource`, and GraphQL catalogs (`cargo test -p rustok-rbac --lib` — 83/83 PASSED).

---

### CC-04: Caching, Invalidation & Tenant Cache Isolation

- **Canonical Documents:** [crates/modules/rustok-cache/README.md](../../crates/modules/rustok-cache/README.md), [docs/standards/patterns-vs-antipatterns.md](./patterns-vs-antipatterns.md) §6.
- **Primary Owner Crates:** `rustok-cache`, `rustok-tenant`, `apps/server/src/services/settings_service.rs`.
- **CodeGraph Target:** `CacheService get_cached set_cached invalidate CacheKey TieredCache`

#### Sub-Audit Checklist
- [x] **CC-04.01 — Tenant Scoping in Cache Keys:** Verified `CacheKeyBuilder` enforces `tenant_or_global` in the 6-tuple prefix, strictly bounding key components (capped at 512 bytes with deterministic SHA-256 fallback) without cross-tenant key pollution (`crates/modules/rustok-cache/src/key.rs`).
- [x] **CC-04.02 — Mutation Path Invalidation:** Audited write/mutation paths across `channel`, `rbac`, `pages`, `seo`, `tenant`, and `field_definition` modules; verified mutations trigger deterministic multi-node cache invalidations via pub/sub broadcast and local cache purging (`node scripts/verify/verify-rbac-two-process-redis-restart-source.mjs` — PASSED).
- [x] **CC-04.03 — Stampede & Dogpile Prevention:** Verified singleflight request coalescing (`CacheLoadCoordinator` in `load_or_fill`) preventing dogpiles on cold start with bounded in-flight gates and per-(backend, key) locks (`cargo test -p rustok-cache --lib` — 228/228 PASSED).
- [x] **CC-04.04 — Negative Caching & Multi-Node Sync:** Verified negative caching bounds (`NegativeCachePolicy` with 64KB max encoded bytes and non-zero TTL) for non-existent entities, fallible epoch clock fail-closed semantics (`node scripts/verify/verify-cache-clock-contract.mjs` — PASSED), and atomic invalidation on creation.

---

### CC-05: Event Bus, Transactional Outbox & Async Messaging

- **Canonical Documents:** [docs/references/outbox/README.md](../references/outbox/README.md), [docs/architecture/event-flow-contract.md](../architecture/event-flow-contract.md), [docs/standards/transactional-outbox.md](./transactional-outbox.md).
- **Primary Owner Crates:** `rustok-events`, `rustok-outbox`, `rustok-events-module`, `rustok-iggy`.
- **CodeGraph Target:** `publish_in_tx TransactionalEventBus Outbox sys_events publish`

#### Sub-Audit Checklist
- [x] **CC-05.01 — Atomic `publish_in_tx` Invariant:** Verified all domain mutations write outbox events inside the active database transaction boundary (`publish_in_tx` / `write_envelope_in_tx` in `rustok-outbox`) before commit (`node scripts/verify/verify-outbox-contract-write-once-causation.mjs` — PASSED).
- [x] **CC-05.02 — Event Payload Integrity & PII Boundary:** Verified `EventEnvelope` enforces metadata (ULID id, tenant_id, correlation_id, trace_id), strict schema versioning, and zero PII leaks (`cargo test -p rustok-events --lib` — 72/72 PASSED, `node scripts/verify/verify-event-contract-digest-admission.mjs` — PASSED).
- [x] **CC-05.03 — Consumer Idempotency:** Verified durable owner-scoped idempotency receipts in `owner_operation_receipts` preventing duplicate processing; validated lease recovery and retryable admission (`cargo test -p rustok-outbox --lib` — 24/24 PASSED).
- [x] **CC-05.04 — Outbox Retention & DLQ Pipelines:** Verified lease batch claiming, exponential backoff, DLQ quarantine for malformed events, and retention pruning (`node scripts/verify/verify-outbox-retention-and-event-metrics.mjs`, `node scripts/verify/verify-outbox-dlq-tenant-rbac.mjs`, `node scripts/verify/verify-event-dlq-duplicate-alert-observability.mjs` — all PASSED).

---

### CC-06: API Surfaces, Transports & Error Hygiene

- **Canonical Documents:** [docs/architecture/api.md](../architecture/api.md), [docs/architecture/dataloader.md](../architecture/dataloader.md), [docs/standards/errors.md](./errors.md).
- **Primary Owner Crates:** `apps/server/src/controllers/*`, `apps/server/src/routes.rs`, `rustok-api`, `rustok-web`, `rustok-mcp`.
- **CodeGraph Target:** `build_application_router HttpError ErrorBody MergedObject McpRouter Dataloader`

#### Sub-Audit Checklist
- [x] **CC-06.01 — Error Envelope Consistency:** Verified all REST controllers and GraphQL handlers return structured JSON `ErrorBody` responses with sanitized internal messages (`node scripts/verify/verify-port-error-public-safety.mjs` — PASSED, `cargo test -p rustok-web --lib` — 7/7 PASSED).
- [x] **CC-06.02 — DTO vs SeaORM Entity Boundary:** Verified zero direct leakage of raw SeaORM `Entity`/`Model` instances to API clients; typed DTO conversions enforced across REST and GraphQL (`node scripts/verify/verify-api-surface-contract.mjs` — PASSED).
- [x] **CC-06.03 — GraphQL Dataloader & N+1 Elimination:** Verified DataLoader batching across nested relations (`TenantNameLoader`, `ProfileSummaryLoader` in `apps/server/src/graphql/schema.rs`) eliminating N+1 query patterns.
- [x] **CC-06.04 — List Pagination & Limit Clamping:** Verified pagination parameters clamp limits (default 20, max 100) and preserve stable cursor navigation across list endpoints.
- [x] **CC-06.05 — MCP Tool Schema Validation:** Verified Model Context Protocol tools enforce typed JSON-RPC schema validation and structured outputs (`crates/modules/rustok-mcp`, `node scripts/verify/verify-mcp-admin-boundary.mjs` — PASSED, `cargo test -p rustok-mcp --lib` — 25/25 PASSED).

---

### CC-07: Multi-Tenancy & Effective Context Isolation

- **Canonical Documents:** [crates/modules/rustok-tenant/docs/README.md](../../crates/modules/rustok-tenant/docs/README.md), [docs/standards/patterns-vs-antipatterns.md](./patterns-vs-antipatterns.md) §3.1, §6.
- **Primary Owner Crates:** `rustok-tenant`, `rustok-channel`, `apps/server/src/middleware/tenant.rs`.
- **CodeGraph Target:** `TenantContext resolve_tenant Column::TenantId ensure_tenant_access`

#### Sub-Audit Checklist
- [x] **CC-07.01 — Universal Query Filter Invariant:** Verified domain SeaORM queries enforce `.filter(Column::TenantId.eq(tenant_id))` or composite tenant keys (`node scripts/verify/verify-tenant-fba.mjs` — PASSED, `cargo test -p rustok-tenant --lib` — 9/9 PASSED).
- [x] **CC-07.02 — Request Host Smuggling & Boundary Defense:** Verified middleware defense against duplicate Host/Forwarded headers, trailing dots, and case normalizations (`node scripts/verify/verify-tenant-resolution-architecture.mjs` — PASSED).
- [x] **CC-07.03 — Cross-Tenant Relation Verification:** Verified cross-tenant relation verification preventing entities from referencing records in other tenants across cart, payment, search, and admin endpoints (`node scripts/verify/verify-cart-checkout-tenant-context.mjs`, `node scripts/verify/verify-payment-collection-tenant-context.mjs`, `node scripts/verify/verify-search-graphql-mutation-tenant-scope.mjs` — all PASSED).
- [x] **CC-07.04 — Context Propagation:** Verified explicit typed context propagation (`TenantContext`, `ChannelContext`, `LocaleContext`) without manual downstream header parsing (`cargo test -p rustok-channel --lib` — 39/39 PASSED, `node scripts/verify/verify-runtime-context-invariants.mjs` — PASSED).

---

### CC-08: Database Transactions, Concurrency & Data Integrity

- **Canonical Documents:** [docs/architecture/database.md](../architecture/database.md), [AGENTS.md](../../AGENTS.md) §7.
- **Primary Owner Crates:** `migration/`, `crates/modules/*/migration/`, SeaORM entities.
- **CodeGraph Target:** `DatabaseConnection::begin TransactionTrait find_by_id insert update`

#### Sub-Audit Checklist
- [x] **CC-08.01 — Atomic Multi-Table Mutations:** Verified multi-row domain mutations commit atomically in `db.begin()` transaction boundaries (`cargo test -p rustok-migrations --lib` — 42/42 PASSED, `node scripts/verify/verify-db-multilingual-contract.mjs` — PASSED).
- [x] **CC-08.02 — TOCTOU Race Condition Guards:** Verified compare-and-swap (CAS) via `expected_revision`, unique constraints, and row-level locks across tenant, locale, role, and cart operations (`node scripts/verify/verify-tenant-locale-policy-migration.mjs` — PASSED).
- [x] **CC-08.03 — Soft Delete & Archive Discipline:** Verified business aggregates use explicit lifecycle states (`Archived`, tombstone receipts) instead of destructive physical deletes.
- [x] **CC-08.04 — Non-Blocking Transaction Scope:** Verified transactions do not hold database connections/locks across external network HTTP calls, timers, or slow I/O (`node scripts/verify/verify-migration-compatibility-contract.mjs` — PASSED).

---

### CC-09: Background Workers, Schedulers & Async Runtime Safety

- **Canonical Documents:** [docs/guides/runtime-guardrails.md](../guides/runtime-guardrails.md), [AGENTS.md](../../AGENTS.md) §10, §14.
- **Primary Owner Crates:** `crates/workers/*`, `rustok-workflow`, `rustok-sandbox`, `apps/server/src/services/app_runtime.rs`.
- **CodeGraph Target:** `spawn_worker StopHandle AppRuntime shutdown_runtime_workers tokio::fs`

#### Sub-Audit Checklist
- [x] **CC-09.01 — Non-Blocking Async Invariant:** Verified zero blocking calls in async execution paths (`crates/workers/*`, `rustok-workflow`, `rustok-sandbox`, `apps/server/src/services/app_runtime.rs`).
- [x] **CC-09.02 — Graceful Worker Drain:** Verified `StopHandle` cancellation and bounded graceful drain on shutdown (`node scripts/verify/verify-worker-runtime-policy.mjs` — PASSED).
- [x] **CC-09.03 — Concurrency Limiting:** Verified explicit Semaphore concurrency boundaries on heavy background workloads (`cargo test -p rustok-workflow --lib` — 26/26 PASSED, `node scripts/verify/verify-workflow-fba.mjs` — PASSED).
- [x] **CC-09.04 — Sandbox Isolation:** Verified resource quotas, execution timeouts, instruction counters, and memory bounds in execution sandboxes (`cargo test -p rustok-sandbox --lib` — 26/26 PASSED, `node scripts/verify/verify-sandbox-worker-isolation.mjs` — PASSED, `node scripts/verify/verify-sandbox-worker-deployment.mjs` — PASSED).

---

### CC-10: Observability, Distributed Tracing & Telemetry

- **Canonical Documents:** [docs/standards/logging.md](./logging.md), [docs/standards/opentelemetry-integration.md](./opentelemetry-integration.md), [docs/standards/distributed-tracing.md](./distributed-tracing.md).
- **Primary Owner Crates:** `rustok-telemetry`, `apps/server/src/controllers/metrics.rs`, `apps/server/src/controllers/health.rs`.
- **CodeGraph Target:** `init_telemetry register_metrics tracing::instrument health_check`

#### Sub-Audit Checklist
- [x] **CC-10.01 — Ad-Hoc Print Elimination:** Verified zero `println!`, `eprintln!`, or `dbg!` statements in production crates; enforce structured `tracing::*` (`eprintln!` in `native_server_adapter.rs` and audit evidence replaced with `tracing::warn!`).
- [x] **CC-10.02 — PII & Secret Redaction in Spans:** Verified `#[instrument]` macros skip sensitive arguments (`skip(self, security, input, txn, password, token)`) across `rustok-blog`, `rustok-forum`, and `rustok-pages` (0 sensitive unskipped params remaining).
- [x] **CC-10.03 — Telemetry Lifecycle Safety:** Verified singleton metrics handles (`OnceCell<Arc<MetricsHandle>>`, `OnceCell<Registry>`) and safe global subscriber initialization (`cargo test -p rustok-telemetry --lib` — 16/16 PASSED).
- [x] **CC-10.04 — Health & Readiness Probes:** Verified distinct liveness (`/health/live`) and readiness (`/health/ready`) with tri-state status reporting (`Healthy`, `Degraded`, `Unhealthy`) in `apps/server/src/controllers/health.rs`.

---

### CC-11: Dynamic Settings, Tenant Configuration & Secrets

- **Canonical Documents:** [docs/architecture/settings.md](../architecture/settings.md), [DECISIONS/2026-09-22-platform-module-settings-architecture.md](../../DECISIONS/2026-09-22-platform-module-settings-architecture.md).
- **Primary Owner Crates:** `apps/server/src/services/settings_service.rs`, `crates/modules/*/settings`.
- **CodeGraph Target:** `SettingsService load_tenant_settings Zeroize`

#### Sub-Audit Checklist
- [x] **CC-11.01 — In-Memory Secret Zeroization & Redaction:** Verified secret wrapper `Secret` redacts in `Debug`/`Display` (`<redacted>`), and `redact_secrets` scrubs SMTP passwords, Redis passwords, and connection URLs on read (`node scripts/verify/verify-server-platform-settings-tenant-scope.mjs` — PASSED).
- [x] **CC-11.02 — Dynamic Settings Schema Validation:** Verified `ValidatorRegistry` in `SettingsService` enforces typed domain validators prior to persisting tenant settings.
- [x] **CC-11.03 — Hierarchy & Fallback Resolution:** Verified 3-tier settings resolution: tenant database row -> host YAML configuration (`yaml_defaults_for`) -> compiled-in defaults.
- [x] **CC-11.04 — Environment Variable Containment:** Verified `std::env::var` reads are restricted to host startup and SSR transport entrypoints; domain modules consume typed context facades.

---

### CC-12: Internationalization (i18n), Localization & Translation Storage

- **Canonical Documents:** [docs/architecture/i18n.md](../architecture/i18n.md), [docs/architecture/database.md](../architecture/database.md).
- **Primary Owner Crates:** `rustok-translation`, `rustok-translation-targets`, `rustok_flutter_i18n`, `apps/server/src/middleware/locale.rs`.
- **CodeGraph Target:** `constrain_locale_to_tenant TranslationService _translations`

#### Sub-Audit Checklist
- [ ] **CC-12.01 — Relational Translation Schema:** Verify entity translations adhere to `base + *_translations` pattern with `(entity_id, locale)` composite primary key.
- [ ] **CC-12.02 — Fallback Negotiation Chain:** Verify locale negotiation: requested locale -> tenant default -> platform fallback.
- [ ] **CC-12.03 — User-Facing String Decoupling:** Verify backend returns typed error codes/translation keys; presentation strings resolve via FTL catalogs.
- [ ] **CC-12.04 — Fluent & ICU Format Safety:** Verify Fluent message formatting symmetry and non-panicking argument evaluation.

---

### CC-13: Documentation, ADRs, Manifests & Verification Gates

- **Canonical Documents:** [docs/index.md](../index.md), [DECISIONS/README.md](../../DECISIONS/README.md), [AGENTS.md](../../AGENTS.md) §1, §2.
- **Primary Owner Crates:** Platform documentation, `xtask/`, `scripts/verify/*`.
- **CodeGraph Target:** `build_registry RusToKModule modules.toml rustok-module.toml`

#### Sub-Audit Checklist
- [ ] **CC-13.01 — Registry vs Manifest Parity:** Verify all modules declared in `modules.toml` match server registry and `rustok-module.toml` manifests.
- [ ] **CC-13.02 — Active ADR Alignment:** Verify code reflects accepted ADRs in `DECISIONS/` and ADRs conform to canonical governance template.
- [ ] **CC-13.03 — Reference Schema Parity:** Verify committed routes and API contracts adhere to platform API contracts.
- [ ] **CC-13.04 — Documentation Integrity:** Verify documentation links, module diagrams, and metadata pass `node scripts/verify/verify-docs.mjs`.

---

### CC-14: Dynamic Extensibility, Flex Fields & Runtime Scripting

- **Canonical Documents:** [crates/modules/flex/README.md](../../crates/modules/flex/README.md), [crates/modules/alloy/README.md](../../crates/modules/alloy/README.md), [DECISIONS/2026-07-11-neutral-sandbox-foundation.md](../../DECISIONS/2026-07-11-neutral-sandbox-foundation.md).
- **Primary Owner Crates:** `flex`, `alloy`, `crates/workers/rustok-sandbox`.
- **CodeGraph Target:** `FieldDefinitionService FieldDefRegistry flex_attached_localized_values RhaiEngine AlloyScript`

#### Sub-Audit Checklist
- [ ] **CC-14.01 — Unified Donor Opt-in:** Verify attached donors use canonical Flex engine (`flex`) rather than ad-hoc metadata parsing.
- [ ] **CC-14.02 — Attached Multilingual Separation:** Verify localized attached values reside in `flex_attached_localized_values` rather than inline base JSON.
- [ ] **CC-14.03 — Fail-Closed Data Classification & AI Export:** Verify fail-closed privacy policies preventing secret or immutable fields from AI exports.
- [ ] **CC-14.04 — Rhai/Alloy Sandbox Safety:** Verify instruction counters, call stack limits, memory bounds, and timeouts in Rhai execution engines.

---

### CC-15: Object Storage, Media Assets & Direct-Upload Lifecycle

- **Canonical Documents:** [crates/utils/rustok-storage/README.md](../../crates/utils/rustok-storage/README.md), [crates/modules/rustok-media/docs/README.md](../../crates/modules/rustok-media/docs/README.md), [DECISIONS/2026-07-22-direct-object-store-runtime-owner-local-lifecycle.md](../../DECISIONS/2026-07-22-direct-object-store-runtime-owner-local-lifecycle.md).
- **Primary Owner Crates:** `rustok-storage`, `rustok-media`, `crates/modules/rustok-media-transport`.
- **CodeGraph Target:** `ObjectStorage StorageBackend generate_presigned_url MediaService CanonicalObjectKey`

#### Sub-Audit Checklist
- [ ] **CC-15.01 — Tenant-Scoped Object Keys:** Verify canonical storage keys are strictly structured with tenant scope, preventing cross-tenant enumeration.
- [ ] **CC-15.02 — Path Traversal Prevention:** Verify strict ASCII allow-lists on key segments, blocking `../`, absolute paths, and null bytes.
- [ ] **CC-15.03 — Presigned Upload Expiration & Verification:** Verify bounded expiry on upload sessions, byte length checks, and magic byte signature verification.
- [ ] **CC-15.04 — Media Extraction SSRF Guard:** Verify outbound media URL fetches enforce DNS pinning and block RFC 1918 / cloud metadata IPs.

---

### CC-16: CQRS Read Projections, Search & Index Synchronization

- **Canonical Documents:** [crates/modules/rustok-index/docs/README.md](../../crates/modules/rustok-index/docs/README.md), [crates/modules/rustok-search/README.md](../../crates/modules/rustok-search/README.md), [DECISIONS/2026-07-24-index-storage-layout.md](../../DECISIONS/2026-07-24-index-storage-layout.md).
- **Primary Owner Crates:** `rustok-index`, `rustok-search`.
- **CodeGraph Target:** `IndexService SearchService index_products index_content ReindexHandler`

#### Sub-Audit Checklist
- [ ] **CC-16.01 — CQRS-Lite Separation:** Verify storefront and catalog reads query denormalized search/index models rather than joining normalized write tables.
- [ ] **CC-16.02 — Event-Driven Reindexing Consistency:** Verify domain aggregate mutations emit outbox events that project state deterministically into indexes.
- [ ] **CC-16.03 — Search Tenant Scoping:** Verify search queries strictly enforce tenant, channel authority, and locale scopes fail-closed.
- [ ] **CC-16.04 — Reindex Idempotency & Batching:** Verify batch replay and reconciliation runners enforce leased attempt fencing, cursors, and idempotency.

---

### CC-17: Ingress Sanitization, Input Validation & SSRF/XSS Defense

- **Canonical Documents:** [docs/guides/input-validation.md](../guides/input-validation.md), [docs/standards/security.md](./security.md), [docs/standards/patterns-vs-antipatterns.md](./patterns-vs-antipatterns.md) §7.5, §11.
- **Primary Owner Crates:** `rustok-web`, `packages/richtext`, all DTO modules.
- **CodeGraph Target:** `validator::Validate sanitize_html UrlAllowlist SecurityHeaders`

#### Sub-Audit Checklist
- [ ] **CC-17.01 — Universal DTO Ingress Validation:** Verify universal DTO validation contracts (`validator::Validate`, `.validate()?`) and fail-closed validation ordering.
- [ ] **CC-17.02 — HTML & Richtext Sanitization & Strict CSP:** Verify user markup passes schema-bounded sanitization and zero inline styles across hosted UI packages.
- [ ] **CC-17.03 — Outbound SSRF Verification:** Verify outbound HTTP clients enforce DNS resolution pinning, reject redirects, and block private/loopback IPs.
- [ ] **CC-17.04 — Path-Based Rate Limiting:** Verify sliding-window rate limiting on ingress entrypoints with per-IP isolation and lockout windows.
