# rustok-modules logic audit — 2026-10-21

## 1. Scope, method, and verification limits

Scope: the mandatory Core control-plane crate `rustok-modules`
(`crates/modules/rustok-modules`, slug `modules`, entry `ModulesModule`) —
211 source files, ~117,200 LOC in `src`, plus its 54 migrations, 30
integration-test files, public facade (`control_plane.rs`), and every
production call site in `apps/server`, `alloy`, `rustok-distribution`,
`rustok-mcp`, `rustok-modules-translation`, and the worker crates.

Method: exhaustive static inspection per `AGENTS.md` — full-file reads of the
policy, executor, admission, lifecycle, transition, delivery, settings, and
governance paths; scripted inventories (error types, panics, RLS statements,
table ownership, external symbol usage, unbounded queries); cross-checks of
migrations against raw SQL actually issued by the crate.

**Verification limit (important).** The audit environment has no Rust
toolchain (`cargo`/`rustc` absent), so `cargo check`, `cargo test`, and
`clippy` were **not executed**. Every finding below is static evidence with
`path:line` references. Static count finds 447 `#[test]`/`#[tokio::test]`
functions in 112 files; the continuous-review ledger's earlier "316 tests
pass" claim (FS-03, 2026-10-17) is neither confirmed nor refuted here and
must be re-proven by a real run before remediation starts.

## 2. Executive summary

The crate contains two different products under one roof:

1. A **wired, working core**: static module lifecycle, tenant policy,
   governance/marketplace publication, artifact bindings, settings,
   composition snapshots. This part is mostly sound, with several real
   correctness and safety defects (Sections 4.1, 4.2, 4.4).
2. A **large dormant second control plane** — OCI-pull admission
   (`ModuleInstaller`), node reconciliation, both rollout engines, data
   snapshot/broker, source-object CAS, GC coordinator, wave rollout,
   transition coordinator, dynamic lifecycle — tens of thousands of lines
   with **zero production callers**. Some of it is well engineered
   (node reconciliation), some of it **fabricates data** and would be
   actively dangerous if wired as-is (`dynamic_lifecycle.rs`,
   `release_admission_journal.rs`, in-memory `gc.rs`).

The most dangerous systemic pattern is **security mechanisms that exist on
paper but are bypassed or never engaged in production**: RLS without
`FORCE`, a policy cache that never expires and is not invalidated by
quarantine events, a security epoch that is always `1`, a transition
watchdog that auto-converges with no health evidence, and artifact
definitions that never enter the effective-policy catalog at all.

## 3. What is good (keep and reuse)

- `oci_transport.rs` — HTTPS-only, same-origin upload locations, no
  redirects, bearer-realm validation, credentials withheld cross-origin,
  streamed size caps.
- `artifact_node_reconciliation.rs` — authorizer port, fenced leases,
  claim convergence, observation revisions. Best concurrency design in the
  crate (currently unused).
- `composition.rs` — CAS update + outbox idempotency lease/replay used
  correctly; canonical manifest hashing.
- `artifact_schema.rs` — bounded (128) validator cache, regex size limits,
  no external resolvers.
- `schedule_delivery.rs` / `event_delivery.rs` claim paths —
  `FOR UPDATE SKIP LOCKED`, attempt+worker fencing, claim-time
  revalidation of admission/uninstall/security state.
- `source_object.rs` blob publication — create-new temp file, `sync_all`,
  hard-link publish, re-hash on read (`get_source_blob`).
- Migrations: 52 of 54 have real `DROP`-based `down()`; digest/format
  `CHECK` constraints are consistently applied.

## 4. Findings

### 4.1 Critical / high severity

**H-1. Artifact modules never enter effective policy; latent privilege
escalation in the unwired path.**
The host only calls `ModuleControlPlane::lifecycle()`
(`control_plane.rs:791`); `artifact_lifecycle()` (`:829`),
`static_distribution_lifecycle()` (`:808`), and
`ModuleDefinition::from_artifact_descriptor` (`definition.rs:127`) have zero
production callers, so admitted artifacts are invisible to the policy
catalog. Worse, `from_artifact_descriptor` maps the *untrusted* descriptor
kind `ArtifactModuleKind::Core` to platform `Core` (auto-enabled, not
tenant-disableable), and descriptor validation does not reject `Core`.
Wiring this path as-is would let a published artifact claim Core status.

