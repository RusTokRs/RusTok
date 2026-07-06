---
id: doc://docs/guides/error-handling.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Error Handling Guide

The complete error handling guide for RusToK is in [`docs/standards/errors.md`](../standards/errors.md).

## Quick Summary

RusToK uses a unified `RichError` type (RFC 7807 compatible) with categories:

| HTTP | Category | When to use |
|------|----------|-------------|
| 400 | `Validation` | Input validation errors |
| 401 | `Unauthenticated` | Authentication required |
| 403 | `Forbidden` | No access rights |
| 404 | `NotFound` | Resource not found |
| 409 | `Conflict` | Duplication or race condition |
| 429 | `RateLimited` | Rate limit exceeded |
| 500 | `Internal` | Unexpected error |
| 502/503 | `ExternalService` | External service error |
| 504 | `Timeout` | Request timeout |

## Rules

1. All functions that can fail return `Result<T, RusToKError>`.
2. Use of `.unwrap()` / `.expect()` is forbidden (except in tests).
3. Internal errors are not disclosed to the client — only `user_message`.
4. Tracing uses `request_id` from the request context.

## Full Documentation

→ [`docs/standards/errors.md`](../standards/errors.md)
