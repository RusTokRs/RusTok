# H-1: Scheduled Publishing — Implementation Summary

- **Date**: 2026-10-09
- **Status**: Implemented
- **Owner**: `rustok-blog`

## Overview

Blog posts can now be scheduled for automatic publication at a future time.
A Draft post with `scheduled_at` set will be automatically published by the
scheduler worker when the timestamp is reached.

## Changes

### Database

- **Migration** `m20261009_000033_add_blog_post_scheduled_at`:
  - Adds `scheduled_at: TIMESTAMP WITH TIME ZONE NULL` to `blog_posts`
  - Creates index `idx_blog_posts_scheduled` on `(tenant_id, status, scheduled_at)` for efficient scheduler queries

### Entity

- `blog_post::Model` gains `scheduled_at: Option<DateTimeWithTimeZone>`

### DTO

- `CreatePostInput` gains `scheduled_at: Option<DateTime<Utc>>`
- `UpdatePostInput` gains `scheduled_at: Patch<DateTime<Utc>>` (supports set/clear/keep)
- `PostResponse` gains `scheduled_at: Option<DateTime<Utc>>`
- `PostSummary` gains `scheduled_at: Option<DateTime<Utc>>`

### Domain Events

Two new events in `rustok-events`:

- `BlogPostScheduled { post_id, scheduled_at }` — emitted when a schedule is set
- `BlogPostScheduleCancelled { post_id }` — emitted when a schedule is cleared

### Service Logic

- **Create**: If `scheduled_at` is set and `publish=false`, the post is saved as Draft with the schedule. Validation: `scheduled_at` must be in the future; cannot be combined with `publish=true`.
- **Update**: `scheduled_at` can be set, changed, or cleared via `Patch` semantics. Same future-date validation applies.
- **Publish (manual)**: When a post is manually published, `scheduled_at` is automatically cleared.
- **`publish_scheduled_posts()`**: New method that finds all Draft posts with `scheduled_at <= now` and publishes them. Designed to be called by an external scheduler worker (cron job or daemon).

### GraphQL

- `GqlPost` and `GqlPostListItem` gain `scheduledAt: Option<String>` (ISO 8601)
- `CreatePostInput` (GraphQL) gains `scheduledAt: Option<String>`
- `UpdatePostInput` (GraphQL) gains `scheduledAt: MaybeUndefined<String>`
- Helper `graphql_patch_datetime()` converts `MaybeUndefined<String>` to `Patch<DateTime<Utc>>`

### REST API

- No changes to route definitions; the existing endpoints accept `scheduled_at` in the request body via the updated DTOs.

### Admin UI

- **Leptos admin**: `BlogPostListItem` and `BlogPostDetail` gain `scheduledAt` field; GraphQL queries updated to fetch it.
- **Next.js admin**: Post form gains a "Schedule publication" datetime-local input. The field is disabled when "Publish immediately" is checked. API types updated.

### Storefront

- No changes. Scheduled posts are Draft and not visible publicly.

## Scheduler Worker

The `publish_scheduled_posts()` method is ready to be called by an external
scheduler. Implementation options:

1. **Cron job**: Run `cargo run --bin blog_scheduler` every minute
2. **Long-running daemon**: Loop with configurable sleep interval
3. **Integrated worker**: Call from the existing worker infrastructure

The method is idempotent: if a post is already published or the schedule was
cleared, it will be skipped.

## Validation Rules

- `scheduled_at` must be in the future (UTC)
- `scheduled_at` cannot be set when `publish=true`
- `scheduled_at` is only meaningful for Draft posts
- When a post is manually published, `scheduled_at` is cleared automatically

## Invariants

- A published post never has `scheduled_at` set
- A Draft post with `scheduled_at` in the past will be published on the next scheduler tick
- The scheduler uses the same `apply_status_transition_in_tx` as manual publish, preserving the first-publication `published_at` contract