**H-2. Effective-policy cache serves stale authorization indefinitely.**
`policy_cache.rs` is keyed by `tenant_id` only; `get_latest()` (`:49`) is
served with no freshness check, there is no TTL/eviction/bound, `cached_at`
is unused, and `insert()` (`:78`) swallows a poisoned lock and returns
`Ok`. Host invalidation covers only ~12 `DomainEvent` variants; missing
notably `ModuleArtifactSecurityStateChanged` (quarantine/revoke),
deactivate/uninstall/admit/reverify, `StaticDistributionReleaseRevoked`,
and `ModuleEffectivePolicyRevisionChanged`. Consequence: a quarantined
module can remain authorized (`guards/module.rs` consumes this cache)
until process restart.

**H-3. Toggle executor: permanent aggregate deadlock + non-atomic commit.**
`operation_store.rs:535` `claim` creates a durable claim with **no expiry,
lease, or reclaim** — a crash locks `(tenant, module)` forever.
`mark_committed` (`operation_store.rs:373`) is called inside the state
transaction *before* post-hooks; a hook failure flips the operation
`Committed → Failed` although state and outbox already committed.
`executor.rs:194,263,392` swallow claim-release errors; success-path
`release()?` can error *after* commit. Pre-hooks run pre-transaction,
unscoped, with no operation identity and no rollback. A no-op toggle still
records operation/claim/revision/override yet skips hooks.

**H-4. Row-level security is decorative.**
69 `ENABLE ROW LEVEL SECURITY` statements, **0 `FORCE`** across all
migrations. The application role owns the database, so owners bypass RLS.
Policies of the form `scope_kind='platform' OR tenant=...`
(e.g. `m20260904_000052_module_source_objects.rs:33`) expose all platform
rows to any tenant session, and only 3 `set_config` call sites exist for
123 `current_setting('rustok.tenant_id')` references. Tenant isolation
currently rests entirely on hand-written `WHERE` clauses in raw SQL.

**H-5. `dynamic_lifecycle.rs` fabricates installations.**
`execute_install` hardcodes registry `ghcr.io` / `rustok/{slug}`
(`:505`), an empty dependency lock with `graph_revision: 1` (`:484`),
`size_bytes=1024`, `verification_evidence="{}"`, and no idempotency.
`AttachRetained` ignores the `continuity_token`; publisher check is
fail-open. `execute_enable` has a TOCTOU generation check, trusts the
caller-supplied `smoke_test_passed` flag, and does not check
`rows_affected` (plus `unwrap` at `:439,451,488`). Zero production callers
today — it must be deleted or rewritten before anyone wires it.

> Addendum (2026-10-21, remediation review): `dynamic_lifecycle.rs` and
> the related `gc.rs`, `operator.rs`, `wave_rollout.rs` were deleted in
> remediation Phase E, then **restored** after a provenance check showed
> they are deliverables of the active
> `docs/modules/module-release-rollback-plan.md` and that
> `dynamic_lifecycle` is the sole writer of
> `module_artifact_work_generations`, which canonical delivery code
> reads. The factual core of H-5 stands (fabricated admission evidence;
> never wire as-is). Per-module classification — parallel implementation
> vs unfinished planned functionality — and dispositions are recorded in
> `logic-audit-remediation-2026-10-21.md` §5.

**H-6. Governance authorization by display-label string equality.**
`principal_matches_governance_actor` (`governance/mapping.rs:541`) falls
back from `user_id` to `subject` string, then to **display/legacy label
equality**. It backs manage-request, manage-release, transfer-owner, and
— critically — `governance_actor_can_review_request`, so a module owner
can review their own publish request, and any actor whose principal JSON
carries a colliding label passes ownership checks. Principals are untyped
`serde_json::Value` throughout governance. Additionally,
`governance_actor_can_create_publish_request` allows *any* authenticated
user to claim an unowned slug (namespace squatting by first-comer).

