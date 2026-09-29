# rustok-api

## Purpose
`rustok-api` is the shared web/API adapter layer for RusToK. It hosts reusable request, tenant, auth, locale, permission, port and GraphQL-facing contracts used by `apps/server` and module crates that expose GraphQL or HTTP adapters.

## Responsibilities
- Provide reusable tenant and auth request context types.
- Provide reusable channel request context types for channel-aware runtime resolution.
- Provide the neutral `rustok-api::richtext` document, profile-identifier, and
  read-projection contracts shared by Next, Leptos, GraphQL, and native
  server-function adapters.
- Provide framework-neutral platform build/release snapshots and typed status,
  stage, and deployment-profile codes shared by owner ports, GraphQL adapters,
  and browser-safe native UI transports.
- Provide the host-safe artifact UI projection and redacted binding-audit DTOs
  shared by owner modules, server HTTP, GraphQL, Leptos, Next, and Flutter
  adapters without exposing descriptor-local catalogs, permissions, bindings,
  executable UI material, inputs, outputs, actors, traces, or credentials.
- Provide the minimized federated module-registry freshness DTO shared by the
  owner catalog port, GraphQL, native Leptos transport, and Next/headless
  clients without exposing endpoints or remote error content.
- Provide host/API request, tenant, auth, channel, locale, permission and port contracts.
- Provide the framework-independent manifest-to-runtime registry comparison contract used by the server composition root.
- Provide GraphQL helper types and error helpers shared across modules.
- Provide request-level locale and tenant resolution primitives that do not belong in domain crates.
- Provide `HostRuntimeContext` for server-side Leptos/native adapters that need host-owned runtime handles without importing a host-wide application context.
- Provide neutral port context/error primitives, policy helpers (`PortCallPolicy`) and typed error constructors for module-owned ports; `rustok-region` and migrated tenant, channel, product, customer, media, workflow, RBAC, tax, fulfillment, payment, pricing, cart, inventory, comments, search, order, index, email delivery, outbox relay, and page-builder publish paths use these shared primitives for FBA read/write boundaries.
- Own neutral permission contracts (`Permission`, `Action`, `Resource`) and host request-locale policy contracts. Framework-neutral `Accept-Language` field parsing is delegated to `rustok-ui-i18n`; the legacy `extract_locale_tag_from_header` name remains a compatibility re-export.
- Carry typed channel-resolution diagnostics (`channel_id`, `channel_slug`, `channel_resolution_source`, `channel_resolution_trace`) from host middleware into module adapters.
- Keep web-framework-oriented dependencies out of `rustok-core` while still allowing modular reuse.
- Stay a thin shared host/API layer. It must not absorb module-specific business logic, resolvers, or controllers.
- Prevent duplicate implementations of the same web/API contract in `apps/server` or individual module crates.

## Interactions
- Used by `apps/server` as the current composition root.
- Used by module crates such as `rustok-blog`, `rustok-content`, `rustok-commerce`, and others when their GraphQL/REST adapters need shared host/API contracts.
- All feature profiles, including `runtime` and `server`, remain independent from `rustok-core`.
- `rustok-core` consumes API-owned contracts and adds runtime RBAC/security policy.
- Runtime-specific composition helpers remain owner-owned; `rustok-api` does not depend on outbox runtime wiring.

## Boundary Rules
- `apps/server` may wire and re-export `rustok-api`, but must not grow a second parallel shared API layer.
- Module crates may depend on `rustok-api` for shared host contracts, but keep module-specific transport code and domain behavior locally.
- New cross-module request/auth/GraphQL/port helpers should go into `rustok-api` only when they are genuinely shared and host/API-level.
- UI route/query/input helpers belong in `rustok-ui-core` and `leptos-ui-routing`, not in `rustok-api`.
- UI message catalogs, translation-key resolution, Unicode locale mechanics, and reusable `Accept-Language` parsing belong in `rustok-ui-i18n`; `rustok-api` owns host/request precedence and compatibility contracts, not a second parser.
- `rustok-api::normalize_locale_tag` is the host/storage projection over the canonical `rustok-ui-i18n` Unicode/CLDR normalizer and enforces the platform's 32-byte normalized locale width; it does not implement a second locale grammar.
- `RequestContext` is a projection of trusted request extensions; it never reconstructs tenant or effective-locale authority when upstream middleware evidence is missing.
- Richtext executable policy, profile definitions, validation, rendering, and
  plain-text extraction belong in `rustok-content::richtext`, not here.

## Entry points
- `src/lib.rs`
- `src/artifact_ui.rs`
- `src/context/`
- `src/request.rs`
- `src/runtime.rs`
- `src/module_registry_contract.rs`
- `src/module_marketplace.rs`
- `src/ports.rs`
- `src/permissions.rs`
- `src/platform_build.rs`
- `src/locale.rs`
- `src/graphql/`
- `src/richtext.rs`

## Features

- `default = []`: neutral contracts with no core runtime dependency.
- `runtime`: SeaORM-backed host runtime context without HTTP or GraphQL frameworks.
- `server`: server-side auth/request/GraphQL adapters; it includes `runtime` and adds Axum and Async-GraphQL.

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../../docs/index.md)
