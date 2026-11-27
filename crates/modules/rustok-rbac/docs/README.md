# `rustok-rbac` Documentation

`rustok-rbac` is the canonical RBAC runtime module in RusToK. Local
documentation for this module must live inside the crate, not spread across
`docs/architecture/*` or server-only notes.

## Purpose

- publish a unified RBAC runtime contract for permission resolution and checking;
- keep permission policy/evaluator and integration event contracts inside the module;
- keep `apps/server` in the adapter/wiring layer role, not as a second RBAC runtime.

## Scope

- relation-based source of truth: `roles`, `permissions`, `user_roles`, `role_permissions`;
- `PermissionResolver`, `RuntimePermissionResolver`, policy/evaluator and tenant policy authorization flow;
- cross-module event contracts for role assignment changes;
- permission-aware runtime contracts and typed RBAC primitives in conjunction with `rustok-core`;
- absence of rollout-mode and shadow-runtime logic in the live surface.

## Read-only role overview

GraphQL and native admin delegate to `load_tenant_roles_overview_on`. One
statement reads roles, tenant-matching grants and localized display names;
roles with no grants remain visible and empty tenants return an empty list.
Listing does not seed or repair authority. Bootstrap and explicit operator
repair retain their separate write boundaries. Both transports use system-first,
slug-ordered results. Regression source is authored; execution is pending.

## Transaction-bound management alternatives

GraphQL role update and deletion revalidate the requested tenant, role UUID
and slug in the locked row query. A rename between the initial locator read
and transaction admission returns not-found instead of applying stale slug
policy (including break-glass protection) to the renamed role. The regression
exercises both mutations; runtime verification is pending.

The host RBAC control-plane authority returns only the effective intersection
of the admitted request and current module-filtered grants. The GraphQL owner
also applies its request ceiling before testing management alternatives.
A disjoint old/new management permission pair never authorizes a mutation;
a concrete grant can survive a manage wildcard in either direction, but
cannot be widened to manage. Credential deadline and database fences remain
mandatory; source regressions are unrun.

Structured artifact namespace purge also consumes this host mutation authority:
credential/module/role fences are acquired on the Modules owner transaction
before receipt lookup and activation locks. Exact terminal replay requires
current admitted authority, and each commit checks the credential deadline.
Operational drain, legal holds and global lock-order execution evidence are
separate gates.

## Standalone role lock ordering

PostgreSQL standalone replacement takes the target user fence, then the
existing target membership, destination and SuperAdmin continuity roles in
canonical UUID order before per-role checks. Bootstrap assignment fences
the user and exact system role before touching shared permission vocabulary;
a late role conflict fails via NOWAIT. Lazy missing-role creation and bulk
repair remain outside a complete cross-owner ordering proof. PostgreSQL
regression evidence is authored but unrun.

## Integration

- `SeaOrmRelationPermissionStore` owns persisted relation reads for both cached
  runtime and uncached decisions in a single SQL statement joining the exact
  tenant/user, memberships, roles, and grants. `authorize_current_permission` evaluates
  those current grants on a host-supplied connection or transaction through the
  canonical tenant policy engine; it never
  consumes request snapshots and does not establish a revocation fence;
- `resolve_persisted_permissions_on` uses that same reader on a host-supplied
  connection or transaction; server authoritative reads delegate here;
- `read_user_membership_revision_on` reads the durable membership epoch under
  the caller's user mutation fence. Relation insert/delete/changed identity
  advances affected users atomically, independent of cache invalidation.
  See the [membership epoch ADR](../../../../DECISIONS/2026-11-27-rbac-membership-revision.md).
