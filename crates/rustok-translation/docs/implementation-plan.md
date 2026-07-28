---
id: doc://crates/rustok-translation/docs/implementation-plan.md
kind: module_plan
language: en
status: in_progress
last_reviewed: 2026-07-28
---

# Translation implementation plan

## Scope

Build the optional, admin-only translation control plane described by the
central plan without taking ownership of domain-localized data or runtime locale
selection.

## Current State

- The platform module, manifest, RBAC resources, migration source, and
  distribution feature are present.
- The module persists tenant-scoped inventory identities and provider cursor
  checkpoints without storing source or translated text.
- `TranslationInventoryService` consumes only the neutral provider registry,
  checks Translation workflow permission, delegates owner authorization to the
  provider, validates bounded requests, rejects cross-provider identities and
  non-advancing or missing cursors, collapses duplicate identities, and
  advances checkpoints with optimistic revision protection.
- Integration evidence covers cursor replay, tenant-isolated inventory and
  checkpoints, invalid bounds, provider outage, missing cursors, and
  cross-provider identity rejection without partial persistence. Bounded
  full-rescan drains the owner cursor, replaces one provider projection
  atomically, and rolls back if that checkpoint advances during listing.
- The first manual-workflow persistence slice creates tenant-scoped jobs,
  immutable owner-provider source snapshots, proposal/approval tables, and
  application-receipt tables. `TranslationWorkflowService` exposes idempotent
  job creation and item admission plus owner-validated proposal save, submit,
  and approval transitions. Request hashes, item/job revision CAS, persisted QA
  issues, current-proposal checks, and translator/reviewer separation guard the
  implemented flow.
- Media is the first owner provider with durable change-cursor repair.
- Durable owner-apply intent/reconciliation, assignments, memory, glossaries,
  interchange, transports, UI, and AI integration are not implemented yet.

## FFA/FBA status

- FFA status: `not_started`
- FBA status: `in_progress`
- Structural shape: `core_no_ui`
- Evidence:
  - module-owned core and neutral provider dependency are separated;
  - no transport or UI has been published;
  - admin UI is planned for Leptos and Next with native/GraphQL parity.
- Last verified at (UTC): 2026-07-28
- Owner: Translation module maintainers

## Milestones

1. Complete Media multi-replica evidence for the implemented inventory replay,
   tenant isolation, stale-checkpoint conflict, provider outage, and
   full-rescan recovery contracts.
2. Extend the implemented manual job/snapshot/proposal/review foundation with
   assignments and a durable owner-application state machine that reconciles
   unknown outcomes before recording a terminal receipt.
3. Add rebuildable progress projections and operator recovery.
4. Add translation memory and versioned glossaries with separate permissions.
5. Add bounded owner-aware import/export.
6. Publish module-owned GraphQL and native server-function adapters.
7. Add Leptos and Next admin surfaces in parity.
8. Integrate `rustok-ai-translation` only after manual apply and structured AI
   execution foundations pass their gates.

## Verification

- `cargo check -p rustok-translation`
- `cargo test -p rustok-translation`
- `cargo xtask module validate translation`
- `cargo xtask validate-manifest`
- `cargo check -p rustok-server --lib --no-default-features --features mod-translation`
- `node scripts/verify/verify-translation-surface-registry.mjs`

## Update Rules

- Never query or mutate owner tables from this module.
- Never count runtime fallback as an exact translation.
- Never let AI output apply owner data without deterministic validation and
  review.
- Keep the local FFA/FBA block and central readiness board synchronized when
  transport or UI appears.
- Keep `rustok-translation-targets` dependency-neutral even if its directory is
  physically colocated with this module later.
