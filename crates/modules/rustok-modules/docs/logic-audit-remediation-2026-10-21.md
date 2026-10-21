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
- **D5 — One implementation per concept.** Admission commit =
  `installation.rs commit_admission`; the pre-CAS *staging intent* for OCI
  ingress is owned by `release_admission_journal` (kept and fixed in
  Phase E — it is a distinct stage, not a duplicate of the commit owner).
  Lifecycle writes = `lifecycle_writer`. Rollout = the
  node-reconciliation engine (fenced leases, observation revisions).
  Descriptor loading for deliveries = one shared helper. Duplicates are
  deleted, not deprecated-in-place.
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
  display labels. Self-review by the namespace owner is retained
  deliberately: the product model is a single-operator namespace where
  requester == publisher == owner, and separation of duties is provided by
  the `can_manage_modules` capability plus the validation stages, not by a
  second human. (Revisit only if multi-operator namespaces appear.)
- **D9 — Real rollback safety.** The security epoch is durable (singleton
  `module_security_epoch` row, migration `m20261021_000055`) and advanced
  atomically with every quarantine/revocation. Convergence is never
  automatic: the checkpoint model records no health evidence while
  observing, so the only convergence path is the explicit,
  actor-authorized `finalize` attestation; an elapsed observation window
  fails the checkpoint closed and retains predecessor holds. If a future
  health-evidence source is recorded on the checkpoint, evidence-gated
  automatic convergence can be reintroduced on top of that data.
- **D10 — Schema ownership.** This crate's migrations create every table
  its SQL touches; tests run the real migrations. Backend is a validated
  newtype (Postgres | Sqlite); no MySQL-as-Sqlite fallthrough, no backend
  `unreachable!`.

## 2. Execution phases

| Phase | Scope (audit refs) | Status |
|---|---|---|
| A | H-1 Core-kind rejection; H-6 governance identity; M-8 lock invariants; M-1 delivery `payload_digest` | implemented (commit `b876c15`) |
| B | H-2 policy cache TTL/bound/poison + host invalidation completeness | implemented (`087acf3`) |
| C | H-3 claim lease/reclaim + executor outcome ordering | implemented (`052053a`) |
| D | M-4 settings strictness; M-6 materializer honesty; M-9 numeric index canonicalization | implemented (`9249557`) |
| E | D5/D6 initially deleted `dynamic_lifecycle`/`gc`/`wave_rollout`/`operator`; **restored and reclassified** after provenance review (see §5); journal fixed in place (see §4) | partially reverted — modules restored, disposition per §5 |
| F | D4 RLS FORCE + GUC coverage (migration; needs PostgreSQL evidence) | pending |
| G | D9 durable epoch + attestation-only watchdog convergence | implemented |
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

- **E (deletions, later reverted):** `dynamic_lifecycle.rs`, `operator.rs`,
  `wave_rollout.rs`, `gc.rs` and their tests/facade methods were deleted
  after a workspace-wide usage scan showed zero external callers. On
  review this was **reverted**: the scan proved only "unwired today", and
  a provenance check found all four are deliverables of the active
  `docs/modules/module-release-rollback-plan.md`, with their schema
  created by migration `m20260904_000051`. Deleting them also orphaned
  the sole writers of `module_artifact_work_generations` (read by the
  canonical delivery descriptor loader) and
  `module_production_operations`. All four modules, their five test
  files, the `lib.rs` exports, and the `ModuleControlPlane` facade
  accessors are restored; §5 records the per-module classification
  (parallel implementation vs unfinished planned functionality) that
  determines each module's disposition. They remain unwired in
  production until that disposition is executed.
- **E (journal):** `release_admission_journal` was *kept and fixed* rather
  than deleted: inspection showed it is the pre-staging intent journal the
  OCI admission layers record before CAS mutation, complementing the
  installer's in-transaction bind. Fixes: insert-conflict re-read (no
  fabricated `Staged`), full-identity scoping on bind, bounded stale scan.
