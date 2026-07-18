# rustok-ai-product implementation plan

## Current state

`rustok-ai-product` owns descriptors and generated-payload validation for
`product_copy` and `product_attributes`. `rustok-ai` consumes the registration
API and composes execution. Product attributes receive product context through
the host-composed public `ProductCatalogReadPort`; this support crate must not
own catalog persistence or provider routing. Product copy continues to call the
product owner's explicit localized update service, rather than introducing an
AI-owned write path.

The crate also owns the `product_copywriter` and
`product_attribute_enricher` agent declarations plus the sequential,
approval-gated `product_enrichment` workflow. It validates the owner-level
`product_id` admission shape; the product direct handler remains responsible
for complete tenant, locale, and persistence validation.

## FFA/FBA readiness

- FFA status: `not_started` (no standalone support-adapter UI).
- FBA status: `boundary_ready` (`no_ui_boundary`).
- Structural shape: `no_ui_boundary`
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
host-composed `ProductCatalogReadPort`, returns validated suggestions marked
`review_required=true` and `persistence=none`, and verifies that the localized
product record is unchanged. The same direct path has explicit unavailable-port
and deadline tests: it generates from submitted prompt context, records a
typed degraded reason, skips catalog enrichment, and still cannot write product
data. Applying an attribute remains an explicit owner-owned operator action
rather than an implicit AI write.

## Next results

1. **Keep generated-output safety covered.** Product-copy has direct
   owner-persistence evidence that preserves a non-target locale; product
   attributes are explicitly review-only and cannot write a product. Extend
   these tests whenever a product-owned apply command is introduced.
2. **Composed product-agent workflow evidence is covered.**
   `service::product_agent_workflow_persistence_tests::product_enrichment_workflow_persists_owner_bindings_and_approval_gates`
   creates the product-owned principals and capability-compatible model
   assignments, validates both owner inputs, and proves the durable approval,
   lease, dependency-promotion, and completion lifecycle for
   `product_enrichment`. Its `attributes` stage executes through the canonical
   task runner and registered `product_attributes` direct handler with a
   deterministic test provider, while the `copy` handler retains its separate
   owner-persistence runtime evidence. No product-specific executor exists in
   `rustok-ai`.

## Host-composed adapter controls

`rustok-ai-product` has no standalone Leptos or Next.js screen. Product copy
and attribute controls are composed by the capability-owned `rustok-ai` admin
surfaces through the standard native task-job / parallel GraphQL contract. This
keeps product AI as an adapter and prevents a second product-owned AI route.

## Verification

- `npm run verify:ai-product:fba`
- `npm run verify:ai:domain-verticals`
- `cargo test -p rustok-ai-product --lib`

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [AI product FBA registry](../contracts/ai-product-fba-registry.json)
