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
                                              Token validated (exists + not expired)
                                                    ↓
                                              Post returned (even if Draft)
```

### Security Model

- **Token creation**: Requires `blog_posts:update` permission + ownership (author or admin)
- **Token access**: Unauthenticated — anyone with the token can view the post
- **Token revocation**: Requires `blog_posts:update` permission + ownership
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
  - `create_token(tenant_id, post_id, security, ttl_hours)` → `PreviewToken`
  - `get_post_by_token(token, locale)` → `Option<PostResponse>`
  - `revoke_token(tenant_id, token_id, security)` → `()`
  - `list_tokens(tenant_id, post_id, security)` → `Vec<PreviewToken>`
  - `cleanup_expired()` → `u64` (rows deleted)

### GraphQL

- **Type**: `GqlPreviewToken { id, token, postId, expiresAt, createdAt }`
- **Input**: `CreatePreviewTokenInput { postId, ttlHours? }`
- **Mutation**: `createPreviewToken(input)` → `GqlPreviewToken`
- **Mutation**: `revokePreviewToken(tokenId)` → `Boolean`
- **Query**: `postByPreviewToken(token, locale?)` → `GqlPost?`

### REST API

- **GET** `/api/blog/preview/{token}?locale=en` → `PostResponse` (no auth required)

### PostService

- `find_post()` visibility changed from `pub(super)` to `pub(crate)` to allow cross-service access within the blog module

## Validation Rules

- Token TTL must be between 1 and 720 hours (30 days)
- Default TTL is 168 hours (7 days)
- Only the post author or users with `blog_posts:update` can create/revoke tokens
- Expired tokens return 404 on access
- Invalid tokens return 404 on access

## Cleanup

Expired tokens should be cleaned up periodically by calling
`PreviewTokenService::cleanup_expired()`. This can be done:
1. Via a cron job
2. Integrated into the existing scheduler worker
3. Lazily on token creation (delete expired tokens for the same post)

## Invariants

- A token is unique across the entire system (not just per-tenant)
- A token grants read-only access to a single post
- A token cannot be used to access posts in other tenants
- Deleting a post cascades to delete all its preview tokens
- The system security context is used for token-based access (bypasses normal RBAC)