- `lock_invite_role_grant_binding_on` locks all issuer membership roles and
  the requested role in stable identity order and returns their canonical
  revision binding. Static grants and role identity changes advance the role
  epoch transactionally; see the [role epoch ADR](../../../../DECISIONS/2026-11-27-rbac-role-grant-revision.md).
  Migration 000006 adds PostgreSQL nonblocking BEFORE relation guards for
  surviving membership/grant revision parents, followed by referenced
  role/permission FK parents. The existing AFTER revision triggers remain
  the sole revision writers. Late contention returns SQLSTATE 55P03;
  roll back the entire transaction. Direct artifact grants and receipts
  also pre-lock actor, role and definition FK parents with KEY SHARE NOWAIT.
  This does not cover arbitrary bulk child-row acquisition or cascade waits
  before the trigger runs. No SQLite behavior or stored revision changes.
  PostgreSQL regression evidence remains unrun.
  `lock_actor_role_grants_on` uses the same ordered static-role fence as
  invitation grant binding, without requiring a destination role. The caller
  holds the actor user fence and owns the subsequent permission decision.
- `load_role_user_ids_on` owns exact persisted role-membership reads and excludes
  cross-tenant subjects even when malformed membership rows exist;
- authenticated request snapshots are inputs to the owner `PermissionResolver`
  contract. Single, any, and all permission decisions use the same tenant policy
  engine; the server never decides permission membership independently. Snapshot
  use preserves the request permission ceiling and does not prove current grants
  or an atomic revocation fence. Credential-bearing snapshots fail closed at
  their exclusive deadline or a fault of the shared Auth local clock; matching
  invalid scopes never fall back to persisted grants;
- `RbacRoleAssignmentDbWriter::remove_tenant_role_assignments_on` validates
  persisted subject tenancy and owns membership deletion inside the host transaction;
- `plan_persisted_user_role_mutation_on` reads and locks current target authority,
  exact assignments, and continuity facts. `ensure_user_authority_continuity_on`
  also covers status-only changes and removal; `replace_persisted_user_role_on`
  applies ordinary replacement before host commit and cache delivery;
- bounded cached permission resolution returns a typed unavailable error when
  generation-aware publication rejects every attempt; it never returns a rejected
  read or turns contention into a successful empty grant set. The default cache
  adapter methods do not provide generation guarantees. See the
  [cache-resolution decision](../../../../DECISIONS/2026-11-02-rbac-cache-resolution-consistency.md);
- `apps/server` owns only authenticated adapters, cache integration, transport
  extractors and observability; it no longer implements the persisted relation reader;
- GraphQL role query/mutation/types live in `rustok-rbac`; `apps/server` only composes roots and passes adapter role records to runtime persistence;
- `rustok-core` remains the owner of typed primitives (`Permission`, `Resource`, `Action`, `SecurityContext`);
- live authorization goes only through tenant policy evaluation, without a relation-only/shadow parity path;
- `RbacPermissionDecisionPort` resolves its tenant/user decision through the
  authoritative `PermissionResolver`; request claims are not used as an
  independent permission source;
- `RbacArtifactPermissionCatalog` is the durable owner adapter for immutable
  artifact permission vocabulary. One language-neutral definition owns scope,
  installation, module, release and permission identity; localized labels and
  descriptions live in the owner translation table with `VARCHAR(32)` locale
  storage. Registration normalizes every locale through
  `rustok_api::normalize_locale_tag`, rejects semantic duplicates after
  normalization, and stores only the canonical tag. Registration is idempotent
  and never writes `roles` or `role_permissions`. Its owner migrations are
  aggregated by `rustok-migrations::Migrator`, the installer and the CLI schema
  path used by production hosts;
- `RbacArtifactPermissionAssignmentService` owns explicit, idempotent
  tenant-role grants and revocations for that vocabulary in
  `rbac_artifact_role_permissions`. Grants and operation receipts reference the
  exact immutable permission definition and admitted scope, validate the exact
  installation and platform-or-tenant scope before writing, record the acting
  operator, and never mutate static `role_permissions`. Actual state changes
  reserve the existing durable permission generation before event publication;
  no-ops and exact retries do not. Actor identity and exact permission
  definition remain protected through commit;
- `SeaOrmArtifactPermissionAuthorizer` resolves a role-derived grant only when
  tenant, user, role, exact permission definition, admitted scope, installation
  and permission key all match. The platform artifact HTTP and command routes
  are runtime consumers; they never interpret a module-defined permission as a
  static `Permission` enum value;
