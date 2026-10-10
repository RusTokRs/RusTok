# H-6: Bulk Operations — Implementation Summary

- **Date**: 2026-10-09
- **Status**: Implemented
- **Owner**: `rustok-blog`

## Overview

Blog administrators can now perform bulk operations on multiple posts simultaneously:
- **Bulk Publish**: Publish multiple draft posts at once
- **Bulk Unpublish**: Unpublish multiple published posts at once
- **Bulk Archive**: Archive multiple published posts at once
- **Bulk Delete**: Delete multiple draft/archived posts at once

## Design

### Error Handling

Each post is processed independently. Failures on individual posts do not abort
the entire operation. The result includes:
- `succeeded`: count of successfully processed posts
- `failed`: count of failed posts
- `succeeded_ids`: list of successfully processed post IDs
- `failures`: list of failures with post ID and reason

### Permission Model

- **Bulk Publish/Unpublish/Archive**: Requires `blog_posts:publish` permission
- **Bulk Delete**: Requires `blog_posts:delete` permission
- Ownership checks are applied per-post (author or admin)

### Status Transitions

Bulk operations respect the same state machine as single-post operations:
- **Publish**: Only Draft → Published
- **Unpublish**: Only Published → Draft
- **Archive**: Only Published → Archived
- **Delete**: Only Draft or Archived (Published posts must be unpublished first)

## Changes

### DTO

- `BulkOperationResult` — result of a bulk operation with success/failure counts
- `BulkOperationFailure` — individual failure with post ID and reason

### Service

- `PostService::bulk_transition_posts()` — bulk status transition
- `PostService::bulk_delete_posts()` — bulk deletion
- `PostService::transition_post()` — internal helper for single-post transition

### GraphQL

- **Input**: `BulkOperationInput { postIds: [UUID!]! }`
- **Output**: `GqlBulkOperationResult { succeeded, failed, succeededIds, failures }`
- **Failure**: `GqlBulkOperationFailure { postId, reason }`
- **Mutations**:
  - `bulkPublishPosts(input)` → `GqlBulkOperationResult`
  - `bulkUnpublishPosts(input)` → `GqlBulkOperationResult`
  - `bulkArchivePosts(input)` → `GqlBulkOperationResult`
  - `bulkDeletePosts(input)` → `GqlBulkOperationResult`

## Usage Example

```graphql
mutation {
  bulkPublishPosts(input: {
    postIds: [
      "550e8400-e29b-41d4-a716-446655440000",
      "550e8400-e29b-41d4-a716-446655440001",
      "550e8400-e29b-41d4-a716-446655440002"
    ]
  }) {
    succeeded
    failed
    succeededIds
    failures {
      postId
      reason
    }
  }
}
```

## Invariants

- Each post is processed in its own transaction (no bulk transaction)
- Events are published for each successful operation
- Failed posts are recorded but do not affect other posts
- Maximum batch size is not enforced (client responsibility)
