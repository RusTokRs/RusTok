# FORUM-24I canonical topic route storefront mount

Status: **source-ready / maintainer execution pending**

## Scope

FORUM-24I composes the localized Forum topic route contract into the Rust storefront host:

```text
/{locale}/forum/t/{short_id}/{slug}
```

The slice includes:

- module-owned canonical topic-card href construction;
- a selected-path storefront transport facade with native `#[server]` and GraphQL adapters;
- exact audience-aware canonical target revalidation in both paths;
- an Axum host mount in the shared `rustok_storefront::router()` used by standalone and embedded SSR;
- private permanent redirects to the owner-provided canonical path;
- fail-closed missing, hidden, deleted and tombstone behavior.

## Module and host boundaries

`rustok-forum-storefront` owns route DTOs, selected-path transport selection and topic-card href policy. The host does not query Forum persistence, inspect the alias ledger, resolve merge receipts, calculate redirect targets or authorize topics.

The host receives only a visibility-admitted route resolution and performs HTTP composition:

- exact `CANONICAL` path: render the existing Forum module page with the owner-resolved topic UUID injected into request context;
- `REDIRECT` or a nonexact raw locale/identity/slug path: return private `308 Permanent Redirect` to `canonical.path`;
- missing, deleted, hidden, channel-restricted or `GONE`: return private `404 Not Found`;
- transport/domain failure without a public typed route result: return private `503 Service Unavailable`.

Redirect and terminal responses use `Cache-Control: private, no-store`.

## Dual transport

The storefront package exposes one facade:

```rust
resolve_storefront_topic_route(locale, short_id, slug)
```

SSR/hydrate selects the native server function. Headless/CSR selects the additive `forumStorefrontTopicRoute` GraphQL query. There is no automatic fallback between selected paths.

The native path composes `ForumTopicRouteService` with `ForumTopicAudienceReadService`, the routed tenant, optional trusted auth snapshot, request channel and exact native audience port context. The GraphQL query is aligned to the same audience owner and exact GraphQL port context.

## Topic-card cutover

Topic cards no longer emit `?topic=<uuid>` links. The module core derives the deterministic twelve-hex short identity from the already admitted topic UUID and builds the localized canonical route from the effective locale and topic slug.

Category selection remains on the existing module query route. Selecting a canonical topic route reopens the topic through the owner-resolved UUID before rendering.

## Deliberate exclusions

FORUM-24I does not add:

- public `410 Gone` responses;
- visibility snapshots for deleted routes;
- category route identity;
- canonical, hreflang or alternate document metadata;
- Forum-specific SEO head composition;
- Next storefront route mounting;
- runtime, browser or registered-host evidence.

The tombstone ledger still lacks a visibility snapshot, so `GONE` continues to collapse to the same public `404` as hidden or missing content.

## Compatibility

The GraphQL field remains additive. No migration, storage schema, owner write method, semantic event, admin mutation or receipt changes.

The existing generic module route remains available for category navigation and direct noncanonical module access. Topic-card navigation cuts over to the canonical topic route.

## Verification handoff

No tests, verifiers, formatting, Cargo commands, workflows, registered-host runs or browser scenarios were executed while preparing this source slice.

Maintainers can run:

```bash
node scripts/verify/verify-forum-topic-route-storefront-graphql.mjs
node scripts/verify/verify-forum-topic-route-storefront-mount.mjs
cargo test -p rustok-forum --test topic_route_storefront_graphql_contract -- --nocapture
cargo test -p rustok-forum-storefront core::tests -- --nocapture
cargo test -p rustok-storefront forum_topic_route::tests -- --nocapture
cargo check -p rustok-forum --all-targets
cargo check -p rustok-forum-storefront --all-targets --features ssr
cargo check -p rustok-storefront --all-targets --features ssr
```

## Remaining FORUM-24 scope

- visibility-authorized deleted-route disclosure and optional public `410`;
- category route identity and historical aliases;
- canonical/hreflang document policy;
- SEO integration across storefront hosts;
- maintainer runtime evidence.
