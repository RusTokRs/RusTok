# Pages / Page Builder Storefront Composition ETag Packet

Date: 2026-08-06  
Status: source-ready / execution-pending

## Rechecked boundary

The registered Pages host already performs channel-module admission, localized route resolution, publication/channel visibility rechecks and terminal response composition before SEO and SSR. The native Pages data path already uses route/page/artifact generations for its owner cache.

The missing source boundary was the final SSR document: Navigation menus and SEO were resolved independently from the Pages generations and Navigation components could issue their own SSR reads after the host had already begun composition.

## Host composition

For an exact localized canonical Pages route, the host now composes:

```text
Pages route decision and visibility gate
  → Pages route/page/artifact generation snapshot
  → Navigation-owned header/footer reads
  → SEO-owned resolved page context
  → deterministic composition ETag
  → conditional 304 or SSR render
```

Redirect, gone, missing, conflict and route-runtime failures still terminate before Navigation/SEO composition and retain `private, no-store`.

## Navigation ownership

`rustok-navigation-storefront` now exposes its public menu models, active-menu transport and a `StorefrontNavigationSnapshot`. The host uses the existing Navigation transport for Header and Footer. During the same SSR render the snapshot is provided as Leptos context, so `NavigationHeaderMenu` and `NavigationView` reuse it instead of performing duplicate reads.

No menu policy, binding, locale fallback or Navigation database ownership moves to Pages or the host.

## Composition identity

`pages_storefront_composition_v1` serializes and hashes:

- canonical page id, slug and effective locale;
- request locale and channel identity;
- Pages route, page and artifact generations;
- the resolved Navigation header/footer payloads;
- the resolved SEO page context.

The serialized payload is hashed with SHA-256 and emitted as a strong ETag. The key is produced only for a complete canonical decision with all three Pages generations. Missing cache runtime or generation reads disable the ETag but do not disable SSR.

A matching strong, weak or comma-separated `If-None-Match` returns `304 Not Modified`. Canonical ETag responses use `Cache-Control: private, no-cache` so a user agent may retain and revalidate the composed document without treating it as an anonymously shareable CDN object.

## Source evidence

- `crates/rustok-pages/storefront/src/transport/host_route_adapter.rs`;
- `crates/rustok-navigation/storefront/src/model.rs`;
- `crates/rustok-navigation/storefront/src/ui/menu.rs`;
- `apps/storefront/src/shared/context/pages_composition.rs`;
- `apps/storefront/src/lib.rs`;
- `crates/rustok-pages/contracts/evidence/pages-storefront-composition-etag-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-storefront-composition-etag.mjs`.

## Deliberate limits

This slice does not:

- add a shared/CDN full-document cache;
- change Navigation menu ownership or persistence;
- change SEO ownership, target providers or schemas;
- change Page Builder/Fly documents, artifacts, publish or rollback;
- change Pages database schemas, events or native data cache namespaces;
- add GraphQL, REST or admin surfaces;
- claim browser, server, cache-provider, workflow, CI or rollout execution;
- promote FFA or FBA.

## Maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-storefront-composition-etag.mjs
cargo test -p rustok-storefront --features ssr --lib -- --nocapture
cargo test -p rustok-pages-storefront --features ssr \
  --test host_route_decision_sqlite -- --nocapture
cargo test -p rustok-navigation-storefront --features ssr --all-targets -- --nocapture
cargo check -p rustok-storefront --features ssr --all-targets
```

Execution evidence remains pending.
