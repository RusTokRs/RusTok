# H-4: Draft Preview Tokens — Implementation Summary

- **Date**: 2026-10-09
- **Status**: Implemented
- **Owner**: `rustok-blog`

## Overview

Blog authors can now generate time-limited preview tokens for draft posts.
These tokens allow unauthenticated access to the post content via a unique URL,
enabling editors, reviewers, and stakeholders to preview drafts before publication.

## Design

### Token Lifecycle

```
Author creates draft → generates preview token → shares URL with reviewer
                                                    ↓
                                              Reviewer opens URL
                                                    ↓
                     Token validated (exists, unexpired, tenant still has Blog enabled)
                                                    ↓
                        Post returned only while the post is still a Draft
                                                    ↓
        Post leaves draft (publish, scheduled publish, archive) → its tokens are revoked in the
        same lifecycle transaction; unpublish/restore does not revive them
```

### Security Model

- **Token creation**: Requires `blog_posts:update` permission + ownership (author or admin)
- **Token access**: Unauthenticated — anyone with the token can view the post
- **Token revocation**: Requires `blog_posts:update` permission + ownership
- **Token invalidation on status change**: A token authorizes a draft read only, so the lifecycle
  transition that takes the post out of `draft` (manual publish, `blog_scheduled_publish`, archive)
  revokes the post's tokens inside the same transaction
- **Token expiry**: Configurable TTL (default 7 days, max 30 days)
- **Token format**: 32 bytes of cryptographic randomness, base64url-encoded (43 chars)

## Changes

### Database

- **Migration** `m20261009_000034_create_blog_preview_tokens`:
  - Table `blog_preview_tokens` with columns: `id`, `tenant_id`, `post_id`, `token` (unique), `created_by`, `expires_at`, `created_at`
  - FK to `blog_posts` with CASCADE delete
  - Indexes: token lookup, post-based cleanup, expiry-based cleanup

### Entity

- `blog_preview_token::Model` — SeaORM entity for the tokens table

### Service

- `PreviewTokenService` with methods:
  - `create_token(tenant_id, post_id, security, ttl_hours)` → `BlogResult<PreviewToken>`. A TTL
    outside `1..=720` hours is refused with a validation error instead of being clamped.
  - `get_post_by_token(token, locale, fallback_locale)` → `BlogResult<Option<PostResponse>>`
  - `revoke_token(tenant_id, token_id, security)` → `BlogResult<()>`
  - `list_tokens(tenant_id, post_id, security)` → `BlogResult<Vec<PreviewToken>>`
  - `cleanup_expired(tenant_id, limit)` → `BlogResult<u64>` (rows deleted, `limit` clamped to
    `MAX_PREVIEW_TOKEN_CLEANUP_BATCH` = 500)

### GraphQL

- **Type**: `GqlPreviewToken { id, token, postId, expiresAt, createdAt }`
- **Input**: `CreatePreviewTokenInput { postId, ttlHours? }`
- **Mutation**: `createPreviewToken(input)` → `GqlPreviewToken`
- **Mutation**: `revokePreviewToken(tokenId)` → `Boolean`
- **Query**: `postByPreviewToken(token, locale?)` → `GqlPost?`

### REST API

- **GET** `/api/blog/preview?locale=en` with the token in the `x-blog-preview-token` header → `PostResponse` (no auth required; the token never travels in the URL)

### PostService

- `find_post()` visibility changed from `pub(super)` to `pub(crate)` to allow cross-service access within the blog module

## Validation Rules

- Token TTL must be between 1 and 720 hours (30 days)
- Default TTL is 168 hours (7 days)
- Only the post author or users with `blog_posts:update` can create/revoke tokens
- Expired tokens return 404 on access
- Invalid tokens return 404 on access

## Cleanup

Retention is wired into the host scheduler: the Blog-owned `blog_preview_token_cleanup`
module-work registration calls `PreviewTokenService::cleanup_expired(tenant_id, limit)` for
one tenant per work item, so housekeeping is tenant-scoped and bounded by
`MAX_PREVIEW_TOKEN_CLEANUP_BATCH` instead of deleting a whole backlog in one pass. Deleting a
post additionally removes its tokens in the same transaction and through the table's cascade.

## Invariants

- A token is unique across the entire system (not just per-tenant)
- A token grants read-only access to a single post
- A token cannot be used to access posts in other tenants
- A token read resolves through the draft-only preview accessor, not through system authority,
  and only while the token's own tenant still has Blog enabled; an expired, revoked or
  module-disabled token answers "not found" without leaking whether the post exists
- Deleting a post cascades to delete all its preview tokens
- A post that leaves `draft` loses its tokens in the same lifecycle transition (publish by the
  owner command or the scheduler, archive), so a token row never outlives the state it
  authorizes; `unpublish` and `restore` return the post to draft without reviving a token that
  the earlier transition revoked
