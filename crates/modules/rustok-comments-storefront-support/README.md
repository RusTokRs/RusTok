# rustok-comments-storefront-support

Reusable Leptos storefront authoring UI owned by the Comments capability.

## Responsibilities

- Own the public comment composer state, richtext frame, validation, busy state, and accessible result feedback.
- Accept an owner-bound submit action instead of exposing `target_type` or arbitrary target identity to the browser.
- Consume localized copy prepared from the host-provided effective locale; this support crate never selects a locale.
- Keep consumer modules responsible for validating their exact target and invoking their native/GraphQL owner command.

## Interactions

Blog composes `CommentComposer` with its post-bound comment-create action. Product reviews and later consumers may reuse the same component with their own exact target-bound commands. The component uses the canonical `comment` richtext profile through `leptos-ui::RichTextEditorFrame`.

This is a support crate rather than a host-mounted module surface, so it intentionally has no transport adapter or route manifest. Transport ownership remains with each consuming domain module.

## Entry points

- `CommentComposer`
- `CommentComposerCopy`
- `is_richtext_blank`

See the [module UI package implementation guide](../../docs/UI/module-package-implementation.md) and the [Comments implementation plan](../rustok-comments/docs/implementation-plan.md).
