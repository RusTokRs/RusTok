# Forum owner-transactional projection invalidation

## Owner and current runtime contract

`src/services/projection_invalidation.rs` owns Forum projection invalidation.
Canonical category, topic, reply and moderation mutations invoke this owner in the
same owner transaction as their source changes. Transports do not publish a second
invalidation or infer successful writes from client state.

The root event is `rustok_events::DomainEvent::ReindexRequested`, whose registered
event type is `index.reindex_requested` (not `search.reindex_requested`). Existing
targets are unchanged:

- **Full Forum scope** (`forum`, no target ID): category copy, hierarchy, lifecycle
  and inherited audience/visibility changes can affect descendants.
- **Category target** (`forum_category`, category ID): topic and public-reply counts.
- **Topic target** (`forum_topic`, topic ID): content, translations, tags, channels,
  lock/solution/local audience state and child reply lifecycle/content.

Topic-target processing rebuilds the atomic Forum scope so topic and child reply
projections remain aligned; a target does not authorize a Search disclosure.
Search retains its own exact audience eligibility boundary.

## Persistence and delivery

The Forum Search projection driver is PostgreSQL-only. Direct owner invalidations
on PostgreSQL persist through `TransactionalEventBus::publish_root_in_tx_with_envelope_id`
and the canonical `sys_events` outbox, with no second connection or hand-built
envelope. In the current implementation they also publish the existing typed
projection event and record its owner revision in the same transaction. See the
[versioned publisher contract](./forum-23b2g2b3b2-versioned-invalidation-publisher.md)
for that later extension; the FORUM-20BK JSON records its original cutover scope.

On non-PostgreSQL backends, **direct** invalidation is validation only: the root
event is checked but no unsupported Search driver or owner-revision ledger is
invented. Event-bus-backed owner paths retain their configured event-bus behavior;
this direct-path exception must not be generalized to silently discard their events.

Delivery is at-least-once. Duplicate invalidations are allowed; consumers must
remain idempotent. Source writes, projection events and owner revision allocation
must roll back together if publication fails.

## Composition and downstream boundaries

Current implementations live in `category.rs`, `category_command.rs`,
`category_lifecycle.rs`, `category_audience_owner.rs`, `topic_audience.rs`,
`topic.rs`, `reply.rs`, their public owner facades and `moderation_owner.rs`.
`services/mod.rs` declares these modules; retired `*_inline.rs` and category
`*_owner.rs` include layouts are not alternative implementations.

FORUM-20BL owns visibility-scoped bulk reads. The subsequent scope-preserving
rebuild and approved-reply contracts are linked by
[`forum-projection-invalidation.json`](../contracts/forum-projection-invalidation.json).
Invalidation never replaces those read/visibility checks.

## Verification and limits

Run `node scripts/verify/verify-forum-projection-invalidation.mjs` for the bounded
source/composition contract and `node --test scripts/verify/verify-forum-projection-invalidation.test.mjs`
for positive and negative verifier fixtures. These are not runtime evidence.

Integration still requires the crate-scoped Rust tests recorded in the JSON
contract, including PostgreSQL owner/outbox rollback and Search scope-preservation
execution. Restoring this missing contract note does not advance an audit ledger or
claim execution of those tests.
