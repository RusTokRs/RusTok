# rustok-modules remediation plan — target state and execution record

Companion to [logic-audit-2026-10-21.md](./logic-audit-2026-10-21.md).
This document fixes the *target architecture* first ("how it should be"),
then records what was changed, in what order, and how it was verified.

## 1. Target-state decisions (the "ideal")

- **D1 — Kind/trust boundary.** `Core` status is a platform-composition
  decision (static registry / composition manifest), never an
  artifact-asserted property. Artifact descriptors that claim `Core` are
  rejected at validation, and the descriptor→definition adapter can only
  ever produce `Optional`, regardless of payload.
- **D2 — Authorization freshness.** The effective-policy cache is a
  bounded read-through cache with TTL. *Every* lifecycle/security event
  that can change enablement invalidates it. A poisoned lock is an error,
  never a silent `Ok`. Absence of an invalidation path for an event class
  is a bug, not an optimization.
- **D3 — Claims and operation outcomes.** Every durable claim carries a
  lease with expiry and reclaim. Operation outcomes are append-only
  facts: once state commits, the operation is `Committed`; post-hook
  failure is recorded as a separate hook-failure fact (with compensation
  visibility), never by rewriting `Committed → Failed`.
- **D4 — Tenant isolation.** `FORCE ROW LEVEL SECURITY` with a non-owner
  runtime role; every tenant transaction sets `rustok.tenant_id`;
  platform-scoped paths set an explicit bypass GUC inside audited code
  paths only. Until the role separation lands in deployment manifests,
  application-level `WHERE` scoping remains mandatory (defense in depth,
  both layers, neither optional).
- **D5 — One implementation per concept.** Admission =
  `installation.rs commit_admission`. Lifecycle writes =
  `lifecycle_writer`. Rollout = the node-reconciliation engine (fenced
  leases, observation revisions). Descriptor loading for deliveries = one
  shared helper. Duplicates are deleted, not deprecated-in-place.
- **D6 — No simulated services in production API.** In-memory fakes live
  under `#[cfg(test)]` or a `test-support` feature. Code that fabricates
  digests, sizes, evidence, metrics, or predecessor state is deleted.
  Dormant subsystems are either wired to a real consumer in the same
  change or removed from the public surface.
- **D7 — Canonical digests.** Every persisted/compared digest is computed
  over canonical JSON (sorted keys, sorted lock nodes).
  `graph_revision` originates from a durable counter and is `> 0` by
  construction-time invariant, so admission and reconciliation agree.
- **D8 — Typed identity.** Governance principals are typed; identity
  matching uses stable IDs (`user_id` / service subject) only — never
  display labels. A request's owner/publisher cannot review their own
  request.
- **D9 — Real rollback safety.** The security epoch is durable and
  actually advanced on capability-grant changes. Watchdog convergence
  requires positive health evidence within a policy-configured
  observation window; timeout alone never releases predecessor holds.
- **D10 — Schema ownership.** This crate's migrations create every table
  its SQL touches; tests run the real migrations. Backend is a validated
  newtype (Postgres | Sqlite); no MySQL-as-Sqlite fallthrough, no backend
  `unreachable!`.

## 2. Execution phases

| Phase | Scope (audit refs) | Status |
|---|---|---|
| A | H-1 Core-kind rejection; H-6 governance identity; M-8 lock invariants; M-1 delivery `payload_digest` | in progress |
| B | H-2 policy cache TTL/bound/poison + host invalidation completeness | pending |
| C | H-3 claim lease/reclaim + executor outcome ordering | pending |
| D | M-4 settings strictness; M-6 materializer honesty; M-9 numeric index canonicalization | pending |
| E | D5/D6 deletions: `dynamic_lifecycle`, `release_admission_journal`, in-memory `gc`/`wave_rollout`/`operator` fakes | pending |
| F | D4 RLS FORCE + GUC coverage (migration; needs PostgreSQL evidence) | pending |
| G | D9 durable epoch + evidence-based watchdog convergence | pending |
| H | M-12 schema ownership + backend newtype; typed errors at pub boundary; bounded lists | pending |

## 3. Verification strategy (no local toolchain)

The development sandbox has no `cargo`/`rustc`. Verification therefore is:

1. Static self-review of every diff (types, imports, call sites updated
   via workspace-wide search).
2. GitHub CI on the remediation PR (`fmt`, `clippy`, `check`, tests) as
   the compiler of record. Baseline: `main` @ `8aa989c` CI run
   `39567567551` is already red (fmt/clippy/check/deny/docs), so the bar
   is *no new failures attributable to this crate* plus green
   crate-scoped jobs where they exist; every remaining red job must be
   shown to pre-exist on `main`.
3. New unit tests for each behavior change, written to fail on the old
   code (Core-kind rejection, label-matching rejection, lock revision,
   cache TTL/invalidation, claim reclaim, settings nested-required).

## 4. Change log

_(filled in as phases land)_
