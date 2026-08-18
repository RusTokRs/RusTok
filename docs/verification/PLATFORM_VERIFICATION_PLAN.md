---
id: doc://docs/verification/PLATFORM_VERIFICATION_PLAN.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# RusToK Main Platform Verification Plan

- **Structure update date:** 2026-08-04
- **Status:** Cycle active
- **Mode:** Cyclic, resumable pre-release defect-removal sweep
- **Goal:** Repeatedly inspect and repair the platform before release, prioritizing critical defects and cross-module contract failures

## Scope and terminology

This document is the durable cursor and reset-friendly checklist for the current
verification cycle. Detailed checks remain in the specialized plans in this folder.
The actual state, findings, fixes, evidence, and next action for a component belong in
that component's local `docs/implementation-plan.md`.

- **Core modules** are modules declared with `required = true` in `modules.toml`.
- **`rustok-core`** is a foundation crate and is checked in the foundation wave.
- **`apps/server`** is the composition root and runtime host, not a module.
- The module manifest is the queue source of truth. Update this plan when the manifest
  changes module identity or dependency order.

## Current cycle cursor

- Cycle: `cycle-001`
- Cycle status: `active`
- Current item: `core/rbac`
- Next item: `core/rbac`
- Started at (UTC): `2026-07-20`
- Last handoff at (UTC): `2026-08-18`
- Release readiness: `not_assessed`
- Current RBAC revision: source baseline PR #2980 merged as
  `f4d89c26f1a30079918660280150016930c837a4`; architecture-guard fix PR #3563
  merged as `eedd1954bf0db9920c7557b691863e316a00befa`; runtime-evidence PR #3570
  merged as `9d7a8d4790c66bbcee3479cb880dc2008e5765b4`; Redis-restart evidence PR #3579
  merged as `6cb7d26734661b17f9b2ca8fead6e46c552bc3eb`; registered-CLI evidence PR #3590
  merged as `67ed475549598720486188f175fcfce0ab826a3b`; artifact-event-digest evidence PR
  #3617 merged as `65a63a33d457ed5ff7c592f2c81b839cbf690d96`; source-verifier refresh PR #3629 is
  open from fresh-main base `967bbbfbebdf3bfcedef35745029b0149aa07321`.
- Current RBAC state: `P0=0, P1=11, P2=1, P3=2`; all product findings remain
  source-fixed, PR #3629 corrects stale verification contracts without changing RBAC
  runtime or migration semantics, and broad execution verification remains incomplete,
  so `core/rbac` remains `in_progress`.
- Current RBAC verification delta: PR #3590 exact-head run `31885429843` at
  `06554657c50535204cdca7f7baf87c7b8d55ba65` completed successfully. All four retained
  source contracts passed; the mutation architecture guard passed; PostgreSQL concurrency
  packet #2849 passed 3/3; durable watchdog packet #2853 passed 1/1; Redis
  available/outage/restart packet #2856 passed 1/1; and registered-CLI repair propagation
  #2862 passed 1/1. Push-to-main RBAC Runtime Evidence run `32004633206` then succeeded
  on merge SHA `67ed475549598720486188f175fcfce0ab826a3b`; artifact `9280508296` retains
  exact-SHA, DEBUG=0, PostgreSQL 16, Redis 7.0.15 where applicable, and Rust 1.97.1
  provenance. PR #3617 subsequently closed the generator-produced event-digest gate:
  push-to-main run `32061362433` succeeded on merge SHA
  `65a63a33d457ed5ff7c592f2c81b839cbf690d96`; artifact `9298285369` records matching
  expected/actual SHA, Rust 1.97.1, generated digest file SHA-256
  `ca261acbf570615cb1ea180854a2d927aeca385db74917cbb487e0dfd9b730b9`, and a
  zero-byte generated diff. The fresh exact-head core packet then passed RBAC compilation,
  focused Rust tests, module validate/test, and the corrected mutation architecture guard.
  PostgreSQL runs terminated during linking with `ld` killed by signal 9 under
  `CARGO_PROFILE_TEST_DEBUG=0`; no runtime assertion executed, so this is runner/linker
  pressure rather than a product defect. Migration Compatibility correctly rejected
  superseded draft #3627 because its proposed dependency edges reordered the published
  base prefix. Forum, Blog, and RBAC migration bodies operate on separate tables and
  expose no cross-module schema/data dependency, so no forward remediation migration is
  warranted for that historical release-order assumption.
