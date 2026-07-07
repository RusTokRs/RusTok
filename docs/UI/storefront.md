---
id: doc://docs/UI/storefront.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Storefront: Host and Contract

RusToK supports two web storefront host applications and a separate mobile storefront host:

- `apps/storefront` — primary Leptos SSR-first host;
- `apps/next-frontend` — parallel Next.js host;
- `rustok_mobile/apps/rustok_frontend_mobile` — Flutter customer storefront mobile host.

All implementations must maintain a unified backend, routing, locale, and module contract. Leptos storefront remains the primary Rust SSR/hydrate host path, Next.js storefront is the headless parallel, and Flutter storefront mobile is the mobile/headless host without Flutter-only backend API.

## Host Contract

- Host renders the shell and generic module pages.
- Module-owned storefront packages are connected via manifest-driven wiring.
- Checkout module surfaces are mounted via platform-known slots `checkout_shipping_handoff`, `checkout_payment_handoff` and `checkout_result_handoff`; slots do not transfer ownership of business logic to the host application.
- Generic storefront routes live in the `/modules/{route_segment}` family with a locale-aware variant where required by the host runtime.
- Module-owned packages must build internal links via the host route context, not via hardcoded route strings.
- For Leptos storefront packages, query/state reads must also go through the shared helper layer
  `leptos-ui-routing`; the storefront does not create a second package-local route helper on top of `UiRouteContext`.

## Data-Layer Contract

- For Leptos storefront, the default path in production runtime is: `UI -> local API -> #[server] -> service layer`.
- The external GraphQL contract `/api/graphql` remains mandatory and is a supported parallel path.
- The host selects native `#[server]` or GraphQL by build/runtime profile; it does not automatically switch from native execution to GraphQL after a native error.
- New module-owned storefront UI should not be designed as GraphQL-only if it can work via `#[server]`.
- Standalone CSR for a Leptos storefront package is considered a debug/compatibility profile: such a package must have a GraphQL/REST fallback and must not require `/api/fn/*`.
- Module-owned storefront packages must not collapse typed business snapshots into summary-only UI state:
  if the backend already returns typed adjustments, delivery ownership, or other language-agnostic business keys,
  the package API and UI must preserve these fields, not discard `scope`/metadata at the last mile.

## Canonical Routing and Locale

- Canonical URL policy and alias storage live in the backend/domain layer, not in the storefront host.
- The storefront uses a backend preflight for canonical route resolution before rendering a page.
- Effective locale is selected by the runtime/host layer once and then passed through to the UI surface.
- Manifest-entry adapters use `UiRouteContext.locale` and module-owned locale bundles from `[provides.storefront_ui.i18n]`; hardcoded copy is acceptable only as the last fallback for a missing key.
- Query-based locale fallback is allowed only as a backward-compatible path; module-owned UI must not introduce its own fallback chain.
- Route/query parity between `apps/storefront`, `apps/next-frontend` and `rustok_frontend_mobile` must be maintained at the level of
  key semantics and host contract, even if specific helper implementations differ.

## Parity with Next.js and Flutter Storefront

- `apps/next-frontend` must maintain parity with `apps/storefront` in route, auth, i18n, and backend contracts.
- `rustok_frontend_mobile` must use the same customer-facing storefront contract and not mix with admin/operator mobile UX.
- Flutter catalog/cart surfaces live in `rustok_mobile/packages/rustok_catalog_mobile` and are mounted by the host via the repository boundary; read/write cart actions go through the canonical storefront GraphQL surface in the host-owned repository, the cart id is stored only in the host-owned cart id store, and the package does not create its own GraphQL client, tenant resolver, locale fallback chain, or cart storage contract.
- Next.js and Flutter storefront must not duplicate storage or canonical-routing logic in the frontend layer.
- The source of truth for transport and canonical routing remains on the backend side.

## Verification

- `npm.cmd run verify:storefront:routes`
- targeted storefront contract and smoke checks for affected module-owned surfaces
- cross-reference with the [manifest-layer contract](../modules/manifest.md) when UI wiring changes

## Related Documents

- [Leptos storefront docs](../../apps/storefront/docs/README.md)
- [Next.js storefront docs](../../apps/next-frontend/docs/README.md)
- [Flutter storefront mobile docs](../../rustok_mobile/apps/rustok_frontend_mobile/README.md)
- [Flutter package catalog/cart](../../rustok_mobile/packages/rustok_catalog_mobile/README.md)
- [Manifest-Layer Contracts](../modules/manifest.md)
- [ADR: SSR-first Leptos hosts with headless parity](../../DECISIONS/2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md)
- [UI index](./README.md)
- [Documentation Map](../index.md)
