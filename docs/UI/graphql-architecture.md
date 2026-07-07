---
id: doc://docs/UI/graphql-architecture.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# GraphQL and Leptos Server Functions

This document captures the current transport contract for RusToK UI circuits.

## Main Rule

For Leptos UI, the platform uses a dual-path model on top of the SSR-first runtime:

- native `#[server]` functions — preferred internal data layer for `apps/admin`, `apps/storefront` and module-owned Leptos UI packages in `ssr`/`hydrate`/monolith profiles;
- GraphQL `/api/graphql` — mandatory parallel transport contract for Next.js hosts, Flutter hosts, headless clients, and fallback branches in Leptos.

`#[server]` does not supersede GraphQL at the platform level. It adds a shorter internal path for Leptos hosts when the host actually runs via SSR/hydrate runtime.

CSR remains a mandatory compatibility/debug profile for standalone Trunk/WASM and checking module-owned UI packages, but is not considered a production runtime default.

## Why This Approach

- SSR/hydrate is better suited for monolith deployment: `apps/admin` and `apps/storefront` work same-origin with `apps/server`, use server-side auth/session/policy, and do not require an extra CORS/proxy scheme.
- `#[server]` provides the shortest internal Rust path from Leptos UI to the service layer and does not force every internal host action to become a public GraphQL mutation.
- GraphQL/REST remain mandatory because headless is a separate production mode for Next.js hosts, external clients, integrations, and mobile applications.
- CSR/Trunk is retained as a debug/compatibility profile: it is needed for local verification of module-owned UI packages and catches accidental server-only dependencies in WASM builds.
- The decision applies not only to `apps/admin` and `apps/storefront`, but to all module-owned UI packages in `crates/*/admin` and `crates/*/storefront`, because the host only mounts these surfaces.

## Matrix by UI Hosts

| Host / profile | Runtime default | Preferred transport | Mandatory parallel transport |
|------|-----------------|--------------------|------------------------------|
| `apps/admin` `ssr`/`hydrate` | SSR-first monolith | `#[server]` | GraphQL/REST |
| `apps/storefront` `ssr`/`hydrate` | SSR-first monolith | `#[server]` | GraphQL/REST |
| module-owned Leptos UI in SSR hosts | SSR/hydrate | `#[server]` | GraphQL/REST |
| module-owned Leptos UI standalone `csr` | debug/compatibility | GraphQL/REST | — |
| `apps/next-admin` | headless | GraphQL/REST | — |
| `apps/next-frontend` | headless | GraphQL/REST | — |
| `rustok_mobile/apps/rustok_admin_mobile` | headless/mobile host | GraphQL/REST (+ `/api/graphql/ws`) | — |
| external/mobile clients | headless | GraphQL/REST | — |

## Contract for Leptos UI

- A Leptos host or module-owned package should first design a local API layer for the SSR/hydrate `#[server]` path if the surface is an internal Leptos runtime surface.
- If a native path does not yet cover the required scenario or the surface must work in standalone `csr`, a fallback to GraphQL/REST is required.
- New Leptos UI should not be designed as GraphQL-only for monolith runtime if a `#[server]` path is realistic.
- New Leptos UI should not be designed as `#[server]`-only if the surface is needed for standalone CSR debug or headless parity.
- GraphQL queries and mutations must not be removed just because a native path has been introduced.

Basic pattern:

```text
UI component
  -> local API function
  -> in SSR/hydrate: try native #[server]
  -> in CSR/headless-compatible path: use GraphQL/REST fallback
  -> service layer
```

## Contract for GraphQL

Storefront payment reads follow the same dual-path contract: module-owned `rustok-payment-storefront` publishes native endpoints `payment/payment-collection` / `payment/refund-summary` and parallel GraphQL reads `storefrontPaymentCollection(cartId)` / `storefrontRefunds(orderId, filter)`. Collection read checks tenant/cart customer access, refund read checks tenant/order customer ownership; DTO and decimal-safe refund aggregation belong to the payment package. Aggregate commerce UI only composes owner transport results and does not own a separate payment read contract.

Product/search picker metadata follows the same contract: `rustok-product-storefront` publishes a native endpoint `product/storefront/catalog-search-options` and a parallel GraphQL read `storefrontCatalogSearchOptions(locale: String!)`. Public payload is limited to category ids/labels and filterable/sortable attribute codes/labels, uses tenant/channel guards and host effective locale without admin permission or package-local locale fallback; `apps/storefront` only maps owner DTO into search-owned UI props.
Next storefront repeats this boundary via host composition: `apps/next-frontend/src/features/search` passes route locale, tenant slug, and enabled modules to product-owned `packages/rustok-product::fetchCatalogSearchOptions`, while `packages/search` receives only safe category/attribute option props.

GraphQL remains:

- the public backend contract;
- the primary transport layer for Next.js and Flutter hosts;
- a fallback path for Leptos hosts;
- a transport surface for websocket subscriptions and headless client compatibility.

Security and allow/deny policies for sensitive admin operations must be determined by the server-side runtime layer, not by client-supplied `operationName` or app-local heuristics.

## Host Application Responsibilities

### `apps/admin`

- consider SSR/hydrate the preferred production runtime for monolith;
- use the native-first pattern for Leptos data access in SSR/hydrate;
- maintain GraphQL path as a live parallel contract;
- support CSR compatibility for standalone debug via GraphQL/REST, without mandatory `/api/fn/*`;
- do not push transport policy into app-local ad hoc code.

### `apps/storefront`

- consider SSR/hydrate the preferred production runtime for monolith;
- use the native-first pattern for host shell and module-owned storefront packages in SSR/hydrate;
- maintain GraphQL path for fallback and parity with headless storefront clients.

### `apps/server`

- keep `/api/fn/*` and `/api/graphql` as parallel runtime surfaces;
- do not treat the introduction of server functions as a reason to remove GraphQL schema or resolvers;
- apply shared policy equally to HTTP GraphQL and websocket execution paths.

## What Is Forbidden

- describing Leptos UI as GraphQL-only if a `#[server]` path already exists in the code;
- describing Leptos migration as abandoning GraphQL entirely;
- describing CSR/Trunk as the production default for Leptos hosts;
- removing a GraphQL route or resolver solely because a native Leptos transport appeared;
- introducing different transport contracts for an app host and module-owned UI without an explicit platform-level decision.

## Related Documents

- [UI index](./README.md)
- [Storefront contract](./storefront.md)
- [`apps/admin` Documentation](../../apps/admin/docs/README.md)
- [`apps/storefront` Documentation](../../apps/storefront/docs/README.md)
- [`apps/server` Documentation](../../apps/server/docs/README.md)
- [Flutter Admin Mobile Documentation](../../rustok_mobile/apps/rustok_admin_mobile/README.md)
- [ADR: SSR-first Leptos hosts with headless parity](../../DECISIONS/2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md)
- [Documentation Map](../index.md)