**H-7. Transition/rollback safety is theatrical.**
`SecurityEpochRegistry` is in-memory only; `advance_epoch` has zero
production callers, so the epoch is permanently `1`; history is an
unbounded `Vec`. `finalize()` validates against a freshly constructed
registry — it passes only because both sides hardcode `1`. Production
activation bypasses `TransitionCoordinator` entirely: the checkpoint is
inserted directly as `Observing{now+300s}` (hardcoded window), fenced with
`affected_nodes=&[]`, and the watchdog auto-converges on timeout with **no
health evidence**, releasing predecessor retention holds. The real
rollback window is a fixed 300 seconds regardless of module health.
Coordinator itself: skips `Preflighting`, self-validates the epoch, never
enters `Activating`, ignores `timeout_at`, permits point-of-no-return
from any non-terminal state, skips retention when the ledger is `None`,
and takes authorization as a caller-supplied `bool`.

**H-8. Duplicate, defective admission journal in the public API.**
`release_admission_journal.rs` re-implements admission next to the correct
`installation.rs commit_admission`: unconditional `mark_status` UPDATE
with `rows_affected` ignored, `find(operation_id)` without a tenant
predicate, `failed_for_tenant` unbounded, fabricated `Ok(Staged)`, and
`bind_committed_installation` keyed by `idempotency_key` **only** —
a cross-tenant bind primitive. `unfinished_admissions()` fabricates
recovery records; its only consumer, `ArtifactAdmissionReconciler`, is
itself dead code. Same family: `operator.rs` fake projection
(predecessor := current clone, `affected_tenants_count=1` even for
Platform scope, invented `Uuid::new_v4()` on corrupt IDs) and
`lifecycle_writer.rs persist()`, which fabricates
`previous_enabled = !enabled`.

### 4.2 Medium severity

**M-1. Delivery security checks ignore `payload_digest`.**
`module_artifact_security_states` is keyed by
`(module_slug, module_version, payload_digest)`
(`m20260722_000036…:26`), and the policy-side resolver uses the exact
release. But delivery revalidation matches slug+version only
(`schedule_delivery.rs:790`, `event_delivery.rs:791`): quarantining one
build blocks every build with the same coordinate (availability), and the
two enforcement points disagree about what "quarantined" means.

**M-2. Simulated control plane in production API.**
`gc.rs` — the coordinator **and all 12 adapters** are in-memory
`HashMap`/`HashSet` fakes shipped as `pub` production API (contra
AGENTS.md §14). Logic bug inside the simulation:
`collect_with_token` consumes the token before purge; a purge failure
strands the tombstone in `FinalRecheckPassed`, which can never re-enter
`evaluate_final_recheck` (requires `ActiveGrace|GraceExpired`) —
permanently stuck. Token digest is unkeyed over public fields.
`wave_rollout.rs` duplicates durable `distribution_rollout.rs` in memory:
empty cohorts are vacuously "pre-staged", unknown-node reports are
silently ignored, duplicate node IDs are all updated.
`VerifiedPayloadCache` (`executor_readiness.rs:244`) is unbounded, clones
the payload `Vec<u8>` on every get, never re-verifies, and panics on a
poisoned `RwLock`.

**M-3. Dormant subsystems (zero production callers, verified).**
`ModuleInstaller` (referenced externally only in a doc comment),
`ArtifactAdmissionService`, `ModuleArtifactNodeReconciliationService`
(3,423 LOC), `ModuleStaticDistributionRolloutService` (3,073 LOC),
`ModuleArtifactDataSnapshotService`, `ArtifactDataSnapshotGc`,
`SeaOrmArtifactDataBroker`, `SeaOrmArtifactBindingIdempotencyStore`,
`SourceObjectStore`, `GcCoordinator`, `WaveRolloutCoordinator`,
`ModuleTransitionCoordinator`, `resolve_module_dependencies`, 18 public
modules with no external usage, 15 of ~60 facade methods never called.
This is the crate's largest cost: reviews, migrations, and "audit passed"
claims cover code that cannot execute, while its tables and vocabulary
compete with the wired implementations over the same domain.

**M-4. Settings validation gaps (`settings.rs`).**
Empty schema returns settings as-is — fail-open arbitrary keys (`:214`).
Nested `required` is never enforced (only present keys are validated);
nested defaults are never filled. Defaults are inserted by normalize
without `validate_setting_value`, and schema validation checks a default's
type/options but not top-level min/max/length, so an out-of-range default
is materialized into tenant settings. `integer` accepts floats with
`fract < EPSILON`; min/max compare via `as_f64`, losing precision above
2^53. A second, stricter settings system (JSON Schema) exists for
artifacts — semantics diverge between static and artifact modules.

