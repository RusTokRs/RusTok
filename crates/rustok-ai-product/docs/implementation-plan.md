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

## Completed direct-execution evidence

`direct::tests::direct_product_copy_updates_only_the_requested_locale_through_catalog_owner`
executes `product_copy` through the composed `rustok-ai` handler and the public
`rustok-product::CatalogService`. Its SQLite fixture contains English and Russian
translations; after a Russian request, the test proves that the Russian owner
translation changes while the English title and handle remain unchanged. This
keeps generated copy inside the product owner boundary and does not add an
AI-specific product port or persistence path.

`direct::tests::direct_product_attributes_returns_review_only_suggestions_without_product_write`
proves the complementary attributes boundary. It reads the product through the
owner service, returns validated suggestions marked `review_required=true` and
`persistence=none`, and verifies that the localized product record is unchanged.
Applying an attribute remains an explicit owner-owned operator action rather
than an implicit AI write.

## Next results

1. **Execute the catalog-read consumer contract.** Add a composed runtime test
   for projection reads, deadline/error propagation, and every declared
   degraded behavior. Done when the static matrix has concrete runtime evidence
   for the `rustok-ai` consumer.
2. **Keep generated-output safety covered.** Product-copy has direct
   owner-persistence evidence that preserves a non-target locale; product
   attributes are explicitly review-only and cannot write a product. Extend
   these tests whenever a product-owned apply command is introduced.
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
