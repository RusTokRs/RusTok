---
id: doc://docs/verification/PLATFORM_VERIFICATION_PLAN.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# RusToK Main Platform Verification Plan

- **Structure update date:** 2026-07-20
- **Status:** Cycle active
- **Mode:** Cyclic, resumable pre-release defect-removal sweep
- **Goal:** Repeatedly inspect and repair the platform before release, prioritizing critical defects and cross-module contract failures

## Scope and Terminology

This document is the durable cursor and reset-friendly checklist for the current
verification cycle. Detailed checks remain in the specialized plans in this folder,
while the actual state, findings, fixes, evidence, and next action for a component are
written to that component's local `docs/implementation-plan.md`.

The following terms must not be conflated:

- **Core modules** are the platform modules declared with `required = true` in the
  `[modules]` section of `modules.toml`. They are always active and participate in the
  module/runtime contract.
- **`rustok-core`** is a platform foundation crate. It is not a Core module and is
  checked in the separate foundation-crate wave.
- **`apps/server`** is the composition root and runtime host. It is not a module.

The module manifest is the source of truth. When `modules.toml` adds, removes, renames,
or changes dependencies of a module, update the queue in this plan in the same change.

## Current Cycle Cursor

This block is the first place an agent reads after `docs/index.md`.

- Cycle: `cycle-001`
- Cycle status: `active`
- Current item: `core/auth`
- Next item: `core/auth`
- Started at (UTC): `2026-07-20`
- Last handoff at (UTC): `2026-07-20`
- Carried release blockers: `none recorded`
- Release readiness: `not_assessed`

Allowed cycle statuses are `ready`, `active`, and `closing`. An item uses `pending`,
`in_progress`, `completed`, or `blocked` in its local handoff block. Only one item may
be `in_progress` at a time.

## Agent Start and Resume Protocol

1. Read `docs/index.md`, this cursor, the target component's `README.md`, local
   `docs/README.md`, and local `docs/implementation-plan.md`. Read the relevant
   architecture and module-owner documents before changing code.
2. If the cycle is `ready`, change it to `active`, set `Current item` to
   `core/modules`, and keep `Next item` at the first unfinished queue entry.
3. If the cycle is already `active`, resume `Current item`. If the cursor and queue
   disagree, use the first unchecked item, but first reconcile its local handoff block.
4. A local `completed` result counts only when its `Cycle` equals the current cycle.
   A result from an earlier cycle is evidence, not completion for the current cycle.
5. Before inspecting or editing a component, set its local handoff to `in_progress`.
   After verification, update the component's actual implementation plan and the
   handoff, then update this cursor and queue in the same work unit.
6. Never skip an item silently. Mark it `blocked` locally with the exact blocker,
   reproduction command, observed output summary, and next action. Mark its master
   checkbox as visited and append `— blocked` to the queue row so the cursor can advance.
   Every blocked item must be revisited in the closing gate.
7. Do not begin with a full-workspace build that prevents targeted progress. Run the
   quick manifest/architecture preflight, then use targeted component checks. Run the
   expensive workspace, migration, and end-to-end gates when the queue closes or when
   a changed contract requires them earlier.

## Defect Policy

Use these severities consistently:

- `P0` — exploitable security/tenant-isolation failure, data loss/corruption, invalid
  authorization grant, or platform-wide inability to start or serve critical traffic.
- `P1` — serious cross-module inconsistency, broken transaction/outbox/replay path,
  stale authorization/cache/index state, migration failure, or major release path
  failure without a safe operational workaround.
- `P2` — functional defect with bounded impact or a safe workaround.
- `P3` — minor correctness, resilience, diagnostics, or maintainability defect.

Fix reproducible `P0` and `P1` defects immediately when the correction is within the
current component or its directly involved owners. Add regression evidence before
marking the item complete. If a safe fix requires broader authority or a separate
architectural decision, mark the item `blocked`, record the owners and exact next
action, and continue the sweep so that other critical defects can still be found.

An item is not `completed` merely because it compiles. Completion requires inspection
of its applicable cross-module matrix, targeted tests, documentation truthfulness, and
no unresolved `P0`/`P1` finding in that item's scope. `P2`/`P3` findings that are not
fixed in the pass must become explicit nearest priorities in the local implementation
plan; they must not exist only in chat or terminal output.

## Local Implementation-Plan Handoff

Every visited module, foundation crate, and application must contain one current block
with this exact heading in its existing `docs/implementation-plan.md`:

```md
## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `pending | in_progress | completed | blocked`
- Last verified at (UTC):
- Scope inspected:
- Findings: `P0=0, P1=0, P2=0, P3=0`
- Fixed in this pass:
- Remaining risks or blockers:
- Evidence:
- Next action:
- Resume command:
```

