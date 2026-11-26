# Forum reply restoration requires a non-archived parent

- Date: 2026-11-26
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-forum
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

`ReplyService::restore` replays a reply's immutable delete snapshot, including its
accepted-solution relation. The database rejects inserting a solution beneath an
archived topic. Without parent lifecycle admission, restoring an accepted reply
there fails with a database error, while restoring an ordinary reply can succeed.
A restore command must not depend on whether historical solution metadata happens
to be present, nor bypass the archived parent's content-write boundary.

## Decision

Restoring any reply beneath an archived topic returns `FORUM_TOPIC_ARCHIVED`.
A moderator must explicitly reopen the topic before restoring its replies.
Do not weaken the solution-target trigger, silently discard solution metadata,
or implicitly reopen a topic as part of reply restoration.

## Sources of truth and ownership

`rustok-forum::ReplyService` owns restoration. The locked topic row determines
parent lifecycle; the reply tombstone and its delete snapshot determine restoration
content and historical solution metadata. Category/member counts, last-reply state,
search invalidations and transport responses are derived representations.
REST and GraphQL delegate to the same owner and introduce no alternate admission.

## Invariants

### Allowed states

- A deleted reply under a live, non-archived topic may be restored subject to the
  existing permission, category, merge and solution-conflict checks.
- A topic may be explicitly reopened, then its deleted reply restored exactly once.

### Forbidden states

- Restoration beneath an archived or deleted parent.
- Partial restoration, lost delete snapshots, or changed revisions/outbox records
  after rejected parent admission.
- Different archived-parent behavior for ordinary and previously accepted replies.

## Non-goals

Changing deletion permissions, category restoration, author edit windows, or the
existing behavior of closed/locked topics. No new UI command or implicit lifecycle
transition is introduced.

## Data, transaction, and concurrency boundary

Recheck the parent after acquiring the tenant category-tree fence and topic row
lock, before the first restoration write. Successful restoration uses the existing
category -> topic -> solution scope -> reply lock order. Keep source restoration,
solution replay, aggregate updates, outbox writes and snapshot removal in one
transaction. Existing immutable-tombstone and active-solution database guards stay
unchanged. A repeated restore retains the existing unavailable response.

## Context dimensions

Tenant and principal/manage permissions are mandatory. Locale and channel do not
change parent lifecycle admission; reply body/revision locales remain unchanged.
Trace context is unchanged. Timezone and currency are not applicable.

## Events and projections

Rejected restoration publishes no event or invalidation. Successful restoration
retains existing status-change and category/topic projection events in the owner
transaction; no new event type or backfill is needed.

## Failure semantics

An archived parent produces the typed domain error `FORUM_TOPIC_ARCHIVED`, HTTP
409 through the shared REST mapping. GraphQL receives the same owner error.
The caller can explicitly reopen the parent and retry. Storage/policy failures
continue to propagate; rejection must preserve the snapshot for a later retry.

## Migration and cutover

No schema migration or data rewrite. Both transports immediately inherit the owner
check. The intentional compatibility change is rejection of ordinary-reply restores
that previously slipped through beneath archived topics; accepted-reply restores
now fail with a domain conflict instead of an incidental database error.

## Alternatives considered

- Permit all archived-parent restores: conflicts with the active-solution insertion
  invariant and expands archived content mutation semantics.
- Restore only the body and omit a historical solution: loses snapshot fidelity.
- Implicitly reopen the parent: changes a second resource's lifecycle without an
  explicit moderator command.

## Verification

The shared SQLite/PostgreSQL service regression must cover ordinary and accepted
replies: archive, delete, reject restoration with no state/history/outbox changes,
reopen, restore, and reject a second restoration without double-counting.
SQL-only checks may prove the existing solution guard rejects archived targets;
they do not substitute for executing the owner tests or lock-contention validation.
Runtime Rust validation remains open while Cargo is unavailable.

## Consequences

Moderators perform two explicit commands rather than receiving success for some
replies and a storage error for others. There is no schema or client payload change.
The extra lifecycle admission reuses a lock already acquired by restoration.