- the operator-facing admin overview lives in `rustok-rbac-admin` and is structured as FFA `core` + native-only `transport` + `ui/leptos` adapter;
- new public RBAC surfaces and event contracts require synchronization of module docs, server docs and verification plan.

## Artifact authorization lifecycle and teardown

Artifact authorization rows fail closed under parent lifecycle changes:

- role, actor and permission-definition foreign keys use `ON UPDATE RESTRICT`
  and `ON DELETE RESTRICT`; a direct parent update or hard delete cannot leave a
  grant or idempotency receipt orphaned;
- tenant-scoped definitions may be referenced only with
  `permission_scope_key = tenant:<tenant_id>`; platform definitions use the
  explicit `platform` scope. A grant cannot bind one tenant to another tenant's
  definition even through a direct database write;
- language-neutral permission definitions are immutable after admission.
  Localized translations remain updateable and are removed with their owning
  definition only after authorization references have been removed;
- the current Auth Admin `delete_user` operation is account deactivation and
  redaction, not a hard user-row delete. Existing artifact grants and operation
  receipts therefore retain their actor identity;
- no public role or tenant hard-delete path may bypass RBAC. A future hard-delete
  workflow must run in one owner-coordinated transaction and remove dependent
  artifact operation receipts, then grants, then admitted definitions and
  translations when their installation is removed, before deleting role, user
  or tenant parents. Until that workflow exists, `RESTRICT` is the canonical
  behavior and a blocked delete is an integrity result, not a condition for a
  compatibility fallback or cascade.

## Observability and release gates

Canonical authorization signals:

- `rustok_rbac_permission_cache_hits`
- `rustok_rbac_permission_cache_misses`
- `rustok_rbac_permission_checks_allowed`
- `rustok_rbac_permission_checks_denied`
- `rustok_rbac_claim_role_mismatch_total`
- `rustok_rbac_engine_decisions_policy_total`
- `rustok_rbac_engine_eval_duration_ms_total`
- `rustok_rbac_engine_eval_duration_samples`

Canonical durable-invalidation signals:

- `rustok_rbac_invalidation_durable_generation` — database source-of-truth generation;
- `rustok_rbac_invalidation_applied_generation` — generation applied to this process;
- `rustok_rbac_invalidation_generation_lag` — signed durable minus applied generation; a negative value is a database regression;
- `rustok_rbac_invalidation_watchdog_running` — `1` while the supervised watchdog worker is running;
- `rustok_rbac_invalidation_watchdog_restarts_total{reason}` — bounded reasons `panic`, `unexpected_exit`, or `runtime_replaced`;
- `rustok_rbac_invalidation_database_read_errors_total` — durable-generation read failures after installation;
- `rustok_rbac_invalidation_recoveries_total{reason}` — bounded reasons `initial_sync`, `generation_advanced`, or `generation_regressed`;
- `rustok_rbac_invalidation_full_clears_total{reason}` — process-wide permission snapshot clears caused by the same bounded recovery reasons.

The invalidation metrics deliberately have no tenant, user, role, permission, session,
client, or cache-key labels. They describe process-level recovery state and must remain
bounded across replicas.

Release gates for changes in the module:

- update unit tests for changed domain logic;
- verify compatibility with server adapters;
- synchronize `README.md`, local docs and verification docs;
- do not reintroduce rollout-mode or a second live authorization path.

The GraphQL `update_role` grant-edit path reserves the durable generation in the
same transaction as a supplied grant replacement. The server then clears its local
cache and attempts all-users fan-out; the generation watchdog can reconcile missed
notifications. This is eventual recovery rather than an outbox or synchronous
revocation fence; the added regression tests have not yet run. See the
[decision](../../../../DECISIONS/2026-11-02-rbac-graphql-grant-invalidation.md).

## Durable invalidation alert policy

The watchdog reconciles every five seconds. Operators should apply these baseline
thresholds and tune only after retaining workload evidence:

- warning when `rustok_rbac_invalidation_generation_lag > 0` for two consecutive
  polls (10 seconds); critical when it remains positive for 30 seconds;
- immediate critical when `rustok_rbac_invalidation_generation_lag < 0`, because
  the database generation has regressed below the process checkpoint;
