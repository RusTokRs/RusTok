# Logic Audit R3 — rustok-modules (2026-10-25)

Third complete review of the Core control-plane crate `rustok-modules`
(`crates/modules/rustok-modules`, slug `modules`; 234 `.rs` files under
`src/`, 126,239 lines including `cli/`, 73 migrations, ~967 re-exported
items from `lib.rs`, 27 integration test files / 475 test functions at
review start), executed per the repository governance contract
(`AGENTS.md`, §2.1). Every subsystem was inspected against the six
Universal Logic Dimensions: (1) preconditions and bounds, (2) branch
completeness and anti-swallowing, (3) state-transition integrity,
(4) side-effect completeness, (5) calculation/algorithmic correctness,
(6) order-of-operations and concurrency/TOCTOU.

Baseline: R1 (`logic-audit-2026-10-21.md`), its remediation record, and R2
(`logic-audit-r2-2026-10-23.md`). R2-1…R2-16 were re-verified against
source and are **not** re-reported as open. This report records new
findings, regressions-by-design that R2 accepted as residuals but that
have since become load-bearing, and whole classes of debt that R1/R2's
point fixes did not generalize.

Base commit: `a3c886c4ad23b89be45cfe57b3cbc85bedd770fc` (`main`).

**Deliverable shape.** This is an audit report with remediation
recommendations. No in-crate fixes are included in this change set; the
roadmap in §6 is the implementation input. Nothing in this document
claims a fix that is not in the tree.

**Evidence status.** The authoring sandbox has no Rust toolchain
(searched: `PATH`, `~/.cargo`, rustup homes — absent). Therefore
`cargo check/test/clippy -p rustok-modules` were **not run** and no
green status is claimed anywhere below. All findings are from full-source
reading, per-file pattern censuses, call-site sweeps across `apps/server`,
and DDL/code parity checks. §7 lists the maintainer evidence loop that
must accompany remediation.

**Scope notes.** Host composition in `apps/server` was read only where a
crate contract depends on it (authorization, watchdog scheduling,
per-request facade construction). `rustok-sandbox`, `rustok-storage`,
`rustok-outbox` internals are out of scope except at their boundaries.

---

## 1. Executive summary

The crate is unusually disciplined at the small scale — exhaustive
backend matching, CAS-guarded writes, bounded streaming ingress,
checked arithmetic in most money-free counters, almost no production
`unwrap`/`panic` (0 real `unsafe`, 0 TODO/FIXME). The defects are at the
*system* scale, and they cluster into five themes:

1. **Caller-asserted attestations.** Several security-relevant records
   are minted from caller-supplied booleans/strings instead of being
   produced by the verifying subsystem: executor smoke-readiness
   receipts (R3-1), admission reverification (R3-4), registry
   "marketplace approval" evidence (R3-3), synthetic SBOM/provenance
   (R3-31). Any host path (or future RPC adapter) that can call these
   services can fabricate the attestation the rest of the platform
   trusts.
2. **Saga windows without a reaper.** The toggle executor, installer,
   and transition machinery split one logical operation across many
   transactions with durable intermediate states (`Validated`,
   `Running`, `Observing`, published-but-uncommitted blobs) and rely on
   callers/watchdogs that either do not exist for operations (R3-6) or
   actively falsify state (R3-8: every activation "fails closed" after
   300 s while the candidate keeps serving).
3. **Idempotency identity includes volatile fields.** Trace/correlation/
   actor/reason are part of replay comparison in the operation journal,
   installation operations, binding idempotency ledger, and governance
   receipts (R3-11). A legitimate retry with a fresh trace context is a
   permanent conflict. This is one of the most replicated bugs in the
   crate because the command-plumbing was copy-pasted per aggregate.
4. **Fork-instead-of-parameterize.** `distribution_rollout.rs` and
   `artifact_node_reconciliation.rs` are two forks of the same
   desired/observed engine (37 identically-named functions,
   3.1k + 3.4k lines); the same pattern repeats for OCI vs
   external-prebuilt admission, event vs schedule queues, the
   "effective installations" SQL (×4), UUID/row decoding (×6), and
   trust `admitted()` (×2). Every fix must be applied N times; R3 found
   multiple places where it was applied to one fork only.
5. **PostgreSQL is effectively untested.** 91 test harnesses use
   `sqlite::memory:`; **zero** PostgreSQL in `tests/`. Dual-dialect SQL
   is hand-written per call site (778 raw statements), so PG-only breakage
   hides in production-only paths — e.g. readiness receipt reads decode
   UUID columns as `String` and cannot succeed on PostgreSQL (R3-2).
   RLS is currently decorative (owner role, no FORCE) and several
   platform flows will silently go wrong the day it becomes real (R3-12).

### Severity table

| Severity | Count | Headline items |
| --- | --- | --- |
| Critical | 0 | — (no remotely-triggerable code execution found) |
| High | 12 | R3-1…R3-12 |
| Medium | 25 | R3-13…R3-37 |
| Low | 13 | R3-38…R3-50 |

