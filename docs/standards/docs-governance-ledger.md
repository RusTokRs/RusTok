# Architecture, Documentation & ADR Governance Ledger (DAR)

This file is the canonical living review ledger for **architecture, documentation, and ADR governance audits** across the RusToK platform.
While the vertical audit ([continuous-review-ledger.md](./continuous-review-ledger.md)) verifies crate-scoped code logic and the horizontal audit ([cross-cutting-review-ledger.md](./cross-cutting-review-ledger.md)) verifies system-wide capabilities, this ledger governs the **authoritative target architecture and source-of-truth layer**: ADRs, central architecture maps, domain module contracts, operational specifications, and verification tooling.

> [!IMPORTANT]
> **Core Principle: Deep Architectural Understanding (Anti-Slop / Anti-Cargo-Cult)**
> Documentation audit is not bureaucratic checkboxing or blind enforcement of obsolete conventions.
> In accordance with [AGENTS.md](../../AGENTS.md) §1, §4, and §11:
> - Contributors and AI agents MUST deeply understand the domain contracts and system semantics rather than mechanically copying flawed, outdated, or redundant rules.
> - Demonstrably flawed, obsolete, or contradicted decisions MUST be corrected, consolidated, or superseded through ADR governance, not perpetuated merely because they were documented.
> - Machine-generated placeholder spam, duplicate specifications, and dead implementation diaries MUST be consolidated and purged.

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
4. Resolves discrepancies: corrects stale documentation, updates or supersedes outdated ADRs, consolidates fragmented or duplicate specs, and purges obsolete scratch files.
5. Runs scoped verification (`npm run verify:docs`, `npm run verify:adrs`, `npm run verify:manifest`, `cargo check -p <affected_crate>`).
6. Marks the sub-items and phase complete in this ledger and records integration evidence.

---

## Current Cycle Status

- **Status:** ACTIVE
- **Active Cycle:** DAR Cycle 1
- **Active Phase:** DAR-04 (Domain Module Contracts & Ephemeral Plan Garbage Collection)
- **Completed Phases:** 3/6 (50.0% PASS)
- **Active Branch:** `main`

---

## Document Taxonomy & Governance Hierarchy

Every document in the repository MUST belong to exactly one canonical category with a clear lifecycle:

| Document Type | Location | Purpose & Mutability | Lifecycle Policy |
| :--- | :--- | :--- | :--- |
| **Current Contract** | `docs/architecture/`, `crates/*/README.md`, `apps/*/README.md` | Living source of target architecture & component contracts. | **Mutable.** Updated in lockstep with code changes. Must always reflect 100% executable truth. |
| **Decision Record (ADR)** | `DECISIONS/YYYY-MM-DD-*.md` | Immutable record of an architectural choice, context, and tradeoffs. | **Immutable once accepted.** When architecture changes, create a new ADR and mark the old one `Superseded`. |
| **Implementation Plan** | `crates/*/docs/implementation-plan.md` | Ephemeral breakdown of current in-flight work and regression gates. | **Transient.** Once implementation merges and verifies, delete or consolidate into the component README / contract. Do NOT leave dead plans in root docs. |
| **Operational Spec** | `docs/operations/` | Concrete runbooks, deployment guides, and consolidated configuration references. | **Consolidated.** High-signal, human-readable reference; strictly no single-key generated markdown files. |
| **Archive** | `docs/archive/` | Historical exploratory research, legacy proposals, or deprecated migration logs. | **Read-only.** Preserved for historical context; never authoritative over active contracts. |

---

## Phase Backlog — Architecture & Documentation Governance Sequence (6 Phases)

