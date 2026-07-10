# Implementation Plan for `rustok-product`

## Current state

`rustok-product` owns the catalog, variants, category-bound attribute schemas,
typed attribute values, and product admin/storefront packages. Product UI uses
owner-owned core, transport, and Leptos adapter layers. Native server functions
use `HostRuntimeContext` and a typed event bus; GraphQL remains the parallel
selected path. The product packages contain no package-local Loco or outbox
Loco-adapter dependency.

`ProductCatalogReadPort` / `product.catalog_read.v1` is implemented by
`CatalogService`. `boundary_ready` on no-compile runtime fallback evidence is
supported by the provider registry, static contract matrix, and fallback smoke.
`transport_verified` still requires live provider execution evidence.
Product runtime contract, commerce transport, and module metadata remain synchronized.
The category-bound admin transport keeps native server functions as the
internal path and parallel GraphQL operations for the public/headless path.
The DB-level tenant consistency audit, `VARCHAR(32)` locale storage, optional catalog filters/sorts, detached-value marker contract, and no-compile schema guardrail are source-locked.

## FFA/FBA status

- FFA status: `in_progress` — both owner UI surfaces exist and must preserve
  the core/transport/UI split and native/GraphQL parity.
- FBA status: `boundary_ready` — read-port policy, metadata, and fallback
  profiles are source-locked; no persistence-backed execution has been shown.
- Evidence: `crates/rustok-product/contracts/product-fba-registry.json`,
  `crates/rustok-product/contracts/evidence/product-runtime-contract-smoke.json`,
  `crates/rustok-product/contracts/evidence/product-runtime-fallback-smoke.json`,
  `scripts/verify/verify-product-runtime-fallback-smoke.mjs`,
  `scripts/verify/verify-product-admin-boundary.mjs`,
  `scripts/verify/verify-product-storefront-boundary.mjs`, and
  `scripts/verify/verify-ai-product-fba.mjs` for the AI consumer contract.

## Open results

1. Execute `ProductCatalogReadPort` against persistence for
   `read_product_projection` and `list_published_products`. Done when real
   calls prove read-policy ordering, tenant and locale handling, bounded
   pagination, and typed `PortError` mapping rather than source markers.
   Dependency: a runnable product persistence environment. Verification:
   `npm run verify:product:runtime-fallback-smoke` plus targeted port tests.
2. Prove the consumer profiles with observed fallback behaviour before changing
   FBA status to `transport_verified`. Done when commerce checkout/storefront,
   pricing enrichment, and `rustok-ai` product context each exercise their
   declared fallback or degraded mode against the live provider.
   Dependency: priority 1 and the respective consumer composition. Verification:
   `npm run verify:ecommerce:fba` and `npm run verify:ai-product:fba`.

## Verification

- `npm run verify:product:runtime-fallback-smoke`
- `npm run verify:product:admin-boundary`
- `npm run verify:product:storefront-boundary`
- `npm run verify:ecommerce:fba`

## Boundaries

- Product owns catalog data and the `ProductCatalogReadPort` implementation.
- `rustok-commerce`, pricing, and AI consume public product contracts; they do
  not regain catalog service, DTO, or entity ownership.
- Hosts compose product UI packages and pass the effective locale and runtime
  context without adding a package-local locale or transport fallback.