Severity is impact × reachability under the *documented* host
composition (server checks authorization before calling most services).
Where a finding is catastrophic only if a host composes it naively, it
is rated Medium and the recommendation is to make the crate API
un-composable-wrong, per AGENTS.md §5/§7 ("make invalid states
unrepresentable").

---

## 2. High-severity findings

### R3-1. Executor readiness receipts are fabricated, not produced by a smoke execution — `executor_readiness.rs`

`ExecutorReadinessService::evaluate_readiness` does not execute anything.
It persists a durable receipt with `smoke_passed = true` when the caller
passes `candidate_smoke_passed: true` and when the stored capability
route *names* intersect the caller's expected set. There is no sandbox
invocation, no binding execution, and no binding of the receipt to the
bytes that would run:

- `EvaluateReadinessCommand::descriptor` is never validated
  (`descriptor.validate()` is not called) and its `artifact_digest` is
  never required to equal `target.payload_digest`.
- `installed_artifact` is read (when an `InstallationId` target is used)
  and then not used for any check after the blob prefetch.
- The command carries no actor/authorization context at all, unlike every
  other durable write in the crate.
- `is_valid_for` ignores `placement` and `placement_policy_revision`, and
  receipts have no expiry: a receipt minted once remains valid forever
  for its (release_digest, fingerprint, pool_generation) key even after
  placement policy or pool contents change within the same generation.
- A stored `placement` value outside the known set decodes to
  `SandboxExecutorPlacement::InProcess` (`.unwrap_or_default()` /
  default-on-unknown in `receipt_from_row`, ~line 701) — corrupt data
  *downgrades* isolation instead of failing closed. PostgreSQL CHECK
  constraints make this unreachable via SQL today; the code-level default
  is still the wrong direction.
- The `infrastructure` field is dead: assigned by the builder, never
  read; ids/timestamps come from `Uuid::new_v4()`/`Utc::now()` directly,
  defeating the fake-clock/id test seam the rest of the crate uses.

Downstream, `require_receipt` is the gate that lets automatic rollout
treat a release as smoke-verified on a pool. The gate currently attests
to the *caller's claim*.

**Recommendation.** Make the receipt a by-product of an actual
execution: the service (or its caller-facing command handler) must run
the candidate's declared smoke/lifecycle binding through
`ArtifactRuntime` against the exact admitted descriptor + payload digest
+ runtime fingerprint, and persist the outcome bound to those digests
plus placement and placement-policy revision. Add a TTL/`valid_until`
and include placement + policy revision in `is_valid_for`. Decode
unknown placement as an error. Delete the boolean inputs from the
command; if an operator override is genuinely needed, model it as a
separate, explicitly-named attestation with actor + reason, never as
`smoke_passed`.

### R3-2. Readiness receipt reads cannot work on PostgreSQL (UUID decoded as `String`) — `executor_readiness.rs::get_receipt`

`module_executor_readiness_receipts.id/operation_id/installation_id`
are `UUID` on PostgreSQL (migration `m20261022_000057`) and `TEXT` on
SQLite. `get_receipt` selects them without casts and decodes
`String` then parses. sqlx will not decode a PG `uuid` column into
`String`; the query errors, so `get_receipt`/`require_receipt` always
fail with `Store(...)` on PostgreSQL while SQLite tests pass. The crate
already owns the correct conventions — `CAST(x AS TEXT)` (used in
`binding_idempotency.rs`, governance) and backend-branched
`uuid_from_row` helpers (used in `source_object.rs`,
`release_admission_journal.rs`, `data/helpers.rs`) — this service uses
neither. There are six separate UUID-from-row helpers in the crate
(`data/helpers.rs`, `artifact_settings.rs`,
`artifact_settings_recovery.rs`, `installation.rs`,
`operations_tool.rs`, `operator.rs`) plus open-coded variants, which is
how one service ended up with a seventh, broken one.

**Recommendation.** Route every UUID read through one
`backend.uuid_from_row` / `optional_uuid_from_row` owner in
`backend.rs`; delete the other five helpers. Add a PostgreSQL
integration lane (see R3-36) — this class of bug is invisible to the
current test suite by construction.

### R3-3. Registry releases are mutable; "marketplace approval" evidence is self-issued — `governance/publication.rs`

Two defects in `publish_request`:

1. **`(slug, version)` identity is overwritten.** If a release already
   exists for `(slug, version)`, publication `UPDATE`s the row: new
   `checksum_sha256`, `artifact_storage_key`, `publisher_principal`,
   `published_at = now`, and — critically — `yanked_reason/by/at = NULL`
   with `status = 'active'`. Republishing a yanked (e.g. compromised)
   version un-yanks it and swaps its bytes under the same
   `(slug, version)`, violating the platform's own immutable/digest-pinned
   release invariant at the registry layer. Digest-pinned installations
   are unaffected, but every consumer that resolves `(slug, version) →
   digest` (marketplace, fresh installs by version, audit) sees the
   identity change retroactively, and the yank audit trail is erased.
2. **Self-issued approval.** The same transaction inserts a
   `registry_publication_evidence` row with
   `authority = 'marketplace_approval'`, `issuer_identity` derived from
   the *publishing actor*, `signature_digest_sha256 = NULL`. The
   "marketplace approval" that later readers treat as an independent
   authority attestation is minted by the actor performing the publish.

**Recommendation.** Make `(slug, version)` insert-only: publishing to an
existing release identity must fail unless the stored checksum,
publisher, and artifact contract are byte-identical (idempotent replay),
and a yank must only be reversible through an explicit, separately
authorized un-yank command that preserves the yank history. Remove the
in-transaction marketplace-approval fabrication; approval evidence must
arrive via the review command path (recorded before publication, by a
principal authorized to approve, ideally ≠ publisher — see R3-30) and
publication should *verify its existence*, as it already does for
author-signature and platform-admission evidence.

### R3-4. Admission reverification trusts caller-asserted evidence and can flip live state — `installation.rs::reverify_admission`

`ArtifactAdmissionReverification` carries a fully caller-constructed
`ArtifactVerificationEvidence` (public fields, `Deserialize`). The
service never calls a `TrustVerifier`; it evaluates
`evidence.admitted()` — five caller-supplied booleans plus syntactically
valid evidence references — and writes the resulting status. The
`admitted()` logic is duplicated from `trust.rs`
(`TrustVerificationDecision::admitted`) but *without* the
`admitted_for` policy-revision binding R2-1 added: reverification does
not check `evidence.trust_policy_revision` against the current trust
policy, so stale-basis evidence "reverifies" fine. `verified_at` is
caller-supplied and persisted as if it were the verification time.

The state write is also unguarded: the `UPDATE` has no `status`
predicate and no uninstall exclusion. Reverify can flip an **active**
installation to `admitted`/`failed` mid-serving (the runtime resolver
then stops selecting it with no transition/fence coordination), and can
record receipts/events for already-uninstalled installations.

**Recommendation.** Reverification must re-run verification inside the
owner boundary: accept `(installation_id, scope, expected_revision,
context)` and internally rebuild the `TrustVerificationRequest` from the
stored reference/descriptor with the *current* policy revisions, call
the `TrustVerifier` port, and persist `from_decision(...)` output.
Guard the status transition with an explicit allowed-from set
(e.g. `admitted|installed|inactive|failed`, never `active`), exclude
uninstalled installations, and emit the active-transition check the
tenant-lifecycle path already performs. Delete the duplicated
`admitted()`; both callers use `TrustVerificationDecision`.

### R3-5. Non-static toggle path ignores claims, CAS, and expected revision — `executor.rs`, `recovery.rs`

The static lifecycle path is carefully fenced (claim → hooks →
revision-CAS commit). The artifact/non-static path in `executor.rs`
skips the operation claim, ignores `expected_revision`, and persists
through `persist_tenant_state` unconditionally: concurrent toggles for
the same `(tenant, slug)` are last-write-wins, and pre/post hooks can
execute in an order inverted relative to the final persisted state
(enable's post-hook after disable's pre-hook). `recovery.rs` retry
inherits the same gap — a retry can run concurrently with a fresh
toggle of the same aggregate with no shared fence. Caller-precomputed
settings (snapshot taken before the saga) are likewise applied without
re-validation at commit time on this path.

**Recommendation.** One execution pipeline for both kinds: claim (or the
static store's revision CAS) is acquired by a single owner before any
hook runs, `expected_revision` is enforced identically, and the commit
re-reads/re-validates settings identity under the claim. Delete the
parallel non-static sequencing rather than fencing it a second time.

### R3-6. Toggle operations strand in `Validated`/`Running`; there is no operations reaper — `executor.rs`, `operation_store.rs`

One toggle spans ≥5 transactions (journal insert → claim → pre-hook →
state commit → post-hook → release). A crash/OOM/cancel between claim
and release strands the operation in `Validated` or `Running` forever:
no component scans for stale non-terminal operations (grep for
reaper/watchdog over `module_lifecycle_operations` finds none; the
existing watchdog covers transition checkpoints only). The tenant's
aggregate stays "operation in progress" until manual DB intervention.
Replaying the same idempotency key against a stranded operation is
reported as a generic persistence error rather than a resumable state.
Separately, transient claim contention is recorded as a **permanent
operation failure**, which poisons the idempotency key: the client's
retry replays the recorded failure instead of executing.

Supporting defects in `operation_store.rs`:

- The initial journal `INSERT` maps `Err(_)` wholesale: any database
  error (outage, constraint other than the unique key) is later
  misreported as an inconsistent-state/conflict outcome.
- `StaticClaim::AlreadyHeld` returns the existing claim **without**
  verifying `expected_revision`, so a caller holding a stale revision
  joins someone else's operation silently.
- Guarded status-transition violations surface as
  `ModuleOperationStoreError::Database(String)`; callers cannot
  distinguish "lost the race" from "database down" except by text.
- The 300 s claim lease has no heartbeat; a hook that legitimately runs
  longer loses the claim, a second executor re-runs the operation, and
  the first one's post-commit `release` then errors *after* success,
  masking the real outcome (see also R3-17).

**Recommendation.** Add an owner-side reaper: non-terminal operations
whose claim is expired transition to a typed `Orphaned` state (with
event), releasing the aggregate; replay of the same key then resumes or
fails with a typed, retryable error. Record claim contention as
*contention*, not operation failure; the operation row should either not
exist yet or be marked `ClaimContended` non-terminally. Narrow the
insert error match to the unique-violation case (backend error
classification helper in `backend.rs`). Verify `expected_revision` on
the `AlreadyHeld` path. Add claim heartbeat for long hooks or bound
hook wall-clock below the lease in the sandbox policy.

### R3-7. Policy publication advances over `ResyncRequired` and can cement a never-current revision — `lifecycle_writer.rs`, `policy_revision_consumer.rs`, `policy_transition_event.rs`

`publish_and_advance` treats `ModulePolicyTransitionOutcome::ResyncRequired`
as success and publishes the toggle anyway. The transition it publishes
was computed in the writer from *unlocked* reads and from the revision
**consumer cursor**, not from the locked current policy. Two concurrent
toggles of different slugs can therefore both compute transitions from
cursor `R0`; each commits its own successor; the cursor can advance to a
revision that was never the tenant's actual current policy, after which
every subsequent event reports `ResyncRequired`, readiness stays
degraded, and the "fix" each time is again to publish over it. The
consumer further identifies revisions by content hash, which is
*unordered*: a stale redelivered event whose hash differs from the
cursor is indistinguishable from a newer one, so the gate can Rewind the
cursor to an older policy.

**Recommendation.** Publish under the same lock/CAS that selects the
predecessor: the transition must be computed from the row locked in the
commit transaction (expected-predecessor check in SQL), and
`ResyncRequired` must abort the toggle with a retryable conflict, not
publish. Give revisions a monotone per-tenant sequence (the hash stays
as content identity, not ordering); the consumer accepts only
`seq = cursor + 1` (or an explicit resync snapshot), never "hash ≠
cursor".

### R3-8. Every activation is put on a 300-second death row; "FailedClosed" changes nothing that serves traffic — `installation.rs`, `transition_service.rs`, `retention.rs`

`activate_artifact` always writes a transition checkpoint in
`Observing { timeout_at: now + 300s }` (hardcoded; a second hardcoded
300 s lives in `transition_coordinator.rs`) plus an
`ActiveRolloutWindow` retention hold expiring at the same instant.
Convergence requires an explicit operator `finalize` (GraphQL) with the
exact operation id + revision within those 5 minutes; otherwise the
server watchdog marks the checkpoint `FailedClosed`. Three incoherences:

1. `FailedClosed` does not deactivate the candidate, restore the
   predecessor, or touch the admission: the "failed" release keeps
   serving indefinitely. The checkpoint ledger and the serving
   selection permanently disagree, and operators learn to ignore
   `FailedClosed`, defeating its purpose.
2. `RetentionHoldLedger::active_holds_count` never evaluates
   `expires_at`; holds are released only by explicit finalize/rollback.
   Every non-finalized activation therefore pins its predecessor's CAS
   bytes **forever** ("retaining predecessor holds" is deliberate in the
   watchdog, but with a 5-minute default window the steady state is
   unbounded retention growth, one leaked hold per activation).
3. While the checkpoint is non-terminal, tenant lifecycle operations
   for that slug are blocked (R3-24) — i.e., for exactly the 5 minutes
   after every activation, *by design*.

Plain activations are not canary rollouts; forcing them through the
rollout state machine with a 5-minute attestation deadline inverts the
intended default (safe = converged).

**Recommendation.** Do not create an `Observing` checkpoint for a plain
activation: either record activation as immediately converged (no
predecessor, or operator did not request observation), or make the
observation window an explicit, configurable command parameter created
only for rollout/update flows. `FailedClosed` must have a serving
consequence (restore predecessor selection inside the watchdog
transaction, or rename the state to what it is and stop blocking on
it). Enforce `expires_at` in the ledger and add a hold reaper; retention
must be time-bounded even when nobody finalizes. Extract the window to
one named constant/config shared by both writers.

### R3-9. Blob reconciler can delete a blob that became referenced after its snapshot — `installation.rs::delete_unreferenced_published`, `referenced_blob_digests`

`delete_unreferenced_published` (1) loads the referenced set,
(2) lists published digests, (3) deletes each unreferenced digest the
retention policy allows. A blob admitted and referenced between (1) and
(3) is deleted while live; `may_delete` is a caller-built snapshot
policy and does not re-read references. The tombstone/grace machinery
in `gc.rs` that exists precisely for this race is not used here (and is
itself inert — R3-35). Additionally, `referenced_blob_digests` runs on
the bare pool with no transaction and no RLS scope: today (owner role)
it reads everything; on a non-owner PostgreSQL role with the admissions
policy, the unset tenant GUC hides tenant-scoped admissions, so tenant
blobs look unreferenced and the TOCTOU becomes wholesale deletion
(R3-12).

**Recommendation.** Two-phase collection: mark candidates
(tombstone row with `first_seen_unreferenced_at`), delete only after a
grace period with a final `NOT EXISTS` recheck in the deleting
transaction, or delete via a single SQL statement that re-evaluates
referenced-ness at delete time. Scope the referenced query explicitly
(platform scope) like every other cross-tenant read.

### R3-10. Installer replay depends on the registry and leaks the staged payload; permissions are registered outside the admission's atomic boundary — `installation.rs::ModuleInstaller::admit`

`admit` calls `find_admission` first but then **always** fetches the
package from the registry (up to 64 MiB) before honoring the existing
admission. Consequences: idempotent replay fails if the registry object
was GC'd or the registry is down, even though the installation is fully
admitted locally; and on the replay branch, when the payload arrived as
`ArtifactPayloadSource::TemporaryFile`, the function returns `existing`
without removing the staged temp file (removal happens only in the
fresh-admit match arm) — one orphaned payload copy in the temp dir per
replayed OCI admission.

After commit, `register_admitted_permissions` runs as a trailing
non-transactional step. Its failure makes `admit` return `Err` for an
installation that **is** durably admitted; a caller that does not retry
leaves an admitted module whose permissions were never registered or
projected, so later activation/dispatch fails far from the cause. The
retry path does heal it — but only by accident of replay re-running
registration, and only if the caller retries.

**Recommendation.** Short-circuit replay before any registry I/O; load
the descriptor from the stored installation row for permission
registration (extend `find_admission`'s result or add an installation
read). Move permission registration behind the existing
`ModuleArtifactAdmitted` outbox event with an idempotent consumer (or
record a durable registration state on the installation and surface it
in the admission result), so "admitted" and "permissions projected" are
separately observable instead of one lying `Err`. Remove the temp file
on every exit path (guard type, not per-branch calls).

### R3-11. Idempotency replay identity includes volatile observability fields (systemic)

Across the crate, replay comparison includes `trace_id`,
`correlation_id`, `actor_id`, and sometimes free-text `reason` —
fields that legitimately change between a client's attempts:

- `operation_store.rs`: journal fingerprint folds requester/trace/
  correlation into the compared identity (and two replay paths compare
  *different* field sets, so the same retry can replay via one path and
  conflict via the other).
- `installation.rs` activation/uninstall/etc. replay checks compare
  actor + trace + correlation + reason exactly.
- `binding_idempotency.rs`: stored trace/correlation must match or the
  claim returns `Conflict` instead of `Replay` — an HTTP client
  retrying with a new `traceparent` can never retrieve its completed
  result.
- Governance receipts (`governance/receipts.rs`) replicate the pattern
  per operation table, including comparing `actor_can_manage_modules`.

The idempotency contract (AGENTS.md §8) is: same key + same semantic
request → same result. Observability context is not semantic.

**Recommendation.** Define one replay-identity owner: identity =
`(scope, idempotency_key)` + digest over a canonical *semantic* request
struct that excludes `ModuleCommandContext` except where actor is a
genuine ownership boundary (then key scope includes actor by design and
says so). Persist `request_digest` uniformly (several aggregates
already do) and delete field-by-field comparisons. This is a mechanical
sweep once the owner exists; it will also delete a few thousand lines
of copy-pasted replay blocks (see R3-33).

### R3-12. RLS is decorative today and incoherent for the day it becomes real — `backend.rs`, `migrations/`

67 tables have `ENABLE ROW LEVEL SECURITY` + one policy each; none use
`FORCE`. With the standard single owner-role connection, RLS never
applies — the code is honest about this in `set_tenant_scope`'s docs
(application `WHERE` predicates are the real boundary, and they are
generally present). The trap is the *transition*: only **one** policy
(`module_artifact_installations`) accepts the
`rustok.module_control_plane_owner = 'platform'` GUC that
`set_platform_scope` sets, while its doc comment claims "the
module-control-plane RLS policies accept this setting". Concrete
breakage when a non-owner role + FORCE lands:

- Platform activation's UI-collision check reads tenant installations'
  admissions; the admissions policy keys off `rustok.tenant_id`, so in a
  platform transaction tenant admissions are invisible and the
  collision check silently passes (today it works only via owner
  bypass).
- `referenced_blob_digests` and other unscoped pool reads return
  partial results instead of erroring (→ R3-9).
- `transition_service::finalize` calls `load_operation(&self.db, …)`
  **before** opening/scoping its transaction (the in-transaction call
  is correctly scoped; the first is not).

**Recommendation.** Decide the deployment role story explicitly and
encode it: either (a) keep RLS as documented defense-in-depth and add a
census test proving every production query carries its own tenant
predicate (the real boundary), removing misleading policy/GUC claims;
or (b) do the non-owner migration properly — every tenant-table policy
gets the platform-owner clause (or platform reads move to a dedicated
role/`SECURITY DEFINER` views), FORCE is enabled per table with a PG
integration test proving both tenant isolation *and* platform flows.
Either way, delete unscoped entry points: every store method should
begin by setting tenant or platform scope (several already do;
`referenced_blob_digests`, the pre-scope `finalize` read, and
`executor_readiness` — whose receipts table policy is tenant-shaped
while the service never sets scope — do not).

---

## 3. Medium-severity findings

### Policy core (`policy.rs`, `definition.rs`, `resolution.rs`)

**R3-13. Policy revision hash lies about executor availability and
unused inputs.** The revision is computed with `executor_available`
forced to `true`, so an executor outage flips enablement decisions
without changing `policy_revision`; every cache keyed by the revision
(policy cache, gate, consumers) now disagrees with `contains()`.
Conversely, tenant overrides for unknown slugs and overrides on `Core`
modules are hashed into the revision but ignored by evaluation, and
typo'd platform defaults change the hash while changing nothing else —
the revision is simultaneously too stable and too volatile. Decide:
either availability is part of policy (hash it, accept revision churn
on outages, and stop calling it transient) or decisions must expose
availability separately from enablement and caches must key on both.
Prune or reject unknown/Core overrides at write time instead of hashing
dead inputs.

**R3-14. `ModuleEffectivePolicy` deserializes without re-establishing
its invariants.** The type is `Serialize`/`Deserialize` with public
construction semantics via serde: a deserialized policy can carry a
`policy_revision` that was never computed from its decisions,
`enabled_modules` inconsistent with `decisions`, and decisions for
modules not in any catalog. Boundary consumers (cache identity,
runtime dispatch gate) trust all three. Either make fields private with
a validating constructor used by `Deserialize` (`#[serde(try_from)]`),
or stop deserializing the aggregate across boundaries and pass the
revision + decision view (which is already the browser-safe pattern in
`ModuleEffectivePolicyView`).

**R3-15. The definition catalog validates far less than its consumers
claim.** `ModuleDefinitionCatalog::insert` checks slug non-empty and the
artifact settings-schema rule — nothing else. `policy.rs` comments
assert "the catalog validates both [versions] at construction"; it does
not parse versions, version requirements, dependency existence,
self-dependencies, cycles, or digest formats. Effects: invalid versions
become silent per-module denials instead of composition errors; a
self-dependency makes a module permanently unenablable with a
misleading denial reason; a dependency cycle of default-enabled modules
survives the policy fixpoint enabled (each sees its dependency
enabled). Additionally `from_static_registry_module` fabricates
`version_requirement: "*"` for every static dependency and
`label = description = key` English "localizations" for every static
permission — invented data that downstream UI/audit surfaces as if
declared. PromotedNative's `distribution_version` is filled with the
*module's own* version, mislabeling the field. Validate at
`insert`/catalog build (semver parse, requirement parse, no self-dep,
digest formats, duplicate deps/bindings), detect cycles once at catalog
freeze, and carry real static dependency constraints from
`RusToKModule` (or type the absence explicitly instead of `"*"`).

**R3-16. Resolver robustness and diagnostics.** In
`ResolutionSnapshot::collect`, every candidate returned by the provider
is version-parsed and platform-range-parsed *before* eligibility
filtering, so one corrupt or historical (yanked) release with a malformed
version/range hard-fails the entire solve for that slug instead of being
skipped with a diagnostic. The snapshot is unbounded (provider-driven
memory). On conflict, PubGrub's derivation is discarded
(`map_err(|_| …)`) and the error names only root slugs, so deep
transitive conflicts are unactionable. Skip-and-record ineligible/
malformed candidates (surface them in `ModuleResolutionResult::conflicts`,
which is currently always empty), bound snapshot size, and map the
solver's derivation into the existing conflict DTO (involved slugs +
ranges) without exposing pubgrub types.

### Lifecycle writing and execution (`lifecycle_writer.rs`, `executor.rs`, `recovery.rs`)

**R3-17. Executor outcome semantics.** A bundle of smaller lies in the
toggle saga: (a) recording an override that does not change the
effective state still runs lifecycle hooks and can fail on an
unavailable executor, so a no-op-intent write errors; (b) a committed
replay returns *current* module state/settings rather than the
operation-time result, so the same idempotency key yields different
payloads over time; (c) failure classes are derived from error-message
prefixes (`post-hook:` etc.) despite `ModuleOperationStatus` being
typed — legacy `Failed` operations with the prefix never resolve even
after a successful retry, and a post-hook failure inside the retry
perpetuates the classification forever; (d) retry/original completion
is two non-atomic writes and can re-run hooks; (e) follow-up
writes/releases mask the primary error on several paths. Persist the
operation result snapshot at commit and replay *that*; record a typed
`failure_stage` column at failure time and delete prefix parsing;
complete original+retry in one transaction; propagate the first error
and attach secondary failures as context.

**R3-18. Writer turns storage failures into policy facts.**
`lifecycle_writer.rs` classifies opaque `String` errors from the
installation/sandbox-policy resolvers as "not installed": a transient
database failure produces a plausible *disabled* policy (with facts
claiming no installation) that is then published and cached. Missing
settings similarly fabricate `{}` as the tenant's settings in decision
facts. Resolver ports must return typed errors with an explicit
`Unavailable` (state) vs `Store` (infrastructure) split — the runtime
resolver errors already model this — and the writer must fail the
resolve on `Store`, never degrade it into enablement data. Missing
settings should be `None` in facts, not an empty object.

**R3-19. Settings CAS is half-enforced.** When only one of
`expected_enabled`/`expected_settings` is provided, the provided half
is silently *ignored* (the command validates "both or neither" deep in
mapper code that returns a `Database` error variant for a validation
failure). When both are provided, `expected_enabled` (an effective
bool) is compared against the stored tri-state `Option<bool>` intent,
so modules enabled by default or by Core status conflict spuriously.
Compare like with like: expected intent vs stored intent, and expected
effective vs resolved effective, as two distinct fences; reject
one-sided expectations at command validation with a typed error.

**R3-20. Whole-catalog work per resolve, repeated.** Every
`ModuleEffectivePolicy` resolve re-hashes the entire catalog, runs an
O(modules² × dependencies) closure, and materializes full fact vectors
for every module; the writer then resolves multiple times per toggle
and opens a transaction per artifact installation lookup (the SQL
resolver does `BEGIN` + RLS set + `COMMIT` per module on what is a
read). At a few hundred modules this is the toggle/readiness hot path.
Cache the catalog hash on the catalog (it is immutable per composition
revision), compute facts lazily for decision views, and batch
installation resolution into one query per resolve. The same
per-module duplication exists between the writer's ordering logic and
the executor's — one topological-order owner.

### Installation & admission (`installation.rs`, `oci*.rs`)

**R3-21. `installation.rs` is five aggregates in one file; its error
variant set has collapsed.** At 8,200 lines the file contains blob
stores, admission, activation/deactivation, tenant lifecycle, rollback,
uninstall, reverification, migration checkpoints, sandbox-policy and
installation resolvers. `AdmissionRevisionConflict` is returned 123
times for: missing installations, nil ids, invalid command context,
empty reasons, wrong status, idempotency conflicts, and actual revision
conflicts. Clients cannot distinguish "retry with fresh revision" from
"your request is malformed" (see also R3-34). `positive_revision`
accepts 0. Rollback fabricates `capability_grant_revision = 1` when the
predecessor grant row is missing and silently clamps stored `<= 0`
revisions to 1 (`rev.max(1)` + `unwrap_or(1)`) instead of treating them
as the invariant violations they are, potentially rolling back under
grant terms that never existed. Split the file by aggregate and give
each command a closed error enum (`NotFound`, `InvalidCommand`,
`RevisionConflict { current }`, `IneligibleStatus { status }`,
`IdempotencyConflict`, `Store`).

**R3-22. Admission limits and zero semantics.** `ArtifactAdmissionLimits`
accepts `actual = 0` (a zero-byte payload whose descriptor claims the
empty-hash passes `verify`), and the limits struct itself is never
validated: `max_*_bytes = 0` silently means "reject everything" here but
"unbounded/disabled" in cache configurations elsewhere in the crate.
Pick crate-wide zero semantics per config type, validate limits at
construction, and reject empty payloads in `ModuleArtifactPackage::verify`
(the publication side already does).

**R3-23. UI-contribution collision check loads the world.**
`assert_artifact_ui_contributions_available` runs an unbounded
`query_all_raw` over **all** active installations and deserializes every
full descriptor to extract UI resources, on every activation/tenant
enable. Runtime cost grows linearly with total active artifacts and
descriptor size, inside the activation transaction, under row locks.
Project UI resources into a table at admission time (installation ×
resource kind/key) with a unique index per effective surface; the
check becomes an indexed lookup and the lock-only
`module_artifact_ui_contribution_locks` table (rows inserted forever,
never deleted) can be retired.

**R3-24. Tenant-lifecycle transition fence is a racy, partial scan.**
`set_artifact_tenant_enabled` loads *all* active transition checkpoints
(`list_active_checkpoints`, unbounded, cross-tenant data inside a tenant
transaction) and blocks only when `tenant_id == Some(request.tenant_id)`.
Platform-scope transitions for the same slug — which change what the
tenant resolves — do not block; and nothing is locked, so a transition
created after the check proceeds concurrently (check-then-act). Query
by `(module_slug, tenant_id OR platform)` with an index, inside the
activation lock the activation path already takes (tenant lifecycle
should take the same lock for the same aggregate).

**R3-25. Stored OCI descriptors are trusted on reuse.**
`oci_admission.rs`/`external_prebuilt_ingress.rs` deserialize the stored
descriptor JSON on the already-admitted path without re-running
`descriptor.validate()` before trust verification, and read
`payload_size_bytes` with `as u64` (a corrupt negative becomes ~2⁶⁴ —
see R3-32). The two ingress pipelines (570 + 993 lines) duplicate their
trust-policy and CAS-payload helper logic, so fixes land twice or not
at all. Validate on load (cheap relative to registry I/O) and merge the
forks behind one admission pipeline parameterised by origin.

### Runtime & delivery (`runtime.rs`, `dispatcher.rs`, `event_delivery.rs`, `schedule_*.rs`)

**R3-26. Event projection uses a different enablement system than
execution.** The projector enqueues deliveries for installations that
are admission-active + lifecycle-enabled, ignoring policy toggles
(tenant overrides/defaults) entirely; at execute time the runtime then
denies by policy, and `DeniedByPolicy` is *terminal* — events that
arrive while a module is policy-disabled are dead-lettered permanently,
even if the module is re-enabled a minute later. Symmetrically,
`is_terminal()` classifies every `ArtifactRuntimeError` as retryable,
including permanent ones (schema violation, descriptor invalid,
binding not admitted), and the queue retries them up to `max_attempts`
regardless of the binding's declared `ModuleBindingIdempotency::None` —
re-executing side-effecting bindings that explicitly declared they are
not retry-safe. All failures are then recorded with only
`execution_failed`/`execution_rejected`, discarding the cause operators
need to triage the DLQ. Unify enablement: project against the same
policy inputs the writer resolves (or park deliveries in a `held` state
while denied instead of dead-lettering); make terminal/retryable a
property of typed runtime error variants; never auto-retry
`Idempotency::None` bindings after an ambiguous outcome (timeout);
persist a bounded, redacted error detail alongside the code.

**R3-27. One bad installation stalls all event/schedule work for the
tenant.** `effective_subscriptions` (events) and `effective_bindings`
(schedules) deserialize every active installation's descriptor; a
single corrupt descriptor or a duplicate active slug
(`AmbiguousSubscription`/`AmbiguousBinding`) fails the whole projection
call — and because the event transport decorator projects *before*
downstream publish, the outbox relay retries that event forever
(head-of-line poison). The full scan + full descriptor parse per event
is also the hot path for every tenant event. Materialize a subscription
index (installation × binding × topics) at activation; isolate
per-installation failures into a quarantine list with metrics instead
of failing the tenant-wide call; treat duplicate-active-slug as an
alerting invariant breach that disables only the offending slug.

**R3-28. Schedule materialization cost and admission holes.** Six-field
crons allow per-second schedules; nothing enforces a minimum interval,
and `latest_due` scans up to 100,000 occurrences per binding per poll
(a per-second binding with a stale cursor burns unbounded scheduler CPU
and can never catch up under `RunOnce`). Per binding the materializer
opens 3–5 transactions (cursor read, maybe init, overlap probe, one per
slot enqueue, cursor store) per tenant poll. `OverlapPolicy::Forbid` is
check-then-enqueue across processes with no lock, so two schedulers can
both create overlapping slots. Enforce a minimum schedule interval at
descriptor validation (e.g. ≥ 60 s, or tier by trust), evaluate due
windows with `Schedule::after` bounds instead of occurrence counting
where possible, batch cursor+enqueue per binding into one transaction,
and take the per-binding lock before the overlap probe (the cursor row
`SELECT … FOR UPDATE` is the natural owner).

**R3-29. Dispatcher API does not carry its own safety.**
`ModuleExecutionDispatcher` happily dispatches command/event/HTTP
bindings with no effective-policy input; enablement is re-checked only
inside `ArtifactRuntimeLifecycleExecutor::dispatch_binding`, and only
for `phase != Lifecycle`. Any other `ArtifactBindingExecutor` (the
in-memory registry one, test adapters wired in hosts, future adapters)
gets no fence, and lifecycle-phase dispatch bypasses policy by design
without the type system showing *who* authorized the lifecycle call.
Per-dispatch cost is also maximal: full `descriptor.validate()`
(recomputes every schema digest), full lock-graph validation inside
`sandbox_request`, and SHA-256 of the whole payload twice
(`get_verified` + `sandbox_request`). thread a `&ModuleEffectivePolicy`
(or an opaque "admitted for execution" token minted by the policy
owner) through dispatch, split lifecycle dispatch into its own port
that only the executor holds, and cache validated descriptors/keyed
validators by descriptor digest (schema validators are already cached;
the descriptor itself is not).

### Governance (`governance/`)

**R3-30. The approval-override gate exists only in projections.**
`approval_override_required` (approved request with non-passed
validation stages) is computed for UI actions/next-step hints, but
`publish_request` never checks validation stage statuses and never
requires `approval_override` when stages are incomplete: existence of
evidence rows (which request managers can record) substitutes for
stage completion. A host that forgets the projection-side gate
publishes without the override the UI promised. Enforce in
`verify_publish_request_prerequisites`: either all required stages for
the origin are `passed`, or a valid override (reason code allowlist
already exists) is present *and* its recorded `validation_stages`
snapshot matches the currently incomplete set. Make the same
authorization posture uniform: evidence/ownership commands check
`governance_actor_can_*` in-service; review actions
(reject/hold/changes/resume) and publish do not check anything beyond
command shape — state in `CRATE_API.md` which commands are
host-authorized, or (preferably) check in-service everywhere, including
separation of duties for approve/publish vs submitter.

**R3-31. First-party synthetic SBOM/provenance dilutes evidence
semantics — `oci.rs`.** For Alloy-authored workspaces the platform
generates a CycloneDX document containing only the file itself (no
components) and an SLSA statement with `resolvedDependencies: []` /
`internalParameters: {}`, then has them signed; downstream trust logic
counts `Sbom`/`Provenance` evidence kinds as satisfied. As a first-party
builder attestation this is defensible only if consumers can tell it
apart from an independently produced SBOM — today the evidence *kind*
is identical and the source dependency lives in a non-standard field
that SLSA tooling ignores. Populate `resolvedDependencies` with the
source digest (it is already known), enumerate the Rhai module imports
as components, and carry an explicit `evidence_origin:
platform_generated` discriminator through to trust policy so "has SBOM"
can mean different assurance levels per origin.

### Cross-cutting

**R3-32. Numeric conversion ownership is inconsistent.** The census
found 77 `as u64`, 32 `as i64`, 39 `.len() as …`. Most are fine; the
pattern failures are: stored `i64` sizes/generations read with `as u64`
(negative → huge: OCI/external payload sizes, governance artifact size,
readiness `pool_generation`/`placement_policy_revision`, snapshot
readiness counts, `dependency_lock.graph_revision != persisted as u64`
comparisons that "work" by wrapping), and domain `u64` revisions written
with `as i64` (wrap to negative past `i64::MAX` — practically
unreachable, but three different conventions coexist: guarded
`> i64::MAX` checks in lifecycle commands, `try_from` in newer code,
raw `as` in readiness). One owner in `backend.rs`:
`revision_value(u64) -> Result<Value>`, `count_from_row(...) ->
Result<u64>` (already exists in `data/` as `nonnegative_usage` — reuse
crate-wide), and a lint-level ban on `as` between signed/unsigned in
store code.

**R3-33. The command/replay/receipt pattern is copy-pasted ~15 times.**
Migration checkpoints duplicate their replay block twice *inside one
function*; activation, uninstall, tenant lifecycle, reverification,
finalize, both rollout engines, and governance receipts each hand-roll
reserve → check → insert-on-conflict → re-read → complete with
divergent details (which fields form identity — R3-11 — whether replay
commits or rolls back, whether scope is set before the first read).
This is the single largest line-count and drift driver in the crate.
Extract one `IdempotentOperation` helper over a small trait
(table, key columns, request digest, receipt codec) and migrate
aggregates opportunistically; at minimum, new code must use it.

**R3-34. Error taxonomy breaks client retry logic.**
Beyond R3-21: `lifecycle_writer` collapses all context validation to
`InvalidIdempotencyKey`; `contracts.rs` validation/conflict
constructors are indistinguishable and a category can round-trip to a
different one; `validate_settings_command` returns a `Database` error
for a client mistake; `FromStr for LifecycleState` has `Err = ()`.
Existing typed variants often carry only `String` (46 `Store(String)`-
style sites), and sandbox/static hook errors cross boundaries as
strings whose *prefixes* drive recovery logic (R3-17). Adopt the rule:
every error crossing a crate port is an enum whose variants encode
retryability (`Conflict`/`Invalid`/`Unavailable`/`Store`), and `String`
payloads are detail-only, never classification.

**R3-35. Dead and speculative subsystems are exported as product.**
`gc.rs` (1,160 lines): `GcCoordinator` plus ten adapters with `pub`
`HashMap` fields simulating stores — referenced only by its own
integration test, yet re-exported from `lib.rs` beside the real
retention/GC services. `wave_rollout.rs` honestly self-labels
"NOT wired into production composition" (and has its own holes: empty
cohorts pass the pre-staging barrier vacuously, rollback is callable
from `Preparing`), but is likewise public API. `InMemoryArtifactBlobStore`
implements **`DurableArtifactBlobStore`** — a volatile test double
satisfying a durability trait, re-exported publicly. Per AGENTS.md
§2.2/§14 these must move behind `#[cfg(test)]`/a `test-support` feature
or be deleted; keeping design sketches in the public API invites a host
to compose them for real.

**R3-36. Verification topology: SQLite-only, all-migrations-per-test,
hand-copied schemas.** 91 `setup_test_db()` harnesses connect
`sqlite::memory:` and replay **all 73 migrations per test**; zero
PostgreSQL anywhere in `tests/`; several unit tests hand-write
`CREATE TABLE` (121 non-migration occurrences incl. fixtures) that can
diverge from real DDL (e.g. missing CHECK constraints that production relies on).
Consequences already visible: R3-2 shipped; SQLite CHECK constraints are weaker
than PG (`length(x)=71` vs `~ '^sha256:[0-9a-f]{64}$'`), so states PG
forbids are creatable in tests; the suite's cost profile discourages
running it. Add: (1) a PG lane (testcontainers or CI service) running at
least the store-level suites for the newest subsystems; (2) a shared
migrated-database fixture (migrate once, snapshot/clone per test);
(3) a parity check that fixture DDL is generated from migrations, not
hand-copied. This is the precondition for trusting any future RLS or
dialect change (R3-12).

**R3-37. Data-plane delete ordering.** `data/gc.rs` deletes object
bytes from storage **inside** the open DB transaction, while holding
the namespace row lock, before deleting the candidate row: a crash or
commit failure after the storage delete leaves a candidate pointing at
missing bytes (next sweep's behavior depends on whether the storage
adapter treats delete-missing as success), and the namespace lock is
held across a network call. `installation.rs::admit_file` has the
mirror-image shape (publish blob → commit admission; failure leaves a
published-orphan that only the racy reconciler of R3-9 cleans). Use the
standard order everywhere: durably mark intent (candidate state
transition + commit) → perform external effect idempotently → record
completion.

---

## 4. Low-severity findings

- **R3-38. Hot-path waste in policy facts.** Each resolve builds full
  fact vectors (incl. per-co-requisite catalog lookups with an
  `.expect("co-requisite provider was normalized…")` panic path if a
  caller feeds un-normalized co-requisites through the public query
  type). `decision()` clones the whole decision per call and fabricates
  an `UnknownModule` decision for absent slugs — fine if documented as
  total, but callers cannot distinguish "not in catalog" from "denied"
  without matching on reasons. Normalize co-requisites in the query
  constructor (private fields), return `Option<&Decision>` from a new
  accessor, keep the fabricating one only for views.
- **R3-39. Slug validation is length-only in the policy toggle path**
  while `artifact.rs` owns a strict `is_valid_module_slug`; the writer
  accepts whitespace/control slugs that can never match a definition
  and reports them as unknown-module at a later stage. Reuse one slug
  newtype at command boundaries.
- **R3-40. Descriptor validation gaps.** No upper bounds on
  `bindings`/`capabilities`/`permissions`/`dependencies` counts, and
  duplicate checks are O(n²) prefix scans — bounded in practice by
  ingress byte caps, but a 256 KiB manifest can still force ~10⁷
  comparisons at admission. Permission key suffix after `"{slug}."` is
  unvalidated (empty suffix, whitespace, uppercase all pass);
  `entrypoint`s are not path-validated (`../` passes descriptor
  validation and relies on the sandbox loader to confine);
  `limit_profile` is a free string never checked against known
  profiles at admission. Add count caps mirroring the schema/localization
  caps, `HashSet` dup checks, key-suffix and relative-path validators.
- **R3-41. Schedule identity includes formatting.** `schedule_binding_digest`
  hashes the raw cron string: `"0 0 * * *"` and `"0  0 * * *"` are
  different schedules to the cursor/dedup machinery (cursor reset +
  potential duplicate slot), though `schedule_cron_expression` already
  tokenizes fields. Hash the normalized field-joined form. DST fold
  occurrences rely on the `cron` crate's iteration semantics; ambiguous
  local times produce at most one UTC slot — document or test this.
- **R3-42. HTTP binding paths cannot express parameters.**
  `valid_http_relative_path` rejects `{id}`, uppercase, and dots, so
  artifact HTTP APIs are literal-path-only forever. If intentional,
  state it in `CRATE_API.md`; otherwise add a single-segment parameter
  grammar now, before descriptors ossify around workarounds.
- **R3-43. "Latest row" queries without tie-breaks.**
  `ORDER BY created_at/submitted_at/published_at DESC LIMIT 1` in
  governance projections (lines ~193/~210) and staging-evidence
  selection order by timestamps at millisecond precision; equal
  timestamps pick an arbitrary row (the file itself uses `id DESC` as
  tie-break in one place). Add the primary key as secondary sort
  everywhere determinism matters for replay/audit.
- **R3-44. Empty `version_requirement` means different things.**
  Policy treats `""` as "any" (deps and co-requisites); the resolver
  and descriptor validation hand it to `VersionReq::parse`. Canonicalize
  in `ModuleDependencyConstraint` construction (empty → `"*"` or
  reject), so stored descriptors cannot split the two engines.
- **R3-45. OCI staging files are world-readable in a shared temp dir.**
  `ArtifactStagingFile` uses `std::env::temp_dir()` +
  `create_new` (good) with default 0644 permissions (payload code
  readable by other local users) and relies on consumers remembering
  `remove_staged_file_best_effort`. Use a dedicated 0700 staging
  directory from composition, `mode(0o600)` on unix, and a Drop-guard
  wrapper type for `ArtifactPayloadSource::TemporaryFile` so new
  consumers cannot leak (R3-10 found one that does). Minor: the
  300-second admission deadline is a literal in the error string while
  the constant lives elsewhere; format from the constant.
- **R3-46. CLI cargo timeouts orphan grandchildren.** `cli/src/lib.rs`
  `run_cargo_command` kills the direct `cargo` child on timeout/output
  overflow; `rustc`/build-script descendants survive, holding target-dir
  locks into the next command. Spawn in a process group
  (`process_group(0)` on unix / Job Object on Windows) and kill the
  group. The two `let _ =` swallows around kill/wait are acceptable on a
  deadline path but should be logged.
- **R3-47. Lock tables grow without bound.** `module_artifact_activation_locks`
  and `module_artifact_ui_contribution_locks` accumulate one row per
  (scope, slug) / resource forever — small, but there is no cleanup when
  a slug's last installation is uninstalled. Bound by lifecycle: delete
  on uninstall (guarded by the same lock) or document as intentionally
  permanent tombstones.
- **R3-48. Capability HTTP client ergonomics/safety nits** —
  new `reqwest::Client` per call (no connection reuse), `no_proxy`
  entries ignored by the custom SSRF guard, multi-value headers
  collapsed, response bodies lossy-decoded as UTF-8, resolved private
  IPs echoed in error strings (information disclosure to artifact
  callers), NAT64/6to4 embedded-IPv4 ranges not covered by the blocklist.
  Reuse one client, return bytes + content-type to the caller, redact
  addresses from errors, and reuse `oci_transport`'s resolver policy
  instead of a second implementation.
- **R3-49. `ModuleControlPlane` composition is recreated per request
  by the host** (51 `ModuleControlPlane::new(db)` sites in
  `apps/server`), which also recreates anything cache-like behind it
  and makes "which infrastructure does this service use"
  request-dependent. Crate-side: `control_plane.rs::release()` is a
  verbatim duplicate of `publication()`; several `new(db)` constructors
  accept a default infrastructure whose event writer is absent and only
  fail mid-transaction. Make the facade cheap-but-shared by documenting
  `Arc<ModuleControlPlane>` composition in `CRATE_API.md`, delete the
  duplicate accessor, and have DB-only constructors either build a real
  outbox writer for the given DB or not exist (builder with required
  parts, per AGENTS.md §5 dependency rules).
- **R3-50. Timestamp producers on SQLite = 3.** `backend.now_sql()`
  (`%f` = fixed 3-digit millis), `datetime_value` (`to_rfc3339()` =
  variable 0/3/6/9-digit fractions), and sea-orm entity
  `DateTime<Utc>` columns in `transition_store.rs` (sqlx encoding).
  They happen to sort consistently for `+00:00` values, but
  `transition_store`'s entity writes bypass the crate's canonical
  converter that R2-3 established, and any future format change must
  find all three. Route entity timestamps through the same converters
  (or store transition checkpoints with the raw-SQL pattern used
  elsewhere) and note the invariant in `backend.rs`.

---

## 5. Checked and found sound (do not re-litigate)

Recorded so future audits don't "fix" correct code:

- **OCI fetch path** (`oci.rs`): declared-size pre-checks, streaming
  digest verification, exact-one-layer selection, manifest digest
  pinning, admission deadline. The `let _config_size` dead binding is
  cosmetic.
- **Dependency lock graph** (`dependency.rs`): iterative cycle
  detection, canonical-order digest, zero-revision rejection, node cap.
  (Field-level format validation gaps are noted in R3-15's catalog
  context, not here.)
- **Data broker** (`data/broker.rs`): namespace row lock serializes
  quota accounting on PostgreSQL (the COUNT/SUM is safe *because* of
  `namespace_lock_clause`); keyset pagination with `LIMIT n+1` and
  escaped `LIKE` prefixes is correct.
- **Claim patterns** in event/schedule queues and both rollout engines:
  `FOR UPDATE SKIP LOCKED` on PG, attempt-fenced completions, poison
  candidate skip bound (the R3-26/R3-27 issues are about *policy*, not
  the claiming mechanics).
- **Transition checkpoint CAS**: `save_checkpoint` enforces
  revision+1 with a guarded `UPDATE`; concurrent coordinators cannot
  silently overwrite (the read-then-check prologue is redundant but
  harmless).
- **Hygiene census**: no production `unsafe`, no `todo!`/`unimplemented!`,
  no `#[ignore]` masking found in the crate's tests, one justified
  `#[allow]` (`source_object.rs`). The remaining production `expect`s
  are enumerated in the appendix; most guard genuinely-proven
  invariants (two exceptions feed R3-38 and the appendix).
- **Resolver comparator conversion** (`resolution.rs`): the
  semver→`Ranges` translation (caret/tilde/wildcard/partial
  comparators incl. the `^0` vs `^0.0` distinction) was verified
  case-by-case and is correct; prerelease exclusion is consistent and
  documented as a v1 constraint *in the error text* (drop the "in v1"
  wording per zero-legacy naming, cosmetic).
- **`format!`-composed SQL**: the 672 format-composed statements
  interpolate placeholders/`now_sql()`/lock clauses only; values are
  bound. No injection instance was found. The residual risk is
  hand-numbered placeholder drift, which is why R3-36's PG lane matters
  more than a style rewrite.

---

## 6. Remediation roadmap (priority order)

**Phase A — attestation integrity (do first; security semantics).**
R3-1 (real smoke execution bound to digests, TTL, placement in
validity), R3-4 (owner-run reverification with current revisions +
status guards), R3-3 (insert-only releases, un-yank as its own command,
remove self-issued approval; enforce stage/override gate per R3-30).
Exit criteria: no public API persists a verification outcome from
caller-supplied booleans; fuzz/negative tests prove fabricated commands
cannot mint receipts/evidence.

**Phase B — saga completion & state honesty.**
R3-6 (operations reaper + typed orphan state + contention ≠ failure),
R3-5 (single toggle pipeline with claims/CAS for artifacts), R3-8
(no Observing-by-default; FailedClosed acts or is renamed; hold expiry
+ reaper), R3-7 (locked predecessor for policy publish; sequenced
revisions; `ResyncRequired` aborts), R3-9/R3-37 (mark → grace → recheck
→ delete; external effects after durable intent).

**Phase C — identity & taxonomy sweeps (mechanical, high value).**
R3-11 + R3-33 (one idempotent-operation owner; semantic digests only),
R3-34 + R3-21 (closed per-command error enums; split
`installation.rs`), R3-32 (conversion owners), R3-2's helper
consolidation. These delete code and make Phase D reviewable.

**Phase D — engine de-duplication.**
Merge `distribution_rollout`/`artifact_node_reconciliation` behind one
desired/observed engine parameterised by target/identity types; merge
OCI/external admission (R3-25); one "effective installations" query
owner feeding projector, materializer, resolver, writer (R3-27);
one topological-order/dependency-closure owner (R3-20).

**Phase E — delivery semantics.**
R3-26 (park-on-denied or policy-aware projection; terminality from
typed errors; honor binding idempotency; keep error causes), R3-28
(min interval, locked overlap, batched materialization), R3-24
(indexed, locked transition fence incl. platform transitions),
R3-23 (UI resource projection table).

**Phase F — PostgreSQL reality.**
R3-36 (PG test lane + shared fixture), then R3-12 (choose and encode
the RLS/role story; scope-first entry points; census test for tenant
predicates). R3-2 is fixed as part of Phase C but *proven* here.

**Phase G — surface cleanup.**
R3-35 (gc.rs/wave_rollout/test doubles out of public API), R3-49
(facade composition + duplicate accessor), R3-14 (policy aggregate
invariants), R3-15 (catalog validation at build; no fabricated static
metadata), R3-29 (dispatcher carries policy token; cached descriptor
validation), and the R3-38…R3-50 tail. Reconcile `CRATE_API.md`'s
"bounded API" claim with ~967 re-exports: either curate the surface
down (with `#[doc(hidden)]` internals as a first step) or rewrite the
claim; link R2/R3 audits from `docs/README.md` (R2 is currently
missing there).

---

## 7. Verification loop for remediation (maintainer)

No evidence is claimed by this report. For each phase, on the
remediation branch:

```
cargo check -p rustok-modules
cargo test -p rustok-modules
cargo clippy -p rustok-modules --all-targets
cargo fmt --all
git diff --check
```

Plus, per AGENTS.md §10–§11: scoped `scripts/verify/*` for touched
contracts, and — once Phase F exists — the PostgreSQL lane for every
store touched. Workspace-wide builds and `apps/server` compilation
only when a phase changes public APIs consumed there (Phases A, C, G
will: coordinate with `apps/server` call sites for
`ModuleControlPlane::new`, `evaluate_readiness`, `reverify_admission`,
and governance commands). **No `cargo build -p rustok-server` without
explicit user instruction.**

Every High finding should land with a regression test that fails on the
current code for the stated reason (not merely for a renamed symbol):
fabricated-attestation rejection (R3-1/R3-3/R3-4), PG receipt read
(R3-2), concurrent artifact toggles (R3-5), crash-injected saga +
reaper (R3-6), concurrent different-slug toggles (R3-7),
activation without finalize (R3-8), admit-during-GC race (R3-9),
replay with rotated trace context (R3-11), registry-down replay (R3-10).

---

## Appendix — censuses used (production `src/` + `cli/`, test code excluded unless noted)

| Signal | Count | Note |
| --- | --- | --- |
| raw `Statement` sites | 778 | values bound; risk is placeholder drift, not injection |
| format-composed SQL | 672 | placeholders/`now_sql()`/lock clauses only |
| transaction `begin()` | ~190 | scope-less begins: `referenced_blob_digests` (no tx), finalize pre-read |
| `expect` (prod) | 47 | mostly proven invariants; see R3-38 |
| `let _ =` (prod) | 15 | serious: `data/helpers.rs` object delete, `installation.rs` staged discard, tx rollbacks, CLI kill/wait |
| `unwrap_or*` (prod) | 79 | dangerous instances in R3-1/R3-21; most fail-closed |
| `as` numeric casts | 134 | R3-32 |
| `Utc::now` direct (bypassing infrastructure clock) | 65 | concentrates in readiness/transition/gc-model code |
| `Uuid::new_v4` direct | 17 | same |
| TODO/FIXME/prod `unwrap`/`panic!` | 0 | confirmed |
| test files / test fns | 27 + inline / 475 | all SQLite (91 harnesses), 0 PostgreSQL |
| tables / RLS-enabled | 120 / 67 | 0 `FORCE`; 1 policy reads the platform GUC |
| `lib.rs` modules / re-exports | 94 / ~967 | vs `CRATE_API.md` bounded-surface claim |
| files > 1,500 lines | 14 | largest: `installation.rs` 8,203 |

---

## Addendum — remediation status (2026-10-26)

Implemented on branch `arena/b606e380-rustok` (§6 phases A–G). **Status
vocabulary:** *Source* = implemented in source, **not yet compiled or
tested** — the remediation environment had no Rust toolchain, so the §7
verification loop is still owed in full by the maintainer and nothing below
is claimed green; *Partial* = the noted remainder is open by decision or by
phase; *Open* = not started; *Decided* = closed by an explicit documented
decision. A manual call-graph/type review plus a tree-sitter syntax pass
over every changed Rust file stands in for the first compilation so far;
that review already caught and fixed real compile blockers (lost module
declarations and imports), which is evidence the loop cannot be skipped.

| ID | Status | Notes / remainder |
|---|---|---|
| R3-1 | Source | Smoke execution is real and digest/placement/generation-bound; pool identity and generations are durable (`module_executor_pools`, migration 000087); isolated-pool fingerprints come only from the worker's mTLS identity. |
| R3-2 | Partial | Safe part implemented; `artifact_settings.rs` remains a documented exception. *Proof* is Phase F (PostgreSQL lane). |
| R3-3 | Source | Insert-only releases; un-yank is its own command; self-issued approval removed; stage/override gate enforced per R3-30. |
| R3-4 | Source | Owner-run reverification with current revisions and status guards; no caller-asserted evidence. |
| R3-5 | Source | Single toggle pipeline with claims/CAS for artifacts. |
| R3-6 | Source | Operations reaper, typed orphan state, contention ≠ failure. |
| R3-7 | Source | Locked predecessor, sequenced revisions, `ResyncRequired` aborts publication. |
| R3-8 | Source | No Observing-by-default death row; holds expire and are reaped; governed activation only through the checkpoint saga. |
| R3-9 | Source | Mark → grace → recheck → delete for blobs. |
| R3-10 | Source | Replay no longer depends on the registry; permissions register inside the admission boundary. |
| R3-11 | Source | Semantic replay digests exclude trace/correlation/free-text reason. |
| R3-12 | Open | RLS/role story undecided; Phase F (needs the PG lane first). UI projection table (000085) is intentionally non-RLS. |
| R3-13 | Partial | Decided and documented in the policy owner: the revision identifies durable intent/evidence; volatile executor liveness is excluded from the hash and exposed as a decision fact (`ModuleEffectivePolicyFact::Executor`), so outages do not churn the revision. Residual, accepted: overrides for unknown slugs are still hash inputs although evaluation ignores them (persisted overrides are not tightened retroactively, R3-39); pruning them changes issued revisions and requires an `effective_policy_revision` contract bump. |
| R3-14 | Partial | Internal consistency of the aggregate re-established on construction/deserialization; this is consistency, not provenance — no stronger claim is made. |
| R3-15 | Partial | Local catalog validation done. Not claimed: typed absence for `"*"`/localizations, distribution/module version equality, dependency existence on incremental insert. |
| R3-16 | Source | Malformed historical candidates are isolated with diagnostics; malformed root/request/provider/snapshot excess stay hard errors. |
| R3-17 | Source | Outcome semantics typed end to end; see also the `ModuleWorkOutcome::Held` addition under R3-26. |
| R3-18 | Source | Absence is a policy fact; storage/invariant failure is a hard error. |
| R3-19 | Source | Settings CAS compares effective enablement. |
| R3-20 | Source | Canonical-byte definition cache; `resolve_active_many` batches owner-level resolution; payload rehash and dependency-lock validation retained. |
| R3-21 | Partial | Error-variant collapse addressed where commands needed it; **splitting `installation.rs` remains open** and is deliberately sequenced after the first green compile. |
| R3-22 | Source | Admission limits and zero semantics fixed. |
| R3-23 | Source | Durable UI-resource projection (migration 000085), created only by the admission funnel/backfill. |
| R3-24 | Source | Indexed, locked transition fence; Platform scope locked before Tenant. |
| R3-25 | Source | Stored OCI descriptors revalidated on reuse; OCI/external admission unified. |
| R3-26 | Source | Policy denial parks without consuming attempt budget; `Idempotency::None` never auto-retries after possible guest execution; denial is the typed `ModuleWorkOutcome::Held`, string-code sniffing deleted. |
| R3-27 | Source | Corrupt installation/duplicate slug isolated per tenant; storage failures propagate; one effective-installations query owner. |
| R3-28 | Source | One transaction per binding, Postgres-only `FOR UPDATE`, 10,000-slot cap. |
| R3-29 | Source | Dispatcher carries the opaque `ArtifactExecutionAdmission` token; runtime still re-checks fresh policy. |
| R3-30 | Source | Stage/override gate enforced in `verify_publish_request_prerequisites`; authorization posture (which commands are host-authorized, incl. governance review actions) stated in `CRATE_API.md`. In-service reviewer checks remain possible future hardening. |
| R3-31 | Partial | Bounded fix only; no broader evidence-kind redesign. |
| R3-32 | Source | Conversion owners consolidated. |
| R3-33 | Source | Single idempotent-operation owner; semantic digests only. |
| R3-34 | Partial | Settings-command `Database` misclassification fixed; `ModuleErrorCode` round-trips exactly (closed enum + `Unknown`); the `LifecycleState` `Err = ()` FromStr no longer exists in the crate; context-validation collapse into `InvalidIdempotencyKey` fixed 2026-10-26 (`InvalidCommandContext(#[from] ModuleControlPlaneError)` in both lifecycle error enums, cause preserved). Residual: `validation`/`conflict` constructors do not constrain which code they accept, and `String`-payload variants elsewhere remain detail-carrying rather than fully closed per-command enums. |
| R3-35 | Source | `gc`/`wave_rollout`/test doubles behind `test-support`, never in production composition. |
| R3-36 | Open | PostgreSQL lane + shared fixture not built; Phase F, maintainer-owned (needs CI/testcontainers), still the precondition for trusting dialect/RLS work. |
| R3-37 | Source | Data GC claims and commits before crossing the storage boundary; completion recorded after (mirrored for publish order in R3-9). |
| R3-38 | Partial | Bounded hot-path fixes only. |
| R3-39 | Source | Typed slug validation in the toggle path; persisted overrides/runtime inputs deliberately not tightened retroactively. |
| R3-40 | Partial | Entrypoint confinement without traversal; `/` is not rejected outright by design. |
| R3-41 | Source | Schedule identity canonicalized (digest upgrade note in `docs/README.md`). |
| R3-42 | Source | Literal-segment HTTP binding paths are the stated v1 contract (`CRATE_API.md`); parameters need a schema revision. |
| R3-43 | Source | PK tie-breaks added only where selection is outcome-sensitive. |
| R3-44 | Source | `version_requirement` semantics unified. |
| R3-45 | Source | OCI staging files no longer world-readable in shared temp. |
| R3-46 | Source | Unix Cargo process-group termination (no orphaned grandchildren). |
| R3-47 | Source | Lock rows retained as tombstones by decision; growth bounded by aggregate count, not deleted. |
| R3-48 | Partial | Bounded capability-HTTP fixes only. |
| R3-49 | Source | Facade composed once per process; duplicate accessor removed. |
| R3-50 | Partial | Timestamp canonicalization owned by the backend owner; no blanket producer rewrite. |

**Phase status.** A, B, E: implemented in source. C: implemented except the
`installation.rs` split (R3-21). D: one topological-order/closure owner and
one effective-installations owner done; the
`distribution_rollout`/`artifact_node_reconciliation` engines are
semantically aligned but **not merged** — alignment included reviving the
static rollout's previously dead post-convergence drift/`Degraded` recovery
loop (converged/degraded rollouts stay claimable, `failed` re-preparable
only while `Degraded`, identity anchored to the active head, no recommit on
reconvergence); the merge itself is sequenced after the first green
compile. F: open (R3-36, R3-12, R3-2 proof). G: implemented in source,
including the `CRATE_API.md` reconciliation — the crate root re-exports
~1,000 names as a workspace convenience; the document now states that
plainly and names the supported composition surface instead of claiming a
bounded API. Curating the surface down remains open code work.

**Related platform completion in the same effort.** The governed dynamic
artifact transition is wired end to end (checkpoint-fenced activation under
`FOR UPDATE`, owner-derived Automatic preflight, per-pool smoke readiness,
durable pool registry, sandbox-worker identity over mTLS, server host
composition, tenant-scoped GraphQL start) — Automatic mode only;
maintenance-classified changes fail closed until a guarded migration owner
exists. The static-distribution rollout owner is complete, but its
authenticated outside-candidate deployment transport remains the canonical
plan's explicitly uncomposed boundary
(`docs/implementation-plan.md`); it is host/deployment work, not an owner
gap, and must not be faked with an in-server agent.

**Next, in order:** (1) maintainer runs the §7 loop crate-scoped and the
branch absorbs compile/test fallout; (2) `installation.rs` split and the
Phase D engine merge as post-compile refactors; (3) Phase F PostgreSQL
lane, then R3-12/R3-2 closure; (4) static-rollout deployment transport as a
separate host slice; (5) public-surface curation.
