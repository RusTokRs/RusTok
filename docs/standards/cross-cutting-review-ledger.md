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
- **Active Cycle:** Cross-Cutting Cycle 1 (Fresh Start from Scratch)
- **Active Phase:** CC-01: Fluid Backend Architecture (FBA) & Module Sovereignty
- **Completed Phases:** None (0/17 completed)
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

- [ ] **[CC-01: Fluid Backend Architecture (FBA) & Module Sovereignty](#cc-01-fluid-backend-architecture-fba--module-sovereignty)** (0/6 completed)
- [ ] **[CC-02: Fluid Frontend Architecture (FFA) & UI Adapter Purity](#cc-02-fluid-frontend-architecture-ffa--ui-adapter-purity)** (0/5 completed)
- [ ] **[CC-03: RBAC, Auth & Security Enforcement](#cc-03-rbac-auth--security-enforcement)** (0/8 completed)
- [ ] **[CC-04: Caching, Invalidation & Tenant Cache Isolation](#cc-04-caching-invalidation--tenant-cache-isolation)** (0/4 completed)
- [ ] **[CC-05: Event Bus, Transactional Outbox & Async Messaging](#cc-05-event-bus-transactional-outbox--async-messaging)** (0/4 completed)
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

#### Sub-Audit Checklist
- [ ] **CC-01.01 — Module Physical Shape & Hexagonal Separation:** Verify no executable `.rs` code is hidden in `contracts/` or `docs/`. Run `node scripts/verify/verify-module-source-layout.mjs` for canonical module source layout verification.
- [ ] **CC-01.02 — Inter-Module Dependency Sovereignty:** Run `python scripts/architecture_dependency_guard.py` and enforce module import boundaries. Ensure zero direct dependencies on other modules' private entities or repositories.
- [ ] **CC-01.03 — Port & Seam Purity:** Verify cross-module interactions communicate strictly through owner-defined port traits, transactional event contracts, and provider seams rather than raw internal table access.
- [ ] **CC-01.04 — FBA Registry & Capability Profiles:** Verify `rustok-fba` capability definitions (`BackendTopology`, `TransportProfile`, `CapabilityId`, `FbaProviderDescriptor`, `FbaConsumerDependency`) and embedded-first in-process execution.
- [ ] **CC-01.05 — Host Minimalism (`apps/server`):** Verify `apps/server` acts purely as composition root and host with zero leaked module business rules.
- [ ] **CC-01.06 — CLI Package Decoupling:** Verify module maintenance commands reside in isolated `cli/` packages over `rustok-cli-core` without bloating domain crates with CLI dependencies.

---

### CC-02: Fluid Frontend Architecture (FFA) & UI Adapter Purity

- **Canonical Documents:** [docs/UI/module-package-architecture.md](../UI/module-package-architecture.md), [docs/UI/graphql-architecture.md](../UI/graphql-architecture.md), [AGENTS.md](../../AGENTS.md) §5.
- **Primary Owner Crates:** `crates/modules/*/admin`, `crates/modules/*/storefront`, `crates/ui/*`, `apps/admin`, `apps/storefront`, `apps/next-admin`, `apps/next-frontend`, `apps/next-admin/packages/*`, `apps/next-frontend/packages/*`.
- **CodeGraph Target:** `leptos_dom view rsx TransportFacade ViewModel rustok_ui`

#### Sub-Audit Checklist
- [ ] **CC-02.01 — Zero Framework in Core:** Verify every `crates/modules/*/core` crate contains strictly zero imports of `leptos::*`, `dioxus::*`, or other UI frameworks. Models must be pure Rust data types verified by unit tests.
- [ ] **CC-02.02 — Zero Domain Logic in UI Adapters:** Verify `crates/modules/*/admin` and `storefront` adapters contain zero business calculations, price/discount math, status validations, or command construction inside view templates (`view!`, `rsx!`).
- [ ] **CC-02.03 — Transport Facade Boundary:** Ensure UI adapters access backend capabilities strictly through module-owned `transport/` facades, never calling raw fetch or ad-hoc API endpoints directly.
- [ ] **CC-02.04 — Design Token & UI Primitive Reuse:** Verify module UIs consume canonical visual primitives (`rustok-ui`, `rustok-ui-forms`, `rustok-grid`, `rustok-ui-i18n`) rather than implementing ad-hoc form engines or custom styling.
- [ ] **CC-02.05 — Dual-Stack UI & Admin Synchronization (Leptos ↔ Next.js Parity):** Verify full parity between Leptos and Next.js frontend/admin implementations for all modules. Check route availability (no 404s on root module paths like `/dashboard/<module>` and `/[locale]/<module>`), operational workspace parity (moderation tables, status filters, taxonomy trees, detail thread previews), and strict GraphQL schema conformance (variable nullability, connection pagination fields).

---

### CC-03: RBAC, Auth & Security Enforcement

- **Canonical Documents:** [crates/modules/rustok-rbac/docs/README.md](../../crates/modules/rustok-rbac/docs/README.md), [crates/modules/rustok-auth/docs/README.md](../../crates/modules/rustok-auth/docs/README.md), [docs/standards/security.md](./security.md).
- **Primary Owner Crates:** `rustok-rbac`, `rustok-auth`, `rustok-api`, `apps/server/src/middleware/auth_context.rs`, `apps/server/src/services/rbac_*.rs`.
- **CodeGraph Target:** `RequirePermission check_permission RbacService AuthContext PermissionKey`

#### Sub-Audit Checklist
- [ ] **CC-03.01 — Controller & Resolver Permission Coverage:** Audit REST endpoints in `apps/server/src/controllers/*` and GraphQL operations in `apps/server/src/graphql/*` to verify permission guards are enforced on all operations.
- [ ] **CC-03.02 — Tenant Isolation in Security Context & Authoritative Queries:** Verify `SecurityContext` strictly requires and enforces tenant scope and principal claims, preventing cross-tenant role leakage or privilege escalation.
- [ ] **CC-03.03 — RBAC Cache Invalidation & Canonical API Seams:** Verify role and permission mutations commit atomically within transaction boundaries and broadcast multi-node invalidation via `publish_user_rbac_invalidation`.
- [ ] **CC-03.04 — Password & Token Security:** Verify Argon2id password hashing parameters, unique random salts, 256-bit entropy refresh tokens, strict JWT validation (RFC 7519), and session revocation on password changes.
- [ ] **CC-03.05 — FFA Purity & i18n in RBAC UI:** Enforce FFA i18n invariants in RBAC UI components, resolving messages via Fluent message catalogs without hardcoded language branching.
- [ ] **CC-03.06 — Built-in Role Resource Parity & Completeness:** Audit built-in system role definitions across `SUPER_ADMIN_PERMISSIONS`, `ADMIN_PERMISSIONS`, `MANAGER_PERMISSIONS`, and `CUSTOMER_PERMISSIONS` in `rustok-core::rbac`.
- [ ] **CC-03.07 — GraphQL Mutation Race Elimination & Role Invalidation:** Verify permission insertion handles concurrency deterministically and invalidates cached permissions for all affected users upon role permission updates.
- [ ] **CC-03.08 — Platform Permission Catalog Exhaustiveness:** Verify 100% parity across all resource variants between `rustok-core::rbac`, `rustok-api::Resource`, and GraphQL admin permission query catalogs.

---

### CC-04: Caching, Invalidation & Tenant Cache Isolation

- **Canonical Documents:** [crates/modules/rustok-cache/README.md](../../crates/modules/rustok-cache/README.md), [docs/standards/patterns-vs-antipatterns.md](./patterns-vs-antipatterns.md) §6.
- **Primary Owner Crates:** `rustok-cache`, `rustok-tenant`, `apps/server/src/services/settings_service.rs`.
- **CodeGraph Target:** `CacheService get_cached set_cached invalidate CacheKey TieredCache`

#### Sub-Audit Checklist
- [ ] **CC-04.01 — Tenant Scoping in Cache Keys:** Verify `CacheKeyBuilder` requires `tenant_or_global` in the 6-tuple prefix, preventing cross-tenant key pollution.
- [ ] **CC-04.02 — Mutation Path Invalidation:** Audit all write/mutation paths across modules to verify mutations trigger deterministic cache invalidations locally and via pub/sub broadcast.
- [ ] **CC-04.03 — Stampede & Dogpile Prevention:** Verify singleflight request coalescing (`CacheLoadCoordinator` in `load_or_fill`) to prevent dogpiles on cold start.
- [ ] **CC-04.04 — Negative Caching & Multi-Node Sync:** Verify negative caching bounds for non-existent entities and atomic invalidation on creation.

---

### CC-05: Event Bus, Transactional Outbox & Async Messaging

- **Canonical Documents:** [docs/references/outbox/README.md](../references/outbox/README.md), [docs/architecture/event-flow-contract.md](../architecture/event-flow-contract.md), [docs/standards/transactional-outbox.md](./transactional-outbox.md).
- **Primary Owner Crates:** `rustok-events`, `rustok-outbox`, `rustok-events-module`, `rustok-iggy`.
- **CodeGraph Target:** `publish_in_tx TransactionalEventBus Outbox sys_events publish`

#### Sub-Audit Checklist
- [ ] **CC-05.01 — Atomic `publish_in_tx` Invariant:** Verify all domain mutations write outbox events inside the active database transaction boundary (`publish_in_tx`).
- [ ] **CC-05.02 — Event Payload Integrity & PII Boundary:** Verify `EventEnvelope` enforces metadata (ULID id, tenant_id, correlation_id, trace_id), strict schema versioning, and zero PII leaks.
- [ ] **CC-05.03 — Consumer Idempotency:** Verify durable owner-scoped idempotency receipts in `owner_operation_receipts` to prevent duplicate processing.
- [ ] **CC-05.04 — Outbox Retention & DLQ Pipelines:** Verify lease batch claiming, exponential backoff, DLQ quarantine for failed events, and retention pruning.

---

### CC-06: API Surfaces, Transports & Error Hygiene

- **Canonical Documents:** [docs/architecture/api.md](../architecture/api.md), [docs/architecture/dataloader.md](../architecture/dataloader.md), [docs/standards/errors.md](./errors.md).
- **Primary Owner Crates:** `apps/server/src/controllers/*`, `apps/server/src/routes.rs`, `rustok-api`, `rustok-web`, `rustok-mcp`.
- **CodeGraph Target:** `build_application_router HttpError ErrorBody MergedObject McpRouter Dataloader`

#### Sub-Audit Checklist
- [ ] **CC-06.01 — Error Envelope Consistency:** Verify all REST controllers and GraphQL handlers return structured JSON `ErrorBody` responses with sanitized internal errors.
- [ ] **CC-06.02 — DTO vs SeaORM Entity Boundary:** Verify zero leakage of raw SeaORM `Entity`/`Model` instances directly to API clients; enforce typed DTO conversions.
- [ ] **CC-06.03 — GraphQL Dataloader & N+1 Elimination:** Verify DataLoader batching across nested relations to prevent N+1 query patterns.
- [ ] **CC-06.04 — List Pagination & Limit Clamping:** Verify limit clamping (max 100) and standard pagination across REST and GraphQL endpoints.
- [ ] **CC-06.05 — MCP Tool Schema Validation:** Verify Model Context Protocol tools enforce typed JSON-RPC schema validation and structured output.

---

### CC-07: Multi-Tenancy & Effective Context Isolation

- **Canonical Documents:** [crates/modules/rustok-tenant/docs/README.md](../../crates/modules/rustok-tenant/docs/README.md), [docs/standards/patterns-vs-antipatterns.md](./patterns-vs-antipatterns.md) §3.1, §6.
- **Primary Owner Crates:** `rustok-tenant`, `rustok-channel`, `apps/server/src/middleware/tenant.rs`.
- **CodeGraph Target:** `TenantContext resolve_tenant Column::TenantId ensure_tenant_access`

#### Sub-Audit Checklist
- [ ] **CC-07.01 — Universal Query Filter Invariant:** Verify every domain SeaORM query enforces `.filter(Column::TenantId.eq(tenant_id))` or composite tenant keys.
- [ ] **CC-07.02 — Request Host Smuggling & Boundary Defense:** Verify middleware defense against duplicate Host/Forwarded headers, trailing dots, and case normalizations.
- [ ] **CC-07.03 — Cross-Tenant Relation Verification:** Verify cross-tenant relation checks preventing entities from referencing records in other tenants.
- [ ] **CC-07.04 — Context Propagation:** Verify explicit typed context propagation (`TenantContext`, `ChannelContext`, `LocaleContext`) without manual downstream header parsing.

---

### CC-08: Database Transactions, Concurrency & Data Integrity

- **Canonical Documents:** [docs/architecture/database.md](../architecture/database.md), [AGENTS.md](../../AGENTS.md) §7.
- **Primary Owner Crates:** `migration/`, `crates/modules/*/migration/`, SeaORM entities.
- **CodeGraph Target:** `DatabaseConnection::begin TransactionTrait find_by_id insert update`

#### Sub-Audit Checklist
- [ ] **CC-08.01 — Atomic Multi-Table Mutations:** Verify multi-row domain mutations commit atomically in `db.begin()` transaction boundaries.
- [ ] **CC-08.02 — TOCTOU Race Condition Guards:** Verify compare-and-swap (CAS) via `expected_revision`, unique constraints, and row-level locks.
- [ ] **CC-08.03 — Soft Delete & Archive Discipline:** Verify business aggregates use explicit lifecycle states (`Archived`, tombstone receipts) instead of destructive physical deletes.
- [ ] **CC-08.04 — Non-Blocking Transaction Scope:** Verify transactions do not hold database connections/locks across external network HTTP calls or timers.

---

### CC-09: Background Workers, Schedulers & Async Runtime Safety

- **Canonical Documents:** [docs/guides/runtime-guardrails.md](../guides/runtime-guardrails.md), [AGENTS.md](../../AGENTS.md) §10, §14.
- **Primary Owner Crates:** `crates/workers/*`, `rustok-workflow`, `rustok-sandbox`, `apps/server/src/services/app_runtime.rs`.
- **CodeGraph Target:** `spawn_worker StopHandle AppRuntime shutdown_runtime_workers tokio::fs`

#### Sub-Audit Checklist
- [ ] **CC-09.01 — Non-Blocking Async Invariant:** Verify zero blocking calls (`std::thread::sleep`, `std::fs::*`, sync Mutex) in async execution paths.
- [ ] **CC-09.02 — Graceful Worker Drain:** Verify `StopHandle` cancellation and bounded graceful drain on shutdown.
- [ ] **CC-09.03 — Concurrency Limiting:** Verify explicit Semaphore concurrency boundaries on heavy background workloads.
- [ ] **CC-09.04 — Sandbox Isolation:** Verify resource quotas, execution timeouts, instruction counters, and memory bounds in execution sandboxes.

---

### CC-10: Observability, Distributed Tracing & Telemetry

- **Canonical Documents:** [docs/standards/logging.md](./logging.md), [docs/standards/opentelemetry-integration.md](./opentelemetry-integration.md), [docs/standards/distributed-tracing.md](./distributed-tracing.md).
- **Primary Owner Crates:** `rustok-telemetry`, `apps/server/src/controllers/metrics.rs`, `apps/server/src/controllers/health.rs`.
- **CodeGraph Target:** `init_telemetry register_metrics tracing::instrument health_check`

#### Sub-Audit Checklist
- [ ] **CC-10.01 — Ad-Hoc Print Elimination:** Verify zero `println!`, `eprintln!`, or `dbg!` statements in production crates; enforce structured `tracing::*`.
- [ ] **CC-10.02 — PII & Secret Redaction in Spans:** Verify `#[instrument]` macros skip sensitive arguments (`skip(self, security, input, txn, password, token)`).
- [ ] **CC-10.03 — Telemetry Lifecycle Safety:** Verify singleton metrics handles and safe global subscriber initialization.
- [ ] **CC-10.04 — Health & Readiness Probes:** Verify distinct liveness (`/health/live`) and readiness (`/health/ready`) with tri-state status reporting.

---

### CC-11: Dynamic Settings, Tenant Configuration & Secrets

- **Canonical Documents:** [docs/architecture/settings.md](../architecture/settings.md), [DECISIONS/2026-09-22-platform-module-settings-architecture.md](../../DECISIONS/2026-09-22-platform-module-settings-architecture.md).
- **Primary Owner Crates:** `apps/server/src/services/settings_service.rs`, `crates/modules/*/settings`.
- **CodeGraph Target:** `SettingsService load_tenant_settings Zeroize`

#### Sub-Audit Checklist
- [ ] **CC-11.01 — In-Memory Secret Zeroization & Redaction:** Verify secret zeroization in memory, absence of Debug leaks, and secret redaction on settings reads.
- [ ] **CC-11.02 — Dynamic Settings Schema Validation:** Verify tenant settings schemas are validated prior to persistence against typed domain validators.
- [ ] **CC-11.03 — Hierarchy & Fallback Resolution:** Verify 3-tier settings resolution: tenant database row -> host YAML configuration -> compiled-in defaults.
- [ ] **CC-11.04 — Environment Variable Containment:** Verify `std::env::var` reads are restricted to host startup; domain modules consume typed context facades.

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
