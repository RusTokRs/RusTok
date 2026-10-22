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
  Namespace bootstrap follows the same model: creating the first publish
  request for an unowned slug requires `can_manage_modules` (the first
  published release durably binds its publisher as owner, so a first-comer
  claim by any authenticated user would be namespace squatting — H-6
  remainder, closed in `52f26ba`).
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
| E | D5/D6 initially deleted `dynamic_lifecycle`/`gc`/`wave_rollout`/`operator`; restored and reclassified after provenance review (see §5); §5.1 executed (generations folded into canonical owners, parallel service deleted); journal fixed in place (see §4) | partially reverted — `gc`/`wave_rollout`/`operator` remain restored and unwired per §5.2–§5.4 |
| F | D4 RLS FORCE + GUC coverage (migration; needs PostgreSQL evidence) | blocked — readiness inventory, prerequisites, and interim contract recorded in §6; no blind FORCE migration |
| G | D9 durable epoch + attestation-only watchdog convergence | implemented |
| H | M-12 schema ownership move + `SupportedBackend` newtype; typed errors at pub boundary; tri-state `tenant_modules.enabled` (closes the M-5 residual) | implemented — see the Phase H slice entries at the end of §4; each slice green on the focused CI gate (`Atomic module settings CAS`) |

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
  created by migration `m20261022_000057_admitted_oci_releases`
  (renamed from `m20260904_000051` in the M-12 ordinal cleanup).
  Deleting them also orphaned
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
  table_info` on SQLite). The table was then owned by
  `rustok-migrations` and preceded this migration in the sorted
  production stack; since Phase H-3 it is owned by this crate
  (`m20250101_000060_create_tenant_modules`, still preceding by name
  order). The guard only affects isolated crate-only test
  compositions (focused CI run `39632605124`:
  `artifact_settings_recovery` tests failed with "no such table").
- **CI evidence (focused workflow, `f59142d`):** `cargo test -p
  rustok-modules --lib settings_` compiled the full crate and ran 20
  settings tests: 18 passed, 2 failed only on the migration-table issue
  above. The `cargo fmt -p rustok-modules -- --check` step flagged 9
  spots; all reformatted in the Phase G commit. Workspace-level
  `Cargo Check` failure on the same PR is in `rustok-forum`
  (E0061/E0252/E0382/E0425) — pre-existing on the red `main` baseline.
- **M-10 (source-object store):** `source_object.rs` hardened end to end.
  All async-path file I/O is `tokio::fs` streamed in 64 KiB chunks (no
  blocking `std::fs`, no whole-file loads); a defensive
  `MAX_SOURCE_OBJECT_BYTES` (64 MiB) ceiling rejects oversize publishes
  and reads with typed `TooLarge`. CAS layout is two-level fan-out
  (`<2>/<2>/<hex>`); per §13 the flat layout is not migrated — old CAS
  directories are discarded and republished. Existing blobs are
  content-verified by streamed SHA-256 on publish (corruption is a typed
  `DigestMismatch`, never silently trusted or overwritten), and
  `get_source_path` is now async and really verifies before returning a
  path. Receipt reads take a mandatory `&ModuleInstallationScope` — the
  `None` escape that skipped the tenant boundary is gone. Blob→receipt
  ordering is kept (a receipt must never point at a missing blob); a
  failed receipt insert removes a freshly created, still-unreferenced
  blob with combined-error reporting instead of `let _ =` suppression,
  and crash-orphaned blobs remain owned by retention GC (§5.2,
  accepted-unfinished). New tests cover streamed file publish, verified
  path/blob reads, corrupt-blob fail-closed on republish and read,
  oversize rejection on both publish paths, and NotFound reads.
- **M-5a (policy engine, `685866a` + fmt `5ae1de9`):** all eight M-5 policy
  sub-findings closed in `policy.rs`. Duplicate tenant overrides and
  duplicate runtime inputs are typed errors (`DuplicateTenantOverride` /
  `DuplicateRuntimeInput`) instead of silent `BTreeMap` last-wins.
  `executor_available` is excluded from the revision hash via a shadow
  projection (contract bumped to `rustok.module_effective_policy.v3`;
  the flag still gates availability, so worker flapping no longer churns
  the durable revision). Declared dependency `version_requirement`s are
  now enforced both at toggle admission
  (`ModuleToggleValidationError::IncompatibleDependencies`) and in the
  availability cascade (new `DependencyVersionMismatch` denial, distinct
  from `DependencyUnavailable`; rustok-api view, stable code, GraphQL
  projection, and the server toggle-error mapping updated in the same
  commit). Toggle admission is still deliberately *not* widened to
  co-requisites/channel/maintenance/runtime: that is the documented
  layering (`ordering_enabled_modules` contract) — toggle records tenant
  intent against catalog topology; the effective policy decides
  availability. The `TenantOverride` decision fact is suppressed for Core
  definitions (it falsely claimed tenant intent was honored where Core
  ignores overrides). The revision gate's `Stale` outcome is replaced by
  gap-advancing `ResyncRequired` (no dual outcomes, §13): a missed
  transition no longer wedges consumers — the revision consumer persists
  the cursor on resync and the transition coordinator publishes on
  resync; only `Duplicate` remains a rejection. New unit tests cover
  duplicates, executor-flap revision stability, cascade/toggle version
  mismatch, the Core fact, and gate resync semantics.
- **M-5b (lifecycle writer + settings store, `c2675dd` + fmt `8c297e8`):**
  `persist_settings` no longer freezes derived effective availability (or
  a forced Core flag) into `tenant_modules.enabled`: existing rows keep
  their recorded intent untouched, and a first row created by a settings
  save records the caller-supplied platform default
  (`TenantModuleSettingsRequest.default_enabled` replaces
  `is_core`/`is_effectively_enabled`). The boolean column still cannot
  express "settings without intent"; the tri-state split remains tracked
  under M-12. `runtime_policy_inputs` is fallible: security-resolution
  errors propagate as typed `ModuleLifecycleDbWriterError::Security`
  (absence is an explicit `Clear` snapshot, so every error is genuine),
  while the installation/sandbox ports keep failing closed with the
  rationale recorded in code (opaque `String` port errors cannot
  distinguish absence from storage failure; typed ports are Phase H).
  Runtime evidence is resolved once per command context and shared by all
  policy projections (previously re-queried N×3 per projection).
  `overrides()` is bounded by the same 1000-row contract as
  `tenant_override_snapshots` (`LIMIT 1001` + typed rejection), and the
  definition catalog is shared via `Arc` with a `OnceLock` cache for the
  registry-built catalog instead of a deep clone/rebuild per call.
- **H-6 remainder (namespace squatting, `52f26ba` + fmt `df3be21`):** the
  first-comer arm of `governance_actor_can_create_publish_request` is
  removed. The first published release durably binds its publisher as the
  slug owner (`registry_module_owners` insert in `publication.rs`), so
  admitting any authenticated user to create a publish request for an
  unowned slug was a namespace capture path. Namespace bootstrap now
  requires `can_manage_modules`, consistent with the D8 single-operator
  model; the bound owner publishes without the capability. A future
  self-service marketplace must introduce an explicit namespace-claim
  flow rather than widening this gate. Covered by updated unit
  assertions plus a store-backed rejection test (empty owner table,
  non-manager actor, rejected before any write).

Phase H was executed in full ideal scope per the 2026-10-22 directive
(no deferral options). Slice labels below (H-1…H-5) are Phase H work
slices, **not** audit finding IDs — audit H-4 is Phase F (§6), audit
H-5 is dispositioned in §5.5. Slices, in execution order:

- **H-2 (migration ordinal hygiene, `4beaa97`):** duplicate migration
  ordinals renamed; registration is lexically ordered with unique
  ordinals, enforced by
  `migration_registration_is_lexically_ordered_with_unique_ordinals`.
- **H-1 (`SupportedBackend` newtype, through `f930d2f`):**
  `src/backend.rs` validates the connection backend once at every store
  boundary (`Postgres | Sqlite`, exhaustive matching). All
  `DbBackend::Postgres => …, _ => …` MySQL-as-SQLite fallthroughs and
  backend `unreachable!` panics are deleted across migrations, data,
  governance, operation/settings/recovery stores, `installation.rs`,
  and the long tail; the public surface no longer exposes raw
  `DbBackend`. Conversion scripts retained under `scripts/`.
- **H-4 (typed errors at the pub boundary):** every `Result<_, String>`
  host/runtime port now carries a dedicated typed error owned by the
  trait-defining module, with consumers mapping via `#[from]`/`#[source]`
  and denials remaining Ok-side decisions. H-4a (`8f18818`/`570c505`):
  runtime resolvers + `ArtifactBindingDispatchError::is_terminal`
  (terminal rejections dead-letter as `execution_rejected` instead of
  retrying forever). H-4b (`fd0d9cc`…`00545dd`):
  `TrustVerificationError` across trust/ingress/admission and the
  verification workers; alloy registry failures no longer masquerade as
  verification failures. H-4c (`d369cfb`/`32c49cc`): composition
  enqueue, delivery tenant source, backfill processor,
  artifact-node/static-distribution topology resolvers, release
  verifier, publication registry provider. H-4d (`70ad7e5`…`d2f8ebd`):
  `OciTransportError` for the strict OCI distribution transport
  (`Rejected` preserves the registry HTTP status; `DigestMismatch` is
  an integrity failure, never transient).