- **E (payload cache):** `VerifiedPayloadCache` is now bounded by entry
  count and aggregate bytes and degrades to misses on a poisoned lock
  instead of panicking.
- **G (durable epoch):** new `SeaOrmSecurityEpochStore` over the singleton
  `module_security_epoch` table (migration `m20261021_000055`, seeded at
  epoch 1). `SecurityEpochRegistry` is now only an in-memory validation
  snapshot (`from_current`). Quarantine/revoke advance the epoch inside
  the same transaction as the security write; installer checkpoints record
  the real durable epoch instead of constant 1; `finalize` and the
  watchdog validate against the database value, so preemption works
  across processes and restarts.
- **G (watchdog):** `evaluate_watchdog`/`evaluate_transition_watchdog` no
  longer take a caller-built registry and no longer auto-converge on
  timeout: stale-epoch and expired-window checkpoints fail closed with
  holds retained. The server watchdog worker passes only the connection.
  Watchdog tests rewritten: expired window asserts `FailedClosed` + both
  holds retained; preemption test advances the durable epoch.
- **CI fix (migration `m20261021_000054`):** the claim-lease column
  migration now guards table/column existence (`IF EXISTS` /
  `ADD COLUMN IF NOT EXISTS` on PostgreSQL; `sqlite_master` + `PRAGMA
  table_info` on SQLite). The table is owned by `rustok-migrations`
  (`m20250101_000003`) and precedes this migration in the sorted
  production stack; the guard only affects isolated crate-only test
  compositions (focused CI run `39632605124`:
  `artifact_settings_recovery` tests failed with "no such table").
- **CI evidence (focused workflow, `f59142d`):** `cargo test -p
  rustok-modules --lib settings_` compiled the full crate and ran 20
  settings tests: 18 passed, 2 failed only on the migration-table issue
  above. The `cargo fmt -p rustok-modules -- --check` step flagged 9
  spots; all reformatted in the Phase G commit. Workspace-level
  `Cargo Check` failure on the same PR is in `rustok-forum`
  (E0061/E0252/E0382/E0425) — pre-existing on the red `main` baseline.

## 5. Appendix: classification of the four restored modules (2026-10-21)

Provenance. All four modules arrived in the single repository-import
commit `d69e768` (12 453 files), so per-file git history carries no
intent signal. The authoring plan is `docs/modules/module-release-rollback-plan.md`
(status `active`), which names all four as implemented deliverables with
checked work items and "verified by tests" claims, and migration
`m20260904_000051` creates their schema (`module_admitted_oci_releases`,
`module_external_prebuilt_ingress`, `module_executor_readiness_receipts`,
`module_artifact_work_generations`, `module_production_operations`).
The restored integration tests run the real crate migrations on SQLite.
None of the four has a production caller: in the whole workspace they are
referenced only by `lib.rs` re-exports, two `ModuleControlPlane` facade
accessors, and their own tests. "Unwired" is therefore fact; the open
question per module is whether the code is (a) a parallel implementation
of a canonical owner, (b) unfinished planned functionality, or (c) both.

### 5.1 `dynamic_lifecycle.rs` — mixed, predominantly (a) parallel

- Parallel re-implementation: `execute_install` / `execute_enable` /
  `execute_disable` duplicate canonical owners (`installation.rs`
  admission commit, executor + `lifecycle_writer`) with weaker evidence
  and safety: fabricated admission fields (registry `ghcr.io`,
  repository `rustok/{slug}`, empty dependency lock `graph_revision: 1`,
  `size_bytes = 1024`, `verification_evidence = "{}"`), no idempotency
  check on `operation_id`, TOCTOU work-generation check (read outside the
  transaction; the UPDATE does not predicate the generation, and enable /
  disable never check `rows_affected`), caller-asserted
  `smoke_test_passed` instead of readiness evidence, no context
  validation, authorization, or tenant GUC configuration, and `unwrap()`
  on stored UUIDs. If wired as-is it would admit installations the
  canonical path rejects.
