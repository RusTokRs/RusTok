# rustok-pricing-storefront

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../../docs/UI/module-package-verification.md)

## Purpose

`rustok-pricing-storefront` provides the module-owned Leptos storefront route for
public pricing discovery.

## Responsibilities

- Render the public pricing atlas for published catalog entries.
- Read pricing summary and variant-level price data through native `#[server]`
  functions backed by `rustok-pricing::PricingService` through
  `HostRuntimeContext` DB/event-bus handles, without a host-framework runtime
  or framework-specific outbox adapter dependency.
- Surface effective variant pricing from the typed resolver when the route carries
  optional pricing context query values such as `currency`, `region_id`,
  `price_list_id`, `channel_id` / `channel_slug`, and `quantity`.
- Surface normalized `discount_percent` for sale rows and effective prices so
  storefront pricing UI does not have to derive sale math from `compare_at`
  fields on its own.
- Surface active tenant-scoped price lists and available channels as pricing-owned
  selectors when the route already carries an effective pricing context.
- Surfaces `seller_id` in pricing cards as the language-neutral seller boundary;
  `vendor` remains a catalog merchandising label and is not used for ownership.
- Links back to the product storefront module with `handle` when available,
  keeping display labels out of the pricing-owned route state.
- Keep the existing GraphQL storefront contract as a parallel fallback path; the
  fallback now also reads `storefrontPricingChannels` and
  `storefrontActivePriceLists(channelId, channelSlug)` from the commerce facade so
  channel-aware selector state stays aligned when native server functions are not
  available, and uses `storefrontPricingProduct` to keep variant-level
  `effective_price` parity for explicit pricing context.
- Validate pricing resolution context consistently before GraphQL selected-path execution and
  across native `#[server]` handlers: `currency_code` must be a three-letter
  ASCII code, `quantity` must be at least `1`, and `region_id`,
  `price_list_id`, or `quantity` require explicit currency; malformed explicit
  `channel_id` is rejected instead of falling back to host channel context.
- Consume the host-provided effective locale from `UiRouteContext` and resolve selected pricing-card copy against that locale before falling back to another translation.

## Entry points

- `PricingView`
- `transport::fetch_storefront_pricing`

See also `../README.md` and `../docs/README.md`.


## Authoritative quote read continuation (2026-10-29)

Effective prices use the bounded canonical Pricing batch port, retaining source-row /
rule provenance, normalized context and resolution time on both selected transports.
The public atlas uses real owner coverage (`storefrontPricingCatalog`), not fabricated
SKU counts/currency defaults or a product-detail waterfall. Raw base/scoped rows are
inspection data and do not replace an absent contextual quote. Selector URLs are
encoded by the shared query primitive; a missing channel selector means the current
host channel, not an implicit global preview.

Source-only: no tests, Rust compilation, mounted or browser execution are certified.
Durable Pricing write receipts/CAS and list-change event completeness remain pending
in the [owner plan](../docs/implementation-plan.md). Product's real sparse-axis buyer
selection, quantity refresh and cart-context handoff remain a separate next block.