**M-5. Policy engine inconsistencies (`policy.rs`, `lifecycle_writer.rs`).**
Duplicate `tenant_overrides`/`runtime_inputs` are last-wins while other
inputs reject duplicates. `validate_module_toggle` ignores version,
co-requirements, channel, and runtime inputs. Disabling a Core module is
ignored yet an override fact is still emitted with `enabled=true`.
Cascade ignores `version_requirement`. The revision hash includes the
flapping `executor_available` flag, so revisions change without policy
changes. `ModulePolicyRevisionGate` requires an exact predecessor chain —
one missed revision yields permanent `Stale`. `lifecycle_writer` performs
3 DB round-trips per artifact definition per policy call, swallows
policy/security errors with `.ok()`, deep-clones the catalog per call, and
`overrides()` is unbounded. `persist_settings` freezes a *derived*
`enabled` value as an explicit tenant override.

**M-6. Schedule materializer semantics (`schedule_materializer.rs`).**
`skipped_misfires` is hardcoded `0` (fabricated metric). `RunOnce`
executes the **oldest** missed slot; grace applies only to `Skip`;
enqueue→cursor advance is non-atomic; `Forbid` advances the cursor and
silently drops slots; every tenant poll loads **all** platform
installations; `BacklogLimitExceeded` is declared but never constructed;
DST behavior is untested.

**M-7. Delivery queue drain and retry behavior.**
Claim returns `Ok(None)` after cancelling/dead-lettering a single poison
item, so a backlog drains one item per poll and valid work waits behind
the oldest poison. `expire_claims` resets `available_at=now` — no backoff
on lease expiry, so a crash-looping slot hot-retries until
`max_attempts`. `worker_id` is only shape-validated free text (any worker
can claim any identity). `load_admitted_descriptor` is copy-pasted
between `schedule_delivery.rs` and `event_delivery.rs` and already
diverging (schedule inline-checks security/work-generations; event uses a
separate validity query).

**M-8. Digest/canonicalization discipline is inconsistent.**
A canonical hasher exists (`canonical_manifest_snapshot_json`, used by
composition and data snapshots), but `binding_idempotency.rs`
(`artifact_binding_request_digest`) and `promotion.rs digest_json` hash
raw `serde_json::to_vec` — correctness then depends on workspace feature
unification (`preserve_order`). `dependency.rs graph_digest` (`:111`) is
node-order dependent. Cross-subsystem invariant mismatch:
`DependencyLock::validate` (`dependency.rs:44`) accepts
`graph_revision == 0`, but node reconciliation permanently rejects
identities with `dependency_graph_revision == 0`
(`artifact_node_reconciliation.rs:500`) — such artifacts can be admitted
but never reconciled. No production code originates `graph_revision` from
a durable counter; the installer trusts the caller's lock
(`resolution.rs`, the only producer, is itself unwired, has a dual-semver
engine, an `expect()` panic path, and solves synchronously inside an
async fn).

**M-9. Data broker index equality is representation-sensitive.**
Index values are stored and queried as `serde_json::to_string(value)`
(`data/structured_persistence.rs:387`, `data/validation.rs:88`). Numbers
`1` and `1.0` serialize differently, so index queries silently miss
records depending on which JSON representation the guest emitted.
`index_contract_digest` is order-sensitive over the index list — the same
set in a different order yields contract-mismatch rejections.

**M-10. Source-object store hazards (`source_object.rs`).**
Blocking `std::fs` I/O (full-file `read`, `write_all`, `sync_all`) inside
async fns; `publish_source_file` loads an unbounded file into memory.
Flat CAS layout (all blobs in one directory) degrades at scale.
`destination.exists()` skips content verification on publish.
`get_source_path` returns an **unverified** path despite docs claiming
"verified". The tenant boundary is opt-in: callers may pass
`calling_scope=None` and skip `enforce_rls_boundary` entirely. Blob
publish precedes receipt insert, so a crash orphans blobs with no wired
sweeper (see M-3: the GC that would collect them is simulated).

**M-11. Dispatcher partial-failure semantics (`dispatcher.rs`).**
Binding resolution takes the first `.find(kind)` match; duplicate
bindings are not rejected. Event dispatch drops actor/trace context, and
one binding failure aborts the remaining fan-out after earlier bindings
already executed — callers cannot tell what ran.

