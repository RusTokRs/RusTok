---
id: doc://crates/rustok-forum/docs/forum-12c-mention-events.md
kind: implementation_record
language: en
status: delivered
owners:
  - rustok-forum
  - rustok-notifications-program
last_reviewed: 2026-07-22
---

# FORUM-12C mention events and relation reads

FORUM-12C turns the immutable added-target diff produced by FORUM-12B1/B2 into
versioned owner events. Forum still does not call Notifications synchronously.

## Event contract

`ForumMentionEvent` is a sealed `rustok-events` family with two v1 events:

- `forum.mention.user_added` carries source kind, source ID, relation revision,
  locale and resolved user ID;
- `forum.mention.audience_added` carries the same source identity plus a typed
  audience token.

Payloads contain target identity only. They do not contain email, phone, contact
address, profile handle snapshot or rendered source text.

## Atomic publication

After `MentionRelationService::persist_in_tx` has appended a new relation
revision, it publishes only `added_user_ids` and `added_audiences`. Identical
replay returns before publication, while removed and unchanged targets emit no
event.

Each event is written through `TransactionalEventBus` to the canonical outbox.
The returned envelope UUID is reused as `forum_domain_events.event_id`, so the
outbox record and Forum-owned append-only journal row have one shared identity
inside the same topic/reply transaction.

PostgreSQL and SQLite journal constraints now accept the two mention event types.
The SQLite migration preserves sequence numbers and event IDs while rebuilding
the constrained append-only table.

## Bounded owner read

`ForumRelationReadService` returns either the latest relation snapshot or one
exact positive revision for a tenant, source and locale. Reads require the
canonical topic/reply read permission, return a safe
`FORUM_RELATION_REVISION_UNAVAILABLE` result for invalid or absent identities and
fail closed if persisted rows exceed the 32-mention or 32-quote contract.

The public response includes resolved user IDs, typed audiences and revision-
bound quote identities. It deliberately excludes handle snapshots and projection
fingerprints. No REST or GraphQL endpoint is added in this slice.

## Remaining boundary

Quote command DTOs, transport adapters for quote input, Notifications fan-out,
source/target visibility rechecks and executable PostgreSQL/concurrency/delivery
evidence remain separate work. Maintainer verification was not executed while
publishing this slice.
