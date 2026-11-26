# Forum topic restoration may replay its archived solution snapshot

- Date: 2026-11-26
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-forum
- Extends: [Reply restore parent admission](./2026-11-26-forum-reply-restore-parent-admission.md)
- Supersedes: None
- Superseded by: None

## Context

A topic archived before deletion can retain an accepted-solution relation. Topic
deletion captures that relation in its immutable delete snapshot and removes it.
`TopicService::restore` restores the topic's original status before replaying the
solution. The ordinary solution-target guard therefore rejects restoration when
the snapshot's original status was archived, even though restoring a topic without
a solution succeeds. Reopening a deleted topic is not an available prerequisite.

## Decision

The topic lifecycle inverse restores the original archived status and its exact
historical solution target. Permit solution INSERT beneath an archived topic only
while its matching topic/reply delete snapshots remain available in the transaction:

- the topic is live, archived, and its snapshot records the previous archived status;
- the exact tenant/topic snapshot names this solution reply;
- the corresponding thread reply snapshot records an approved reply;
- the actual reply is live, approved and still belongs to that tenant/topic.

The owner replays original marker metadata, then removes the snapshots before
commit. No ordinary archived solution INSERT or target UPDATE is admitted. This is
not standalone reply restoration: that command still requires explicit parent
reopen, as defined by the extended decision.

## Sources of truth and ownership

Forum owns the topic/reply tombstones, delete snapshots, solution relation and
restore transaction. Category/member counters and search projections are derived.
Both transports delegate to `TopicService::restore`; no client payload or new API
is required.

## Invariants

### Allowed states

A deleted thread can restore exactly its snapshot status, replies and historical
solution, including a thread that was already archived before deletion. Archived
threads contribute no category/member counts until explicitly reopened.

### Forbidden states

A fresh solution, another reply or tenant, a deleted or non-approved reply, or a
solution target UPDATE cannot use the archived snapshot replay exception.
Snapshots are consumed only after a complete successful restore. Restoration
must not silently open the topic or discard accepted-solution metadata.

## Non-goals

Changing ordinary solution moderation, standalone reply parent admission, topic
merge policy, or archived visibility. No generic bypass flag or session setting.

## Data, transaction, and concurrency boundary

Migration 47 replaces the existing solution-target validator with an INSERT-only,
snapshot-bound exception on SQLite and PostgreSQL. Existing solution scope locks,
uniqueness, tenant keys and tombstone guards remain active. The topic restoration
transaction already serializes deletion/restoration and category policy; it must
hold the topic row before the solution scope and child restoration writes.
Snapshot replay, aggregates, outbox and snapshot removal commit together.

## Context dimensions

Tenant and topic manage permission are mandatory. Locale/channel do not alter
snapshot identity. Original solution marker/time are replayed by the owner. Trace
context is unchanged; currency and display timezone are not applicable.

## Events and projections

Retain existing topic/category projection invalidations. Restoring to archived
must not publish a first-publication event or re-enter category/member aggregates.
Reopening later applies the existing single contribution. No new event type.

## Failure semantics

Invalid/missing snapshot references fail closed. Failed restoration rolls back all
source/relation/projection/outbox changes and preserves snapshots for retry.
Ordinary archived-target solution writes continue to fail under the database guard;
standalone reply restore continues returning `FORUM_TOPIC_ARCHIVED` from its owner.

## Migration and cutover

Apply migration 47 before using the restored owner. No data rewrite or backfill.
Down restores the strict pre-47 validator; it does not remove a historically
restored archived solution, which is already a valid state produced by archival.
After rollback, deleting/restoring that archived thread with its solution again
requires the corrected migration. No parallel storage or compatibility flags.

## Alternatives considered

- Open the topic implicitly: violates snapshot status and may expose content.
- Omit or defer its solution: violates full snapshot fidelity or needs new state.
- Reject these topic restores: permanently strands valid deletion snapshots.
- Permit arbitrary archived solution writes: weakens the normal moderation invariant.

## Verification

SQL guard tests must reproduce the old failure and test exact replay, tenant/topic/
reply mismatches, missing or non-approved snapshots, deleted sources, target UPDATE,
snapshot consumption, and down/up behavior on SQLite and PostgreSQL. Shared service
regressions must delete/restore archived topics with and without a solution, preserve
status/marker metadata, retain zero public aggregates, and reopen exactly once.
Seven extracted-SQL scenarios pass on SQLite and PostgreSQL 17.6, including 15
source/snapshot mismatch subcases, INSERT/UPDATE separation and down/up templates.
SQL-only evidence is not full Rust migration/service or contention evidence;
shared service regressions remain authored but unexecuted without Cargo.

## Consequences

Historical inverse operations are explicit and bounded by canonical snapshots,
not an unrestricted archived-write capability. The schema remains unchanged;
the new guard is more selective than simply removing the archived predicate.
Rust runtime and full migration-chain validation remain required before integration.
