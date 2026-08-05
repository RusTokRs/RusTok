# Pages / Page Builder host route response packet

Date: 2026-08-06  
Status: **source-ready / execution-pending**

## Scope

This packet connects the Pages-owned localized route identity from PR #3018 to the
public SSR host. It does not change Fly/Page Builder document, sanitizer,
materialization, renderer or artifact ownership.

The public host now asks the Pages storefront adapter for one route decision before
SEO resolution or module rendering:

```text
GET /{locale}/modules/pages?slug={slug}
  → trusted tenant/request context
  → channel Pages-module admission
  → requested locale
  → tenant default locale
  → platform fallback locale
  → PageRouteService::resolve
  → published target lifecycle recheck
  → page channel-visibility recheck
  → host HTTP response
```

## Host response contract

- exact localized canonical route: continue normal SSR;
- immutable alias, legacy unprefixed route or other noncanonical route:
  **308 Permanent Redirect** to the encoded localized canonical path;
- immutable gone decision: **410 Gone**;
- unknown or channel-ineligible route: **404 Not Found**;
- ambiguous current/history ownership: **409 Conflict**;
- operational route-decision failure: **503 Service Unavailable**.

Every terminal Pages route response carries `Cache-Control: private, no-store`.
Channel module admission runs before the Pages route owner, and canonical target
visibility is rechecked before a redirect or successful continuation is disclosed.

Canonical redirect locations percent-encode the slug query value, including Unicode
and reserved bytes.

## Source ordering

```text
Storefront router
  → resolve_pages_route_response
  → pages/route-decision registered server function
  → channel module admission
  → PageRouteService::resolve
  → target published/channel checks
  → canonical: continue SSR
     redirect: 308
     gone: 410
     not found: 404
     conflict: 409
  → only canonical continuation reaches SEO and SSR render
```

## Source evidence

- `crates/rustok-pages/storefront/src/transport/host_route_adapter.rs`;
- `crates/rustok-pages/storefront/src/transport/mod.rs`;
- `crates/rustok-pages/storefront/src/lib.rs`;
- `apps/storefront/src/lib.rs`;
- `crates/rustok-pages/storefront/tests/host_route_decision_sqlite.rs`;
- `crates/rustok-pages/contracts/evidence/pages-host-route-response-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-host-route-response.mjs`.

The focused SQLite/Axum harness retains the registered Leptos server-function path
and source scenarios for two historical aliases, current canonical identity,
missing route, gone route, current/history conflict and channel-module denial.

## Explicit non-goals

- no delete tombstone writer;
- no historical route backfill/import;
- no Page Builder/Fly behavior change;
- no page body or artifact schema change;
- no GraphQL schema change;
- no cache policy or event schema change;
- no optional external event infrastructure work;
- no FFA/FBA promotion.

## Maintainer-owned validation

Suggested commands only; they were not run by the implementation agent:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-host-route-response.mjs

cargo test -p rustok-pages-storefront \
  --features ssr \
  --test host_route_decision_sqlite -- --nocapture

cargo test -p rustok-storefront --features ssr --lib -- --nocapture
cargo check -p rustok-pages-storefront --features ssr --all-targets
cargo check -p rustok-storefront --features ssr --all-targets
```

Execution evidence remains pending.
