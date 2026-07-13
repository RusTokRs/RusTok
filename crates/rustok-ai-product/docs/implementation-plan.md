# rustok-ai-product implementation plan

## Current state

`rustok-ai-product` owns descriptors and generated-payload validation for
`product_copy` and `product_attributes`. `rustok-ai` consumes the registration
API and composes execution. Product context remains owned by `rustok-product`
through `ProductCatalogReadPort`; this support crate must not own catalog
persistence or provider routing.

The crate also owns the `product_copywriter` and
`product_attribute_enricher` agent declarations plus the sequential,
approval-gated `product_enrichment` workflow. It validates the owner-level
`product_id` admission shape; the product direct handler remains responsible
for complete tenant, locale, and persistence validation.

## FFA/FBA readiness

- FFA status: `in_progress`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Structural shape: `core_transport_ui`
- `ProductCatalogReadPort` / `product.catalog_read.v1` provides
  `read_product_projection`. Degraded behavior is `generate_from_prompt_only`,
  `skip_catalog_enrichment`, and `require_operator_review`.
- Evidence: `crates/rustok-ai-product/contracts/ai-product-fba-registry.json`,
  `crates/rustok-ai-product/contracts/evidence/ai-product-consumer-static-matrix.json`,
  `crates/rustok-ai-product/contracts/evidence/ai-product-runtime-fallback-smoke.json`,
  and `scripts/verify/verify-ai-product-fba.mjs`.

## Next results

1. **Execute the catalog-read consumer contract.** Add a composed runtime test
   for projection reads, deadline/error propagation, and every declared
   degraded behavior. Done when the static matrix has concrete runtime evidence
   for the `rustok-ai` consumer.
2. **Prove localized write safety for generated output.** Cover product-copy
   and attributes through the direct runtime up to the owner persistence
   boundary, including locale resolution and operator review where catalog
   context is unavailable. Done when output cannot silently overwrite a
   different locale or bypass the product owner.
3. **Render the owner-admin package in its hosts.** Connect the existing
   core/transport/UI package to admin routes and verify native server functions
   with parallel GraphQL/headless parity. Done when host-level evidence covers
   both paths.
4. **Exercise the composed product-agent workflow.** Cover principal/model
   assignment, stage admission approval, canonical direct task execution, and
   product-owner validation without introducing a product-specific executor in
   `rustok-ai`.

## Verification

- `npm run verify:ai-product:fba`
- `npm run verify:ai:domain-verticals`
- `cargo test -p rustok-ai-product --lib`

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [AI product FBA registry](../contracts/ai-product-fba-registry.json)