- RBAC evidence still required: PR #3629 exact-head source and merge confirmation;
  exact-head format and any remaining affected compile/module gates; Migration
  Compatibility plus PostgreSQL clean apply, N-1 upgrade, integrity and rollback;
  incident/live negative transport evidence; native operator parity;
  composed-host/degraded-path and FFA/FBA promotion evidence.
- Environment classification: no local Rust execution is available in the current agent
  environment. GitHub CI is the executable evidence surface; broad CI failures are not
  product findings until their diagnostics are attributable to the RBAC diff. Linker
  signal-9/OOM-style termination is recorded as environment evidence unless a product
  assertion or deterministic source failure precedes it.

## Carried release blockers

Wave 0 and the closing gate must revisit every item below with its local reproduction
command and owner plan.

- `core/auth`: implicit refresh-token authority for auto-created OAuth applications
  whose persisted grant types omit it.
- `core/cache`: failed Redis invalidations can become untracked when the bounded
  tombstone tracker is saturated.
- `core/channel`: tenant-visible human copy remains in language-neutral rows and the
  staged tenant/concurrency fixes still lack same-revision evidence.
- `core/email`: secret exposure fixes and durable delivery/runtime-setting ownership
  remain incomplete.
- `core/index`: retained PostgreSQL partition, freshness, outage/restart, catch-up and
  replay-repair evidence remains incomplete.
- `core/search`: caller-trusted channel selection and incomplete durable consumer
  receipt/replay ownership remain open.
- `core/outbox`: transport acceptance is not terminal consumer success; durable
  consumer receipt, DLQ and replay ownership remain incomplete.
- `core/tenant`: merged host authority corrections still lack same-SHA source/runtime,
  rotation/revocation, WebSocket, Iggy and multi-replica evidence.
- `core/rbac`: PR #2980 source corrections, PR #3563 architecture-guard correction,
  PR #3570 PostgreSQL/watchdog evidence infrastructure, PR #3579 Redis restart evidence,
  PR #3590 registered-CLI/multi-replica evidence, and PR #3617 generator-produced event
  digest evidence are merged. #2849 PostgreSQL concurrency, #2853 independent-process
  durable-watchdog, #2856 Redis restart and #2862 registered-CLI propagation are retained
  and merge-confirmed, and the event digest is generator-current on the #3617 merge SHA.
  PR #3629 refreshes stale RBAC source guards while preserving the migration plan. The
  rejected #3627 migration reorder is not a product finding: the immutable-prefix gate
  worked and source inspection found no Forum/Blog/RBAC schema/data dependency. Broader
  exact-head source/migration, incident/live negative transport evidence, operator,
  composed-host/degraded-path, and promotion gates remain open.
- Infrastructure issue #2740: the Rust-host PostgreSQL fixture can report a missing
  `rustok_browser` role after a nominally successful setup step.

## Agent start and resume protocol

1. Read `AGENTS.md`, `docs/index.md`, this cursor, the target component README,
   component docs index, and local implementation plan.
2. When the cycle is `active`, resume `Current item`. If the cursor and queue disagree,
   reconcile the local handoff, then use the first unfinished queue item.
3. Before inspection or editing, set the local handoff to `in_progress`.
4. Use targeted checks before broad workspace gates.
5. Record environment OOM, lock, unavailable service, or fixture failures separately
   from product defects.
6. Fix reproducible P0/P1 defects in the current owner scope and add regression evidence.
7. Do not mark an item complete while P0/P1 remains or required evidence is absent.
8. Record deferred P2/P3 work as the nearest local priorities.
9. Update the local handoff and this cursor/checklist in the same work unit.
10. Never skip an item silently. A blocked item needs the exact blocker, reproduction,
    observed result, next action, and closing-gate revisit.

Allowed cycle statuses are `ready`, `active`, and `closing`. Queue items use `pending`,
`in_progress`, `completed`, or `blocked`. Only one item may be `in_progress`.

## Defect policy

- `P0`: exploitable authorization or tenant-isolation failure, data loss/corruption,
  invalid grant, or platform-wide inability to start or serve critical traffic.
- `P1`: serious cross-module inconsistency, broken transaction/outbox/replay path,
  stale authorization/cache/index state, migration failure, or major release-path
  failure without a safe workaround.
