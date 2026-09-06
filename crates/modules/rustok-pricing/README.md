# rustok-pricing

## Purpose

`rustok-pricing` is the default pricing submodule of the `Ecommerce` family.

## Responsibilities

- Own the pricing service, price-related migrations, and pricing runtime metadata.
- Keep the decimal money contract working on the current runtime while preserving
  compatibility with legacy `prices.amount` and `compare_at_amount` storage.
- Start Pricing 2.0 with a typed price-resolution foundation over
  `currency_code + optional region_id + optional quantity`, and now allow
  explicit `price_list_id` overlays when the requested list is active for the
  current tenant/time window, plus a normalized `discount_percent` on resolved
  sale prices; the resolver now also consumes host-provided `channel_id` /
  `channel_slug` and supports channel-scoped base rows and price lists without
  making `rustok-pricing` the owner of channel identity. Pricing resolution
  contracts validate `currency_code` as a three-letter ASCII ISO-style code,
  reject non-positive quantities, and require `currency_code` whenever a caller
  supplies resolution modifiers such as region, price list, or quantity.
  Malformed explicit `channel_id` is also rejected, and pricing UI wrappers now
  pre-validate that contract before falling back from native `#[server]`
  transport to GraphQL.
- Expose a typed percentage-adjustment preview/apply contract inside
  `PricingService`, while keeping legacy `apply_discount` as a compatibility
  wrapper over the new adjustment path; the typed path now supports both the
  canonical base row and active `price_list` override rows.
- Expose active tenant-scoped price lists as a pricing-owned read contract so
  admin and storefront selectors do not depend on raw UUID-only entry, and now
  carry typed rule metadata plus optional channel scope for fallback sale
  semantics.
- Expose first-class `price_list` percentage rules so an active list can derive
  effective sale prices from base rows even without explicit override rows.
- Keep native admin `price_list` rule/scope mutation paths aligned with runtime
  lifecycle validation, so inactive/draft, future, and expired lists plus
  channel mismatches are
  rejected without hidden fallback or unintended override-row writes.
- Provide a module-owned Leptos admin UI package in `admin/` for pricing visibility,
  sale markers, currency-coverage inspection, effective price inspection, and
  operator updates for base rows and active price-list override rows on variant prices,
  including quantity tiers and typed percentage-discount adjustments on the
  canonical base row or the selected active price-list override row.
- Provide a module-owned Leptos storefront UI package in `storefront/` for
  public pricing discovery, currency coverage, sale-marker visibility, and
  effective price previews.

## Interactions

- Depends on `rustok-commerce-foundation` for shared commerce DTOs, entities, and errors.
- Depends on `rustok-product` data model through variant references.
- Used by `rustok-commerce` as the umbrella/root module of the ecommerce family.
- Provides the owner-managed `PricingReadPort` factory consumed by durable
  commerce checkout pricing, REST/GraphQL storefront add-to-cart/repricing,
  GraphQL effective-price projection, and storefront active-price-list
  projection plus admin product-pricing projection, without exposing
  `PricingService` across those boundaries; GraphQL storefront product-pricing
  detail uses the same owner port.
- `apps/admin` consumes `rustok-pricing-admin` through manifest-driven composition,
  and now gets native module-owned base-price and active price-list override write
  actions there, plus base-row percentage-discount preview/apply and selected
  price-list rule/scope editing; the remaining backlog is the broader promotions
  engine rather than the core pricing transport path.
- The parallel `rustok-commerce` GraphQL facade now also exposes admin pricing
  write mutations for variant-price updates, percentage-discount
  preview/apply, and selected active `price_list` rule/scope updates, so
  GraphQL remains a supported transport alongside native `#[server]`
  functions instead of falling back to read-only pricing behavior.
- `apps/storefront` consumes `rustok-pricing-storefront` through manifest-driven
  composition for a public pricing atlas route.
- Current verification baseline for this slice is a wide pricing sweep across
  `pricing_service_test`, the full `graphql_runtime_parity_test`, and SSR suites
  for `rustok-pricing-admin` / `rustok-pricing-storefront`.
- `PricingWritePort` is the typed owner boundary for commerce GraphQL admin
  variant-price upserts, discount application, price-list rules, and scope writes;
  its no-compile evidence remains boundary-level rather than transport execution.

## Entry points

- `PricingModule`
- `PricingService`
- `PriceResolutionContext`
- `ResolvedPrice`
- `rustok-pricing-admin`
- `PricingView`

See also `docs/README.md`.
