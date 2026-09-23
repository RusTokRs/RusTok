# Comments-to-Blog Monotonic Snapshot Cursor

- Date: 2026-09-22
- Decision status: Accepted
- Implementation status: Implemented
- Owners: rustok-blog / rustok-comments
- Extends: [Comments-to-Blog public snapshot invalidation](./2026-09-22-comments-blog-public-snapshot-invalidation.md)
- Supersedes: None
- Superseded by: None

## Context

Blog public-comment snapshots use a projection cursor as part of the cache identity.
The Comments lifecycle projection orders state per comment rather than depending on
global event ordering, so lifecycle envelopes for different comments on the same post
may be processed out of order.

Using the maximum processed event_id as the post-level cursor is therefore not a
monotonic processed-state frontier. If event B is processed before older event A,
MAX(event_id) remains B after A commits even though the public projection changed.
A cached snapshot keyed by B can therefore remain reachable after A is processed.

## Decision

`rustok-blog` maintains a dedicated `projection_revision BIGINT` on every durable
Comments projection delivery row. While holding the existing tenant-scoped Blog post
row lock, the projection allocates the next per-post revision inside the same database
transaction that updates the derived counter, writes the delivery ledger, and publishes
the neutral Blog reindex request.

The public-comment snapshot cursor reads the greatest committed `projection_revision`
for the tenant/post. Every successfully committed lifecycle delivery therefore
advances snapshot identity regardless of cross-comment event ordering. `event_id` remains
responsible for per-comment lifecycle ordering and delivery idempotency; it is no longer
used as the public snapshot cursor.

The snapshot key namespace is bumped from `blog-public-comments-snapshot-v2` to
`blog-public-comments-snapshot-v3`, making all pre-change snapshot keys unreachable
without cache-key enumeration.

## Sources of truth and ownership

- `rustok-comments` owns canonical comment state.
- `blog_comment_projection_deliveries.projection_revision` owns the monotonic snapshot cursor.
- The snapshot cache in `rustok-cache` is a pure projection and not a source of truth.

## Invariants

### Allowed states

- Each committed projection delivery for a post strictly increments `projection_revision`.
- Cache writes verify that `projection_revision` did not change during the live Comments read.

### Forbidden states

- Using `MAX(event_id)` as the cache cursor across multiple comments.
- Storing a cached snapshot when the cursor changed during the upstream Comments read.

## Non-goals

- Does not establish global event ordering across different posts or tenants.
- Does not alter Comments owner storage.

## Data, transaction, and concurrency boundary

- Incrementing `projection_revision` executes within the tenant-scoped Blog post row lock.
- Migration `m20260922_000026_add_blog_comment_projection_revision` adds the column and index.

## Context dimensions

- Cursor is isolated per tenant and post ID.

## Events and projections

- Consumes `comment.created`, `comment.updated`, `comment.status_changed`, `comment.deleted`.
- Advances `projection_revision` on every accepted lifecycle event.

## Failure semantics

- Any transaction failure aborts cursor increment, counter change, and delivery insertion together.
- Race conditions during live reads cause cache writes to be safely skipped.

## Migration and cutover

- Database migration adds `projection_revision` defaulting to 0.
- Cache namespace bumped to `blog-public-comments-snapshot-v3`.

## Alternatives considered

- Using global event sequence numbers (rejected: requires global distributed sequence coordination).
- Invalidating cache keys explicitly (rejected: requires cache enumeration).

## Verification

- `node scripts/verify/verify-blog-comments-event-projection.mjs`
- Unit tests in `crates/modules/rustok-blog/src/integrations/public_comments_snapshot.rs`.

## Consequences

- Out-of-order event delivery safely advances snapshot invalidation frontiers.
- Prevents race conditions where live reads concurrently interleave with projection writes.
