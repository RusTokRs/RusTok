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

The original FORUM-24I host composition was:

- exact `CANONICAL`: render the existing Forum module page;
- `REDIRECT` or nonexact raw path: private `308 Permanent Redirect`;
- missing, hidden, deleted, channel-restricted or `GONE`: private `404 Not Found`;
- transport failure: private `503 Service Unavailable`.

FORUM-24K extends only the terminal decision:

- an owner-authorized `GONE` becomes private `410 Gone`;
- missing, unsnapshotted, formerly private, disabled-channel or channel-mismatched routes remain private `404`;
- malformed decision shapes and snapshot integrity conflicts become private `503`.

The host still does not authorize `GONE`; GraphQL or native transport must already have consumed `ForumTopicRouteTombstoneVisibilityService::can_disclose_public_gone`. Redirect and terminal responses use `Cache-Control: private, no-store`.

Axum-decoded route segments are passed only to the module owner for resolution. Canonical equality is checked against `OriginalUri.path()`, so case variants and percent-encoded noncanonical paths redirect instead of being mistaken for exact canonical requests.

## Dual transport

The storefront package exposes one facade:

```rust
resolve_storefront_topic_route(locale, short_id, slug)
```

SSR/hydrate selects the native server function. Headless/CSR selects GraphQL. There is no automatic fallback between selected paths.

FORUM-24K switches the GraphQL adapter from the legacy canonical/redirect-only `forumStorefrontTopicRoute` field to additive `forumStorefrontTopicRouteDecision`. The native endpoint remains `forum/storefront-topic-route`. Both paths consume the same boolean tombstone owner and share the same `CANONICAL | REDIRECT | GONE` DTO.

Canonical and redirect results retain exact `ForumTopicAudienceReadService` parity. Authentication does not broaden deletion-time public tombstone disclosure.

## Topic-card cutover

Topic cards no longer emit `?topic=<uuid>` links. The module core derives the deterministic twelve-hex short identity from the already admitted topic UUID and builds the localized canonical route from the effective locale and topic slug.

Category selection remains on the existing module query route. Selecting a canonical topic route reopens the topic through the owner-resolved UUID before rendering.

## Compatibility

FORUM-24K adds a new GraphQL field instead of widening the legacy enum or making the legacy canonical field nullable. The existing generic module route remains available for category navigation and direct noncanonical module access.

No FORUM-24K migration, owner write, semantic event, admin mutation, SEO or hreflang change is introduced.

## Verification handoff

No tests, verifiers, formatting, Cargo commands, workflows, registered-host runs or browser scenarios were executed while preparing this source slice.

Maintainers can run:

```bash
node scripts/verify/verify-forum-topic-route-storefront-graphql.mjs
node scripts/verify/verify-forum-topic-route-storefront-mount.mjs
node scripts/verify/verify-forum-topic-route-authorized-gone.mjs
cargo test -p rustok-forum --test topic_route_storefront_graphql_contract -- --nocapture
cargo test -p rustok-forum --test topic_route_authorized_gone_transport_contract -- --nocapture
cargo test -p rustok-forum-storefront model::tests -- --nocapture
cargo test -p rustok-storefront forum_topic_route::tests -- --nocapture
cargo check -p rustok-forum --all-targets
cargo check -p rustok-forum-storefront --all-targets --features ssr
cargo check -p rustok-storefront --all-targets --features ssr
```

## Remaining FORUM-24 scope after FORUM-24K

- category route identity and historical aliases;
- canonical/hreflang document policy;
- SEO integration across storefront hosts;
- maintainer migration, runtime and browser evidence.