**M-12. Split schema ownership.**
23 tables used by this crate's raw SQL are created not by its own
migrations but by `crates/utils/rustok-migrations` (undeclared coupling);
crate tests hand-write DDL instead of running real migrations, so
migration↔SQL drift is undetectable in CI. Backend policy is also
inconsistent: `m20260722_000036` hard-errors on non-PG/SQLite backends
while `m20260904_000052` silently feeds SQLite DDL to any non-Postgres
backend; runtime code systematically treats MySQL as SQLite
(`DbBackend::Postgres => …, _ => …`), and `binding_idempotency.rs:320`
family uses `unreachable!("unsupported database backend was validated")`
— a panic where a typed error belongs. Duplicate migration ordinals on
2026-09-04 (two `000051`, two `000052`, two `000053`) and registration
order ≠ name order (`migrations/mod.rs:57`) invite future collisions.

### 4.3 Low severity / hygiene

- ~521 lines of `String`-typed errors; 26 production `Result<_, String>`
  signatures; 93 error enums with duplicated variants. OCI transport
  flattens all failures to `"OCI registry request failed"` (no
  diagnostics, fixed backoff, no jitter/`Retry-After`, unbounded token
  cache).
- 51 production `unwrap`/`expect` outside tests (worst: `settings.rs` 8,
  `executor_readiness.rs` 6); 11 `unreachable!`; 37 `let _ =` error
  suppressions, including blob-cleanup paths in
  `installation.rs` `ArtifactAdmissionService::admit/admit_file`.
- 69 `Utc::now()` and 23 `Uuid::new_v4()` outside infrastructure
  adapters (the injected `ControlPlaneInfrastructure` clock/ID ports
  exist but are bypassed).
- Helper duplication: `uuid_value` ×9, `storage_error` ×15, placeholder
  builders per file. God file `installation.rs` — 8,216 lines.
- ~292 SELECT statements without `LIMIT` in production code (includes
  several genuinely unbounded list/read paths, e.g. lifecycle
  `overrides()`, journal `failed_for_tenant`, watchdog
  `list_active_checkpoints` loading all rows and filtering in Rust).
- Governance state machine is stringly typed (`status == "approved"`,
  `governance/mapping.rs:490`); one unknown legacy status hard-errors the
  whole projection. Retry eligibility parses a human-readable prefix
  `"Governance rejection reason:"`.
- `dependency.rs` recursive `visit()` is stack-unbounded;
  `definition.rs` fabricates localizations and `"*"` version defaults.
- `transition_store.rs save_checkpoint` is find-then-insert (TOCTOU).

## 5. Six-dimension coverage (AGENTS.md)

| Dimension | Verdict |
|---|---|
| Correctness / logic | Multiple real defects: H-3, M-1, M-4, M-5, M-6, M-8, M-9. |
| Security / tenant isolation | Weakest dimension: H-1, H-2, H-4, H-6, H-8, M-10. |
| Concurrency / consistency | Claim deadlock (H-3), journal TOCTOU (H-8), scattered CAS discipline; bright spots in delivery claims and node reconciliation. |
| Error handling | String errors, swallowed `.ok()`/`let _ =`, fabricated `Ok` and metrics (H-8, M-6). |
| Performance / boundedness | Unbounded caches (H-2, M-2), unbounded lists, N×3 round-trips in policy reads, blocking FS in async (M-10). |
| Maintainability / single source of truth | Duplicate admission, rollout, lifecycle, settings, and descriptor-load implementations; split schema ownership (M-3, M-12). |

## 6. Remediation plan (priority order)

**P0 — safety, before any new feature work**
1. Decide the fate of every dormant subsystem (M-3): wire it behind a
   real consumer with tests, or remove it from the public API
   (`pub(crate)` / feature-gate / delete). Delete or fully rewrite
   `dynamic_lifecycle.rs`, `release_admission_journal.rs`, in-memory
   `gc.rs` adapters, `wave_rollout.rs`, `operator.rs` projections.
2. Reject `ArtifactModuleKind::Core` in descriptor validation; map all
   artifact kinds to non-Core policy definitions (H-1).
3. Policy cache: add TTL + size bound, fail closed on poisoned lock, and
   invalidate on **all** security/lifecycle/revision events (H-2). Add a
   test that quarantine revokes authorization without restart.
