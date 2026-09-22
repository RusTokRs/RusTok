# Comments-to-Blog Monotonic Snapshot Cursor

## Status

Accepted

## Date

2026-09-22

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

rustok-blog maintains a dedicated projection_revision BIGINT on every durable
Comments projection delivery row. While holding the existing tenant-scoped Blog post
row lock, the projection allocates the next per-post revision inside the same database
transaction that updates the derived counter, writes the delivery ledger, and publishes
the neutral Blog reindex request.

The public-comment snapshot cursor reads the greatest committed projection_revision
for the tenant/post. Every successfully committed lifecycle delivery therefore
advances snapshot identity regardless of cross-comment event ordering. event_id remains
responsible for per-comment lifecycle ordering and delivery idempotency; it is no longer
used as the public snapshot cursor.

The snapshot key namespace is bumped from blog-public-comments-snapshot-v2 to
blog-public-comments-snapshot-v3, making all pre-change snapshot keys unreachable
without cache-key enumeration.

## Consequences

- Cross-comment out-of-order projection cannot leave a changed snapshot under the
  previous cache identity.
- Projection rollback also rolls back revision allocation, delivery, counter
  transition, and reindex outbox publication as one transaction.
- Blog business version and updated_at remain untouched by the projection.
- The existing delivery ledger remains the source of the cursor; no second cursor
  table or cache invalidation index is introduced.
- A small migration adds the revision column and a tenant/post/revision index.
  Existing delivery rows start at revision 0; the first newly committed lifecycle
  delivery advances the cursor to 1, while snapshot schema v3 prevents reuse of
  v2 cache entries.
- The live snapshot writer reads the projection revision before and after the
  Comments live-read. It writes a cache entry only when both revisions match.
  A revision change during the read is treated as a consistency boundary: the
  live response remains valid, but no snapshot is stored under a cursor that may
  describe newer projection state than the returned data.
