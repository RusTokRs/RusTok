# rustok-ai-content

## Purpose

`rustok-ai-content` is a domain-owned AI support crate for content verticals.

## Responsibilities

- Own content moderation AI contracts.
- Own blog draft generated payload contracts and validation for all optional text fields.
- Keep content policy wiring outside `rustok-ai` core runtime.

## Interactions

- Uses `rustok-ai` execution/runtime contracts.
- Integrates with content modules (`rustok-blog`, `rustok-forum`, `rustok-comments`).

## Entry points

- `register_content_ai_verticals`
- `register_content_ai_vertical_handlers`
- `validate_blog_draft_payload`
- `validate_moderation_decision`
- `content_ai_policy_matrix`

## Docs

- [Module docs](./docs/README.md)
- Adapter controls are composed by the `rustok-ai` Leptos and Next.js admin
  surfaces; this support crate does not expose a standalone content-admin route.
- [Platform docs index](../../docs/index.md)
