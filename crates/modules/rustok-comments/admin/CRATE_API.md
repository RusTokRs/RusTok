# Crate API: rustok-comments-admin

## Public surface

### UI Components
- `CommentsAdmin`: Root Leptos administration component for comments moderation workspace.

### Core
- Moderation state management, filtering, and rich-text presentation models.

### Transport
- Native server adapter executing moderation actions against `CommentsService`.

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
- Moderation status updates (approve, reject, spam, delete).

### Domain Invariants
- Moderation operations preserve audit integrity and tenant isolation.
- Zero business logic inside UI view layer; pure state mapping in `core`.
- Comment content sanitized and rendered via typed rich text projections.

### Events / Outbox Side Effects
- Updates comment moderation status and persists moderation audit trail.

### Errors / Failure Codes
- Validation failures reported cleanly in UI without crashing views.
- Transport failures display typed error notices.