- `P2`: bounded functional defect with a safe workaround.
- `P3`: minor correctness, resilience, diagnostics, or maintainability defect.

Compilation alone is not completion. Completion requires applicable cross-module
inspection, targeted tests, truthful documentation, mandatory runtime evidence, and no
unresolved P0/P1 in the item scope.

## Local implementation-plan handoff

Every visited component must maintain this block in its existing implementation plan:

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

Overwrite the current-cycle handoff instead of appending execution logs. Durable open
work belongs in local priorities; Git history preserves old handoffs.

## Mandatory cross-module inspection matrix

| Circuit | Required questions and failure probes |
| --- | --- |
| Ownership and boundaries | Is there one typed owner port/transport? Are callers avoiding owner DB entities, private services, host-local facades, compatibility paths, and dependency cycles? Do manifests, runtime wiring, migrations, and docs agree? |
| RBAC and trust | Are identifiers owner-defined and deny-by-default? Are tenant, actor, principal, and channel facts trusted? Do REST, GraphQL, native functions, jobs, CLI, event consumers, and admin paths enforce equivalent authorization? Are writes tenant-composite and durably invalidated? |
| Cache coherence | Do keys include tenant, locale, channel, principal/generation, policy revision, and module state where required? Probe mutation, lifecycle, Redis loss/restart, missed publication, stale negatives, and multi-replica recovery. Caches are never authority. |
| Events and outbox | Do authoritative writes and required outbox inserts share one transaction? Are events typed/versioned, consumers durably idempotent and replay-safe, and retry/backoff/DLQ observable? |
| Indexes and search | Is write storage authoritative? Are tenant, locale and channel preserved through create/update/delete/translation and lifecycle events? Probe replay, rebuild, stale rows, deletion, partial failure and out-of-order delivery. |
| Multilingual DB contract | Are base rows language-neutral, localized short fields in translation tables, heavy bodies separated where needed, locale columns safe, and locale normalization/fallback shared? Check composite integrity, backfill, cache/index/API/UI parity and delete/update behavior. |
| Transactions and concurrency | Are cross-owner calls outside inappropriate open transactions? Are revisions, idempotency and uniqueness correct under retry and concurrency? Probe rollback, timeout, cancellation, partial failure and restart. |
| Tenant and module lifecycle | Are reads/writes tenant-scoped and lifecycle hooks, cleanup, workers, listeners and required/optional semantics consistent without host bypasses? |
| Failure contract and operations | Are typed errors, timeouts, degraded modes, metrics, traces, correlation, health and recovery actions present without secret or PII leakage? Do safety-critical fallbacks fail closed? |

Inspect both ends of every publisher/consumer and caller/provider relationship. Update
both owner plans when a correction changes both contracts.

## Current cycle queue

Queue semantics:

- `[ ]` not yet visited;
- `[ ] ... — in_progress` active cursor;
- `[x]` visited and completed;
- `[x] ... — blocked` visited but incomplete and mandatory at closing.

### Wave 0 — Fast preflight

- [x] Reconcile carried P0/P1 blockers and exact reproduction commands.
- [x] Run `cargo xtask validate-manifest`.
- [x] Run applicable fast architecture/runtime invariant checks.
- [x] Record environment failures separately from product defects.

### Wave 1 — Core modules

- [x] `core/modules` — `crates/rustok-modules`
- [x] `core/auth` — `crates/rustok-auth` — blocked
- [x] `core/cache` — `crates/rustok-cache` — blocked
- [x] `core/channel` — `crates/rustok-channel` — blocked
- [x] `core/email` — `crates/rustok-email` — blocked
- [x] `core/index` — `crates/rustok-index` — blocked
- [x] `core/search` — `crates/rustok-search` — blocked
- [x] `core/outbox` — `crates/rustok-outbox` — blocked
- [x] `core/tenant` — `crates/rustok-tenant` — blocked
- [ ] `core/rbac` — `crates/rustok-rbac` — in_progress
- [ ] Core interaction sweep — auth/tenant/RBAC generation and caches; channel/locale
  cache dimensions; transactional events/outbox; index/search replay and rebuild;
  lifecycle and migration ordering.

For each manifest module, run at minimum:

```powershell
cargo xtask module validate <slug>
cargo xtask module test <slug>
```

