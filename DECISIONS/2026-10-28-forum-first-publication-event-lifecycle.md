# Forum first-publication event lifecycle uses its immutable owner journal

- Date: 2026-10-28
- Decision status: Accepted
- Implementation status: In progress (source-ready, runtime unverified)
- Owners: `rustok-forum` (`services/topic_inline.rs`, `services/reply_owner_inline.rs`, `services/moderation_owner.rs`, `forum_domain_events`), `rustok-notifications` (consumer only)
- Supersedes: the "no approval notification" non-goal and the unconditional pending-topic `ForumTopicCreated` event timing in [the topic pre-moderation ADR](./2026-10-09-forum-topic-pre-moderation.md)
- Extends: [reply pre-moderation](./2026-10-09-forum-reply-pre-moderation-setting.md) and [notifications outbox claims](./2026-10-11-notifications-outbox-intake-decoder-claims.md)

## Problem and contract

The old topic creation path emitted `ForumTopicCreated` while the subject was
`Pending`. Notifications reauthorized visibility at consumption and could
discard the event before approval; moderator approval did not replay it.
The old moderator reopen path instead emitted `ForumTopicCreated` for an
`Archived -> Open` restore, even for previously published topics. Reply
`Rejected -> Approved` also emitted duplicate `ForumTopicReplied` events.

A lifecycle status is not proof of first-ever publication. For example, a
`Pending -> Archived -> Open` topic is first published on its archive
restore, whereas an `Open -> Archived -> Open` topic was already public.

## Decision and source of truth

- The **Forum-owned append-only** `forum_domain_events` journal, written by
  row-level creation and status triggers in the same transaction, is the
  durable historical source for first-publication detection. The **host**
  `sys_events` outbox is a delivery queue with pruning, never a historical
  publication flag. There is no second mutable `ever_published` field.
- A topic has appeared publicly if the journal records its creation with
  status `Open`/`Closed` or a prior status change to `Open`.
  `Pending` creation and `Pending -> Archived` do not qualify.
- A reply has appeared publicly if the journal records creation with status
  `Approved` or any prior status change to `Approved`. `Rejected`,
  `Flagged` and `Hidden` do not independently mean first publication.
- Direct `Open` topic creation and direct `Approved` reply creation
  emit their notification-source event in their insertion transaction.
  Held topic creation does **not** emit `ForumTopicCreated`. The moderation
  owner emits it if and only if a transition to `Open` is the topic's
  first public appearance. The reply moderation owner similarly publishes
  `ForumTopicReplied` only on the reply's first `Approved` appearance.
- The owner reads publication history **before** changing the status so the
  status-change trigger does not contaminate the first-publication test.
  The topic row is locked across the check, mutation, journal trigger and
  transactional outbox event. Reply moderation locks the category writer
  scope and then the topic row to match topic archive and reply creation.
- Status-changed events and Search/category invalidations are still emitted
  for every actual lifecycle transition. Notification fanout separately
  evaluates recipient subscription and current effective read audience;
  publication events do not guarantee eventual delivery to a recipient who
  is no longer authorized.

## Privacy, concurrency, failure and migration

All historical queries are scoped to the tenant, aggregate type and id, and
do not expose the private journal to public transports. A database or journal
read failure aborts the mutation; there is no permissive "first publication"
guess. PostgreSQL and SQLite use the same semantic predicates, with their
native JSON extraction syntax. The existing tenant/aggregate/sequence index
supports bounded existence checks. Concurrent moderation of the same topic
is serialized by the existing row lock; the status machine also rejects an
already-satisfied transition.

This is a source-only pre-release change with no new migration: the Forum
creation/status journal is installed before the topic pre-moderation
migration. Historical unindexed/outbox notifications are **not** synthesized
or backfilled, and old notification delivery cannot be inferred from outbox
retention. In production, rollout must verify pending-created topic behavior
across both Notifications intake and the database's historical event ledger.

## Verification and acceptance

- SQLite integration regressions cover direct publication, pending approval,
  pending rejection then first-ever archive restore, second archive/reopen,
  and reply approval/rejection/reapproval/hide/reapproval.
- Required external verification: compile and run the scoped Forum tests,
  repeat equivalent tests on PostgreSQL, exercise concurrent moderators and
  worker restarts, inspect exactly-one publication-source event and recipient
  delivery, and confirm retry/rollback paths.
- This ADR is **accepted architecture, not runtime verification**. The owning
  implementation plan remains `in_progress` until the maintainer's checks
  and explicit integration authorization.