- Sole-owner planned pieces with canonical readers: work-generation
  create/advance/retire (`module_artifact_work_generations` has no other
  writer; the canonical delivery descriptor loader and `operator.rs`
  read retired generations to fence work), publisher-continuity rule on
  reinstall (currently fail-open: `continuity_token` ignored, NULL
  publisher passes), and the purge-requires-retired invariant.
- Simulated parts: `execute_data_purge` deletes from the nonexistent
  table `module_artifact_data_records` and swallows the error with
  `.ok()` (deletes nothing, reports success); `execute_settings_purge`
  deletes one table with `.ok()` and no receipt; `get_work_generation`
  selects `retired_at` but always returns `None`. Declared actions
  `admit`/`update`/`remove`/`rollback` have no implementing methods.
  The `module_production_operations` journal is written only here and
  read nowhere.
- Disposition: do not wire. Fold work-generation lifecycle, continuity,
  and the retire precondition into the canonical installation/uninstall/
  purge owners (canonical uninstall currently never retires the
  generation, leaving the delivery-side fence permanently inert), then
  delete this parallel service.

### 5.2 `gc.rs` — (b) unfinished planned functionality

Pure-logic tombstone → grace → final-recheck → execution-token →
collect coordinator over the canonical `RetentionTarget` /
`RetentionHoldLedger` (`retention.rs` has no collection machinery;
`data::gc` covers artifact data objects only). Missing for production:
any persistence (all state is in-memory `HashMap`s despite the "durable
coordinator" doc claim), real owner adapters (the 11 adapters are
hand-populated in-memory sets; `execute_physical_purge` removes from a
`HashSet`), and a scheduler/caller. Keep; wire only behind a durable
tombstone store and adapters backed by the real owner stores.

### 5.3 `operator.rs` — (b) unfinished planned read model, fabricated fields

Real SQL reads, invented projection values: blast radius hardcodes
`affected_roles = ["api", "monolith"]`, `affected_tenants_count = 1` for
both scopes, `dependent_modules = []` (the cross-module automatic-mode
denial can never trigger from data), `has_schema_migration` = "candidate
is Static", `is_irreversible = false` always; `predecessor_identity` is a
clone of current; `publisher_identity` hardcoded `"platform"`;
unparseable or missing data-owner/settings IDs silently replaced with
random `Uuid::new_v4()`; `active_retention_holds_count = 0` hardcoded;
support bundle always emits `installation_id: None, operation_id: None`
with no authorization despite the "authorized" contract; `get_status`
INNER JOINs `module_artifact_work_generations`, so installations created
by the canonical path are invisible ("No installation found"). No
canonical equivalent of this operator surface exists (transition
receipts/preflight overlap only partially). Keep restored but unwired;
a rewrite against canonical sources (predecessor chain, dependency
graph, retention ledger, production-operation journal) is required
before any transport exposes it.

### 5.4 `wave_rollout.rs` — (b) unfinished planned policy state machine

Pure in-memory cohort policy (dual pre-staging barrier, sequential wave
mutation, verify-before-widen, single-attempt rollback). No persistence,
no node I/O, `verify_wave` trusts the caller's health assertion. No
canonical wave/cohort counterpart exists (`distribution_rollout` and
`artifact_node_reconciliation` have no wave concepts; per-node mechanics
live in the artifact node agent). Keep; future wiring must drive it
from durable reconciliation state, not from a disposable struct.

### 5.5 Consequence for audit finding H-5 / decision D6

H-5's factual core stands (dynamic_lifecycle fabricates admission
evidence and must never be wired as-is), but its "delete the simulated
services" framing was overbroad for `gc`, `operator`, and `wave_rollout`:
they are unfinished deliverables of an active plan, and the plan's
checked-off "implemented/verified" status for all four is itself
inaccurate. D6 is refined accordingly: simulated behavior must not be
*presented as production* — these modules stay compiled and tested but
explicitly unwired and recorded here as unfinished until their §5
dispositions are executed.