- critical when `rustok_rbac_invalidation_watchdog_running == 0` for more than two
  seconds outside a controlled shutdown;
- warning when watchdog restarts increase by at least three in ten minutes;
  critical at ten in ten minutes or any sustained restart loop;
- critical after three consecutive durable-generation read failures or 15 seconds
  without a successful read after the generation table is installed;
- investigate every `generation_regressed` recovery or full clear. An
  `initial_sync` full clear is expected once per process start, while repeated
  `generation_advanced` clears indicate missed fast-path invalidations or listener lag.

## Durable invalidation incident runbook

### Redis outage or restart

1. Confirm authorization writes still commit and advance
   `rustok_rbac_invalidation_durable_generation`; Redis is not the authority.
2. Confirm the mutating replica applies the committed generation immediately.
3. Watch every other replica. Its positive generation lag must return to zero within
   two watchdog intervals under normal database availability.
4. If lag persists, inspect the watchdog-running gauge and database-read error
   counter before restarting anything. Restore database connectivity or the worker;
   do not reset the durable generation or introduce a Redis counter.
5. Verify a recovery and full-clear increment with reason `generation_advanced`,
   then recheck representative allowed and denied permissions through an approved
   authenticated operator path.

### Missed PubSub event

1. Record the durable and applied generation values and the affected replica.
2. Confirm positive lag is visible before reconciliation.
3. Confirm the watchdog clears process permission snapshots, advances the applied
   generation, increments `generation_advanced`, and returns lag to zero.
4. If the applied generation does not advance, treat the replica as unsafe for
   authorization traffic until the database read or watchdog failure is corrected.

### Generation regression

1. Treat any negative lag as a release-blocking database restore or topology error.
2. Freeze role and permission mutations while comparing the authoritative database,
   replicas and restore point. Do not lower an in-process checkpoint to match the
   regressed database value.
3. Confirm the watchdog records `generation_regressed` and clears all process
   permission snapshots.
4. Restore monotonic database state through the owned migration/repair path and
   verify every replica returns to zero lag before reopening control-plane writes.

### Canonical role repair

1. Produce the read-only repair plan first and review the affected system roles.
2. Execute the approved `rbac repair-system-roles --apply` command against the
   authoritative database.
3. If the committed repair affects existing authorization snapshots, confirm
   the repair transaction commits one new durable generation. A no-op or a
   lost insert race must not claim another writer's change.
4. Confirm local invalidation occurs immediately and remote replicas recover to the
   committed generation without a restart.
5. Verify representative allow and deny decisions. Do not infer success only from
   the CLI exit code or Redis publication.

Single-role seed and grant paths order shared permission vocabulary keys by
(resource, action). Repair seeds only its own successful role insertion;
a concurrent winning persisted system role is left unchanged. The RBAC
generation reservation rejects non-integer SQLite storage and exhausted
counters, and returns the exact assigned successor. PostgreSQL reservation
uses nonwaiting table and singleton fences, matching the artifact grant trigger;
contention requires rollback and fresh admission, not a partial retry. GraphQL
role create/update/delete now preserve owner statement errors and use the bounded
rollback-only retry policy with fresh host prepare/lock and deadline checks.
Existing role IDs remain pinned; post-commit notification stays outside retry.
Host authority-port errors and uncertain commit are not replayed. SQLite regressions and
an opt-in PostgreSQL reservation contention test
(`RUSTOK_RBAC_TEST_DATABASE_URL`) are authored, not run. Full bulk
role/permission ordering and FK/trigger auditing remain open.

The current metrics and runbook make generation recovery observable, but a complete
single-incident chain from evaluator decision through relation state, cache snapshot,
durable generation and recovery action is still an open P1 in the implementation plan.

## Verification

