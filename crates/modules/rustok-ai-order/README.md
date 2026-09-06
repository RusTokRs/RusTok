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


## Dependency boundary (re-verified)

- `rustok-ai-order` intentionally depends only on `serde`/`serde_json` and owns pure domain contracts.
- Provider calls, model runtime, and direct handler execution remain in `rustok-ai` (server runtime crate).
- Integration with `rustok-order` / `rustok-commerce` is currently **contract-level** through runtime flows, not direct compile-time coupling in this crate.

## Docs

- [Module docs](./docs/README.md)
- Adapter controls are composed by the `rustok-ai` Leptos and Next.js admin
  surfaces; this support crate does not expose a standalone order-admin route.
- [Platform docs index](../../docs/index.md)