4. Operation claim: add lease expiry + reclaim (mirror
   `rustok-outbox` idempotency) and fix commit ordering so an operation
   is marked committed only after post-hooks succeed — or record hook
   failure as a distinct outcome, never rewrite `Committed` (H-3).
5. Remove label/subject-string matching from governance authorization;
   introduce a typed `GovernancePrincipal` (user/service IDs only) and
   prohibit self-review (H-6).
6. RLS: either `FORCE ROW LEVEL SECURITY` + non-owner app role +
   `set_config` on every tenant transaction, or delete the policies and
   document that isolation is application-level. Do not keep the current
   half-state (H-4).

**P1 — correctness**
7. Include `payload_digest` in delivery security checks; share one
   `load_admitted_descriptor` + security predicate between schedule and
   event delivery (M-1, M-7). Drain poison items in a bounded loop per
   poll; add backoff on lease expiry.
8. Settings: enforce nested `required`, fill nested defaults, validate
   defaults with the same validators as user input, reject (don't pass
   through) on empty schema unless the module declares no settings
   (M-4).
9. Canonical JSON for every persisted digest; sort lock nodes before
   `graph_digest`; require `graph_revision > 0` in
   `DependencyLock::validate` and originate it from a durable counter
   (M-8). Canonicalize numeric index values in the data broker (M-9).
10. Transition watchdog: require positive health evidence before
    convergence; make the observation window a policy value, not a
    hardcoded 300s; persist the security epoch and wire `advance_epoch`
    — or remove epoch checks that cannot fail (H-7).
11. Policy engine: reject duplicate overrides/inputs uniformly; make
    toggle validation consider version requirements; remove
    `executor_available` from the revision hash; replace the
    exact-predecessor gate with "apply if newer, resync if gap" (M-5).
12. Materializer: count skipped misfires honestly, run the *latest*
    missed slot for `RunOnce`, make enqueue+cursor atomic, add DST tests
    (M-6).

**P2 — structure**
13. Move the 23 externally-owned tables into this crate's migrations (or
    formally declare the dependency) and run real migrations in tests
    (M-12). Introduce a validated `Backend` newtype (PG/SQLite) at the
    boundary; delete MySQL-as-SQLite fallthroughs and backend
    `unreachable!`s.
14. Collapse parallel implementations: one rollout engine parameterized
    by topology identity, one lifecycle writer, one admission path.
    Split `installation.rs`. Extract shared SQL helpers (`uuid_value`,
    `placeholder`, `storage_error`) into one module.
15. Typed errors at public boundaries; ban `Result<_, String>` in `pub`
    API; route clock/ID generation through `ControlPlaneInfrastructure`
    everywhere.

**P3 — hygiene & evidence**
16. Bound or paginate every list query; cap `VerifiedPayloadCache` and
    the OCI token cache; move `source_object.rs` to `tokio::fs` with
    size limits and a fan-out CAS layout; remove unverified
    `get_source_path` or make it verify.
17. Renumber duplicate 2026-09-04 migration ordinals; keep registration
    order sorted. Update `docs/implementation-plan.md` and the
    continuous-review ledger to distinguish *wired* from *dormant*
    surface, and re-run the full scoped verification
    (`cargo test -p rustok-modules`, `cargo xtask module validate
    modules`) on a machine with a toolchain — this audit could not.

## 7. Appendix: static metrics snapshot

- Source: 211 `.rs` files, ~117,197 LOC (`src`); largest file
  `installation.rs` 8,216 lines. Public surface: 91 modules, 105
  `pub use`/`pub mod` lines in `lib.rs`.
- Tests: 447 test functions across 112 files (static count; not executed
  in this audit). 30 files under `tests/`.
- Schema: 108 tables referenced; 23 created by `crates/utils/rustok-migrations`
  instead of this crate; 69 `ENABLE ROW LEVEL SECURITY`, 0 `FORCE`;
  54 migration files, 53 registered + `mod.rs`; 168 `REFERENCES` clauses.
- Risk markers (production code, tests excluded): 51 `unwrap`/`expect`,
  11 `unreachable!`, 37 `let _ =`, 69 `Utc::now()`, 23 `Uuid::new_v4()`,
  26 `Result<_, String>` signatures, 93 error enums, 36 `*_operations`
  tables.
- External usage: 137 symbols used outside the crate; 18 public modules
  and the services listed in M-3 have zero external production callers.
