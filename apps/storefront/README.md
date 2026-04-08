# RusToK Storefront (Leptos)

## Purpose

`apps/storefront` owns the Leptos-based storefront application for RusToK.

## Responsibilities

- Provide the Rust-first SSR storefront host.
- Mount module-owned storefront Leptos packages through manifest-driven wiring.
- Keep the Leptos storefront path aligned with `apps/next-frontend` at the contract level.

## Entry points

- `src/main.rs`
- `src/app.rs`
- `src/modules/*`
- generic module route `/modules/{route_segment}`

## Interactions

- Uses `apps/server` through GraphQL and Leptos `#[server]` transport paths.
- Mounts module-owned storefront packages from `crates/rustok-*/storefront`.
- Stays in architectural parity with `apps/next-frontend` while remaining the Rust-first storefront host.

## Docs

- [App docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
