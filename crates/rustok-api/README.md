# rustok-api

## Purpose
`rustok-api` is the shared web/API adapter layer for RusToK. It hosts reusable request, tenant, auth, and GraphQL-facing contracts that should be available to `apps/server` and, over time, to module crates that expose GraphQL or HTTP adapters.

## Responsibilities
- Provide reusable tenant and auth request context types.
- Provide reusable channel request context types for channel-aware runtime resolution.
- Provide thin UI host route context types when module-owned frontend packages need generic host data such as route segment, nested subpath, locale, and query params.
- Provide GraphQL helper types and error helpers shared across modules.
- Provide request-level locale and tenant resolution primitives that do not belong in domain crates.
- Carry typed channel-resolution diagnostics (`channel_id`, `channel_slug`, `channel_resolution_source`) from host middleware into module adapters.
- Keep web-framework-oriented dependencies out of `rustok-core` while still allowing modular reuse.
- Stay a thin shared host/API layer. It must not absorb module-specific business logic, resolvers, or controllers.
- Prevent duplicate implementations of the same web/API contract in `apps/server` or individual module crates.

## Interactions
- Used by `apps/server` as the current composition root.
- Intended to be used by module crates such as `rustok-blog`, `rustok-content`, `rustok-commerce`, and others when their GraphQL/REST adapters move out of `apps/server`.
- Depends on `rustok-core` for core security and permission primitives.
- Depends on `rustok-tenant` and `rustok-content` for tenant-module enablement checks and locale defaults.

## Boundary Rules
- `apps/server` may wire and re-export `rustok-api`, but must not grow a second parallel shared API layer.
- Module crates may depend on `rustok-api` for shared host contracts, but keep module-specific transport code and domain behavior locally.
- New cross-module request/auth/GraphQL helpers should go into `rustok-api` only when they are genuinely shared and host-level.

## Entry points
- `src/lib.rs`
- `src/context/`
- `src/request.rs`
- `src/ui.rs`
- `src/graphql/`

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
