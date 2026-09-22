# Comments-to-Blog Public Snapshot Invalidation

## Status

Accepted

## Date

2026-09-22

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

## Consequences

- Comment body edits no longer leave the previous public snapshot reachable after
  the corresponding lifecycle event is processed.
- Approval, spam, and trash transitions invalidate the prior snapshot after the
  corresponding status-change event is processed.
- Delete remains an ordinary lifecycle transition and continues to affect
  `comment_count`.
- `comment_count`, `blog_posts.version`, and `blog_posts.updated_at` retain
  their existing ownership semantics.
- Reindex publication still occurs only when the derived `comment_count` changes;
  snapshot invalidation does not manufacture a locale-specific Blog update.
- The new lifecycle event types are additive version-1 event contracts. Their
  introduction therefore requires regeneration and review of the canonical
  `rustok-events` contract digest artifact.
- Runtime execution evidence remains maintainer-owned and is not promoted by this
  source change.

## Verification requirements

Source/runtime verification must prove:

1. an update with a real body mutation publishes exactly one
   `comment.updated` event; a metadata-only/no-write command publishes none;
2. an actual moderation status transition publishes exactly one
   `comment.status_changed` event; a same-status no-op publishes none;
3. Blog projection accepts all four lifecycle event types and preserves the
   active/deleted counter transition semantics;
4. update/status delivery commits the Blog delivery ledger even when
   `comment_count` is unchanged;
5. the processed lifecycle cursor advances to the newest event and is part of the
   public snapshot identity;
6. tenant, post, locale, channel, pagination, and cursor dimensions remain isolated;
7. outbox publication, Blog projection delivery, rollback/retry, and degraded
   snapshot reads preserve the existing ownership and transactional contracts.

## Relation to existing decisions

This decision extends
[Comments-to-Blog Reply Count Projection](./2026-07-16-comments-blog-event-projection.md).
It does not move ownership of `comment_count`, does not expose storage
transactions through `CommentsThreadPort`, and does not promote the Comments FBA
boundary.

It follows
[Event schema release discipline](./2026-07-23-event-schema-release-discipline.md):
the two new lifecycle event types are additive version-1 contracts and the
canonical digest artifact is regenerated from the repository-owned generator.