- **H-3 (schema ownership move, `62af640` + `9f95ff4`):** the 15
  module-domain migrations (`tenant_modules`, all `registry_*` publish /
  validation / governance / publication-evidence tables) moved from
  `crates/utils/rustok-migrations` into this crate with fresh ordinals
  `000060..=000074` (dates preserved, so the globally name-sorted server
  migrator order is unchanged; dev databases reset per §13). Moved
  migrations were converted to the `SupportedBackend` contract; the
  platform `tenants` FK target is declared as a local minimal `Iden`
  (rustok-auth precedent). `InstallerOwnerMigrator` now sources the
  publish-tables migration from `ModulesModule` by name.
  `install_http_jobs` creation deliberately stays in
  `rustok-migrations`: its runtime owner is
  `rustok-installer-persistence`, not this domain. Two orphaned,
  unregistered migration drafts found during the move were deleted as
  dead: `m20260407_000002_expand_registry_request_and_stage_leases`
  (superseded column-for-column by the registered `20260408` pair) and
  `m20260928_000002_add_install_http_job_idempotency` (columns no
  entity reads). Re-verification (2026-10-22) found two census gaps
  the file-level sweep had missed: `module_operation_override_states`
  moved in as `m20260808_000075`, and the `module_operations` table
  creation was extracted from the mixed-domain
  `m20260501_000001_create_platform_composition_state` into
  `m20260501_000076_create_module_operations`. M-12's split-ownership
  finding is closed at that point: every module-domain table this
  crate's SQL touches is created by this crate's migrations. The
  remaining foreign-created tables it accesses are platform-owned
  infrastructure used as a client — `tenants` (FK target),
  `platform_state` (written only through the composition owner
  service), `owner_operation_receipts`, and `sys_events`.
