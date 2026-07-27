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

Canonical root read construction now uses `InProcessPricingReadPort`. The wrapper
retains the delegated `PortContext`, typed identity and bounded request-shape facts,
and exact local outcome context around the unchanged `PricingService` port
implementation. Known identifying validation, not-found, and conflict envelopes are
returned with stable non-identifying messages while preserving kind, code, and
retryability. The legacy module-path read factory remains an explicit compatibility
path, and the write factory remains unchanged.

The port accepts variant-first resolution when a cart snapshot has no product
id and returns the full resolved-price projection required to persist pricing
adjustments; cart storefront repricing therefore no longer calls
`PricingService::resolve_variant_price` directly.
The durable checkout pricing resolver also calls `PricingReadPort` with typed
tenant, service-actor, locale, channel, correlation, and deadline context.
REST, GraphQL, and native storefront cart repricing, plus REST and GraphQL
add-to-cart line-item resolution, use the same projection port instead of
resolving variants through `PricingService` directly. The commerce GraphQL
admin/storefront pricing roots also resolve each effective variant price through
the projection port while preserving an absent-price result as `null`.
The storefront active-price-list GraphQL root now uses a typed list projection
operation with context-derived locale and channel scope.
The admin product-pricing GraphQL root also uses a typed owner projection with
an authenticated actor, locale, channel, correlation, and deadline context.
The storefront product-pricing-by-handle GraphQL root uses the matching public
projection operation and retains its channel-visibility input.
The GraphQL discount preview uses the typed read port with an authenticated actor.
`PricingWritePort` now owns the GraphQL admin mutations for variant-price upsert,
percentage-discount application, active price-list percentage rules, and price-list
channel scope. The provider enforces deadline and idempotency semantics before it
invokes the pricing owner service, returns the saved owner projection, and preserves
the effective locale plus fallback locale for rule updates. This is static boundary
evidence only; no live provider-consumer transport execution has been recorded.

## FFA/FBA status

- FFA status: `in_progress` — the owner UI surfaces exist and must retain
  native/GraphQL parity and the core/transport/UI boundary.
- FBA status: `boundary_ready` — provider metadata and static contract evidence
  are ready, while runtime contract and fallback execution remain pending.
- Structural shape: `core_transport_ui`
- Canonical root read provider: `InProcessPricingReadPort`; owner execution remains
  `PricingService` in `ports.rs`, and `rustok_pricing::ports` is a compatibility path.
- Evidence: `crates/rustok-pricing/contracts/pricing-fba-registry.json`,
  `crates/rustok-pricing/contracts/evidence/pricing-contract-test-static-matrix.json`,
  `crates/rustok-pricing/contracts/evidence/pricing-runtime-contract-smoke.json`,
  `crates/rustok-pricing/docs/read-local-context.md`,
  `scripts/verify/verify-pricing-read-local-context.mjs`,
  `scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs`,
  `scripts/verify/verify-pricing-admin-boundary.mjs`, and
  `scripts/verify/verify-pricing-storefront-boundary.mjs`.

## Open results

1. Execute `PricingReadPort` against live persistence for
   `resolve_product_price` and `read_price_list_projection`, including the
   declared embedded and GraphQL fallback profiles. The owner has a targeted
   SQLite integration test for the two successful read operations and
   missing-deadline rejection; the in-process provider test passed. Done when
   the observed calls also prove the consumer degraded modes rather than only
   static markers.
   Dependency: runnable commerce consumer composition for the declared
   fallback profiles. Verification: `npm run verify:ecommerce:fba` plus
   `cargo test -p rustok-pricing --test pricing_read_port_runtime`.
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
4. Prove canonical pricing diagnostics and retire compatibility bypasses.
   **Status:** source-complete / unvalidated for root read construction. Execute
   invalid-context, product/variant mismatch, missing price/list, duplicate identity,
   inventory conflict, storage, and invariant scenarios and retain traces proving that
   raw handles, SKUs, UUID-bearing messages, quantities, currencies, and prices do not
   cross the canonical boundary. Audit direct `rustok_pricing::ports` callers and
   either migrate or explicitly accept them.

## Verification

- `npm run verify:pricing:admin-boundary`
- `npm run verify:pricing:storefront-boundary`
- `node scripts/verify/verify-pricing-read-local-context.mjs`
- `npm run verify:ecommerce:fba`
- `cargo test -p rustok-pricing --test pricing_read_port_runtime`

## Boundaries

- Pricing owns resolution, price-list/rule lifecycle, and pricing UI policy.
- Product owns catalog and variant data; commerce owns orchestration and any
  multi-layer promotions workflow.
- Hosts only compose owner UI packages and pass effective locale, channel, and
  runtime context without creating package-local fallback chains.
- Keep raw handles, SKUs, currency values, prices, percentages, and returned pricing
  rows out of owner diagnostics; retain only typed identity and bounded shape facts.
