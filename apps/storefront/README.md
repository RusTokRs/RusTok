# RusToK Storefront (Leptos)

## Purpose

`apps/storefront` owns the Leptos-based storefront application for RusToK.

## Responsibilities

- Provide the Rust-first SSR storefront host.
- Mount module-owned storefront Leptos packages through manifest-driven wiring.
- Keep the Leptos storefront path aligned with `apps/next-frontend` at the contract level.

## Entry points

- `src/main.rs`
- `src/lib.rs` — module and canonical Pages route composition
- `src/modules/*`
- generic module route `/modules/{route_segment}`

## Interactions

- Uses `apps/server` through GraphQL and Leptos `#[server]` transport paths.
- Composes cross-module storefront workflows only through public owner contracts; `SearchStorefrontComposition` connects product-owned public catalog options to search-owned UI props.
- Mounts module-owned storefront packages from `crates/modules/rustok-*/storefront`.
- Consumes tenant-aware SEO runtime from `rustok-seo` through the shared Rust renderer `rustok-seo-render`.
- Stays in architectural parity with `apps/next-frontend` while remaining the Rust-first storefront host.

## Known Limitations / Pending Implementation

- The Page Builder landing bridge has no mounted Leptos Cart event consumer or Cart-count
  publisher. Landing Cart controls therefore report unavailable and counts remain unknown until
  the Cart owner wires and verifies that boundary; see the [storefront contract](../../crates/modules/rustok-page-builder/storefront/README.md#purchase-and-cart-bridge).
- The separate Product buyer island does not receive the host's Cart-enabled state, so it can offer
  an add while Cart is disabled. This is a release blocker pending Product/Cart/host capability
  wiring and server-side admission verification; the Next buyer's guard does not cover this host.
- Home and module SSR pass the request nonce to trusted module styles/scripts, including a Pages
  landing rendered in a home slot; nonce-bearing HTML responses are `private, no-store`. Canonical
  Pages compositions with a nonce cannot return a cached ETag/304. The new host response and
  module SSR nonce tests have not yet been run in the real workspace; mounted hydration, navigation
  and enforced-CSP behavior also need CI/maintainer verification. Synthetic browser tests do not
  certify the deployed host.

## Docs

- [App docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
