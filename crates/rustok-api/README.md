# rustok-api

## Purpose
`rustok-api` is the shared web/API adapter layer for RusToK. It hosts reusable request, tenant, auth, and GraphQL-facing contracts that should be available to `apps/server` and, over time, to module crates that expose GraphQL or HTTP adapters.

## Responsibilities
- Provide reusable tenant and auth request context types.
- Provide reusable channel request context types for channel-aware runtime resolution.
- Provide thin UI host route context types when module-owned frontend packages need generic host data such as route segment, nested subpath, locale, and query params.
- Provide framework-agnostic UI input and route-query update helpers (`normalize_ui_text`, `parse_ui_csv`, `UiRouteQueryUpdate`) for FFA module UI cores.
- Provide typed route-selection schemas and sanitization helpers for host-owned URL contracts.
- Provide the framework-independent manifest-to-runtime registry comparison contract used by the server composition root.
- Provide GraphQL helper types and error helpers shared across modules.
- Provide request-level locale and tenant resolution primitives that do not belong in domain crates.
- Provide neutral port context/error primitives, policy helpers (`PortCallPolicy`) and typed error constructors for module-owned ports; `rustok-region` and migrated tenant, channel, product, customer, media, workflow, RBAC, tax, fulfillment, payment, pricing, cart, inventory, comments, search, order, index, email delivery, outbox relay, and page-builder publish paths use these shared primitives for FBA read/write boundaries.
- Own neutral permission contracts (`Permission`, `Action`, `Resource`) and platform locale normalization, matching, candidate, fallback, and `Accept-Language` parsing contracts.
- Carry typed channel-resolution diagnostics (`channel_id`, `channel_slug`, `channel_resolution_source`, `channel_resolution_trace`) from host middleware into module adapters.
- Keep web-framework-oriented dependencies out of `rustok-core` while still allowing modular reuse.
- Stay a thin shared host/API layer. It must not absorb module-specific business logic, resolvers, or controllers.
- Prevent duplicate implementations of the same web/API contract in `apps/server` or individual module crates.

## Interactions
- Used by `apps/server` as the current composition root.
- Intended to be used by module crates such as `rustok-blog`, `rustok-content`, `rustok-commerce`, and others when their GraphQL/REST adapters move out of `apps/server`.
- All feature profiles, including `server`, remain independent from `rustok-core`.
- `rustok-core` consumes API-owned contracts and adds runtime RBAC/security policy.
- Runtime-specific composition helpers remain owner-owned; outbox Loco wiring is exposed by `rustok-outbox::loco`, not this crate.

## Boundary Rules
- `apps/server` may wire and re-export `rustok-api`, but must not grow a second parallel shared API layer.
- Module crates may depend on `rustok-api` for shared host contracts, but keep module-specific transport code and domain behavior locally.
- New cross-module request/auth/GraphQL/UI host helpers should go into `rustok-api` only when they are genuinely shared and host-level.

## Entry points
- `src/lib.rs`
- `src/context/`
- `src/request.rs`
- `src/ui.rs`
- `src/route_selection.rs`
- `src/module_registry_contract.rs`
- `src/ports.rs`
- `src/permissions.rs`
- `src/locale.rs`
- `src/graphql/`

## Features

- `default = []`: neutral contracts with no core runtime dependency.
- `server`: server-side auth/request/GraphQL adapters without a core dependency.

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
