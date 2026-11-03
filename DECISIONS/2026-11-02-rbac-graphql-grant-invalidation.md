# Durable invalidation for GraphQL role-grant edits

- Date: 2026-11-02
- Decision status: Accepted
- Implementation status: In progress
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
transaction. The generation row serializes reservations. Post-commit local
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