The block is overwritten for the current visit; it is not an append-only execution
log. Durable open work belongs in the local plan's current priorities. Durable
contract changes belong in the component docs and, when cross-cutting, central docs or
an ADR. Git history preserves previous handoffs.

At the start of a new cycle, do not mass-edit every local plan. The new cycle identifier
invalidates old completion marks automatically. When the component is visited again,
replace its handoff with the new cycle and current evidence. This keeps local plans
useful without reset-only churn.

## Mandatory Cross-Module Inspection Matrix

Apply every relevant row to every component. Record `not applicable` with a reason;
never omit a row because the integration is indirect.

| Circuit | Required questions and failure probes |
| --- | --- |
| Ownership and boundaries | Does the owner module expose a typed public port/transport contract? Are callers avoiding owner DB entities, private services, host-local facades, compatibility paths, and dependency cycles? Do `modules.toml`, `rustok-module.toml`, runtime wiring, migrations, and docs agree? |
| RBAC and trust | Are permission identifiers owner-defined and deny-by-default? Are tenant, actor, principal, and channel facts derived from trusted runtime context? Do REST, GraphQL, native `#[server]`, jobs, CLI, event consumers, and admin paths enforce equivalent authorization? Are role/permission writes tenant-composite and do they invalidate durable permission state across replicas? |
| Cache coherence | Do keys include every isolation and selection dimension required by the contract, including tenant, locale, channel, principal/auth generation, policy revision, or module state? Do writes, enable/disable, permission changes, and translation changes invalidate correctly? Probe TTL expiry, missed publication, Redis loss/restart, stale negative entries, and multi-replica recovery. A cache must never become authorization or write-side authority. |
| Events and outbox | When atomicity is required, do the domain write and outbox insert share one transaction? Is event ownership and versioning typed and documented? Are retry/backoff/DLQ observable, consumers durably idempotent and replay-safe, and duplicate/out-of-order delivery tested? Confirm the server is not a hidden publisher or consumer owner. |
| Indexes and search | Is write-side storage still authoritative? Do create/update/delete/translation and module lifecycle events update projections? Are tenant, locale, and channel scopes preserved? Probe replay, reindex, deletion, stale rows, out-of-order events, partial failure, and rebuild from source of truth. |
| Multilingual DB contract | Are base rows language-neutral, localized short fields in `*_translations`, heavy localized content in `*_bodies` where applicable, and locale columns safely `VARCHAR(32)`? Are locale normalization and `requested -> tenant default -> first available` selection shared rather than package-local? Check tenant-composite uniqueness/FKs, default-locale integrity, backfill, irreversible narrowing, delete/update behavior, and parity across DB, cache, index, REST, GraphQL, native server functions, and UI. |
| Transactions and concurrency | Are cross-owner calls outside inappropriate open DB transactions? Are revision/idempotency keys and unique constraints sufficient under retry and concurrent requests? Probe rollback, timeout, cancellation, partial failure, and process restart. |
| Tenant and module lifecycle | Are all reads/writes tenant-scoped and RLS/composite integrity preserved? Do Core/Optional semantics, enable/disable, hooks, cache/index cleanup, workers, and event listeners behave consistently without host bypasses? |
| Failure contract and operations | Are timeouts, typed errors, fallback/degraded modes, metrics, traces, correlation IDs, health, and operator recovery actions present and free of secret/PII leakage? Do fallbacks fail closed for authorization and data integrity? |

For any publisher/consumer or caller/provider relationship, inspect and test both ends
in the same pass. Update both local plans when the fix changes both owners. Do not mark
the current item complete while the other end is known to violate the revised contract.

## Current Cycle Queue

The order is deliberate: Core modules first, then the server composition root, then
non-module foundation crates, optional/domain modules in dependency-first order, host
surfaces, and finally platform-wide gates.

Queue marks have durable semantics:

- `[ ]` means not yet visited in this cycle;
- `[ ] ... — in_progress` identifies the cursor item while work is underway;
- `[x]` means visited with a local `completed` handoff;
- `[x] ... — blocked` means visited but not completed and requires closing-gate review.

On normal resume, the cursor item wins. If the cursor is missing or invalid, use the
first unchecked item; never reinterpret a checked `— blocked` row as completed.

### Wave 0 — Fast Preflight

- [x] Reconcile carried `P0`/`P1` blockers and their exact reproduction commands.
- [x] Run `cargo xtask validate-manifest`.
- [x] Run the fast architecture/runtime invariant checks applicable on the current OS.
- [x] Record environment failures separately from product defects.

### Wave 1 — Core Modules

