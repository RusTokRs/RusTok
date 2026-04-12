# RusToK Admin (Leptos)

## Purpose

`apps/admin` owns the primary Leptos-based admin application for RusToK.

## Responsibilities

- Provide the primary operator/admin UI for platform and module management.
- Host manifest-driven Leptos admin surfaces from platform modules.
- Keep the Rust-first admin stack functional in parallel with `apps/next-admin`.
- Own the Leptos host adapter for URL-owned module route-selection state.

## Entry points

- `src/main.rs`
- `src/app.rs`
- module wiring generated through `build.rs`
- generic module route `/modules/:module_slug`

## Interactions

- Uses `apps/server` through GraphQL and Leptos `#[server]` transport paths.
- Mounts module-owned Leptos admin packages from `crates/rustok-*/admin`.
- Provides route query sanitization/writer policy to `leptos-ui-routing` while `rustok-api` owns the typed admin query schema.
- Stays in functional parity work with `apps/next-admin`, but remains the primary auto-deploy admin stack.

## Docs

- [App docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
