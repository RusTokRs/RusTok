# FORUM-20BK — owner-transactional Search projection invalidation

`FORUM-20BK` closes the canonical mutation-to-Search gap left by the initial
Forum projection consumer. On the supported PostgreSQL Search runtime, Forum
owner transactions now insert durable `index.reindex_requested` events for
every canonical mutation that can change an exact public category or topic
document.

The former documentation label `search.reindex_requested` was incorrect. The
registered `DomainEvent::ReindexRequested` type has always emitted
`index.reindex_requested`; this correction changes no runtime payload.

## Existing event contract

The slice reuses the registered root event:

```text
DomainEvent::ReindexRequested { target_type, target_id }
```

No new root event variant, event schema or migration is added. The existing
Search ingestion handler from `FORUM-20BJ` already maps:

- `forum` with no ID to a complete Forum projection rebuild;
- `forum_category` with an ID to one category refresh;
- `forum_topic` with an ID to one topic refresh.

`TransactionalEventBus::publish_root_in_tx` is the new transaction-only outbox
entry point. It preserves both `DomainEvent::validate()` and registered envelope
schema validation, then writes to the canonical `sys_events` outbox through the
owner transaction. Domain services therefore do not create a second database
connection and do not reproduce the event envelope in SQL.

The Search-owned Forum projector is PostgreSQL-only. Direct invalidation helpers
therefore persist to the outbox on PostgreSQL and perform event validation only
on SQLite or another unsupported projector backend. This keeps standalone Forum
SQLite domain fixtures independent from an unused Search/outbox schema rather
than silently creating runtime tables. Existing owner paths that already carry
a configured event bus keep their established transport behavior.

## Invalidation scopes

### Full Forum scope

A category affects more than its own document. Category copy is used as the
subtitle of topic results, and hierarchy, lifecycle, base visibility and richer
audience layers can change inherited visibility for an entire subtree.
Therefore these canonical writes publish `target_type = "forum"`:

- category create and category content or locale update;
- category move and sibling reorder;
- category subtree archive, delete-as-archive and restore;
- base category visibility replacement;
- richer inherited category audience replacement.

Lifecycle no-ops do not publish an event because no projection state changed.
Posting, reply-create and moderation authorization policies are not discovery
policies and do not invalidate Search documents.

### Category target

Category documents include topic and approved-reply counters. These canonical
writes publish `target_type = "forum_category"` with the owning category ID:

- topic create;
- topic delete;
- approved reply create or delete;
- moderation transitions into or out of approved reply state.

A reply transition that does not change public counters does not publish a
category invalidation. Existing reply/topic lifecycle events still refresh the
topic document itself.

### Topic target

Topic-local projected state publishes `target_type = "forum_topic"` with the
topic ID:

- content, locale, metadata, tags or route-channel update;
- delete, including an already-archived topic where no new status event exists;
- lock or unlock;
- solution mark or clear;
- topic-local audience replacement.

Existing topic create, status and pin events already refresh the topic. Duplicate
refreshes are acceptable when an owner command also changes another projection
scope.

## Public owner sealing

The raw category and topic mutation implementations remain crate-private.
Category content, placement and lifecycle writes are routed through private
same-module transactional owner wrappers. Public category and topic audience
service names now alias wrappers that delegate reads but own policy replacement.

Moderation is exposed through a public facade. Its transactional owner and the
legacy delegate are private, so callers cannot construct a bypass around reply
counter, lock or solution invalidation.

This owner composition preserves REST, GraphQL and native call signatures while
keeping read-only internal composers free to use private read implementations.

## Delivery and idempotency

The canonical PostgreSQL outbox is at-least-once. A command may legitimately
produce both an existing lifecycle event and one `ReindexRequested` event, and
dispatcher retries can redeliver either. Search target refresh and Forum scope
replacement are idempotent: they derive current exact owner state rather than
applying a projection delta.

On PostgreSQL, an owner write and its invalidation either commit together or
roll back together. A later Search failure leaves the outbox event retryable.
The broader multi-source full Search rebuild is still not one transaction across
all source modules; that limitation remains downstream.

## Compatibility and remaining work

No route, GraphQL field, public DTO, workspace dependency, lockfile or migration
changes are introduced. Forum remains usable without an active Search listener;
its PostgreSQL outbox events simply have no matching consumer when Search is
absent. SQLite Forum domain fixtures do not require an outbox migration because
there is no supported SQLite Forum Search projector.

Forum categories and topics are not currently registered as translation-control
plane targets. If that integration is added later, its owner transaction must
publish the same category or topic invalidation rather than writing translations
behind these canonical owners.

`implementation-plan.md`, `CRATE_API.md` and the Search owner plan remain
conflict-sensitive synchronization debt. Tests, Cargo commands, formatting,
verifiers, workflows and CI were not run by the implementation agent. Suggested
maintainer commands are recorded in
`contracts/forum-projection-invalidation.json`.
