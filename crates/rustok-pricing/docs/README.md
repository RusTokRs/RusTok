# `rustok-pricing` Documentation

`rustok-pricing` — default pricing submodule of the `ecommerce` family.

## Purpose

- price-related service logic;
- pricing migrations;
- `PricingModule` and `PricingService`;
- typed resolver foundation `PriceResolutionContext -> ResolvedPrice` for
  deterministic price selection by `currency_code`, optional `region_id` and optional
  `quantity`, as well as explicit `price_list_id` overlay for active price lists
  without introducing a full promotions layer; the read-side now also returns
  a normalized `discount_percent` for sale rows and effective prices; the current
  resolver also accounts for host-provided `channel_id` / `channel_slug` and
  can select channel-scoped base rows / active price lists without transferring
  ownership of channel identity into the pricing boundary; contract validation requires
  a three-letter ASCII `currency_code`, rejects `quantity < 1`, does not allow
  passing `region_id`, `price_list_id` or `quantity` without `currency_code` and
  also rejects malformed explicit `channel_id`; pricing UI wrappers meanwhile
  validate this contract before falling back from native `#[server]` transport to GraphQL;
- typed percentage-adjustment contract in `PricingService`: the preview/apply helper
  for percent-based sale mutation now lives in the pricing boundary, and the legacy
  `apply_discount` remains a compatibility wrapper over the canonical base-price row;
  the typed adjustment path can now target not only the base row but also active
  `price_list` override rows, including channel-scoped canonical rows;
- pricing-owned read contract for active tenant-scoped price lists, so
  admin/storefront surfaces do not live on raw UUID-only selector semantics; now
  this read contract also carries typed rule metadata;
- first-class `price_list` percentage rules, so an active list can provide
  promotion-ready sale semantics over base-price rows even without explicit override rows;
- transport parity for admin-side `price_list` rule/scope mutation paths:
  future/expired lists and channel-scope mismatch must now be rejected without
  hidden fallback and without side-effect writes/mutations of override rows;
- the active price lists selector does not drift after scope save/clear: channel-bound
  lists disappear from other channels and return after scope removal;
- module-owned admin UI package `rustok-pricing/admin` for price visibility,
  sale markers, currency coverage inspection and operator-side effective price preview by
  `currency + optional region_id + optional quantity` via native-first `#[server]`
  transport with GraphQL fallback, as well as for authoring base variant price rows and
  active `price_list_id` override rows, including quantity tiers by `min_quantity` /
  `max_quantity`, and now also for typed percentage-discount preview/apply over
  canonical base row or selected active `price_list` override, plus for editing
  selected active `price_list` rule and channel scope on variant price rows / active
  price lists; channel scope authoring now takes selector options from
  `rustok-channel` read model, not from raw UUID/slug text inputs; the active
  `price_list` selector in the admin effective context also now
  recalculates from the explicitly selected `channel_id` / `channel_slug`, rather than
  living on a bootstrap snapshot of the host context;
- module-owned storefront UI package `rustok-pricing/storefront` for public pricing
  discovery, currency coverage, sale-marker visibility and effective price preview by
  optional route context (`currency`, `region_id`, `price_list_id`, `channel_id`,
  `channel_slug`, `quantity`) via native server functions;
  the effective context for channel-aware pricing is not constructed from a package-local
  fallback chain: locale remains host-owned, and channel override arrives only as an
  explicit route/server-function input or from the host `RequestContext`; the GraphQL fallback
  meanwhile also receives `available_channels` and channel-aware active
  `price_lists` via the storefront facade fields `storefrontPricingChannels` and
  `storefrontActivePriceLists(channelId, channelSlug)`, rather than degrading to an empty
  selector state; the pricing detail fallback also no longer lives on a generic
  catalog product contract and uses dedicated facade roots `storefrontPricingProduct`
  and `adminPricingProduct` to maintain `effective_price` parity for explicit
  `currency/price_list/channel/quantity` context; these facade roots validate the
  resolution context as strictly as `PricingService`, so context modifiers
  without `currencyCode` are not silently ignored; the generic `product` /
  `storefrontProduct` should be treated only as a catalog snapshot
  contract, even if they still carry `variants.prices` for compatibility;

## Scope

- runtime dependency: `product`;
- the module owns the pricing boundary and the operator UI surface for prices, including
  the base-price write path for variant pricing;
- the module now also owns the public storefront read-side pricing surface,
  which builds a pricing atlas over the published catalog and variant-level prices;
- the current active resolver uses deterministic precedence
  `explicit override row -> active price_list rule -> base prices`,
  then `exact region -> global` and `higher min_quantity -> lower max_quantity`;
  promotions over multiple list layers and outside the price-list boundary still
  remain a separate follow-up;
- GraphQL and REST transport for promotions/rules still remain in the
  `rustok-commerce` facade, but the base pricing write path and active price-list override authoring for admin are now moved to
  module-owned `rustok-pricing/admin` via native `#[server]` transport; the
  typed base-row percentage adjustment path with preview/apply semantics is also
  wired in; the parallel GraphQL facade now also holds admin pricing write surface
  for variant price updates, typed percentage-discount preview/apply and selected
  active `price_list` rule/scope updates, not just pricing-authoritative
  read roots;
- shared DTOs, entities and error surface come from `rustok-commerce-foundation`.

## Integration

- the module is part of the ecommerce family and must maintain its own storage/runtime boundary
  without returning responsibility to the umbrella `rustok-commerce`;
- transport and GraphQL are still published through `rustok-commerce`, while the pricing-owned admin/storefront
  UX is already published through `rustok-pricing/admin` and `rustok-pricing/storefront`,
  and the admin surface has already switched to native-first `#[server]`
  data layer with GraphQL fallback;
- cross-module contract changes must be synchronized with `rustok-commerce`
  and neighboring split modules.

## Verification

- `cargo xtask module validate pricing`
- `cargo xtask module test pricing`
- targeted commerce tests for the pricing domain when changing runtime wiring
- the current broad verification baseline for the pricing slice includes
  `pricing_service_test`, full `graphql_runtime_parity_test` and SSR suites
  `rustok-pricing-admin` / `rustok-pricing-storefront`

## Related documents

- [README crate](../README.md)
- [README admin package](../admin/README.md)
- [Commerce split plan](../../rustok-commerce/docs/implementation-plan.md)

## FFA separation for admin and storefront

The admin package now uses the `admin/src/transport.rs` facade and an explicit Leptos rendering adapter `admin/src/ui/leptos.rs`; the crate root only connects module layers and re-exports `PricingAdmin`. The storefront package already maintains the separation of `storefront/src/core.rs`, `storefront/src/transport/` and `storefront/src/ui/leptos.rs` with parity between native-first and GraphQL fallback.