- [x] **[DAR-01: ADR Registry Deep Review & Lifecycle Reconciliation](#dar-01-adr-registry-deep-review--lifecycle-reconciliation)** (5/5 items completed — 100% PASS)
- [x] **[DAR-02: Operations Noise Purge & Unified Configuration Reference](#dar-02-operations-noise-purge--unified-configuration-reference)** (4/4 items completed — 100% PASS)
- [x] **[DAR-03: Root Entrypoints, Architecture Map & Glossary Reset](#dar-03-root-entrypoints-architecture-map--glossary-reset)** (4/4 items completed — 100% PASS)
- [ ] **[DAR-04: Domain Module Contracts & Ephemeral Plan Garbage Collection](#dar-04-domain-module-contracts--ephemeral-plan-garbage-collection)** (0/5 items completed — 0%)
- [ ] **[DAR-05: Frontend Client Contracts & FFA Documentation Parity](#dar-05-frontend-client-contracts--ffa-documentation-parity)** (0/4 items completed — 0%)
- [ ] **[DAR-06: Continuous Documentation Verification & Linting Evolution](#dar-06-continuous-documentation-verification--linting-evolution)** (0/4 items completed — 0%)

---

### DAR-01: ADR Registry Deep Review & Lifecycle Reconciliation

**Scope:** All 176 decisions in `DECISIONS/` and `DECISIONS/README.md`.
**Goal:** Align ADR status with actual code reality. Reconcile 52 "In progress" decisions, eliminate ambiguous "Not tracked" legacy statuses, identify superseded decisions, and ensure all ADRs satisfy structural invariants.

- [x] **DAR-01.1:** Audit the 52 `In progress` ADRs against current code. Promoted 21 verified decisions to `Implemented` with concrete crate and code evidence (2026-11-05 pricing port decoupling, 2026-11-04 product-relations dual-DB, 2026-10-15 manifest foundation library, 2026-10-09 forum settings suite [unwired removal, length limits, edit windows, soft defaults, rate limits, user reports], 2026-10-08 blog slug transliteration/locales, 2026-10-07 checkout journal in typed Rust, 2026-09-27 HttpOnly session cookies, 2026-09-22 settings architecture, 2026-09-18 module source layout & reference contract). Verified that genuinely in-flight decisions (variant axis, shipment stock provenance, return restock valuation) remain `In progress`.
- [x] **DAR-01.2:** Reconciled relations and superseded status. Ensured that decisions with updated contracts link their canonical extensions/supersessions cleanly.
- [x] **DAR-01.3:** Triaged the 83 legacy `Not tracked` ADRs (pre-2026-09-18 governance boundary), verified workspace split conformance (`crates/{modules,libs,ui,utils,workers}`).
- [x] **DAR-01.4:** Reconciled ADR Relations graph (Extends / Supersedes / Superseded by). Ensured zero broken links or phantom ADR references exist across all 176 decisions.
- [x] **DAR-01.5:** Ran `node scripts/verify/verify-adrs.mjs` — 176/176 passed with 0 errors. Distribution: 52 Implemented, 31 In progress, 7 Not started, 83 Not tracked, 3 Not applicable.

---

### DAR-02: Operations Noise Purge & Unified Configuration Reference

**Scope:** `docs/operations/` and configuration documentation.
**Goal:** Eliminate 574 auto-generated single-key stub files (`config-config-apps-server-...-expiration.md`). Replace with a single, authoritative, well-structured configuration reference.

- [x] **DAR-02.1:** Synthesized all configuration keys from `apps/server/config/*.yaml` and platform crates into a single canonical specification: [`docs/operations/configuration-reference.md`](../operations/configuration-reference.md) covering server, database pools, JWT auth, tenant profiles, rate limiting, events/outbox, search drivers, caching, and observability.
- [x] **DAR-02.2:** Safely deleted 572 auto-generated machine stubs (`config-*`, `env-*`, `script-*`, `deployment-*`, `operation-*`) in `docs/operations/` that diluted search, context, and link graphs (reducing `docs/` file count from 741 to 171 files — 77% noise reduction).
- [x] **DAR-02.3:** Audited remaining operational documentation in `docs/operations/`: preserved high-signal `tenant-runtime-profiles.md` and updated `README.md` to serve as a clean navigation index.
- [x] **DAR-02.4:** Ran `npm run verify:docs` to ensure zero broken links after the operational purge — 100% PASS across all 6 verification stages.

---

### DAR-03: Root Entrypoints, Architecture Map & Glossary Reset

**Scope:** `docs/index.md`, `ARCHITECTURE.md`, `docs/architecture/*`, `docs/glossary.md`.
**Goal:** Remove historical scratchpad diaries ("step 8 unrun", "27 definitions unrun") from central entrypoints. Present a clean, high-signal architecture map.

- [x] **DAR-03.1:** Clean `docs/index.md`: purged outdated ephemeral October 2026 continuation diaries and transient node/rust definition scratchpads; established clear, high-signal architecture navigation to living owner contracts.
- [x] **DAR-03.2:** Updated `ARCHITECTURE.md`: aligned high-level system entrypoints with current FBA, FFA, and modular microservice reality, adding explicit links to Configuration Reference and DAR Ledger.
- [x] **DAR-03.3:** Audited `docs/architecture/*` (overview, principles, routing, settings, distribution profiles) for obsolete terminology and broken assumptions.
- [x] **DAR-03.4:** Reconciled `docs/glossary.md` with modern platform terms (`DAR`, `PortSeam`, `OutboxDecoder`). All links and anchors pass verification.

---

### DAR-04: Domain Module Contracts & Ephemeral Plan Garbage Collection

**Scope:** `docs/modules/*` and `crates/modules/*/docs/`.
**Goal:** Ensure each domain module has an accurate README and contract. Archive or purge dead `*-implementation-plan.md` files that have long since landed in `main`.

- [ ] **DAR-04.1:** Audit `docs/modules/registry.md` and `docs/modules/crates-registry.md` against actual crates in `crates/modules/`. Ensure 100% parity.
- [ ] **DAR-04.2:** Sweep `crates/modules/*/docs/` for historical implementation plans. Archive landed plans into `docs/archive/plans/` or delete completed scratchpads.
- [ ] **DAR-04.3:** Verify that each module's public ports (`src/ports/`) match its documented API contracts.
- [ ] **DAR-04.4:** Synchronize database schema documentation with actual SeaORM entities and migrations for all domain modules.
- [ ] **DAR-04.5:** Run `npm run verify:manifest` and `npm run verify:module-source-layout` to ensure physical conformance.

---

### DAR-05: Frontend Client Contracts & FFA Documentation Parity

**Scope:** `docs/UI/`, `apps/admin/docs/`, `apps/next-admin/docs/`, `apps/next-frontend/docs/`, `apps/storefront/docs/`.
**Goal:** Align frontend documentation with the Dual-Frontend Fluid Frontend Architecture (FFA) standard.

- [ ] **DAR-05.1:** Audit `docs/UI/` and `docs/UI/ui-workbench-architecture.md` against Leptos and Next.js client realities.
- [ ] **DAR-05.2:** Ensure dual-client parity expectations (routing, admin packages, GraphQL schema synchronization) are explicitly documented for every UI module.
- [ ] **DAR-05.3:** Clean up outdated UI migration guides from old temporary experiments.
- [ ] **DAR-05.4:** Run `npm run verify:ui:parity` and `npm run verify:ffa:ui:migration:docs`.

---

### DAR-06: Continuous Documentation Verification & Linting Evolution

**Scope:** `scripts/verify/verify-docs.mjs`, `scripts/verify/verify-adrs.mjs`, CI workflows.
**Goal:** Upgrade automated verification so documentation drift and phantom references are caught automatically in CI.

- [ ] **DAR-06.1:** Extend `verify-docs.mjs` to detect crate path references and fail if a documented crate or directory path does not exist on disk.
- [ ] **DAR-06.2:** Add orphaned document detection: warn/fail on markdown files in `docs/` that are unlinked from `docs/index.md` or any navigation index.
- [ ] **DAR-06.3:** Enforce ADR lifecycle validation: prevent ADRs from remaining in `In progress` without an active implementation reference.
- [ ] **DAR-06.4:** Run full documentation gate suite (`npm run verify:docs`) and certify zero defects across all repositories.
