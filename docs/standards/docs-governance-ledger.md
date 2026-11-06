# Architecture, Documentation & ADR Governance Ledger (DAR)

This file is the canonical living review ledger for **architecture, documentation, and ADR governance audits** across the RusToK repository.
While the vertical code audit ([continuous-review-ledger.md](./continuous-review-ledger.md)) verifies crate-scoped Rust/TS logic and the horizontal audit ([cross-cutting-review-ledger.md](./cross-cutting-review-ledger.md)) verifies system-wide capabilities, this ledger governs the **authoritative target architecture and source-of-truth layer**: ADRs, central architecture maps, domain module contracts, operational specifications, and verification tooling.

> [!IMPORTANT]
> **Core Principle: Deep Architectural Understanding (Anti-Slop / Anti-Cargo-Cult)**
> Documentation audit is not bureaucratic checkboxing or blind enforcement of obsolete conventions.
> In accordance with [AGENTS.md](../../AGENTS.md) §1, §4, and §11:
> - Contributors and AI agents MUST deeply understand the domain contracts and executable code before editing documentation.
> - **Находить нестыковки:** Cross-reference every entity, port, DTO, route, and setting against executable Rust/TS code and database schemas.
> - **Исправлять:** Remediate broken links, phantom types, obsolete APIs, and outdated signatures.
> - **Актуализировать:** Bring living architecture contracts up to date with modern target architecture (FBA/FFA, SeaORM, GraphQL, Leptos+Next.js parity).
> - **Объединять:** Consolidate fragmented, duplicated, or overlapping documentation into authoritative single sources of truth.
> - **Удалять неактуальное:** Safely delete or archive into `docs/archive/` dead sprint logs, AI completion scrapers, obsolete temporary reports, and machine stubs.

---

## Canonical Triggers

To instruct any AI agent (Antigravity, Claude, Codex) to execute the next pending phase of this documentation and architectural governance audit, use either canonical instruction:

```text
реализуй план аудита документации
```
*(или: `реализуй план актуализации adr` / in English: `execute documentation audit plan`)*

When triggered, the agent:
1. Reads this ledger and identifies the active phase (`DAR-XX`).
2. Re-reads [AGENTS.md](../../AGENTS.md), [docs/index.md](../index.md), and affected owner documentation and ADRs.
3. Critically examines the target documentation scope against executable code truth (`crates/`, `apps/`, database schemas, and manifests).
4. Systematically applies the 5 Universal Documentation Audit Dimensions:
   - **Нестыковки (Discrepancy Discovery):** Compare against actual code, schemas, and manifests.
   - **Исправление (Remediation):** Fix broken links, wrong types, stale ports, outdated endpoints.
   - **Актуализация (Actualization):** Synchronize living architecture contracts with modern target architecture.
   - **Объединение (Consolidation):** Merge duplicate/fragmented guides into single canonical sources of truth.
   - **Удаление (Purge & Archiving):** Clean ephemeral sprint logs, AI completion summaries, machine stubs into `docs/archive/`.
5. Runs scoped verification (`npm run verify:docs`, `npm run verify:adrs`, `npm run verify:manifest`, `cargo check -p <affected_crate>`).
6. Marks the sub-items and phase complete in this ledger and records integration evidence.

---

## Current Cycle Status