- **H-5 (tri-state `tenant_modules.enabled`, `5bc7215`):** the column
  is nullable (amended in `m20250101_000060` in place per §13): NULL =
  "no tenant intent recorded", the platform default applies. A settings
  save creates no intent (INSERT writes NULL; the UPDATE no longer
  touches the column; `TenantModuleSettingsRequest.default_enabled`
  deleted) — the M-5 freeze is gone end-to-end. Policy override reads
  filter `enabled IS NOT NULL`, so settings-only rows are not overrides
  and later platform-default changes take effect for tenants that never
  expressed intent. Clearing an override (recovery
  `apply_tenant_override_enabled(None)`) now sets `enabled = NULL` and
  preserves the row's settings instead of deleting the row. Owner facts
  are tri-state (`Option<bool>`: state/settings records,
  `TenantModuleOverrideSnapshot`, `StaticModuleSettingsSnapshot`, the
  `tenant_module` entity and `TenantModuleResponse`);
  effective-state projections stay `bool` and resolve the platform
  default at the owner boundary (`StaticTenantModuleView`,
  `ModuleLifecycleSettingsResult`), failing loudly for unknown slugs.
  `rustok-api` settings reads accept not-explicitly-disabled rows while
  `is_tenant_module_enabled` keeps counting only explicit enables.
  Consequence to be aware of: raw snapshot readers (e.g. SEO's
  `is_enabled`/`owner_module_enabled`) no longer flip to `true` after a
  mere settings save on a default-enabled module — that flip was the
  freeze defect; consumers needing effective state must use the
  lifecycle/effective-policy projections. `artifact_settings::persist`
  also stopped fabricating `enabled: true` for a table it does not own.

