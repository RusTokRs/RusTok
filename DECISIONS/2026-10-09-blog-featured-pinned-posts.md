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

### Public Listing Sort Order

```sql
ORDER BY is_pinned DESC, pinned_at DESC, published_at DESC, id DESC
```

Pinned posts appear first, ordered by most-recently-pinned. Unpinned posts follow in standard chronological order.

### API Surface

**GraphQL:**
- `Post.isPinned: Boolean!` and `Post.pinnedAt: String` fields
- `PostListItem.isPinned: Boolean!` and `PostListItem.pinnedAt: String` fields
- `CreatePostInput.isPinned: Boolean` and `UpdatePostInput.isPinned: Boolean`
- `pinPost(id: UUID!, tenantId: UUID): Boolean!` mutation
- `unpinPost(id: UUID!, tenantId: UUID): Boolean!` mutation

**REST:**
- `POST /api/blog/posts/{id}/pin` — pin a post
- `POST /api/blog/posts/{id}/unpin` — unpin a post
- `is_pinned` field on `CreatePostInput` and `UpdatePostInput`
- `isPinned` and `pinnedAt` fields on `PostResponse` and `PostSummary`

**Service Layer:**
- `PostService::pin_post(tenant_id, post_id, security)` — pins a post
- `PostService::unpin_post(tenant_id, post_id, security)` — unpins a post
- `PostService::set_pinned(tenant_id, post_id, pinned, security)` — internal helper

## Alternatives Considered

1. **Featured flag without timestamp** — Rejected because ordering among multiple pinned posts would be ambiguous.
2. **Weight/priority field** — Rejected as over-engineered; simple pin + timestamp gives sufficient control.
3. **Separate "featured posts" collection** — Rejected as unnecessarily complex; a boolean flag on the post entity is simpler and integrates with existing listing queries.

## Files Changed

- `migrations/m20261009_000035_add_blog_post_pinned.rs` — schema migration
- `entities/blog_post.rs` — entity fields
- `dto/post.rs` — DTO fields
- `services/post/commands.rs` — pin/unpin/set_pinned methods, create/update integration
- `services/post/queries.rs` — mapping and sort order
- `graphql/types.rs` — GraphQL type fields and input fields
- `graphql/mutation.rs` — pinPost/unpinPost mutations
- `controllers/posts.rs` — REST pin/unpin endpoints
- `controllers/mod.rs` — route registration
- `admin/src/model.rs` — admin UI model
- `admin/src/transport/graphql_adapter.rs` — GraphQL adapter input
- `admin/src/transport/native_server_adapter.rs` — native adapter input
- All test files updated with `is_pinned: None` field
