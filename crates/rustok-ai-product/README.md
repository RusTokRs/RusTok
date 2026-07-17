# rustok-ai-product

## Purpose

`rustok-ai-product` is a domain-owned AI support crate for product verticals.

## Responsibilities

- Hold product-scoped AI vertical contracts (`product_copy`, `product_attributes`),
  product-agent/workflow declarations, and generated payload validators.
- Keep product AI logic owned by product/ecommerce domain instead of `rustok-ai` core runtime.
- Provide registration and validation seams for product AI handlers.

## Interactions

- Exposes generated payload contracts consumed by the `rustok-ai` runtime/orchestrator execution host.
- Integrates with `rustok-product` / `rustok-commerce` service contracts.

## Entry points

- `register_product_ai_verticals`
- `product_ai_agents`, `product_ai_workflows`, `validate_product_agent_stage_input`
- `GeneratedProductCopy`, `GeneratedProductAttributes`, `GeneratedFlexAttribute`
- `validate_product_copy_payload`, `validate_product_attributes_payload`

## Docs

- [Module docs](./docs/README.md)
- Product AI controls are composed by the capability-owned `rustok-ai` admin
  surfaces. This support adapter owns no standalone Leptos or Next.js route.
- [Platform docs index](../../docs/index.md)