## 5. Appendix: classification of the four restored modules (2026-10-21)

Provenance. All four modules arrived in the single repository-import
commit `d69e768` (12 453 files), so per-file git history carries no
intent signal. The authoring plan is `docs/modules/module-release-rollback-plan.md`
(status `active`), which names all four as implemented deliverables with
checked work items and "verified by tests" claims, and migration
`m20261022_000057_admitted_oci_releases` creates their schema (`module_admitted_oci_releases`,
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
- Disposition (EXECUTED): deleted after folding its sole-owner pieces
  into the canonical owners. `installation.rs::commit_admission` now
  creates the work-generation row (generation 1) in the admission
  transaction, and `uninstall_artifact` advances and retires the
  generation (upsert, so pre-fold installations retire from implicit
  generation 1 to 2) and clears tenant intent in the uninstall
  transaction. Investigation showed the purge retire precondition
  needed no fold: the canonical data purge
  (`ensure_artifact_data_purge_target_is_retired`: inactive +
  uninstalled, plus an active-scope collision check) and the canonical
  settings purge (`load_retired_source_in`, authorizer, tombstone
  revision) already enforce it more strongly than this service did.
  Publisher continuity was NOT folded because it guards a decision point
  that does not exist canonically: `ArtifactAdmissionCommand` cannot
  attach a retained data owner (admission always allocates fresh
  owner/settings identities; update continuity is handled by the
  activation cutover). Attach-retained-on-reinstall remains
  unimplemented plan functionality, and the continuity rule must be
  enforced at whatever reattach entry point implements it. The
  read-by-nobody journal table `module_production_operations` is
  dropped by migration `m20261021_000056`. The wave/operator scenario
  tests that used this service as their arrange step were rewritten to
  seed state directly and to retire installations through the canonical
  uninstall; assertions on this service's simulated purges were removed
  (canonical purge suites cover the precondition). Note for context:
  the generation number itself is still consumed by no queue — delivery
  reads only the retired flag (already double-covered by the
  uninstall-operations existence check) — so the fold makes the schema
  concept truthful and gives the operator projection real values;
  generation pinning of queued work remains future plan work.

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

*Executed 2026-10-21* (disposition 2): the module header now states the
unwired, in-memory status explicitly, so the surface cannot be presented
as production; no behavior changed.

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
graph, retention ledger, transition checkpoints) is required
before any transport exposes it.

*Executed 2026-10-21* (disposition 3): rewritten on canonical sources.
Publisher resolves from `module_external_prebuilt_ingress` (`None` for
internally built releases; the hardcoded `"platform"` is gone); the
predecessor is the previous installation in the scope's installation
chain, not a clone of current; blast-radius tenants, nodes, and
dependents derive from installations, node-reconciliation assignments,
and admitted-descriptor dependency declarations (`affected_roles` was
really topology nodes and is renamed `affected_nodes`);
schema-migration derives from admitted-descriptor data-schema
fingerprints (static candidates remain flagged by documented policy —
native migrations are not introspectable here — and an unadmitted
candidate against a live data schema fails closed in automatic mode);
`is_irreversible` is removed because irreversibility is an operation
property owned by `data_upgrade`, not a descriptor fact; malformed
stored UUIDs are typed errors instead of random substitutes, and
owner/settings are `Option`; `get_status` reads work generations with
absent-row semantics (generation 1, not retired) and counts real durable
holds via `RetentionHoldStore`; the support bundle carries the real
installation id, the latest transition-checkpoint operation id, and the
actual hold records, and no longer claims to perform authorization.
Still unwired into any transport by design.

### 5.4 `wave_rollout.rs` — (b) unfinished planned policy state machine

Pure in-memory cohort policy (dual pre-staging barrier, sequential wave
mutation, verify-before-widen, single-attempt rollback). No persistence,
no node I/O, `verify_wave` trusts the caller's health assertion. No
canonical wave/cohort counterpart exists (`distribution_rollout` and
`artifact_node_reconciliation` have no wave concepts; per-node mechanics
live in the artifact node agent). Keep; future wiring must drive it
from durable reconciliation state, not from a disposable struct.