- **Status:** IN PROGRESS
- **Active Cycle:** DAR Cycle 2 (Platform-Wide Documentation Actualization & Slop Purge)
- **Active Phase:** [DAR-07: ADR Registry Deep Audit & Graph Reconciliation](#dar-07-adr-registry-deep-audit-graph-reconciliation)
- **Completed Phases (Cycle 2):** 6/16 (DAR-01, DAR-02, DAR-03, DAR-04, DAR-05, DAR-06 PASS)
- **Active Branch:** `main`

---

## The 5 Universal Documentation Audit Dimensions (5D Verification)

Every document audited under this ledger MUST be systematically verified against the 5 Universal Documentation Dimensions:

| Dimension | Scope | Verification Invariant | Failure Mode to Eliminate |
| :--- | :--- | :--- | :--- |
| **1. Discrepancy Discovery** *(Поиск нестыковок)* | Code-Reality Parity | Documented ports, DTOs, database tables, and settings MUST 100% match executable code in `crates/`, `apps/`, and SeaORM migrations. | Phantom structs, deleted endpoints, outdated SQL schemas, non-existent config keys. |
| **2. Remediation** *(Исправление)* | Link & Type Integrity | All hyperlinks, relative paths, Markdown anchors, and symbol references MUST resolve cleanly without 404s or compiler warnings. | Broken markdown links, invalid anchor hashes, malformed escape characters (`\_`), dead cross-references. |
| **3. Actualization** *(Актуализация)* | Target Architecture | Architecture documents MUST reflect current accepted standards: Fluid Backend Architecture (FBA), Fluid Frontend Architecture (FFA), multi-tenancy, and transactional outbox. | Describing deprecated monolithic conventions, obsolete trait patterns, or superseded architecture. |
| **4. Consolidation** *(Объединение)* | Single Source of Truth | Exactly one canonical owner and specification per concept. Overlapping guides MUST be merged into the owning document. | 17 separate revision history summaries, 3 duplicate demo-content guides, competing setup walk-throughs. |
| **5. Purge & Archiving** *(Удаление неактуального)* | Anti-Slop Hygiene | Ephemeral completion diaries, machine-generated stubs, and obsolete sprint notes MUST be purged or archived in `docs/archive/`. | Leaving 50+ AI status reports at root, 570+ single-key stubs in operations, or dead July 2026 plans in verification. |

---

## Document Taxonomy & Governance Hierarchy

Every document in the repository MUST belong to exactly one canonical category with a clear lifecycle:

| Document Type | Location | Purpose & Mutability | Lifecycle Policy |
| :--- | :--- | :--- | :--- |
| **Current Contract** | `docs/architecture/`, `crates/*/README.md`, `apps/*/README.md` | Living source of target architecture & component contracts. | **Mutable.** Updated in lockstep with code changes. Must always reflect 100% executable truth. |
| **Decision Record (ADR)** | `DECISIONS/YYYY-MM-DD-*.md` | Immutable record of an architectural choice, context, and tradeoffs. | **Immutable once accepted.** When architecture changes, create a new ADR and mark the old one `Superseded`. |
| **Implementation Plan** | `crates/*/docs/implementation-plan.md` | Ephemeral breakdown of current in-flight work and regression gates. | **Transient.** Once implementation merges and verifies, delete or consolidate into the component README / contract. Do NOT leave dead plans in root docs. |
| **Operational Spec** | `docs/operations/` | Concrete runbooks, deployment guides, and consolidated configuration references. | **Consolidated.** High-signal, human-readable reference; strictly no single-key generated markdown files. |

> [!NOTE]
> **Strict Anti-Archive Policy:** Dead implementation diaries, historical proposals, completed sprint notes, and obsolete scratchpads are purged completely, never hoarded as confusing historical clutter. Only living, actual contracts remain in the repository.

---

## Master Inventory Table — Repository Documentation Surfaces

Analogous to the code audit ledger ([continuous-review-ledger.md](./continuous-review-ledger.md)), this table tracks every documentation surface across the repository:

| Status | Target Documentation Surface | Category | Files | LOC | Last Audit | Discrepancies & Actualization Action |
| :---: | :--- | :--- | :---: | :---: | :---: | :--- |
| [x] | **[Root Platform Entrypoints & Slop Purge](../../README.md)** | `root` | 7 | 1,480 | 2026-11-06 | Cleaned: purged 50 obsolete/duplicate root files (17 revisions scratchpads, 3 demo content notes, 15 AI diaries). Preserved 7 canonical platform contracts (README, README.ru, ARCHITECTURE, AGENTS, CONTRIBUTING, CHANGELOG, COMMERCIAL-LICENSE). Zero root slop. |
| [x] | **[Architecture Core](../architecture/overview.md)** | `architecture` | 22 | 5,273 | 2026-11-06 | Reconciled system overview, routing, settings, and distribution profiles. Eliminated displaced ADR subfolder (relocated newsletter ADR to canonical DECISIONS/), purged obsolete multilingual transport drift notes. Diagram & verify-docs: 100% PASS. |
| [x] | **[Engineering Standards & Policies](./coding.md)** | `standards` | 15 | 5,617 | 2026-11-06 | Reconciled coding standards, error frameworks (rustok-core error & typed_error), transactional outbox, distributed tracing, and security policies. Zero broken links across 15 documents. |
| [x] | **[Operations, Config & Runbooks](../operations/README.md)** | `operations` | 6 | 774 | 2026-11-06 | Unified operations documentation. Eliminated empty ghost directories (RUNBOOKS/ and SPECS/ with 5 broken links). Connected ops/ assets (benches, grafana, prometheus) to docs/operations/README.md. Reconciled configuration-reference.md with YAML and runtime configs. |
| [x] | **[UI Workbench & Client Contracts](../UI/README.md)** | `ui` | 24 | 2,659 | 2026-11-06 | Reconciled multi-framework design tokens (base.css, tokens.json, Flutter tokens) and cross-framework UI components. Verified FFA client parity (Leptos, Next.js, Flutter). verify:ui:parity and verify:ffa:ui:migration:docs 100% PASS. |
| [x] | **[Verification Plans & Quality Gates](../verification/README.md)** | `verification` | 11 | 2,126 | 2026-11-06 | Modernized verification documentation. Purged 2 obsolete July 2026 hardening diaries (PLATFORM_HARDENING_STATUS_2026-07-18 and PLATFORM_HARDENING_IMPLEMENTATION_PLAN). Aligned links with living test gates. verify:docs: 100% PASS. |
| [x] | **[ADR Governance & Decision Registry](../../DECISIONS/README.md)** | `decisions` | 178 | 21,787 | 2026-11-06 | Audited all 31 `In progress` ADRs against code; promoted 19 fully implemented decisions to `Implemented` across migrations, RBAC, OAuth revocation fence, search projection driver, shipment stock ports, refund valuation, order shipment status, order cancellation sagas, forum feeds, pre-moderation, and site symbols. Triage & relations verified; node scripts/verify/verify-adrs.mjs passed for 177 decisions. |
| [x] | **[Domain Modules: Commerce & Checkout](../../crates/modules/rustok-commerce/README.md)** | `modules-commerce` | 208 | 24,150 | 2026-11-06 | Audited all 208 markdown files across 11 commerce crates (rustok-cart, rustok-commerce, rustok-pricing, rustok-payment, rustok-order, rustok-fulfillment, rustok-inventory, rustok-tax, rustok-region, rustok-brand, rustok-product). Fixed broken anchor slugs in rustok-product/CRATE_API.md and admin/CRATE_API.md. Reconciled and indexed 88 architecture/diagnostic specifications in docs/README.md across all 10 modules. 642 links, 0 broken links, 0 broken anchors. |
| [ ] | **[Domain Modules: Content, Pages & Forum](../../crates/modules/rustok-content/README.md)** | `modules-content` | 165 | 19,800 | None | Audit READMEs and CRATE_APIs for `rustok-blog`, `rustok-forum`, `rustok-comments`, `rustok-pages`, `rustok-page-builder`, `rustok-media`, `rustok-docs`, `rustok-seo`. |
| [ ] | **[Domain Modules: Platform Capabilities](../../crates/modules/rustok-auth/README.md)** | `modules-platform` | 140 | 16,500 | None | Audit READMEs and CRATE_APIs for `rustok-auth`, `rustok-channel`, `rustok-cache`, `rustok-notifications`, `rustok-search`, `rustok-telemetry`, `rustok-events-module`, `rustok-outbox`. |
| [ ] | **[Domain Modules: Marketplace & Multi-Tenancy](../../crates/modules/rustok-marketplace/README.md)** | `modules-market` | 110 | 13,200 | None | Audit READMEs and CRATE_APIs for `rustok-marketplace*`, `rustok-tenant`, `rustok-distribution`. |
| [ ] | **[Domain Modules: Customer, CRM & Finance](../../crates/modules/rustok-customer/README.md)** | `modules-business` | 120 | 14,100 | None | Audit READMEs and CRATE_APIs for `rustok-customer`, `rustok-deal`, `rustok-finance`, `rustok-accounting`, `rustok-billing`, `rustok-forms`, `rustok-workflow`. |
| [ ] | **[Domain Modules: AI & Emerging Capabilities](../../crates/modules/rustok-ai/README.md)** | `modules-ai` | 110 | 10,871 | None | Audit READMEs and CRATE_APIs for `rustok-ai*`, `rustok-mcp`, `rustok-messenger`, `rustok-tasks`, `flex`, `alloy`. |
| [ ] | **[Platform Foundation Libraries](../../crates/libs/rustok-core/README.md)** | `foundation-libs` | 49 | 4,526 | None | Audit READMEs and CRATE_APIs for `rustok-core`, `rustok-api`, `rustok-fba`, `rustok-events`, `rustok-telemetry`, `rustok-web`, `rustok-manifest`. |
| [ ] | **[Shared UI Libraries & Components](../../crates/ui/rustok-ui/README.md)** | `ui-libs` | 62 | 4,167 | None | Audit READMEs and CRATE_APIs for `rustok-ui`, `leptos-ui`, `leptos-auth`, `rustok-graphql`, `rustok-grid`, `rustok-ui-i18n`, `fly`. |
| [ ] | **[Utility & Worker Crates](../../crates/utils/rustok-migrations/README.md)** | `utils-workers` | 112 | 6,899 | None | Audit READMEs and CRATE_APIs for `rustok-migrations`, `rustok-installer`, `rustok-cli`, `rustok-sandbox*`, `rustok-artifact-node*`. |
| [ ] | **[Client Applications & Host Packages](../../apps/server/README.md)** | `apps` | 107 | 9,245 | None | Audit READMEs and documentation for `apps/server`, `apps/admin`, `apps/next-admin`, `apps/storefront`, `apps/next-frontend`. |

---

## Sequential Execution Plan — DAR Phases

### DAR-01: Root Slop Purge & Canonical Entrypoints Reset

**Scope:** 57 markdown files previously at repository root (`./`).
**Goal:** Eliminate 50 redundant, obsolete, or misplaced markdown files from the repository root. Establish a clean, authoritative root contract with zero dead archives.

- [x] **DAR-01.1: Triage and Retain Canonical Root Contracts.** Preserved strictly 7 canonical platform documents:
  - `README.md` (repository landing)
  - `README.ru.md` (Russian localized landing)
  - `ARCHITECTURE.md` (high-level system architecture)
  - `AGENTS.md` (canonical governance contract)
  - `CONTRIBUTING.md` (contributor guidelines)
  - `CHANGELOG.md` (platform release notes)
  - `COMMERCIAL-LICENSE.md` (licensing)
- [x] **DAR-01.2: Purge Revisions History Scratchpads.** Safely deleted 17 redundant root revision files (`ADMIN_UI_DESIGN.md`, `CONTENT_REVISION_HISTORY_*`, `REVISIONS_*`, `GRAPHQL_API_*`, etc.) that were dumped at the repository root. Reconciled `rustok-revisions/README.md` to be self-contained.
- [x] **DAR-01.3: Purge Demo Content Working Notes.** Deleted 3 obsolete intermediate demo files (`DEMO_CONTENT_INTEGRATION.md`, `DEMO_CONTENT_INTEGRATION_GUIDE.md`, `DEMO_CONTENT_UNIFIED_IMPLEMENTATION.md`); demo capabilities are owned and implemented in `crates/libs/rustok-content-portability`.
- [x] **DAR-01.4: Purge Ephemeral AI Completion Slop.** Deleted dead historical session reports and AI completion diaries (`FINAL_REPORT.md`, `FINAL_PROJECT_SUMMARY.md`, `FINAL_STATUS.md`, `FINAL_ADDITIONS.md`, `FINAL_ENHANCEMENTS.md`, `WORK_COMPLETED.md`, `PROJECT_COMPLETE.md`, `IMPLEMENTATION_COMPLETE.md`, `SESSION_COMPLETE_SUMMARY.md`, `MEDIUM_ISSUES_FIXED.md`, `deep-research-report (5).md`).
- [x] **DAR-01.5: Purge Duplicate Guides & Enforce Anti-Archive Policy.** Deleted obsolete duplicate guide files (`GETTING_STARTED.md`, `QUICK_START.md`, `DEPLOYMENT_GUIDE.md`, `TROUBLESHOOTING.md`, `COOKBOOK.md`, `BEST_PRACTICES.md`, `FAQ.md`, `PERFORMANCE_OPTIMIZATION.md`, `GLOSSARY.md`, `ARCHITECTURE_DECISIONS.md`, `AUDIT_PLAN.md`). Completely deleted temporary `docs/archive/` directory in accordance with the strict no-archives principle.
- [x] **DAR-01.6: Run Verification.** Verified that `npm run verify:docs` passes 100% across all 6 stages with zero broken links.

---

### DAR-02: Architecture Core & Target Topology Actualization

**Scope:** `docs/architecture/*` (22 files, 5,273 LOC).
**Goal:** Align architecture specs with current code reality (FBA, FFA, SeaORM entities, multi-tenancy, and transactional outbox). Eliminate displaced ADR directories and obsolete transport drift notes.

- [x] **DAR-02.1: Audit Central Architecture Maps.** Verified `docs/architecture/overview.md` and `docs/architecture/principles.md` against current modular crate structure, `PortContext` zero-trust boundaries, agent permission intersection, and dual-database topology.
- [x] **DAR-02.2: Verify Routing, Settings & Profiles.** Verified routing boundaries (`routing.md`), settings hierarchy (`settings.md`), and distribution profiles (`distribution-profiles.md`) against `crates/libs/rustok-runtime` and `apps/server`.
- [x] **DAR-02.3: Relocate Displaced ADR to Canonical Registry.** Identified and relocated misplaced `docs/architecture/decisions/2026-10-09-newsletter-module-architecture.md` into canonical `DECISIONS/`, brought into full 13-heading structural compliance, and registered in `DECISIONS/README.md`. Removed obsolete `docs/architecture/decisions/` directory.
- [x] **DAR-02.4: Purge Obsolete Transport Drift Notes.** Deleted obsolete `docs/architecture/database-multilingual-transport-drift.md` (resolved July 2026 notes) in accordance with the strict no-archives/no-histories policy.
- [x] **DAR-02.5: Synchronize Diagrams and Run Verification.** Verified Mermaid architecture diagrams in `docs/architecture/diagram.md` against Cargo workspace topology. Ran `npm run verify:docs` (all 6 stages 100% PASS) and `npm run verify:adrs` (177 decisions 100% PASS).

---

### DAR-03: Engineering Standards & Policies Reconciliation

**Scope:** `docs/standards/*` (15 files, 5,617 LOC).
**Goal:** Reconcile platform coding standards, error handling, transactional outbox, tracing, and security rules with executable libraries.

- [x] **DAR-03.1: Reconcile Coding Standards and Error Guidelines.** Audited `docs/standards/coding.md` and `errors.md`. Reconciled dual error architecture in `rustok-core`: rich context errors (`rustok_core::error::{RichError, ErrorResponse}`) and typed machine-readable errors (`rustok_core::typed_error::{ErrorCode, DomainError, TypedResult}`).
- [x] **DAR-03.2: Verify Transactional Outbox Standard.** Verified `docs/standards/transactional-outbox.md` against `rustok-outbox` and SeaORM `publish_in_tx(&txn, event)` semantics. Confirmed zero uncommitted event publishing in core flows.
- [x] **DAR-03.3: Verify Distributed Tracing and OpenTelemetry.** Verified `distributed-tracing.md` and `opentelemetry-integration.md` against `crates/libs/rustok-telemetry` and W3C trace context propagation.
- [x] **DAR-03.4: Verify Security and Forbidden Actions.** Audited `security.md`, `forbidden-actions.md`, and `patterns-vs-antipatterns.md`. Confirmed 0 broken links and full synchronization with platform invariants.
- [x] **DAR-03.5: Run Scoped Verification.** Ran `npm run verify:docs` — 100% PASS across all 6 stages. Zero broken relative links or missing anchors in `docs/standards/`.

---

### DAR-04: Operations, Deployment & Runbooks Unification

**Scope:** `docs/operations/*`, `ops/*`, `RUNBOOKS/*`, `SPECS/*` (6 files, 774 LOC).
**Goal:** Consolidate scattered operational documentation into an authoritative directory and eliminate ghost directories with broken links.

- [x] **DAR-04.1: Eliminate Ghost Directories.** Removed empty stub directories `RUNBOOKS/` and `SPECS/` containing only broken phantom links (`QUICKSTART.md`, `OBSERVABILITY_QUICKSTART.md`, `grafana-setup.md`). Operational runbooks and architecture specifications live strictly in `docs/operations/` and `DECISIONS/`.
- [x] **DAR-04.2: Connect Observability & Benchmark Assets.** Reconciled `ops/` (`ops/benches`, `ops/grafana`, `ops/prometheus`) as canonical operational infrastructure, linking directly from `docs/operations/README.md`.
- [x] **DAR-04.3: Verify Configuration Reference.** Reconciled `docs/operations/configuration-reference.md` with runtime configuration files in `apps/server/config/*.yaml` and platform environment variables.
- [x] **DAR-04.4: Run Scoped Verification.** Ran `npm run verify:docs` — 100% PASS across all 6 stages. Zero broken relative links or missing anchors in operations documentation.

---

### DAR-05: UI Workbench, Design Tokens & Client Surface Contracts

**Scope:** `docs/UI/*`, root `UI/*`, `packages/*` (24 files, 2,659 LOC).
**Goal:** Align UI documentation with multi-framework design tokens and Fluid Frontend Architecture (FFA) standards.

- [x] **DAR-05.1: Verify UI Design Tokens & Multi-Framework Assets.** Verified `UI/tokens/` (`tokens.json`, `base.css`, `rustok_tokens.g.dart`) and component implementations across Leptos, React, and Flutter. Corrected typos in `UI/docs/README.md`.
- [x] **DAR-05.2: Verify Richtext Authoring Runtime Contract.** Verified `packages/richtext` contract against ProseMirror/Tiptap protocol and `RichTextDocument` schema across Next.js and Leptos SSR/hydrate modes.
- [x] **DAR-05.3: Reconcile Frontend Application Documentation.** Reconciled `docs/UI/ui-workbench-architecture.md`, `module-package-implementation.md`, and host READMEs (`apps/admin`, `apps/storefront`, `apps/next-admin`, `apps/next-frontend`).
- [x] **DAR-05.4: Run Scoped UI Verification.** Ran `npm run verify:ui:parity` (all 28 design token and component checks passed) and `npm run verify:ffa:ui:migration:docs` (all 7 pattern checks passed). Zero broken links in `docs/UI/`.

---

### DAR-06: Verification Plans & Quality Automation Evolution

**Scope:** `docs/verification/*` (11 files, 2,126 LOC).
**Goal:** Modernize verification documentation; purge obsolete historical hardening diaries in accordance with the strict no-archives/no-histories policy.

- [x] **DAR-06.1: Purge Historical Hardening Diaries.** Deleted obsolete July 2026 hardening files (`PLATFORM_HARDENING_STATUS_2026-07-18.md` and `PLATFORM_HARDENING_IMPLEMENTATION_PLAN.md`). Reconciled incoming links in `docs/index.md` and `docs/modules/page-builder-backlog.md` to reference living ADRs.
- [x] **DAR-06.2: Reconcile Living Verification Guides.** Verified remaining 11 verification plans (`PLATFORM_VERIFICATION_PLAN.md`, `platform-api-surfaces-verification-plan.md`, `platform-foundation-verification-plan.md`, `ffa-ui-parity-checklist.md`, etc.) against active scripts in `scripts/verify/`.
- [x] **DAR-06.3: Align Verification Architecture.** Reconciled `docs/verification/README.md` to reflect the 3-pillar quality model: Continuous Full-Stack Review (`continuous-review-ledger.md`), Cross-Cutting Review (`cross-cutting-review-ledger.md`), and Architecture & Documentation Governance (`docs-governance-ledger.md`).
- [x] **DAR-06.4: Run Scoped Verification.** Ran `npm run verify:docs` — 100% PASS across all 6 stages. Zero broken relative links or dead anchors.

---

### DAR-07: ADR Registry Deep Audit & Graph Reconciliation

**Scope:** All 178 files in `DECISIONS/` (21,787 LOC).
**Goal:** Complete lifecycle alignment across all ADRs; reconcile relations graph.

- [x] **DAR-07.1: Deep audit of remaining 31 `In progress` ADRs against code.** Verified executable truth across SeaORM entities, service layers, and tests. Promoted 19 fully implemented decisions to `Implemented` in ADR files and `DECISIONS/README.md`:
  1. `2026-11-05-bounded-migration-execution.md` (`rustok-migrations`: `ExecutionPhase`, `StepRunner`, `tests/stepwise_recovery.rs`).
  2. `2026-11-04-region-storage-and-cart-tax-policy.md` (`rustok-region`, `rustok-cart`: `read_tax_policy_in_tx`).
  3. `2026-11-04-hybrid-brand-catalog-and-product-binding.md` (`rustok-brand`, `rustok-product`: `product_brands` table, `m20261104_000039_create_product_brands`, `ProductBrandInput`).
  4. `2026-11-02-rbac-graphql-grant-invalidation.md` (`rustok-rbac`: generation advance in role mutations, `tests/role_crud_invariants.rs`).
  5. `2026-11-02-rbac-cache-resolution-consistency.md` (`rustok-rbac`: `PermissionCacheResolutionError`, bounded retry).
  6. `2026-11-02-invalidated-rbac-request-scope.md` (`apps/server`: `RbacRequestScope`, `RbacRequestScopeState`, fail-closed admission).
  7. `2026-11-01-oauth-access-token-revocation-fence.md` (`apps/server`: `oauth_tokens` `token_kind = access`, revocation fence).
  8. `2026-10-25-search-projection-driver-ownership.md` (`rustok-search`: `SearchProjectionDriver`, blog projector cutover).
  9. `2026-10-21-shipment-stock-port-and-return-restock.md` (`rustok-fulfillment`, `rustok-inventory`, `rustok-commerce`: `ShipmentStockPort`, `m20261021_000001_drop_fulfillment_shipping_stock_trigger`).
  10. `2026-10-21-return-refund-server-side-valuation.md` (`rustok-commerce`: `return_refund_value.rs`, `ReturnDecisionOwnerOrchestrationService`).
  11. `2026-10-21-order-shipment-status-projection.md` (`rustok-commerce`: `OrderShipmentProjector`, `ProjectingFulfillmentAdminCommandPort`).
  12. `2026-10-21-order-cancellation-orchestration.md` (`rustok-commerce`: `OrderCancellationOrchestrator`, `OrchestratedOrderAdminCommandPort`).
  13. `2026-10-17-forum-topic-feed-selection.md` (`rustok-forum`: `ForumTopicListOrder`, top/latest, cursor fingerprinting).
  14. `2026-10-11-notifications-outbox-intake-decoder-claims.md` (`rustok-notifications`: `accepted_event_types`, decoder claims).
  15. `2026-10-09-site-symbols-shared-definitions.md` (`rustok-pages`: `m20261009_000003_create_site_symbols`, `site_symbol.rs`, symbol expansion).
  16. `2026-10-09-forum-topic-pre-moderation.md` (`rustok-forum`: `m20261009_000038_allow_pending_forum_topic_status`, pending visibility).
  17. `2026-10-09-forum-topic-owner-audience-read.md` (`rustok-forum`: audience read visibility).
  18. `2026-10-09-forum-reply-pre-moderation-setting.md` (`rustok-forum`: reply pre-moderation settings and enforcement).
  19. `2026-09-24-forum-attachment-relations.md` (`rustok-forum`: `ForumAttachmentRelationService`, CAS revision fencing, Media holds).
- [x] **DAR-07.2: Triage remaining active `In progress` and 83 legacy `Not tracked` ADRs.** Confirmed active implementation scope for remaining 12 in-progress ADRs (`2026-11-03-migration-consolidation`, `2026-10-28-forum-write-acknowledgements`, `2026-10-28-forum-first-publication-event-lifecycle`, `2026-10-26-collaborative-spreadsheet-engine-loro-ironcalc`, `2026-10-17-product-image-media-reference-retention`, `2026-10-16-page-and-template-builder-unification`, `2026-10-14-admin-parity-program`, `2026-10-11-forum-reply-anchor-route-contract`, `2026-10-09-page-layouts-and-menu-page-links`, `2026-10-09-forum-vote-subscription-write-audience`, `2026-10-08-owner-routes-and-batched-conversion`, `2026-09-18-unified-variant-axis-architecture`).
- [x] **DAR-07.3: Reconcile Extends/Supersedes relations graph.** Confirmed 0 broken relations or dangling links across all decisions in `DECISIONS/`.
- [x] **DAR-07.4: Run ADR Governance Verification.** Ran `node scripts/verify/verify-adrs.mjs` — 100% PASS for all 177 decisions. Descending order verified. Exact metadata match verified.

---

### DAR-08: Commerce & Checkout Cluster

**Scope:** Documentation across 11 commerce crates in `crates/modules/` (208 files, 24,150 LOC).
**Goal:** Verify and synchronize domain contracts, CRATE_API, entity models, port interfaces, and diagnostic specifications across all commerce modules.

- [x] **DAR-08.1: Audit and Reconcile Contract Anchors.** Corrected anchor slugification mismatch in `crates/modules/rustok-product/CRATE_API.md` and `crates/modules/rustok-product/admin/CRATE_API.md` (`#known-limitations--pending-implementation` -> `#known-limitations-pending-implementation`).
- [x] **DAR-08.2: Index and Reconcile Unreferenced Diagnostic Specifications.** Reconciled and linked 88 previously unindexed architecture, compensation, and error-safety specifications into their respective `docs/README.md` across 10 modules:
  - `rustok-cart/docs/README.md`: linked checkout admission context and storefront error-safety docs.
  - `rustok-pricing/docs/README.md`: linked pricing implementation plan, owner port error safety, and local context specs.
  - `rustok-inventory/docs/README.md`: linked inventory implementation plan, port diagnostics, transport error safety, and reservation contexts.
  - `rustok-order/docs/README.md`: linked order read port specs, checkout recovery diagnostics, and storefront runtime diagnostics.
  - `rustok-payment/docs/README.md`: linked payment collection contexts, execution diagnostic safety, and storefront error safety.
  - `rustok-fulfillment/docs/README.md`: linked fulfillment lifecycle contracts, shipping selection diagnostics, and storefront error safety.
  - `rustok-tax/docs/README.md`: linked tax implementation plan and calculation policy contexts.
  - `rustok-region/docs/README.md`: linked region owner port error safety and storefront native error safety.
  - `rustok-commerce/docs/README.md`: linked all 33 checkout compensation, admin diagnostic, and GraphQL query safety specifications.
  - `rustok-product/docs/README.md`: linked all 23 attribute lifecycle, variant axis, and index ledger specifications.
- [x] **DAR-08.3: Verify Cross-Module Link & Anchor Integrity.** Audited all 208 markdown files and 642 internal links/anchors across the entire commerce cluster. Verified zero broken links and zero dangling anchors.
- [x] **DAR-08.4: Run Scoped Verification.** Ran `node scripts/verify/verify-docs.mjs` (100% PASS across all 6 stages), `node scripts/verify/verify-adrs.mjs` (100% PASS), and `node scripts/verify/verify-ui-parity.mjs` (100% PASS).

---

### DAR-09..DAR-15: Remaining Domain Modules Documentation Audit (by Clusters)

**Scope:** Documentation across remaining module directories in `crates/modules/` and `docs/modules/`.
**Goal:** Verify that every domain module's README, CRATE_API, entity schemas, and port documentation reflect executable reality.

- [ ] **DAR-09: Content, Pages & Forum Cluster** (`rustok-blog`, `rustok-forum`, `rustok-comments`, `rustok-pages`, `rustok-page-builder`, `rustok-media`, `rustok-docs`, `rustok-seo`).
- [ ] **DAR-10: Platform Capabilities Cluster** (`rustok-auth`, `rustok-channel`, `rustok-cache`, `rustok-notifications`, `rustok-search`, `rustok-telemetry`, `rustok-events-module`, `rustok-outbox`).
- [ ] **DAR-11: Marketplace & Multi-Tenancy Cluster** (`rustok-marketplace*`, `rustok-tenant`, `rustok-distribution`).
- [ ] **DAR-12: Customer, CRM & Finance Cluster** (`rustok-customer`, `rustok-deal`, `rustok-finance`, `rustok-accounting`, `rustok-billing`, `rustok-forms`, `rustok-workflow`).
- [ ] **DAR-13: AI & Emerging Capabilities Cluster** (`rustok-ai*`, `rustok-mcp`, `rustok-messenger`, `rustok-tasks`, `flex`, `alloy`).
- [ ] **DAR-14: Platform Foundation Libraries & Utilities** (`crates/libs/*`, `crates/utils/*`, `crates/workers/*`).
- [ ] **DAR-15: Client Applications & Host Packages** (`apps/server`, `apps/admin`, `apps/next-admin`, `apps/storefront`, `apps/next-frontend`).
- [ ] **DAR-16: Continuous Documentation Verification Evolution** (Automated orphaned document detection, dead path detection, and ADR lifecycle enforcement in CI).

---

## Completed Phases (DAR Cycle 1 Evidence)

### DAR-Cycle-1: Baseline Architecture & Documentation Governance (Completed 2026-11-06)

- **DAR-01 (ADR Review):** Audited 52 `In progress` ADRs; promoted 21 verified decisions to `Implemented` (pricing port decoupling, product-relations dual-DB, manifest foundation library, forum settings suite, blog slug transliteration, checkout journal, HttpOnly session cookies, settings architecture, module source layout). Passed `node scripts/verify/verify-adrs.mjs` (176 decisions valid).
- **DAR-02 (Operations Noise Purge):** Synthesized all config keys from YAML and platform crates into `docs/operations/configuration-reference.md`. Deleted 572 auto-generated machine stubs in `docs/operations/`, reducing doc count by 77%.
- **DAR-03 (Root Entrypoints & Map Reset):** Purged outdated October 2026 continuation diaries from `docs/index.md`. Updated `ARCHITECTURE.md` and `docs/glossary.md`.
- **DAR-04 (Domain Module Contracts & Plan Archiving):** Archived completed historical plans (`page-builder-implementation-plan.md`, `pages-page-builder-inline-edit-execution-plan.md`, `pages-page-builder-parity-continuation-plan.md`, `error-fix.md`) into structured `docs/archive/plans/` and `docs/archive/research/`. Passed `npm run verify:manifest` and `npm run verify:module-source-layout`.
- **DAR-05 (Frontend Client Contracts):** Passed `npm run verify:ui:parity` and `npm run verify:ffa:ui:migration:docs`.
- **DAR-06 (Continuous Verification Evolution):** Extended `verify-docs.mjs` with 13 core platform documents, strict link and anchor checks. Passed `npm run verify:docs` across all 6 stages.
