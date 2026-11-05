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
- **Active Phase:** DAR-01 (ADR Registry Deep Review & Lifecycle Reconciliation)
- **Completed Phases:** 0/6 (0% PASS)
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

- [ ] **[DAR-01: ADR Registry Deep Review & Lifecycle Reconciliation](#dar-01-adr-registry-deep-review--lifecycle-reconciliation)** (0/5 items completed — 0%)
- [ ] **[DAR-02: Operations Noise Purge & Unified Configuration Reference](#dar-02-operations-noise-purge--unified-configuration-reference)** (0/4 items completed — 0%)
- [ ] **[DAR-03: Root Entrypoints, Architecture Map & Glossary Reset](#dar-03-root-entrypoints-architecture-map--glossary-reset)** (0/4 items completed — 0%)
- [ ] **[DAR-04: Domain Module Contracts & Ephemeral Plan Garbage Collection](#dar-04-domain-module-contracts--ephemeral-plan-garbage-collection)** (0/5 items completed — 0%)
- [ ] **[DAR-05: Frontend Client Contracts & FFA Documentation Parity](#dar-05-frontend-client-contracts--ffa-documentation-parity)** (0/4 items completed — 0%)
- [ ] **[DAR-06: Continuous Documentation Verification & Linting Evolution](#dar-06-continuous-documentation-verification--linting-evolution)** (0/4 items completed — 0%)

---

### DAR-01: ADR Registry Deep Review & Lifecycle Reconciliation

**Scope:** All 176 decisions in `DECISIONS/` and `DECISIONS/README.md`.
**Goal:** Align ADR status with actual code reality. Reconcile 50 "In progress" decisions, eliminate ambiguous "Not tracked" legacy statuses, identify superseded decisions, and ensure all ADRs satisfy structural invariants.

- [ ] **DAR-01.1:** Audit the 50 `In progress` ADRs against current code. Promote completed decisions to `Implemented` with commit/crate evidence.
- [ ] **DAR-01.2:** Identify contradictory or superseded historical decisions. Update their status to `Superseded` and link replacement ADRs.
- [ ] **DAR-01.3:** Triage the 83 legacy `Not tracked` ADRs. Categorize into `Implemented`, `Superseded`, or `Rejected`.
- [ ] **DAR-01.4:** Reconcile ADR Relations graph (Extends / Supersedes / Superseded by). Ensure no broken links or phantom ADR references exist.
- [ ] **DAR-01.5:** Run `node scripts/verify/verify-adrs.mjs` and record verified status breakdown in this ledger.

---

### DAR-02: Operations Noise Purge & Unified Configuration Reference

**Scope:** `docs/operations/` and configuration documentation.
**Goal:** Eliminate 574 auto-generated single-key stub files (`config-config-apps-server-...-expiration.md`). Replace with a single, authoritative, well-structured configuration reference.

- [ ] **DAR-02.1:** Synthesize all configuration keys from `apps/server/config/*.yaml` and platform crates into a single canonical specification: `docs/operations/configuration-reference.md`.
- [ ] **DAR-02.2:** Safely delete the 570+ auto-generated machine stubs in `docs/operations/` that dilute search, context, and link graphs.
- [ ] **DAR-02.3:** Audit remaining operational runbooks in `docs/operations/` (migrations, backup/restore, deployment, observability).
- [ ] **DAR-02.4:** Run `npm run verify:docs` to ensure zero broken links after the operational purge.

---

### DAR-03: Root Entrypoints, Architecture Map & Glossary Reset

**Scope:** `docs/index.md`, `ARCHITECTURE.md`, `docs/architecture/*`, `docs/glossary.md`.
**Goal:** Remove historical scratchpad diaries ("step 8 unrun", "27 definitions unrun") from central entrypoints. Present a clean, high-signal architecture map.

- [ ] **DAR-03.1:** Clean `docs/index.md`: purge outdated ephemeral October 2026 continuation diaries and replace with clear navigation to living owner contracts.
- [ ] **DAR-03.2:** Update `ARCHITECTURE.md`: align high-level diagrams and layered architecture overview with current FBA, FFA, and modular microservice reality.
- [ ] **DAR-03.3:** Audit `docs/architecture/*` (overview, principles, routing, settings, distribution profiles) for obsolete terminology and broken assumptions.
- [ ] **DAR-03.4:** Reconcile `docs/glossary.md` with modern platform terms (FFA, FBA, TenantContext, PortSeam, OutboxDecoder, etc.).

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
