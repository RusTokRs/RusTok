# Leptos Storefront Documentation

Local documentation for `apps/storefront` as the Leptos SSR-host storefront application.

## Purpose

`apps/storefront` is the Rust-first SSR-first storefront host for RusToK. The application renders shell, home page, generic module pages, and mounts module-owned storefront UI through manifest-driven wiring.

FFA classification: `apps/storefront` is an `FFA-compatible composition host`, not a module-owned UI package. Its FFA responsibility is to maintain storefront shell/routing/context composition and not move module-specific storefront workflows from owner packages into the host.

The first host-level FFA slice has already been applied to the storefront header: route/link policy lives in
`src/widgets/header/core.rs` without Leptos dependencies, while `header/mod.rs` remains the Leptos
render adapter. This split is enforced by the fast verifier `npm run verify:frontend:host-ffa-contract`.

## Responsibility boundaries

- own the Leptos storefront host and its SSR/runtime wiring;
- mount module-owned storefront packages from `crates/rustok-*/storefront`;
- maintain the generic route contract for storefront modules;
- pass `UiRouteContext` and effective locale to module-owned packages;
- not pull module business UI and module transport contracts into the host.

## Runtime contract

- GraphQL transport is not removed and remains a mandatory external contract.
- Native Leptos `#[server]` functions are used as the preferred internal data-layer path in the SSR/hydrate runtime in parallel with GraphQL.
- CSR/WASM for Leptos storefront packages is a compatibility/debug profile. If a package must run standalone, it must have a GraphQL/REST fallback and not require `/api/fn/*`.
- Generic storefront routes live under the `/modules/{route_segment}` and `/{locale}/modules/{route_segment}` families.
- The host first tries to use the native `#[server]` path where available in the SSR/hydrate runtime, and only falls back to GraphQL.
- Generated search mount uses host-owned `SearchStorefrontComposition`: the adapter checks tenant enablement of the `product` module, passes `UiRouteContext.locale` to a public-safe product metadata helper, and maps owner DTO to search props without moving product/search domain logic into the host.
- Module-owned storefront packages must build internal links through `UiRouteContext::module_route_base()`, not through hardcoded route strings.
- Module-owned storefront packages do not define their own locale negotiation policy; the effective locale comes from the host/runtime contract.
- Module-owned Leptos storefront packages read query/state through the common helper layer `leptos-ui-routing`,
  not through package-local direct access to `UiRouteContext.query_value(...)`.

## Module-owned storefront surfaces

This contract is currently used for at least:

- `rustok-pages-storefront`
- `rustok-blog-storefront`
- `rustok-cart-storefront`
- `rustok-commerce-storefront`
- `rustok-fulfillment-storefront`
- `rustok-order-storefront`
- `rustok-payment-storefront`
- `rustok-pricing-storefront`
- `rustok-product-storefront`
- `rustok-region-storefront`
- `rustok-forum-storefront`
- `rustok-search-storefront`

Build-time wiring is generated from `modules.toml` and `rustok-module.toml` through `apps/storefront/build.rs`.
Checkout composition uses separate platform-known slots `checkout_shipping_handoff`,
`checkout_payment_handoff` and `checkout_result_handoff`; the host passes the effective locale through
`UiRouteContext`, while module packages resolve strings from their own manifest-declared directories.

## Data access

Direct storefront server functions currently cover:

- `list-enabled-modules`
- `resolve-canonical-route`
- `storefront/seo-page-context`
- `pages/storefront-data`
- `blog/storefront-data`
- `cart/storefront-data`
- `cart/decrement-line-item`
- `cart/remove-line-item`
- `commerce/storefront-data`
- `commerce/create-payment-collection`
- `commerce/complete-checkout`
- `pricing/storefront-data`
- `pricing/storefront-data` can now also show an effective pricing preview,
  if the storefront route carries optional query context (`currency`, `region_id`, `price_list_id`, `quantity`),
  and exposes a pricing-owned selector of active price lists over this context;
- `product/storefront-data`
- `region/storefront-data`
- `forum/storefront-data`
- `search/storefront-search`
- `search/storefront-filter-presets`
- `search/storefront-suggestions`
- `search/storefront-track-click`

The GraphQL path remains a working and supported fallback contract for module-owned storefront surfaces, `cart/storefront-data` now serves the cart-owned cart workspace with a seller-aware delivery-group snapshot, `cart/decrement-line-item` and `cart/remove-line-item` provide a safe line-item write-side within the cart boundary, and `commerce/storefront-data`, `commerce/select-shipping-option`, `commerce/create-payment-collection`, and `commerce/complete-checkout` serve the aggregate checkout workspace in `rustok-commerce/storefront`, maintaining the seller-aware shipping selection contract end-to-end.

## Canonical routing and locale

- Canonical and alias state is stored in backend/domain layers, not in the storefront host.
- Storefront uses SEO preflight before rendering a page: it first reads `SeoPageContext`, and the canonical-only path remains a fallback branch.
- Consume policy is fixed as deterministic `#[server]` first + GraphQL fallback; on transport errors the host preserves the SSR render path without breaking the route contract.
- `SeoPageContext` is split into `route` and `document`: the route part handles redirect/canonical/hreflang, the document part handles typed SSR head metadata.
- `SeoPageContext.document.structured_data_blocks` contains typed JSON-LD blocks (`schema_kind`, `schema_type`, `source`, payload), not host-local raw schema mapping.
- `storefront/seo-page-context` on SSR now also passes the host `RequestContext.channel_slug` to `rustok-seo`, so channel-restricted forum topics receive SEO head only in the matching public channel.
- Rust-side head serialization is extracted to `rustok-seo-render`, so the host does not maintain its own second renderer over the same SEO contract.
- Locale-prefixed routes are the primary route contract.
- Host locale normalization goes through the shared `rustok_core::normalize_locale_tag`, not through package-local rules.
- Legacy query-based locale fallback is only allowed as a backward-compatible path.

## SEO parity evidence

- Leptos Storefront is the Rust-host reference for SSR SEO rendering: it consumes `SeoPageContext` through `storefront/seo-page-context` and serializes the head via `rustok-seo-render`.
- D8/D9 compile-free evidence is seeded in the Next storefront fixture so route ownership and long-tail differences stay explicit across Rust and Next hosts.
- Final closeout still requires live SSR smoke evidence for runtime page context, robots/canonical/hreflang metadata and structured-data blocks against a running backend.

## Interactions

- `apps/server` provides GraphQL and Leptos server-function surfaces.
- `crates/rustok-*` publish module-owned storefront packages and runtime transport contracts.
- `apps/next-frontend` is a parallel storefront host and must maintain parity at the contract level, not at the literal code structure level.
- `leptos-ui-routing` serves as the common Leptos route/query plumbing for both admin and storefront;
  the storefront host must not duplicate this layer with a separate Rust helper crate.

## Verification

- `npm.cmd run verify:storefront:routes`
- storefront-specific targeted smoke/contract runs on module-owned surfaces
- when changing manifest wiring, cross-reference with `docs/modules/manifest.md`

## Related documents

- [Implementation plan](./implementation-plan.md)
- [Storefront architecture notes](../../../docs/UI/storefront.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)
- [ADR: SSR-first Leptos hosts with headless parity](../../../DECISIONS/2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md)
- [Documentation map](../../../docs/index.md)
