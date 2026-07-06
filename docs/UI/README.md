---
id: doc://docs/UI/README.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# UI Documentation

This section describes the RusToK frontend applications and general rules for integrating UI surfaces.

## UI Landscape

The platform supports host applications for web/headless and mobile slices:

- `apps/admin` — primary Leptos admin host;
- `apps/storefront` — primary Leptos storefront host;
- `apps/next-admin` — parallel Next.js admin host;
- `apps/next-frontend` — parallel Next.js storefront host;
- `rustok_mobile/apps/rustok_admin_mobile` — Flutter admin mobile host (in phased rollout).

Leptos hosts are the primary runtime path for platform-owned UI within the Rust workspace. Next.js hosts run as a parallel headless path and must maintain parity in transport, auth, i18n, and module contracts.

## FFA Status for Frontend Hosts

FFA migration status is maintained for module-owned UI packages in
[`docs/modules/registry.md`](../modules/registry.md#ffafba-readiness-board-module-owned-ui).
The frontend applications themselves do not receive a module FFA status because they are not domain UI
owners. Their target status is **FFA-compatible composition host**:

- `apps/admin` and `apps/storefront` remain Leptos SSR/hydrate hosts that mount
  module-owned surfaces and pass host context;
- `apps/next-admin` and `apps/next-frontend` remain parallel Next.js/headless hosts
  that maintain route/auth/i18n/transport parity;
- host code may contain shell, navigation, routing, install/governance, and other host-owned
  capabilities, but not module-specific CRUD/business workflows;
- FFA coverage of frontends is verified by host contract and parity gates, not by moving
  the `apps/*` themselves into `core/transport/ui` form.

Current host-level FFA slices are enforced by the fast gate
`npm run verify:frontend:host-ffa-contract`: `apps/admin` holds sidebar navigation policy in
`src/widgets/app_shell/core.rs`, and `apps/storefront` holds header route/link policy in
`src/widgets/header/core.rs`; the corresponding Leptos files remain render adapters.

## Basic UI Contract

- Host applications compose UI surfaces but do not pull module business UI into their own code.
- If a module provides UI, that surface remains module-owned regardless of `Core` or `Optional` status.
- Manifest-driven wiring for publishable UI goes through `modules.toml` and `rustok-module.toml`.
- Leptos hosts must use host-provided `UiRouteContext`, including the effective locale and module route base.
- Module-owned UI packages must not introduce their own locale negotiation chain on top of the host/runtime contract.
- For module-owned admin UI, selection state is also host-owned: typed `snake_case` query keys live in the URL,
  local editor/detail state is only hydrated from them, and the absence of a valid key leads to empty state.
- SEO admin route now follows the ownership baseline even more strictly: `seo` uses only `tab`
  and does not maintain a package-local entity selection contract on top of the host schema.
- For cross-cutting capability modules, the same ownership rule applies: capability runtime may provide
  shared widgets/contracts, but entity-specific editor UI stays with the owner module. For SEO, this means
  page/product/blog/forum SEO panels live in the corresponding module-owned admin packages, while
  `rustok-seo-admin` remains only the infrastructure/control-plane surface.
- In practice, this pattern is already implemented via `rustok-seo-admin-support`: `pages`, `product`, `blog`
  and `forum` use a shared SEO panel/tooling layer, and the SEO runtime already holds target kinds
  `forum_category` and `forum_topic` for owner-side forum integration.
- `rustok-seo-admin-support` does not invent its own locale negotiation chain and does not hold an editable
  locale field inside the panel UI: owner-side SEO widgets must take the host-provided effective locale
  and only canonicalize it under the platform i18n contract.
- After cutover, `rustok-seo-admin` itself no longer holds a metadata editor and uses only `tab`
  as the route-owned query state for bulk/redirects/sitemaps/robots/defaults/diagnostics control-plane.
- For module-owned Leptos storefront UI, query/state plumbing must also go through the shared layer:
  `leptos-ui-routing` is reused in both admin and storefront, and direct package-local access
  to `UiRouteContext.query_value(...)` is not considered a canonical pattern.

## Transport and Runtime Contract

- For Leptos hosts, GraphQL and native `#[server]` functions coexist in parallel; adding `#[server]` does not replace `/api/graphql`.
- Backend source of truth for UI hosts is `apps/server`.
- For headless/admin hosts, registry-backed capability descriptors must also be read from the backend contract:
  for SEO, that means GraphQL `seoTargets` or REST `/api/seo/targets`, not host-local mappings of target slugs.
- Read-only remediation hints for SEO linking also come from the backend contract:
  GraphQL `seoCrossLinkSuggestions` and REST `/api/seo/cross-link-suggestions` must be used as the source of truth,
  without host-local link suggestion heuristics bypassing the server runtime.
- For storefront SEO structured data, the backend contract is also the source of truth: hosts consume
  `SeoStructuredDataBlock.schema_kind/schema_type/source/payload` and do not introduce their own schema.org classifier.
- Contract parity between Leptos, Next.js and Flutter is assessed at the level of routes, auth, locale, module wiring, and transport surface, not at the level of literal internal implementation matching.

## Documentation Sections

- [Storefront Contract](./storefront.md)
- [GraphQL Architecture](./graphql-architecture.md)
- [Admin ↔ Server Quickstart](./admin-server-connection-quickstart.md)
- [Rust UI Component Catalog](./rust-ui-component-catalog.md)
- [Rich-text and Visual Page Builder Track](../modules/tiptap-page-builder-implementation-plan.md)

## Application Documentation

- [Leptos Admin](../../apps/admin/docs/README.md)
- [Leptos Storefront](../../apps/storefront/docs/README.md)
- [Next.js Admin](../../apps/next-admin/docs/README.md)
- [Next.js Storefront](../../apps/next-frontend/docs/README.md)
- [Flutter Admin Mobile](../../rustok_mobile/apps/rustok_admin_mobile/README.md)

## Keeping Documentation Up to Date

When changing frontend architecture, routing, UI contracts, or backend integration:

1. Update local docs in `apps/*`.
2. Update the corresponding document in `docs/UI/`.
3. Verify links in the [documentation map](../index.md).
4. For module-owned admin UI, additionally update the route-selection contract and parity notes in
   host docs if the query schema, selection behavior, or helper layer changes.
5. For module-owned storefront UI, also update routing/query parity notes if the
   `leptos-ui-routing` reuse layer, host query semantics, or storefront route/query contract changes.

## Hotspot Contract (DOC-12 / H3)

- Hotspot: `H3` (Admin/storefront host topology).
- Doc contracts updated: `docs/UI/README.md`.
- Owner scope: frontend owners.
- Residual drift risk:
  - when host wiring and transport parity in `apps/*` change without a synchronous
    update to `docs/UI/*`, there is a risk of host contract notes diverging;
  - route/query parity for Leptos/Next may drift during fast UI cutovers.
