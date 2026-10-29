# Crate API: rustok-comments-storefront-support

## Public surface

### Core
- `CommentComposerValidationError`: Validation errors for comment submissions.
- `is_richtext_blank`: Utility to detect empty rich text content.
- `validate_comment_submission`: Enforces non-blank, sanitized comment submission rules.

### UI Components
- `CommentComposer`: Interactive Leptos component for authoring and submitting comments.
- `CommentComposerCopy`: Localized UI copy configuration for composer labels and buttons.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) storefront support component for comment authoring.
- Integrates with `leptos-ui::RichTextEditorFrame` using the canonical `comment` profile.
- Compatible with Leptos hydration and SSR modes.

## Domain Collaborations
- Collaborates with content-hosting modules (such as `rustok-blog` and product reviews) to embed comment authoring.
- Relies on parent modules to bind target identities and execute submission transports.

## Minimum Contract Set

### Input DTOs/Commands
- `CommentComposer`: Storefront component receiving action closures and localized copy.
- `CommentComposerCopy`: Localized string values for composer form elements.

### Domain Invariants
- Zero transport logic inside this support crate; host modules supply submission handlers.
- Submission validation runs in framework-agnostic `core` before triggering submit callbacks.
- Rich text content validated against empty and whitespace-only payloads.

### Events / Outbox Side Effects
- Invokes parent-provided submit action closure upon successful client-side validation.

### Errors / Failure Codes
- `CommentComposerValidationError::BlankContent`: Comment body contains no substantive text.
- Validation failures surface accessible feedback within the composer interface.

