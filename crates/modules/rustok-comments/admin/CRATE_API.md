# Crate API: rustok-comments-admin

## Public surface

### UI Components
- `CommentsAdmin`: Root Leptos administration component for comments moderation workspace.

### Core
- Moderation state management, filtering, and rich-text presentation models.
- Request structures:
  - `CommentThreadsRequest`: Thread list query filters (`page`, `per_page`, `target_type`, `thread_status`, `comment_status`).
  - `CommentThreadDetailRequest`: Thread detail query filters (`thread_id`, `locale`, `page`, `per_page`).
  - `SetThreadStatusCommand`: Thread status command (`thread_id`, `status: CommentThreadStatus`).
  - `SetCommentStatusCommand`: Comment status command (`comment_id`, `status: CommentStatus`, `locale`).
- View models:
  - `CommentThreadListItemViewModel`: Formatted thread list row projection (`id`, `target_label`, `status_label`, `comment_count`).
  - `CommentThreadDetailViewModel`: Header detail projection (`target_label`, `status`, `status_label`, `comment_count`).
  - `CommentRowViewModel`: Moderation row projection (`id`, `author_id`, `created_at`, `body: RichTextView`, `status`, `status_label`, `requested_locale`, `effective_locale`).
- Query intent helpers:
  - `comments_admin_select_thread_query_intent(thread_id)`: Generates push route query intent.
  - `comments_admin_locale_query_intent(locale)`: Generates replace or clear route query intent.

### Transport
- `CommentsAdminTransportError`: Typed transport error wrapping `ServerFn(String)`.
- `CommentThreadsPayload`: Paginated thread payload (`items: Vec<CommentThreadSummary>`, `total: u64`).
- `CommentsAdminTransportPath`: Active transport path indicator (`NativeServer`).
- Native server adapter executing moderation actions against `rustok_comments::CommentsService`.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) administration package for `rustok-comments`.
- Native server functions integration via `HostRuntimeContext`.
- Rich-text comment content rendering via `leptos_ui::RichTextHtml`.
- Internationalization via `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-comments` backend service for comment moderation actions.
- Integrates with content owner modules (e.g., `rustok-blog`) referencing comment threads.

## Minimum Contract Set

### Input DTOs/Commands
- `CommentsAdmin`: Entrypoint moderation view component.
- `CommentThreadsRequest`: Validated pagination and filter bounds (`DEFAULT_PAGE_SIZE = 20`, `MAX_PAGE_SIZE = 100`).
- `CommentThreadDetailRequest`: Validated thread ID and locale bounds.
- `SetThreadStatusCommand`: Validated non-nil thread UUID and target status.
- `SetCommentStatusCommand`: Validated non-nil comment UUID, status, and locale.

### Domain Invariants
- Moderation operations preserve audit integrity and tenant isolation.
- Zero business logic inside UI view layer; pure state mapping in `core`.
- Comment content sanitized and rendered via typed rich text projections (`RichTextView`).
- Native-only comments admin transport exception documented and strictly preserved.
- Pagination bounds enforced (`page >= 1`, `per_page` clamped to `[1, 100]`).
- Nil and invalid UUIDs rejected before server function dispatch.

### Events / Outbox Side Effects
- Updates comment moderation status and persists moderation audit trail via `CommentsService`.

### Errors / Failure Codes
- `CommentsAdminValidationError`:
  - `EmptyThreadId`: Thread ID is empty or whitespace-only.
  - `InvalidThreadId`: Malformed UUID string.
  - `NilThreadId`: UUID is nil.
  - `EmptyCommentId`: Comment ID is empty or whitespace-only.
  - `InvalidCommentId`: Malformed UUID string.
  - `NilCommentId`: UUID is nil.
- `CommentsAdminTransportError`:
  - `ServerFn(String)`: Transport/execution error from server function invocation.
