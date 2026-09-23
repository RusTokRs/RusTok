# Comments-to-Blog Public Snapshot Invalidation

- Date: 2026-09-22
- Decision status: Accepted
- Implementation status: Implemented
- Owners: rustok-blog / rustok-comments
- Extends: [Comments-to-Blog Reply Count Projection](./2026-07-16-comments-blog-event-projection.md)
- Supersedes: None
- Superseded by: None

## Context

`rustok-comments` is the canonical owner of comment lifecycle state. Blog keeps a
public-comment snapshot as a bounded degraded read path when the live Comments
provider is unavailable. The snapshot is keyed by request identity, so a body edit
or moderation status change could previously leave an older approved snapshot
reachable until its TTL expired.

Deleting cache keys is not an ownership-safe solution: the Blog snapshot store is a
narrow cache capability and does not enumerate Redis keys. A second invalidation
index would duplicate projection state and introduce another consistency boundary.

The existing Comments-to-Blog projection already has a tenant-scoped durable
delivery ledger. Its processed lifecycle rows are the Blog read-side record of
which Comments lifecycle facts have crossed the projection boundary.

## Decision

`rustok-comments` publishes the complete lifecycle needed by downstream Blog reads:

- `comment.created`
- `comment.updated`
- `comment.status_changed`
- `comment.deleted`

The publisher remains `rustok-comments`, and every lifecycle event is written
through `TransactionalEventBus::publish_in_tx` in the same transaction as the
owning mutation.

`rustok-blog` consumes all four events through the existing idempotent
`BlogCommentProjectionHandler`. `comment_count` remains derived only from the
active/deleted lifecycle state. Update and status-change events normally advance
the lifecycle delivery cursor without changing the count or Blog business
revision.

The latest processed lifecycle `event_id` for a tenant/post is the Blog
public-comment projection cursor. The public snapshot identity includes that
cursor. Consequently, every processed lifecycle event makes all snapshots built
from an earlier cursor unreachable. No cache-key enumeration, cache-side event
listener, or second invalidation index is introduced.

The cursor is intentionally based on processed events rather than emitted events.
This preserves the existing eventual-consistency boundary: a snapshot may remain
reachable until the lifecycle event has successfully crossed the Blog projection
boundary. Once processed, the old snapshot key is invalid by identity and the live
or degraded read path uses the new cursor.

The snapshot schema/key namespace is bumped from
`blog-public-comments-snapshot-v1` to `blog-public-comments-snapshot-v2`.
No database migration is required.

## Sources of truth and ownership

- `rustok-comments` owns canonical comment lifecycle state.
- `rustok-blog` owns the delivery ledger `blog_comment_projection_deliveries` and cached snapshot projection.
- Derived counters and snapshot identity do not act as authoritative write models.

## Invariants

### Allowed states

- Snapshot identity includes the latest committed projection cursor.
- Out-of-order cross-comment delivery converges deterministically.
- Comment count floors at 0 and saturates without altering business revision.

### Forbidden states

- Enumerating cache keys in external storage (e.g. Redis KEYS/SCAN).
- Second invalidation index duplicating projection state.
- Comment projection modifying `blog_posts.version` or `blog_posts.updated_at`.

## Non-goals

- Does not promote Blog Comments FBA to `transport_verified`.
- Does not expose storage transactions across the port boundary.

## Data, transaction, and concurrency boundary

- Projection updates lock the tenant-scoped Blog post row exclusively.
- Delivery ledger insert and neutral reindex outbox write commit atomically in the projection transaction.
- Monotonic progression per comment enforced by UUID/ULID event ID ordering.

## Context dimensions

- Tenant, post, locale, channel, and pagination parameters form the composite cache identity.
- Cross-tenant event projection is prohibited.

## Events and projections

- Emitted events: `comment.created`, `comment.updated`, `comment.status_changed`, `comment.deleted`.
- Processed by `BlogCommentProjectionHandler` to maintain comment counts and projection cursor.

## Failure semantics

- Obsolete events for deleted posts are acknowledged and dropped.
- Outbox write failures roll back both delivery ledger and counter transitions before retry.

## Migration and cutover

- Snapshot prefix bumped to `blog-public-comments-snapshot-v2`.
- Previous cached keys expire naturally via TTL.

## Alternatives considered

- Redis key scanning/invalidation (rejected: violates bounded cache capability contract).
- Secondary cache tag index (rejected: introduces dual write hazards).

## Verification

- `node scripts/verify/verify-blog-comments-event-projection.mjs`
- `cargo test -p rustok-blog --lib services::comment_projection::tests`
- PostgreSQL integration test `comment_projection_postgres_test.rs`.

## Consequences

- Comment edits and status transitions invalidate stale public snapshots without manual cache flushing.
- Degraded mode continues serving valid cached data without consistency leakage.
