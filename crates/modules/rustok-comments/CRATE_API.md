# rustok-comments / CRATE_API

## Platform Capability Bindings
- `CommentsModule` implements `rustok_core::module::RusToKModule` and `MigrationSource`.
- Declared as `ModuleKind::Optional`.
- Self-check returns `HealthStatus::Healthy`.
- Slug: `comments`.
- Permissions: `Resource::Comments` with `Action::Create`, `Read`, `Update`, `Delete`, `List`, `Moderate`, `Manage`.
- Event listeners: registers `CommentTargetDeletionHandler` handling `DomainEvent::TargetDeleted`.

## Domain Collaborations
- Serves as canonical storage owner for blog comments and opt-in non-forum discussion surfaces.
- Implements `CommentsThreadPort` provider boundary for cross-module comment access.
- Collaborates with `rustok-content` for rich text schema validation and locale fallback.
- Emits events via transactional outbox / domain events.
- Distinct from `rustok-forum`: does not share tables or domain logic with forum topics/replies.

## Minimum Contract Set

### Input DTOs/Commands
- `CreateCommentInput`: requires valid target type, non-nil target ID, optional parent comment ID, and valid rich text document conforming to the comment profile.
- `UpdateCommentInput`: specifies comment ID and updated rich text document.
- `SetCommentStatusRequest`: changes status between `Published`, `PendingModeration`, `Spam`, `Trash`.
- `ListCommentsFilter`: pagination and filtering by thread ID, status, author ID, parent ID.
- `ListThreadsFilter`: filtering threads by tenant, target type, target ID.
- `GetThreadDetailFilter`: query thread summary and comments.

### Domain Invariants
- Comment position allocation is strictly transactional: `comment::ActiveModelBehavior` serializes on the tenant thread row and allocates the next monotonic position.
- Unique constraint on `(thread_id, position)` prevents gap/race duplications.
- Thread identity uniqueness: `(tenant_id, target_type, target_id)` has persistent identity locks preventing duplicate threads.
- Rich text validation: accepts only `RichTextDocument` within the comment profile (`comment` schema).
- Closed threads reject new comment creation.
- Non-moderators cannot set status directly to `Spam` or `Trash`.

### Events / Outbox Side Effects
- Outbox events: comment created, comment updated, comment deleted, comment status changed.
- Inbound event handling: listens for `DomainEvent::TargetDeleted` and cleans up associated comment threads.
- Durable idempotency receipts for `CommentsThreadPort` write operations.

### Errors / Failure Codes
- `CommentsError::NotFound`: requested thread or comment not found.
- `CommentsError::Validation`: invalid input, empty target type, nil IDs, invalid rich text.
- `CommentsError::Forbidden` / `PermissionDenied`: operation not permitted for current actor/role.
- `CommentsError::ThreadClosed`: attempts to add comments to a closed thread.
- `CommentsError::Database`: underlying persistence failure.
- `CommentsError::Conflict`: thread identity conflict.