*Executed 2026-10-21* (disposition 2): the same explicit unwired-status
header was added; no behavior changed.

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

### 5.6 Plan-document status corrections (disposition 4)

*Executed 2026-10-21.* `docs/modules/module-release-rollback-plan.md`
was corrected in place: `DynamicLifecycleService` implementation and
"verified by `dynamic_lifecycle_tests.rs`" claims now name the canonical
owners (`installation.rs` admission/uninstall, `release_admission_journal`)
with dated removal notes, and the items whose evidence died with the
simulation explicitly say coverage must be re-established (the
atomic-generation/delayed-work item is unchecked). `wave_rollout` and
`gc` entries now state the unwired in-memory status next to their test
citations. The two operator-experience items that claimed transport
exposure and full lifecycle-command coverage are unchecked with
corrections (read-model only, no transport, no built-in authorization),
and the automatic-mode enforcement note now matches the rewritten
preview (`generate_preview` for schema/dependents, `data_upgrade` for
irreversibility). Remaining plan mentions of the deleted service exist
only inside these dated corrections, as historical evidence.

## 6. Phase F (H-4 / D4) — readiness inventory and blocked record (2026-10-22)

**Status: BLOCKED. Do not land a `FORCE ROW LEVEL SECURITY` migration
yet.** A blind FORCE migration would either be inert (the application
connects as the database owner/superuser, which bypasses RLS) or, once a
non-owner role is used, silently break the control plane (tenant rows
become invisible wherever the tenant GUC is unset — an empty-result
failure mode, not an error). This section records the verified inventory
and the ordered prerequisites that make Phase F landable.

### 6.1 Inventory (verified against the tree, 2026-10-22)

- This crate's migrations create ~110 tables; ~67 carry
  `ENABLE ROW LEVEL SECURITY` with at least one policy; **0 carry
  `FORCE`**.
- 43 tables have no RLS at all. Platform-global by design (no tenant
  dimension; acceptable as-is): `module_security_epoch` (singleton),
  `module_operations_tool_*` (fleet tooling), and
  `module_artifact_node_reconciliation*` (node/topology state keyed by
  installation, inherits installation scope). The remainder need an
  explicit per-table scoping decision before FORCE — including the
  tenant-bearing `module_transition_checkpoints` (`tenant_id` column),
  the digest-keyed `module_retention_holds` and
  `module_source_object_retention_holds`, the `registry_publish_*` /
  `registry_module_releases*` publisher-lineage set,
  `module_artifact_security_states` / `module_artifact_security_operations`,
  `module_artifact_binding_operations`,
  `module_artifact_data_object_{copy,migration}_operations`,
  `module_artifact_ui_contribution_locks`, and the
  `module_static_distribution_*` / `module_static_promotion*` families
  (composition state whose tenant visibility must be decided per table).
- GUC coverage is two functions — `data::helpers::configure_tenant_scope`
  and `installation::configure_rls_scope` — three `set_config` call sites
  against ~123 `current_setting('rustok.tenant_id')` /
  `rustok.module_control_plane_owner` references in policies. Almost every
  query path in the crate runs without GUCs and relies on hand-written
  `WHERE` scoping.
- No non-owner application database role exists anywhere in provisioning:
  dev connects as `rustok` (`.env.dev.example`), compose as `postgres`
  (superuser). Bounded `NOSUPERUSER` roles exist only inside CI evidence
  harnesses, not for the application.

### 6.2 Prerequisites, in order

1. **Role separation.** Provision a dedicated application role (`LOGIN`,
   `NOSUPERUSER`, no `BYPASSRLS`, not a table owner) in compose/init SQL
   and deployment documentation; the server connects as that role,
   migrations keep running as the owner.
2. **GUC coverage.** Set transaction-local GUCs at every transaction /
   connection boundary that touches tenant- or platform-scoped tables
   (generalize the `configure_rls_scope` pattern to all stores; platform
   work sets `rustok.module_control_plane_owner`, tenant work sets
   `rustok.tenant_id`). No PostgreSQL path may run unscoped.