Source-only checks are supporting evidence, not release evidence.

### Wave 2 — Server composition root

- [ ] `apps/server` wiring, bootstrap, shutdown, middleware order, and workers.
- [ ] Compose every Core module without duplicated owner services, direct model access,
  hidden permission checks, or manual event-listener ownership.
- [ ] Verify migration aggregation, dependency diagnostics, apply-from-zero,
  incremental apply, rollback safety, and cross-module foreign keys.
- [ ] Prove equivalent trust, RBAC, locale, error, and transaction behavior across REST,
  GraphQL, native functions, jobs, consumers, operational endpoints, and CLI adapters.
- [ ] Inject cache, Redis, transport, outbox, index, search, DB timeout, worker restart,
  and graceful-shutdown failures.

Use the Core Integrity and RBAC/Server companion plans.

### Wave 3 — Non-module foundation and shared runtime

- [ ] `foundation/rustok-core` — `crates/rustok-core`
- [ ] `foundation/rustok-api` — `crates/rustok-api`
- [ ] `foundation/rustok-runtime` — `crates/rustok-runtime`
- [ ] `foundation/rustok-web` — `crates/rustok-web`
- [ ] `foundation/rustok-events` — `crates/rustok-events`
- [ ] `foundation/rustok-storage` — `crates/rustok-storage`
- [ ] `foundation/rustok-telemetry` — `crates/rustok-telemetry`
- [ ] `foundation/rustok-test-utils` — `crates/rustok-test-utils`
- [ ] Foundation interaction sweep — public ownership, dependency direction,
  transaction/event primitives, typed context, telemetry, and test fidelity.

### Wave 4 — Optional and domain modules

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
- [ ] Domain interaction sweep — commerce chain; content/taxonomy/product;
  comments/blog/forum/pages/page-builder; media/SEO/storage; workflow/events/outbox;
  Flex donor ownership; AI owner ports and review/persistence boundaries.

Topologically reorder this wave when manifest dependencies change.

### Wave 5 — Applications and public surfaces

- [ ] `apps/admin` and module-owned Leptos admin packages.
- [ ] `apps/storefront` and module-owned Leptos storefront packages.
- [ ] `apps/next-admin` and owner/runtime locale-provider parity.
- [ ] `apps/next-frontend` and storefront contract parity.
- [ ] GraphQL, REST, native functions, OpenAPI/reference artifacts, and headless paths.
- [ ] Shared UI, GraphQL, routing, and i18n libraries used by multiple hosts/modules.

Read the application `AI_AGENT_RULES.md` before frontend changes. Use the API,
frontend, and Leptos-library companion plans.

### Wave 6 — Closing and release gate

- [ ] Revisit every unchecked or blocked item.
- [ ] Run release-profile workspace build, test, and format gates.
- [ ] Run PostgreSQL apply-from-zero and incremental migration smoke.
- [ ] Run security/dependency, documentation, observability, and operational gates.
- [ ] Verify reference artifacts and required FFA/FBA evidence against runtime behavior.
- [ ] Confirm every visited component has a current-cycle handoff and truthful nearest
  priority.
- [ ] Set release readiness to `candidate` only when no unresolved product P0/P1 exists;
  otherwise set `not_ready` and carry blockers forward.

For `page_builder/pages`, also run:

```powershell
node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs
```

## Cycle completion and reset

A cycle is traversed when every item has been visited and is completed or has a current,
reproducible blocked handoff. Traversed does not mean release-ready.

1. Set cycle status to `closing` and reconcile local handoffs.
2. Fill the current cycle summary row.
3. Carry every unresolved P0/P1, owner, and reproduction command forward.
4. Increment the cycle identifier.
5. Add a blank summary row and reset every queue checkbox and suffix.
6. Set status to `ready`, current item to `none`, and next item to `core/modules`.
7. Clear timestamps and set release readiness to `not_assessed`.
8. Start the next cycle at Wave 0 and the first Core module.

Old handoffs remain evidence but never count for a new cycle.

## Cycle summary

| Cycle | Started (UTC) | Traversed (UTC) | Fixed P0/P1/P2/P3 | Remaining P0/P1 | Release result | Evidence reference |
| --- | --- | --- | --- | --- | --- | --- |
| `cycle-001` | `2026-07-20` |  |  |  |  |  |
