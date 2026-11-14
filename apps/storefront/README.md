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

- The shared Cart/Product client now has a source-level landing event consumer and count
  publisher, and Product receives the same host-supplied Cart capability as the drawer. These
  paths remain release blockers until the locked native/WASM builds and real mounted tenant,
  auth, navigation, in-flight and server-admission tests pass. See the [storefront contract](../../crates/modules/rustok-page-builder/storefront/README.md#purchase-and-cart-bridge).
- The client is included only with built Product buyer assets and a request CSP nonce. Without
  it, the SSR Cart trigger remains disabled and landing controls must report unavailable. A
  stopped or externally navigated owner does not remount in the same document; reload is required,
  and reload itself does not prove cancellation of an earlier transmitted request.
- Home and module SSR pass the request nonce to trusted module styles/scripts, including a Pages
  landing rendered in a home slot; nonce-bearing HTML responses are `private, no-store`. Canonical
  Pages compositions with a nonce cannot return a cached ETag/304. The new host response and
  module SSR nonce tests have not yet been run in the real workspace; mounted hydration, navigation
  and enforced-CSP behavior also need CI/maintainer verification. Synthetic browser tests do not
  certify the deployed host.

## Docs

- [App docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