3. **Policy completion.** For each of the 43 uncovered tables, either add
   policies matching the decided scope or record the platform-global
   classification in the migration. Review existing
   `scope_kind = 'platform' OR tenant = …` policies: platform rows are
   visible to any tenant session, which is acceptable only while
   app-level scoping is authoritative.
4. **PostgreSQL evidence workflow.** A CI job (modeled on the RBAC
   PostgreSQL runtime-evidence workflow) that runs the crate's
   PG-backed suites under the application role, plus negative
   cross-tenant visibility tests per policy family.
5. **FORCE rollout.** Only after 1–4: `FORCE ROW LEVEL SECURITY` per
   table family in reviewable steps, each reversible with `NO FORCE`.

### 6.3 Interim contract (binding until Phase F lands)

- Application-level `WHERE` scoping is the authoritative tenant boundary.
  Every new or changed query over scoped tables must carry explicit
  scope predicates; reviewers must not accept "RLS will catch it".
- Existing `ENABLE` + policy DDL is defense-in-depth documentation of
  the intended boundaries. It must not be cited as active enforcement
  in security claims, plan statuses, or verification matrices.

## 7. Remediation status and open follow-ups (2026-10-22)

With Phase H complete, every audit finding is dispositioned: H-1…H-3,
H-6…H-8, M-1…M-12 are implemented (Phases A–E as reclassified in §5, G,
H). The single exception is **Phase F (H-4 FORCE RLS)**, blocked on the
§6 prerequisites (connection-role separation + PostgreSQL evidence) —
it remains explicitly open, with the §6.3 interim contract in force.

Verification note: per §3, every Phase H slice was landed only after
the crate-scoped CI gate (`Atomic module settings CAS`: `cargo fmt -p
rustok-modules -- --check` + `cargo test -p rustok-modules --lib
settings_`) went green, and new tests were written to fail on the old
code (tri-state intent lifecycle in `operation_store`, NULL-intent
settings exposure in `rustok-api::runtime`, typed-error classifications
in H-4a/H-4b). Cross-crate compile surfaces touched by H-3/H-5
(`rustok-migrations`, `rustok-api`, `rustok-tenant`, forum/blog/seo,
`apps/server`) are covered only by the broader workspace jobs, which
are baseline-red on `main`; no green claim is made for them here
beyond the crate-scoped gate and workspace-wide reference sweeps
recorded in the commit messages.

An independent re-verification pass (2026-10-22) then mined the logs
of those baseline-red workspace jobs instead of accepting the
crate-scoped gate as sufficient, and found real regressions the gate
could not see. All are fixed: (1) the storefront native
enabled-modules reader still compiled against the pre-H-5 `bool`
field and read raw rows; it now resolves through the canonical
`SharedModuleEffectivePolicyReader` exactly like admin and GraphQL
(`3b268f2`); (2) four exhaustive matches in `rustok-server` missed
the H-4 variants `ModuleLifecycleDbWriterError::Security` and
`ToggleModuleError::IncompatibleDependencies`; the mappers now cover
them semantically (Policy / BAD_USER_INPUT, with a contract-test
case) (`dda8c2e`); (3) two H-3 schema-ownership census gaps closed
(`000075`/`000076`, see the H-3 slice note), and the stale textual
pins they and earlier slices left in two source-evidence contracts
(`verify-product-lifecycle-collaboration`,
`verify-module-control-plane-write-path`) were re-aimed and both
contracts re-run green. The remaining red workspace jobs were
classified per-job against `main` (baseline fmt/clippy/i18n/docs
failures and the sanctioned append-only migration-plan diffs from
the §13 renames); none is attributable to this remediation.

Open follow-ups recorded during execution (not audit findings):

1. **SQLite "now" format divergence.** Three textual now-formats
   coexist in persisted columns: `datetime('now')`,
   `CURRENT_TIMESTAMP`, and ISO `strftime('%Y-%m-%dT%H:%M:%fZ','now')`
   (settings store, recovery, journals). They were preserved
   byte-for-byte through H-1/H-5 because each is load-bearing for its
   own comparisons; unification needs a dedicated data audit (lexical
   vs chronological ordering per column) and is intentionally not part
   of this remediation.
