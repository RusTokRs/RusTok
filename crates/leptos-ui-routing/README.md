# leptos-ui-routing

## Purpose

`leptos-ui-routing` is a thin shared Leptos routing/query helper crate for RusToK UI packages.

## Responsibilities

- Provide reusable query read/write helpers for Leptos module-owned UI packages.
- Keep route-state plumbing reusable across admin and storefront Leptos packages.
- Stay generic: no admin-specific query schema, no domain logic, no data fetching, no i18n policy.
- Consume host-provided routing policy through context instead of owning route contracts itself.

## Interactions

- Consumed by module-owned Leptos UI crates in both `crates/*/admin` and `crates/*/storefront`.
- Works with `rustok-api`, which owns route/query schemas and host route context contracts.
- Works with host apps such as `apps/admin` and `apps/storefront`, which provide the route sanitization/write policy.

## Entry points

- `src/lib.rs`

## Boundary Rules

- This crate must not define admin-only or storefront-only key names.
- This crate must not embed business validation, module-specific invariants, or locale policy.
- Host apps own adapters and sanitization policy; `rustok-api` owns typed route contracts.

## Docs

- [Platform docs index](../../docs/index.md)
- [UI docs](../../docs/UI/README.md)
- [Routing architecture](../../docs/architecture/routing.md)