- `cargo xtask module validate rbac`
- `cargo xtask module test rbac`
- `cargo test -p rustok-telemetry rbac_invalidation_metrics`
- `cargo test -p rustok-server --lib rbac_invalidation_generation`
- `node scripts/verify/verify-rbac-invalidation-observability.mjs`
- `node scripts/verify/verify-rbac-artifact-permission-tenant-integrity.mjs`
- targeted tests for permission resolution, tenant policy decisions and integration events

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Event flow contract](../../../../docs/architecture/event-flow-contract.md)
- [Verification plan](../../../../docs/verification/rbac-server-modules-verification-plan.md)

### Architectural contracts & principal classification

- [Explicit principal-kind contract](./explicit-principal-kind.md)
- [Owner role mutation contract](./owner-role-mutation-contract.md)

### Verification, evidence & incident harnesses

- [CLI live system-role repair propagation evidence](./cli-live-repair-propagation-evidence.md)
- [Policy incident trace packet](./policy-incident-trace.md)
- [PostgreSQL concurrency evidence harness](./postgres-concurrency-evidence.md)
- [Two-process durable recovery evidence](./two-process-durable-recovery-evidence.md)
- [Two-process Redis restart recovery evidence](./two-process-redis-restart-evidence.md)

### Durable generation DML integrity and transaction tail

Migration 000012 preserves the seeded permissions singleton and requires
strictly increasing integer updates; direct deletion, insertion, PostgreSQL
TRUNCATE and SQLite replacement fail closed. Forward-only recovery jumps
are permitted; normal owner reservation remains UPDATE RETURNING +1. Upgrade
validates existing storage under a writer fence. Privileged DDL and restore
are outside ordinary DML enforcement. User-admin update resolves response role
and tenant before reservation. The generation-bearing outbox insert and final
live credential deadline check remain after reservation and must not acquire
new domain identities. SQLite and opt-in PostgreSQL regressions are unrun.

### Raw relation writes and cached decisions

Unscoped persisted permission resolution validates each cache hit by exact
tenant/user grant-set equivalence in a database statement, then rechecks the
local token. A mismatch is a cache miss; read failure is an authority failure
even with a warm entry. A raw relation change no longer depends on the global
watermark or a pub/sub notification to be observed by the next such decision.
The extra database read is intentional; this does not revoke already admitted
request snapshots or make direct DML advance the recovery watermark. No new
global write locks or duplicate generation reservation are introduced.

### Permanent static authority identities

Migration 000013 backfills and retains an append-only UUID witness for each
existing user, role and permission and for every subsequent successful insert.
A deleted UUID cannot be reused, including SQLite same-ID REPLACE and alternate
text/blob UUID encodings; UUID updates are forbidden. Conflict-do-nothing and
rolled-back births consume nothing. Witnesses contain only kind and UUID, no
email, role slug or permission payload, and have no cascading foreign keys.
The SQLite witness table uses WITHOUT ROWID, preventing replacement through
an unrelated hidden row identity. PostgreSQL uses a nonwaiting per-identity
advisory fence for witness inserts.
The migration cannot reconstruct identities deleted before cutover, and does
not protect privileged DDL/restore, other authority kinds, or SQLite
natural-key REPLACE with a different UUID. Source regressions are unrun.

## Artifact catalog parent and batch lock order

Release admission visits permission keys and canonical localization keys in
sorted order, independent of caller ordering. Scoped projection orders its
release definition and translation reads by the same logical keys within a
backend. Migration 000016 guards installation-to-definition and both
translation FK edges with relation and exact parent KEY SHARE NOWAIT; metadata
updates with unchanged translation parent need no new lock. Existing artifact
grant/receipt guards also take parent relation locks NOWAIT. Contention rejects
the transaction with 55P03; retry only the complete owner transaction. SQLite
retains serialized writers and existing FKs. This is not a proof of arbitrary
bulk SQL or global deadlock freedom; PostgreSQL regression unrun.

The 000016 contention regression also installs the predecessor 000006
triggers and exercises the grant and operation receipt guards against each
actor, role and definition FK parent, with row/table contention and reentrant
owner admission. Cross-tenant FK substitution is rejected; source only.

## Artifact grant generation coupling

