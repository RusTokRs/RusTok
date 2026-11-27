# Durable invalidation for GraphQL role-grant edits

- Date: 2026-11-02
- Decision status: Accepted
- Implementation status: Implemented
- Owners: rustok-rbac (grant write and generation), apps/server (cache integration)
- Extends: 2026-02-26-rbac-relation-source-of-truth-cutover
- Supersedes: None
- Superseded by: None

## Context

The owner GraphQL `update_role` rewrites persisted role-permission grants, but
previously committed without advancing the durable permission invalidation
generation. A best-effort post-commit membership lookup could fail or be
cancelled, leaving caches in this or other replicas unaware of the grant edit.
The generation watchdog cannot recover a change that never advanced the marker.

## Decision

When `update_role` receives a permissions replacement, reserve one durable RBAC
permission generation in the same transaction as the role and grant writes.
Reservation failure aborts the transaction; an omitted permissions field does not
reserve a generation. A supplied but identical set may still be rewritten and
therefore conservatively advances the marker. No new schema is required.

After commit the owner offers the committed generation to a host callback. The
server clears its local permission cache and attempts an all-users invalidation
publication using that generation. Publication is a latency optimization, not the
authoritative record: existing generation watchers reconcile a missed signal.
Host callback failure must not change a successful committed mutation into an
error that implies rollback. A caller still needs idempotency-aware recovery if
execution is cancelled or the response is lost after commit.

## Sources of truth and ownership

`rustok-rbac` owns persisted role grants and the durable generation reservation.
The server owns process-local cache clearing, best-effort fan-out and watchdogs.
The generation is a recovery watermark, not a per-role event or authorization
snapshot. Other hosts must ensure their cache observes the durable generation or
implement equivalent invalidation; the callback has a compatibility default that
delegates to the older membership invalidation hook.

## Invariants

### Allowed states

A committed permissions replacement has a committed generation in the same
transaction. A metadata-only edit need not advance it. Any fast invalidation can
be lost without losing the database watermark.

### Forbidden states

A grant replacement must not commit after generation reservation fails, or use
an uncommitted generation for post-commit notification. A failed notification
must not be presented as though the prior database mutation rolled back.

## Non-goals

This does not implement a durable outbox event, synchronous fleet-wide
revocation, transaction-spanning authorization fencing, coherent concurrent
role-edit validation, or replay-safe mutation receipts. Existing task-local
scope and in-flight snapshot limitations remain. Watchdog recovery depends on
running and healthy readers, not a fixed latency guarantee.

## Data, transaction, and concurrency boundary

The role write, grant replacement, and generation reservation share the owner
transaction. The generation row serializes reservations. On the inspected
role-edit, user-admin, committed role replacement and artifact-assignment
paths it is a late write fence, after tenant/credential/user/role and
grant/receipt identities; subsequent outbox or deadline checks must not
acquire a new earlier-ranked identity. PostgreSQL reserves ROW EXCLUSIVE table
and FOR UPDATE singleton fences with NOWAIT, consistent with the artifact grant
trigger, so a late reservation cannot wait on either lock. Contention fails the
owner transaction; any retry must start with fresh authorization and must not
replay an uncertain commit. The owner reservation preserves the driver DbErr
through the host write adapter; the standalone committed replacement uses the
shared rollback-only retry contract. Request-bound administration still needs
fresh credential admission before it can safely retry. The user deactivation
host path now retries typed reservation conflicts before event publication,
repeating the original ceiling and locked live authority checks. Other admin
paths are not uniformly covered. Admin update now also retries typed generation
reservation conflicts, keeping its original metadata conflict baseline and
recomputing locked plans. Admin create retries typed insertion/assignment
failures only, before publication. GraphQL role create/update/delete now retain
owner statement DbErr until rollback/retry admission, re-enter host authority on
each attempt, and pin existing target UUID and slug. Host authority failures
and commit errors are not automatically retried; notify only after commit.
Role creation uniqueness uses the typed constraint classification rather than
formatted text. Reserve by UPDATE RETURNING, reject
invalid or exhausted integer storage without mutating it, and publish only
the assigned committed generation. Do not use this global watermark as a
per-role authorization fence. The seeded singleton is permanent under ordinary
SQL DML; updates must strictly increase its integer generation. Explicit
forward-only recovery may skip values; ordinary reservation advances by one.
Corrupt or missing predecessor storage requires offline recovery. Privileged
DDL and restoration remain separate boundaries. Single-role grant materialization uses the
same ordered (resource, action) vocabulary keys in control-plane, bootstrap
and repair writers. This is not a complete multi-role/bulk lock-order proof.
Post-commit local
clearing and optional fan-out occur outside it; process crashes can occur before
those actions, leaving the durable generation for eventual reconciliation.

## Context dimensions

The role edit is tenant-scoped. The generation and all-users invalidation are
global to the RBAC permission-cache namespace. Actor, channel, locale and trace
context do not form separate generation counters.

## Events and projections

No new domain event or transactional outbox row is created. The best-effort cache
message carries the committed generation, while the database marker is the
recovery source for watchers. In-flight request and WebSocket snapshots are not
synchronously revoked by a cache clear.

