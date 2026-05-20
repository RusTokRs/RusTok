# rustok-ai-product

## Purpose

`rustok-ai-product` is a domain-owned AI support crate for product verticals.

## Responsibilities

- Hold product-scoped AI vertical contracts (`product_copy`, `product_attributes`).
- Keep product AI logic owned by product/ecommerce domain instead of `rustok-ai` core runtime.
- Provide registration and validation seams for product AI handlers.

## Interactions

- Uses `rustok-ai` runtime/orchestrator as execution host.
- Integrates with `rustok-product` / `rustok-commerce` service contracts.

## Entry points

- `register_product_ai_verticals`

## Docs

- [Module docs](./docs/README.md)
- Leptos admin UI scaffold: [`./admin/README.md`](./admin/README.md)
- Next.js admin UI scaffold: [`../../apps/next-admin/packages/rustok-ai-product/README.md`](../../apps/next-admin/packages/rustok-ai-product/README.md)
- [Platform docs index](../../docs/index.md)