Migration 000017 advances the existing recovery watermark once per actual
artifact grant row birth, removal or authority-tuple update, independent of
entrypoint. Exact retries and unchanged authority tuples do not advance it.
PostgreSQL uses deferred constraint triggers and nonwaiting generation locks;
raw grant deletion and changed identity also take the affected roles in UUID
order with NOWAIT. SQLite uses immediate row triggers under its single writer.
The owner assignment no longer performs a second explicit reservation; event,
receipt, state and watermark roll back together on transaction failure. The
migration invalidates pre-cutover continuity receipts by advancing once and
requires a valid existing watermark. Drain old writers and deploy the matching
owner together. Direct static relation writes still do not advance the global
watermark; exact static reads/revisions remain separate. Arbitrary bulk SQL and
whole-transaction retry policy remain open. Regression sources are unrun.

## Process-local request snapshot revocation

Authenticated host scopes carry the cache epoch sampled before durable grant
resolution. A later targeted stripe or global advance, including delivery by
the existing invalidation listener/watchdog, prevents stale scope reuse or
unscoped fallback. Collisions are conservative denials; no per-actor map or new
persistent generation. Direct SQL without notification, remote commit latency
and in-flight transaction authority remain separate boundaries. Source tests unrun.

## Durable static request revision checks

The host retains the owner membership and role grant revision from admission.
Scope-aware service checks and GraphQL root admission compare it through the
single-statement owner reader. Raw relation changes and ABA cannot pass a
subsequent matching check merely because no notification arrived. WebSocket
revalidation also binds the revision. This adds a read, not a transaction
fence. Synchronous consumers retain their separate contract.
Source regressions are unrun.

Matching authenticated scopes also check live tenant/account, recovery
revision, direct session deadline/revocation, or exact OAuth app/registration
and consent lineage. Service OAuth checks the admitted grant ceiling against
current app grants and token scopes without needing a user membership.
Observation errors invalidate the scope terminally. Async service, root and
explicit nested read gates reuse this check; no raw bearer is retained.
Continuous module policy, signing-key authority and atomic effect fencing
remain separate. Source regressions unrun.

Matching scope reads used for fallback now distinguish absent scope from
invalid authority in one fallible accessor, including a recheck after
cloning. The host's synchronous REST permission gates also verify the
original CurrentUser deadline and local epoch, and intersect effective
checks with any matching scope ceiling. This does not add a durable read
to synchronous gates or fence subsequent effects. Source regressions unrun.

GraphQL HTTP/WS executor data carries the original admitted scope. The
security extension requires that marker to match live task-local authority
and immutable AuthContext, including effective ceiling and credential facts.
Missing or substituted scope denies rather than falling back. Nested query
fields recheck local validity; selected mutations retain receipt serialization.
Root durable revision reads are unchanged; no per-scalar SQL read is added.
Explicit fresh nested reads now use a host-composed shared GraphQL authority
handle: Blog moderation and User live role/can/tenant name/custom fields
revalidate before and after reads. Blog preserves the admitted actor and
requires moderate plus list rather than escalating to System. User mutation
role and tenant name serialize the owner receipt. Unmigrated nested
consumers and continuous credential/module authority remain separate
boundaries. Source regressions unrun.

## Revision-bound permission cache publication

The Moka adapter binds grants to the original coherent static actor revision.
Cache fills recheck the revision after publication and retry on drift even
without local invalidation. Bound hits compare exact vocabulary and revision
in one database statement; ABA is a miss. Missing subjects and legacy entries
retain exact checks. A revision-aware miss with absent subject evidence may
publish only empty grants: a nonempty read without a revision retries before
publication, even if the subject would again be absent afterward. Revision
storage counts toward the cache weight. Exact
validation is retained because database restore can reproduce counters; no
warm-hit speedup or commit-time fence is claimed. Source regressions unrun.

### Tenant-bound role labels and administrator continuity

Persisted role membership reads join the subject's tenant and the role's
tenant, excluding malformed cross-tenant links. The persisted role planner
distinguishes effective hierarchy rank from actual canonical system
`super_admin` membership: only the latter participates in last-active-admin
continuity. Custom-role replacement and status/removal checks use the same
membership predicate. Existing hierarchy and permission-ceiling checks remain.
Source regressions cover a malformed foreign membership and the distinction
between inferred rank and canonical membership; compilation and execution
remain pending.