These are Core modules because the current `modules.toml` declares them with
`required = true`; `rustok-core` is intentionally absent from this list.

- [x] `core/modules` — `crates/rustok-modules`
- [ ] `core/auth` — `crates/rustok-auth` — in_progress
- [ ] `core/cache` — `crates/rustok-cache`
- [ ] `core/channel` — `crates/rustok-channel`
- [ ] `core/email` — `crates/rustok-email`
- [ ] `core/index` — `crates/rustok-index`
- [ ] `core/search` — `crates/rustok-search`
- [ ] `core/outbox` — `crates/rustok-outbox`
- [ ] `core/tenant` — `crates/rustok-tenant`
- [ ] `core/rbac` — `crates/rustok-rbac`
- [ ] Core interaction sweep — auth/tenant/RBAC generation and caches; channel/locale
  cache dimensions; transactional events/outbox; index/search replay and rebuild;
  Core module lifecycle and migration ordering.

For each manifest module, run at minimum:

```powershell
cargo xtask module validate <slug>
cargo xtask module test <slug>
```

Add targeted Rust, PostgreSQL, Redis, and runtime tests according to the inspection
matrix and the module's local plan. A compile-free source check is supporting evidence,
not sufficient evidence for a release-critical runtime contract.

### Wave 2 — Server Composition Root

- [ ] `apps/server` owner/runtime wiring, bootstrap, shutdown, middleware order, and
  background-worker lifecycle.
- [ ] Server composition of every Core module without duplicated owner services,
  direct model access, hidden permission checks, or manual event-listener wiring.
- [ ] Server migration aggregation: apply-from-zero and incremental plans, duplicate,
  missing dependency, cycle, cross-module FK, and rollback-safety diagnostics.
- [ ] Equivalent trust, RBAC, locale, error, and transaction behavior across REST,
  GraphQL, native `#[server]`, jobs, event consumers, operational endpoints, and CLI
  adapters where applicable.
- [ ] Failure-injection checks for cache/Redis, event transport, outbox relay, indexer,
  search, DB timeout, worker restart, and graceful shutdown.

Use [Core Integrity Verification](./platform-core-integrity-verification-plan.md) and
[RBAC, Server and Runtime Module Verification](./rbac-server-modules-verification-plan.md)
as mandatory companion plans for this wave.

### Wave 3 — Non-Module Foundation and Shared Runtime

The first entry is a foundation crate, not a Core module.

- [ ] `foundation/rustok-core` — `crates/rustok-core`
- [ ] `foundation/rustok-api` — `crates/rustok-api`
- [ ] `foundation/rustok-runtime` — `crates/rustok-runtime`
- [ ] `foundation/rustok-web` — `crates/rustok-web`
- [ ] `foundation/rustok-events` — `crates/rustok-events`
- [ ] `foundation/rustok-storage` — `crates/rustok-storage`
- [ ] `foundation/rustok-telemetry` — `crates/rustok-telemetry`
- [ ] `foundation/rustok-test-utils` — `crates/rustok-test-utils`
- [ ] Foundation interaction sweep — public contract ownership, dependency direction,
  transaction/event primitives, typed context propagation, telemetry, and test fidelity.

### Wave 4 — Optional and Domain Modules

This order is dependency-first according to the current manifest. A module is checked
with all of its publishers/consumers even if the other owner appeared earlier.

- [ ] `domain/content`
- [ ] `domain/taxonomy`
- [ ] `domain/product`
- [ ] `domain/profiles`
- [ ] `domain/cart`
- [ ] `domain/customer`
- [ ] `domain/region`
- [ ] `domain/pricing`
- [ ] `domain/inventory`
- [ ] `domain/order`
- [ ] `domain/payment`
- [ ] `domain/fulfillment`
- [ ] `domain/commerce`
- [ ] `domain/marketplace_seller`
- [ ] `domain/marketplace_listing`
- [ ] `domain/marketplace`
- [ ] `domain/comments`
- [ ] `domain/blog`
- [ ] `domain/page_builder`
- [ ] `domain/pages`
- [ ] `domain/forum`
- [ ] `domain/media`
- [ ] `domain/seo`
- [ ] `domain/workflow`
- [ ] `domain/alloy`
- [ ] `domain/flex`
- [ ] `extension/ai`
- [ ] Domain interaction sweep — commerce provider chain; content/taxonomy/product;
  comments/blog/forum/pages/page-builder; media/SEO/storage; workflow/events/outbox;
  Flex donor ownership; AI owner ports and review/persistence boundaries.

Use [Events, Domains and Integrations Verification](./platform-domain-events-integrations-verification-plan.md)
as the mandatory companion plan. If a manifest dependency changes, topologically
reorder this wave rather than preserving a stale hand-written order.

