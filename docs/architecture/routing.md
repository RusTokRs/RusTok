---
id: doc://docs/architecture/routing.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Routing and Transport Layer Boundaries

This document captures the boundaries between GraphQL, REST, module-owned HTTP surfaces
and internal Leptos `#[server]` functions.

## Main Rule

In RusToK, the transport layer is divided by purpose, not by team preference:

- GraphQL — primary UI-facing contract
- REST — integrations, webhooks, ops and compatible transport flows
- `#[server]` functions — internal data layer for Leptos hosts and module-owned UI
- health/metrics endpoints — operational surface

A new endpoint must fit into one of these channels, not create a
fourth local transport style.

## Selection Matrix

| Scenario | Channel |
|---|---|
| Admin/storefront UI query/mutation | GraphQL |
| Leptos internal UI action | `#[server]` function |
| External integration | REST |
| Webhook ingress / callback | REST |
| Health / readiness / metrics | Operational endpoints |
| OpenAPI discovery | REST/OpenAPI |

## GraphQL

GraphQL is used as the single UI-facing contract:

- `apps/admin`
- `apps/storefront`
- `apps/next-admin`
- `apps/next-frontend`
- module-owned UI packages, if they need GraphQL transport

GraphQL must not be diluted into integration-only flows where a stable
REST contract is needed.

## `#[server]` Functions

For Leptos hosts and module-owned Leptos UI, `#[server]` functions are the
preferred internal data layer.

At the same time:

- GraphQL is not removed and remains a parallel contract
- `#[server]` functions must not become a replacement for the external API
- ownership of business logic remains with the module/service layer, not with the UI crate

## REST

REST is used for:

- external integrations
- webhook callback flows
- operational operations
- compatible transport surfaces where GraphQL is not suitable
- module-owned HTTP endpoints, if the module needs an HTTP contract

REST must not duplicate UI-facing GraphQL without a clear reason.

## Module-owned Routing

If a module publishes HTTP routes or UI surfaces:

- routing is declared through `rustok-module.toml`
- owner-owned REST handlers/DTOs live in `crates/rustok-<module>/src/rest` or
  `crates/rustok-<module>/src/controllers`;
- owner-owned GraphQL roots/DTOs live in `crates/rustok-<module>/src/graphql`;
- the host application only mounts the surface and provides runtime/request context;
- the source of truth for wiring lives in the manifest and local docs of the module

The target HTTP declaration is `[provides.http].axum_router`. Its entrypoint receives
`HostRuntimeContext`; the module constructs its own narrow route state and returns its
own `axum::Router`. The generated host composition merges that router once. A module
must not declare both `axum_router` and legacy `routes`.

The presence of a controller or UI sub-crate without manifest wiring is not considered a complete
contract.

For backend implementation details, read the backend module guides:

- [Backend Module Architecture](../backend/module-backend-architecture.md)
- [Backend Module Implementation Guide](../backend/module-backend-implementation.md)
- [Backend Module Verification Guide](../backend/module-backend-verification.md)

New HTTP response formatting must use `rustok-web` helpers such as
`rustok_web::json_response`. Do not add `loco_rs::controller::format` usage.
The active host and generated module composition use Axum routers; Loco route
terms remain only in archived inventory and verifier search patterns.

## Route-selection Contract for Module-owned Admin UI

For module-owned admin UI, a single platform contract applies:

- selection state is stored in query string and is considered a URL-owned source of truth;
- only typed `snake_case` query keys are used: `product_id`, `cart_id`, `order_id`, `thread_id`,
  `media_id`, `channel_id`, `topic_id`, `provider_slug`, `tool_profile_slug`, `task_profile_slug`,
  and for SEO routes — `target_kind`, `target_id`, `locale`, `tab`;
- generic `id`, camelCase keys and other legacy aliases are not read and are not canonicalized;
- absence of a valid selection key means an empty state, not auto-select-first;
- invalid nested keys are cleaned locally and must not break neighboring selection domains;
- changing subpath/tab must prune keys that are invalid for the destination page.

Ownership split:

- `rustok-ui-core` owns the typed UI query schema, invariant rules and sanitization contract;
- `leptos-ui-routing` remains a generic Leptos route/query helper without an admin-specific key registry;
- host applications (`apps/admin`, `apps/next-admin`) own the route writers/adapters and must
  maintain parity on the same query contract.

## Query Contract for Module-owned Storefront UI

For module-owned storefront UI, the same ownership split applies, but without admin-specific typed
selection schema:

- host/runtime passes `UiRouteContext` with effective locale, route base and canonical query snapshot;
- storefront packages read their domain query keys through a common helper layer, not through
  package-local route parsing;
- for Leptos storefront packages, query reads go through `leptos-ui-routing`, not through
  direct `UiRouteContext.query_value(...)`;
- `leptos-ui-routing` remains a generic helper and does not own the storefront key registry,
  canonical slugs, locale policy or module-specific invariants;
- `apps/storefront` and `apps/next-frontend` must maintain parity on query semantics,
  locale propagation and canonical route behavior, not creating a second query policy on top of
  the backend/host contract.

## Locale and Routing

Locale routing is determined by the host/runtime layer:

- Leptos and Next hosts use host-provided effective locale
- module-owned UI packages do not introduce their own query/header/cookie chain
- locale contract must match `docs/UI/*` and local application docs

## What Not To Do

- do not use GraphQL as transport for an external webhook callback
- do not move integration-only REST contract into `#[server]` functions
- do not duplicate the same UI flow in GraphQL and REST without a reason
- do not hide module-owned routing only in the host application

## Related Documents

- [API Architecture](./api.md)
- [GraphQL and Leptos Server Functions](../UI/graphql-architecture.md)
- [Quick Start for Admin ↔ Server](../UI/admin-server-connection-quickstart.md)
- [Backend Module Architecture](../backend/module-backend-architecture.md)
- [Module Architecture](./modules.md)