### Artifact receipt contention

Owner assignment on PostgreSQL refuses a busy tenant/idempotency-key advisory
fence rather than waiting on the receipt unique index while holding role locks.
The lock is transaction-scoped and independent of actor and role. Definition
parent and receipt table admission are nonwaiting as well. The REST adapter
explicitly rolls back before returning `409 idempotency_in_progress`; callers
may retry the same key with fresh authority. The unique constraint still owns
receipt identity; direct SQL that bypasses the owner fence is not covered.
No commit error is automatically replayed. Source regressions are unrun.

### Administrator continuity under snapshot isolation (source only)

The continuity role is now a write fence, not just a row lock: the canonical
super_admin row receives an exact no-op update before counting remaining
active members. PostgreSQL REPEATABLE READ/SERIALIZABLE transactions whose
snapshot predates a committed continuity check cannot both authorize removals
from the old active set; serialization failure requires whole-transaction
retry. The no-op does not change grant_revision or the role's timestamp.
SQLite retains its existing writer fence. Persisted role planners and the
standalone status/removal continuity boundary reserve the existing target
membership, destination and continuity roles in canonical UUID order before
reading hierarchy or membership facts. Missing custom destination fails
closed. Callers must retain the transaction through the effect. Raw user
status DML bypassing the owner, privileged database maintenance, and complete
cross-owner lock ordering are not covered by this change. PostgreSQL
regression source authored; compilation and execution remain pending.

### Continuous static module-policy observation

Composed JWT admissions retain the owner-issued tenant policy identity,
composition revision and availability alongside the original grant ceiling.
Scope-aware async checks re-observe those facts without the TTL cache;
drift or unavailable evidence terminally invalidates the matching scope.
Final admission and existing WebSocket lease revalidation do not bootstrap
missing composition. This is observation-based, not a commit-time fence;
undetected tenant lifecycle ABA, raw-DML continuity and sync-only consumers
remain open. Source regression authored; compilation and execution pending.

### Permanent artifact operation key witnesses

Migration m20261127_000206_preserve_artifact_operation_witnesses retains the tenant/key identity independently of
cleanup of the detailed receipt and its actor/role/definition FKs. Existing
receipts are backfilled under a writer fence; previously deleted receipts
cannot be reconstructed. Receipt updates and truncation are rejected, while
ordinary cleanup DELETE remains legal. PostgreSQL raw receipt inserts/deletes
participate in the same nonwaiting advisory fence as owner writers. SQLite
uses canonical UUID witnesses, serialized writes and explicit ABORT guards
against REPLACE/IGNORE. A live receipt can still answer an exact retry; a
retired key rejects with HTTP 409 idempotency_receipt_retired, not a new effect.
Witnesses are permanent and contain the exact tenant/key, not full payload or
response; do not put secrets in idempotency keys. No automatic commit replay.
Privileged DDL/restore and previously lost history remain outside this guarantee.

### Lifecycle cursor continuity integrity

The tenant `module.lifecycle` cursor is durable security authority, not
disposable consumer progress. The Modules migration prevents ordinary DML
from deleting, replacing, moving or resetting that cursor, including SQLite
implicit REPLACE deletes through rowid or alternate UUID representations.
New lifecycle cursors start at zero with no digest; existing legacy
sequence-zero digests are preserved. An advance requires the exact next
integer sequence and a different valid digest. Exact no-op writes remain
legal. PostgreSQL TRUNCATE is rejected for the cursor table because it would
bypass per-row history; other consumers retain normal row mutation semantics.
Install with old writers drained, validate existing rows and reject
inconsistent identity/sequence evidence rather than reconstruct history.
Lifecycle cursors persist even after tenant removal; do not reuse tenant IDs.
The owner cursor remains the source of truth. No new generation table.
This does not couple direct tenant_modules edits to the cursor or prove that
a manually supplied successor digest describes the current policy; the
existing content observation remains necessary. Privileged DDL, restores
and pre-cutover lost history are not covered. Source regressions unrun.
