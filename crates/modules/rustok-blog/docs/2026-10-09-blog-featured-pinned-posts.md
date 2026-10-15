# M-5: Featured/Pinned Posts

**Date:** 2026-10-09
**Status:** Implemented
**Module:** rustok-blog

## Problem

The blog module had no mechanism to feature or pin specific posts to the top of public listings. Content editors needed a way to highlight important posts regardless of their publication date.

## Decision

Implemented a pin/unpin feature with the following characteristics:

### Database Schema (Migration 000035)

- `is_pinned BOOLEAN NOT NULL DEFAULT false` — whether the post is pinned
- `pinned_at TIMESTAMP WITH TIME ZONE NULLABLE` — when the post was pinned (used for ordering among pinned posts)
- Index `idx_blog_posts_pinned(tenant_id, is_pinned, pinned_at)` for efficient pinned-first queries

### Business Rules

1. **Only published posts can be pinned.** Attempting to pin a draft or archived post returns a validation error.
2. **Pinning is tenant-scoped** and subject to `blog_posts:update` permission.
3. **Ownership scoping** is enforced: users with `owned` scope can only pin their own posts.
4. **Unpinning** always succeeds if the post exists and the user has permission.
5. **Pin state is command-owned.** `is_pinned`/`pinned_at` are not writable through
   `CreatePostInput`/`UpdatePostInput` (`createPost`/`updatePost`,
   `POST /api/blog/posts`, `PUT /api/blog/posts/{id}`) or through any other writer: the
   only write path is `pinPost`/`unpinPost`. This keeps the documented rule above true
   by construction, because `create_post`/`update_post` produce arbitrary lifecycle
   states (drafts, archived revisions) that cannot be pinned coherently.
6. **Pin is atomic.** `set_pinned` runs in one transaction on a row-locked read, so a
   concurrently deleted post fails with `PostNotFound` instead of reporting success.
7. **Pin state is not part of the post revision.** `version` is not advanced and the
   Search projection is not reindexed, because pin state changes list order only and is
   not projected into the search document payload.

### Public Listing Sort Order

```sql
ORDER BY is_pinned DESC, pinned_at DESC NULLS LAST, published_at DESC, id DESC
```

Pinned posts appear first, ordered by most-recently-pinned. Unpinned posts follow in standard chronological order.

The public listing is keyset-paginated, so `PublishedPostCursor` carries the whole sort
key (`is_pinned`, `pinned_at`, `published_at`, `id`). A cursor that only carried
`(published_at, id)` would skip or repeat rows as soon as a pinned post exists.
Legacy tokens are rejected instead of being resumed with a lossy position. The
`v1` token is opaque to clients.

### API Surface

**GraphQL:**
- `Post.isPinned: Boolean!` and `Post.pinnedAt: String` fields
- `PostListItem.isPinned: Boolean!` and `PostListItem.pinnedAt: String` fields
- `pinPost(id: UUID!, tenantId: UUID): Boolean!` mutation
- `unpinPost(id: UUID!, tenantId: UUID): Boolean!` mutation
- `CreatePostInput`/`UpdatePostInput` deliberately expose no pin field (see Business Rule 5)

**REST:**
- `POST /api/blog/posts/{id}/pin` — pin a post
- `POST /api/blog/posts/{id}/unpin` — unpin a post
- `isPinned` and `pinnedAt` fields on `PostResponse` and `PostSummary` (read-only)

**Service Layer:**
- `PostService::pin_post(tenant_id, post_id, security)` — pins a post
- `PostService::unpin_post(tenant_id, post_id, security)` — unpins a post
- `PostService::set_pinned(tenant_id, post_id, pinned, security)` — private transactional helper of `pin_post`/`unpin_post`

## Alternatives Considered

1. **Featured flag without timestamp** — Rejected because ordering among multiple pinned posts would be ambiguous.
2. **Weight/priority field** — Rejected as over-engineered; simple pin + timestamp gives sufficient control.
3. **Separate "featured posts" collection** — Rejected as unnecessarily complex; a boolean flag on the post entity is simpler and integrates with existing listing queries.

## Files Changed

- `migrations/m20261009_000035_add_blog_post_pinned.rs` — schema migration
- `entities/blog_post.rs` — entity fields
- `dto/post.rs` — DTO fields
- `services/post/pin.rs` — transactional pin/unpin commands
- `services/post/queries.rs` — mapping, canonical ordering, pin-aware keyset cursor
- `graphql/types.rs` — GraphQL type fields and input fields
- `graphql/mutation.rs` — pinPost/unpinPost mutations
- `controllers/posts.rs` — REST pin/unpin endpoints
- `controllers/mod.rs` — route registration
- `admin/src/model.rs` — admin UI model
- `admin/src/transport/graphql_adapter.rs` — GraphQL adapter input
- `admin/src/transport/native_server_adapter.rs` — native adapter input
- `services/post/tests.rs` — pinned-keyset pagination regression
  (`public_visible_keyset_keeps_pinned_post_via_cursor_rank`)
- `dto/post.rs` — `PublishedPostCursor` encode/decode round-trip and legacy-token
  rejection tests