### Wave 5 — Applications and Public Surfaces

- [ ] `apps/admin` and module-owned Leptos admin packages.
- [ ] `apps/storefront` and module-owned Leptos storefront packages.
- [ ] `apps/next-admin` and owner/runtime locale-provider parity.
- [ ] `apps/next-frontend` and storefront contract parity.
- [ ] GraphQL, REST, native `#[server]`, OpenAPI/reference artifacts, and headless paths.
- [ ] Shared UI/GraphQL/routing/i18n libraries used by more than one host or module.

Before modifying a frontend, read its root `AI_AGENT_RULES.md`. Use the
[API Surfaces Verification](./platform-api-surfaces-verification-plan.md),
[Frontend Surfaces Verification](./platform-frontend-surfaces-verification-plan.md),
and [Leptos Libraries Verification](./leptos-libraries-verification-plan.md) plans.

### Wave 6 — Closing and Release Gate

- [ ] Revisit every unchecked or locally `blocked` item.
- [ ] Run workspace build/test/format gates appropriate to the release profile.
- [ ] Run PostgreSQL apply-from-zero and incremental migration smoke.
- [ ] Run security/dependency, documentation, observability, and operational-readiness
  gates from the quality plan.
- [ ] Verify reference artifacts and required FFA/FBA evidence against actual runtime
  behavior rather than source-only assertions.
- [ ] Confirm every visited component has a current-cycle local handoff and truthful
  nearest priority.
- [ ] Confirm no unresolved product `P0` or `P1` exists before setting release readiness
  to `candidate`; otherwise set it to `not_ready` and carry the blockers forward.

For `page_builder/pages`, also run:

```powershell
node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs
```

Use [Quality and Operational Readiness Verification](./platform-quality-operations-verification-plan.md)
for the complete closing gate.

## Cycle Completion and Reset

A cycle is **traversed** when every queue item has been visited and is either completed
or has a current, reproducible blocked handoff. Traversed does not mean release-ready.

When the closing gate finishes:

1. Set the cycle status to `closing` and reconcile all local handoffs.
2. Fill the current cycle's compact row in the cycle summary below. Do not copy detailed
   findings here.
3. Carry every unresolved `P0`/`P1`, owner, and reproduction command into
   `Carried release blockers`.
4. Increment the identifier (`cycle-001` -> `cycle-002`).
5. Append one blank summary row for the new cycle and reset every checkbox in the
   current-cycle queue to `[ ]`, removing any `— in_progress` or `— blocked` suffix.
6. Set `Cycle status` to `ready`, `Current item` to `none`, and `Next item` to
   `core/modules`.
7. Clear timestamps and set `Release readiness` to `not_assessed` for the new cycle.
8. Start the next run from Wave 0 and the Core modules again. No earlier completion
   exempts a component from the new cycle.

If the previous cycle left release blockers, Wave 0 attempts them first, but the normal
queue still begins with the Core modules. Old local handoffs remain as evidence and do
not count because their cycle identifier differs.

## Cycle Summary

| Cycle | Started (UTC) | Traversed (UTC) | Fixed P0/P1/P2/P3 | Remaining P0/P1 | Release result | Evidence reference |
| --- | --- | --- | --- | --- | --- | --- |
| `cycle-001` |  |  |  |  |  |  |

## Detailed Plan Set

- [Foundation Verification](./platform-foundation-verification-plan.md)
- [Core Integrity Verification](./platform-core-integrity-verification-plan.md)
- [RBAC, Server and Runtime Module Verification](./rbac-server-modules-verification-plan.md)
- [Events, Domains and Integrations Verification](./platform-domain-events-integrations-verification-plan.md)
- [API Surfaces Verification](./platform-api-surfaces-verification-plan.md)
- [Frontend Surfaces Verification](./platform-frontend-surfaces-verification-plan.md)
- [Leptos Libraries Verification](./leptos-libraries-verification-plan.md)
- [Quality and Operational Readiness Verification](./platform-quality-operations-verification-plan.md)

## Related Documents

- [Verification catalog README](./README.md)
- [Documentation Map](../index.md)
- [Implementation Plans Registry](../modules/implementation-plans-registry.md)
- [Database and Multilingual Storage Contract](../architecture/database.md)
- [Domain Event Flow Contract](../architecture/event-flow-contract.md)
- [Verification scripts README](../../scripts/verify/README.md)
- [Patterns vs Antipatterns](../standards/patterns-vs-antipatterns.md)
- [Forbidden Actions](../standards/forbidden-actions.md)
- [Known Pitfalls](../ai/KNOWN_PITFALLS.md)
