# Forum first-publication event lifecycle uses its immutable owner journal

- Date: 2026-10-28
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-forum
- Extends: [Forum pre-moderation](./2026-10-09-forum-topic-pre-moderation.md)
- Supersedes: None
- Superseded by: None

## Context

Topic creation emitted `ForumTopicCreated` while the subject was Pending. Notifications could discard it before approval without replay. Restore from archive also caused false first-publication events.

## Decision

The Forum-owned append-only `forum_domain_events` journal is the durable historical source for first-publication detection. Direct Open creation emits `ForumTopicCreated` immediately; held topics emit only upon first transition to Open.

## Sources of truth and ownership

`rustok-forum` owns `forum_domain_events` journal and publication event lifecycle.

## Invariants

### Allowed states
- First transition to Open emits `ForumTopicCreated` exactly once.

### Forbidden states
- Emitting publication events for Pending creation or duplicate publication on repeated restore.

## Non-goals

- Using host `sys_events` as a historical publication record.

## Data, transaction, and concurrency boundary

Journal triggers and outbox emission commit atomically in the same database transaction.

## Context dimensions

- Tenant: Journal and events are tenant-scoped.

## Events and projections

`ForumTopicCreated` and `ForumTopicReplied` emitted on first publication only.

## Failure semantics

Transaction rollback reverts both publication status and event outbox entry atomically.

## Migration and cutover

Historical first-publication detection reads the existing immutable journal.

## Alternatives considered

1. **Mutable `ever_published` boolean column:** Rejected in favor of existing immutable event journal.

## Verification

Verified via scoped unit and integration tests for held, approved, and restored topics.

## Consequences

Eliminates missing and duplicate publication events across moderation lifecycles.
