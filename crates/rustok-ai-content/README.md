# rustok-ai-content

## Purpose

`rustok-ai-content` is a domain-owned AI support crate for content verticals.

## Responsibilities

- Own content moderation AI contracts.
- Keep moderation policy wiring outside `rustok-ai` core runtime.

## Interactions

- Uses `rustok-ai` execution/runtime contracts.
- Integrates with content modules (`rustok-blog`, `rustok-forum`, `rustok-comments`).

## Entry points

- `register_content_ai_verticals`

## Docs

- [Module docs](./docs/README.md)
- Leptos admin UI scaffold: [`./admin/README.md`](./admin/README.md)
- Next.js admin UI scaffold: [`../../apps/next-admin/packages/rustok-ai-content/README.md`](../../apps/next-admin/packages/rustok-ai-content/README.md)
- [Platform docs index](../../docs/index.md)
