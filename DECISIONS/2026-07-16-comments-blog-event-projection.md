# Comments-to-Blog Reply Count Projection

## Status

Accepted

## Context

`rustok-comments` owns comment lifecycle and exposes `CommentsThreadPort` to
`rustok-blog`. Blog owns `blog_post.comment_count` and publishes
`BlogPostUpdated` through the transactional outbox. Passing a database
transaction through the public comments port would expose storage mechanics
across module boundaries and would make the port unusable for remote profiles.

## Decision

Comments create and delete operations will publish owner-defined lifecycle
events atomically in the comments transaction through `rustok-outbox`.
Blog will consume those events through an idempotent module event projection
that updates `blog_post.comment_count` and emits the blog-owned update event.
The projection is eventually consistent; comment writes remain durable when
the projection is retried.

The public `CommentsThreadPort` remains the only Blog-to-Comments service
boundary. It must not expose `sea_orm::DatabaseTransaction` or another shared
storage handle.

## Consequences

- `comment_count` can lag comment writes until the projection is delivered.
- Comments owns lifecycle event publication and idempotency metadata.
- Blog owns reply-count projection, deduplication, and its update event.
- Create/delete can be removed from the direct `CommentsService` exception
  after the projection and recovery evidence exist.
- No FBA status is promoted until the provider event, consumer projection, and
  retry/degraded behavior have live execution evidence.
