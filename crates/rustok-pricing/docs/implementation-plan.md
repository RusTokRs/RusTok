# Implementation Plan for `rustok-pricing`

## Current state

`rustok-pricing` owns price resolution, price lists, pricing rules and scopes,
and its admin/storefront UI packages. The resolver already covers the active
price-list overlay, channel-aware context, typed percentage adjustments, and
the deterministic base-row precedence rules. Admin and storefront use
module-owned core/transport/Leptos layers; native server functions are host-neutral
and retain GraphQL as the parallel selected path.

`PricingReadPort` / `pricing.read_projection.v1` is implemented by
`PricingService`. The contract registry and no-compile smoke lock deadline
policy, typed error mapping, and declared fallback profiles, but the FBA
provider has not yet been executed against live persistence or a remote
consumer path.
The port accepts variant-first resolution when a cart snapshot has no product
id and returns the full resolved-price projection required to persist pricing
adjustments; cart storefront repricing therefore no longer calls
`PricingService::resolve_variant_price` directly.

## FFA/FBA status

- FFA status: `in_progress` — the owner UI surfaces exist and must retain
  native/GraphQL parity and the core/transport/UI boundary.
- FBA status: `in_progress` — provider metadata and static contract evidence
  are ready, while runtime contract and fallback execution remain pending.
- Structural shape: `core_transport_ui`
- Evidence: `crates/rustok-pricing/contracts/pricing-fba-registry.json`,
  `crates/rustok-pricing/contracts/evidence/pricing-contract-test-static-matrix.json`,
  `crates/rustok-pricing/contracts/evidence/pricing-runtime-contract-smoke.json`,
  `scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs`,
  `scripts/verify/verify-pricing-admin-boundary.mjs`, and
  `scripts/verify/verify-pricing-storefront-boundary.mjs`.

## Open results

1. Execute `PricingReadPort` against live persistence for
   `resolve_product_price` and `read_price_list_projection`, including the
   declared embedded and GraphQL fallback profiles. Done when the observed
   calls prove deadline handling, owner invocation, typed error mapping, and
   consumer degraded modes rather than only static markers.
   Dependency: runnable pricing persistence and commerce consumer composition.
   Verification: `npm run verify:ecommerce:fba` plus targeted port/runtime
   tests.
2. Complete the dedicated pricing transport handoff from the umbrella
   `rustok-commerce` facade. Done when the owner exposes its selected public
   transport contract directly and commerce composes it without re-exporting
   pricing services, DTOs, or entity aliases.
   Dependency: an approved atomic public-contract migration. Verification:
   `npm run verify:pricing:admin-boundary` and
   `npm run verify:pricing:storefront-boundary`.
3. Finish the remaining Pricing 2.0 rule semantics: tiers, adjustments, and
   deterministic rounding across active price-list rules. Done when resolution
   tests cover precedence and rounding for every supported context; multi-layer
   promotions orchestration remains owned by `rustok-commerce`.
   Dependency: the stable owner transport and product variant data. Verification:
   targeted pricing resolution and money-semantics tests.

## Verification

- `npm run verify:pricing:admin-boundary`
- `npm run verify:pricing:storefront-boundary`
- `npm run verify:ecommerce:fba`

## Boundaries

- Pricing owns resolution, price-list/rule lifecycle, and pricing UI policy.
- Product owns catalog and variant data; commerce owns orchestration and any
  multi-layer promotions workflow.
- Hosts only compose owner UI packages and pass effective locale, channel, and
  runtime context without creating package-local fallback chains.