## Failure semantics

Reservation/storage failure returns a sanitized error before commit and rolls
back writes. Fast-path failure after commit is logged and is not reclassified as
an uncommitted mutation; the generation remains available to watchers. A broken
or stalled watchdog can delay recovery without an upper bound.

## Migration and cutover

The existing `rbac_invalidation_state` migration must be installed before grant
edits. Hosts may override the new generation-aware callback; the default
continues to call the existing membership hook for source compatibility. Clients
see no new field, but unavailable generation storage now rejects grant edits.

## Alternatives considered

- Membership-only invalidation: rejected because lookup, process or fan-out can fail.
- Report an error after commit: rejected because it can misrepresent committed work.
- Rely on cache TTL: rejected because it is not a durable recovery signal and
  does not bound already-created request snapshots.
- Transactional outbox: desirable for richer durable events, but not necessary
  for the existing global-generation reconciliation and not implemented here.

## Verification

Test generation commit and rollback together with role/grant writes, absent or
exhausted generation state, metadata-only updates, generation-aware callback,
legacy callback fallback and callback failure after commit. Exercise real
multi-replica watchers and concurrent PostgreSQL role edits before rollout; unit
and source tests alone do not prove distributed delivery or a revocation fence.

## Consequences

Grant-edit operations now depend on generation storage and can cause global cache
clears. Recovery is durable but eventual, with no promise of immediate denial
for already admitted operations or snapshots.

## Artifact grant database coupling extension (2026-11-27)

Migration 000017 makes the existing artifact grant relation itself responsible
for advancing the same recovery watermark on actual row births, removals and
authority-tuple changes. PostgreSQL advances through deferred constraint
triggers at commit by default and refuses a contended watermark without
waiting; SQLite advances inside the serialized write. Raw grant DELETE and
changed identity require nonwaiting affected-role fences. The artifact owner
no longer separately reserves a generation; its receipt and event share the
transaction, including a commit-time failure. Direct SQL does not synthesize
owner events. This is per changed row, not a new counter or universal
once-per-transaction protocol. Exact state no-ops and replays advance nothing.
Drain old writers and advance once at cutover to invalidate old continuity
receipts. Direct static relation generation coupling and general safe retry
remain open; source regressions authored, unrun.

## Artifact operation receipt lock-order refinement

Owner assignment on PostgreSQL uses nonwaiting definition relation/row and
receipt relation admission, followed by a transaction-scoped try-advisory
fence keyed by tenant and idempotency key before reading or inserting the
receipt. This avoids owner-to-owner unique-index waits under different role
locks. A busy key rejects the transaction without reporting an idempotency
payload mismatch. The owner and HTTP transaction adapters explicitly roll back
on pre-commit errors. The unique constraint remains authoritative; raw receipt
DML that bypasses this advisory fence and receipt retention are separate gaps.
No uncertain commit is replayed. Regression sources are unrun.

## Artifact operation witness retention

A separate permanent key witness, with no FK to the deletable detailed receipt,
consumes each tenant/idempotency-key on actual receipt insertion in the same
transaction. Historical receipts are backfilled; history deleted before cutover
is unavailable. Detailed receipt updates are forbidden; cleanup may delete them
but cannot release the key. Retired keys fail closed (HTTP 409), rather than
reapply an old grant. PostgreSQL guards raw receipt writers with the existing
try-advisory namespace; SQLite explicitly rejects conflict-algorithm bypasses.
The witness is not a full response receipt and is not a commit-recovery API.
It retains exact keys permanently; keys must not contain secrets. Drain old
writers before applying the additive migration, which cannot be downgraded
safely. Privileged DDL/restore is outside ordinary DML guarantees.

## Static relation durable invalidation (2026-11-27)

Migration m20261127_000207_couple_static_relation_generation advances the existing global recovery watermark for actual
membership and static grant inserts/deletes, authority-tuple updates and
role tenant/slug/system identity updates. Cascades use the same row triggers;
metadata and exact tuple updates do not advance it. PostgreSQL defers the
increments to commit by default and refuses contested watermark table/row
locks with NOWAIT. SQLite advances inside the serialized statement. Existing
membership and grant revision witnesses are unchanged. Cutover advances once.
Standalone committed replacement and Auth admin role updates flush only the
named deferred static constraints at the SQL-only tail (SQLite already
advanced) and use the resulting generation instead of reserving an extra
marker. Status-only updates still reserve explicitly. Existing GraphQL grant
replacement and repair notification markers remain conservative additional
advances pending consumer migration. Generation is a recovery watermark, not
an operation counter; gaps require namespace-wide reconciliation. Drain old
writers before applying. No uncertain commit retry. Source tests unrun.

SQLite additionally guards hidden rowid changes and insert collisions on static
relation tables. With recursive triggers disabled, REPLACE could otherwise
remove a different authority owner's relation without its DELETE-trigger history.
The guard rejects all rowid aliases and uses explicit ABORT; normal UUID-keyed
writes remain unchanged. Migration m20261127_000208_fence_static_relation_rowids is forward-only; source regression
unrun.