2. **Raw snapshot readers vs effective policy.** `rustok-api::runtime`
   (`is_tenant_module_enabled`) and SEO's snapshot-based guards answer
   "an explicit enabled row exists", while the control plane's
   effective policy also applies Core/platform defaults. H-5 made the
   divergence honest (a settings save no longer masquerades as intent),
   but new consumers must pick the right question: explicit-row lookup
   for optional-module gating, effective-policy projection for "is it
   on". Hosts composing new gates should prefer the lifecycle reader.
3. **Crate tests still hand-write some DDL.** H-3 made the real
   migrations crate-owned, but several test fixtures still declare
   local `CREATE TABLE tenant_modules` copies (kept in sync manually,
   nullable `enabled` included). Migrating fixtures to run the real
   migration set is worthwhile future hygiene; it was not required to
   close M-12's ownership finding.

## 8. Post-merge re-verification findings (2026-10-22)

After `main` advanced past the branch point, the PR went conflicting
(single content conflict: `Cargo.lock`); merging `origin/main`
(d69e768) in restored the full CI round, and each subsequent failure
was attributed from job logs against `main` itself (push run
39640355254 on the same commit is red for Formatting, Clippy, Cargo
Check ×2, and Documentation, so job color alone proves nothing).
Regressions attributable to this branch, all fixed:

7. **`Cargo.lock` missing workspace edge** (`01e8c6c`). Adding the
   optional `rustok-modules` dependency to `apps/storefront` (the
   branch's only manifest change) had not been reflected in the
   lockfile; `--locked` jobs failed. The lock later took `main`'s
   version wholesale in the merge, with the single sorted edge
   re-inserted (`6ccc81fc`).
8. **H-5 test doubles in `tests/` trees** (`d4217c2f`). The H-5
   struct-literal census had not covered `tests/` directories: three
   `StaticModuleSettingsReader` doubles (server support module, forum
   and search integration tests) still built
   `StaticModuleSettingsSnapshot { enabled: true }`. They model an
   explicitly-enabled module, so `Some(true)` is the faithful
   tri-state form.
9. **H-5 entity reads in server lifecycle tests** (`4b8056b1`).
   `apps/server/tests/module_lifecycle.rs` read the raw
   `tenant_modules` entity `enabled` as `bool` in four
   committed-state assertions; explicit toggles commit
   `Some(true)`/`Some(false)`, so the assertions now check the exact
   tri-state value. (The `ModuleLifecycleStateSnapshot.enabled`
   effective-bool reads in the same file are unchanged and correct.)
10. **dda8c2e import gap** (`5a8dfd71`). The regression-#2 fix added
    `toggle_err_incompatible_dependencies` and its `ToggleCase`, but
    the `mutations.rs` test module imports helpers explicitly; the
    helper was added to that `use super::{...}` list.

Differential compile verdict at `5a8dfd71`: `Cargo Check (stable)`
fails with exactly the three errors `main` itself has (E0425
messenger constants in `notification_outbox_intake_worker.rs` test
code) and nothing else — `rustok-server` lib + integration tests,
`rustok-migrations`, storefront adapter, and the crate all compile.
The `Embedded admin and storefront` job additionally applied the full
composed migration plan to PostgreSQL 16 (step green, including crate
migrations `000075`/`000076` and the tri-state DDL) before failing on
an upstream missing `Arc` import in `commerce_provider_runtime.rs`.

Two red gates need maintainer attention; neither is a code defect in
this remediation:

- **Append-only migration plan** fails by design: its strict
  positional comparator reports the sanctioned §13 ordinal renames
  and the H-3 schema-ownership move as a shifted sequence. The gate
  has no exception mechanism; accepting the diff is a maintainer
  decision at merge.
- **OpenAPI and GraphQL Compatibility** fails in "Export head API
  contracts": the workflow builds `base/` and `head/` checkouts into
  one shared `CARGO_TARGET_DIR`, and cargo reuses the base build's
  `rustok-api` unit for the head build (head sources are older than
  the base artifacts), so head consumers type-check against base's
  `rustok-api` metadata. Any PR that changes a widely-depended
  crate's public types trips this. Suggested fix (infrastructure,
  needs the workflow's own approval step): per-checkout target
  directories.
