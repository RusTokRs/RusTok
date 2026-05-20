# rustok-ai-order

## Purpose

`rustok-ai-order` is a domain-owned AI support crate for order verticals.

## Responsibilities

- Own order AI vertical contracts (`order_analytics`, `order_ops_assistant`).
- Keep order automation/suggestion logic outside `rustok-ai` core runtime.

## Interactions

- Uses `rustok-ai` execution runtime.
- Integrates with `rustok-order` and `rustok-commerce` contracts.

## Entry points

- `register_order_ai_verticals`

## Docs

- [Module docs](./docs/README.md)
- Leptos admin UI scaffold: [`./admin/README.md`](./admin/README.md)
- Next.js admin UI scaffold: [`../../apps/next-admin/packages/rustok-ai-order/README.md`](../../apps/next-admin/packages/rustok-ai-order/README.md)
- [Platform docs index](../../docs/index.md)
