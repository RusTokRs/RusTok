# FORUM-24K authorized topic route `GONE` transport

Status: **source-ready / maintainer execution pending**

## Scope

FORUM-24K consumes the immutable FORUM-24J boolean disclosure owner in the selected Forum storefront route paths and maps an authorized terminal decision to private HTTP `410 Gone`.

The slice changes no route identity, alias persistence, deletion snapshot, audience policy, semantic event, admin command, SEO document or category route.

## Additive GraphQL compatibility

The legacy field remains unchanged:

```graphql
forumStorefrontTopicRoute(...): GqlForumStorefrontTopicRouteResolution
```

It continues to expose only `CANONICAL` and `REDIRECT`, keeps `canonical` non-null, and returns `null` for `GONE`.

FORUM-24K adds a separate field:

```graphql
forumStorefrontTopicRouteDecision(...): GqlForumStorefrontTopicRouteDecision
```

The decision enum contains `CANONICAL`, `REDIRECT`, and `GONE`. Its `canonical` field is nullable because an authorized terminal route has no redirect or render target. Existing generated clients are not forced to accept a wider legacy enum or nullable legacy field.

## Authorization

Both GraphQL and native server composition resolve route identity through `ForumTopicRouteService`.

Canonical and redirect results retain the existing exact `ForumTopicAudienceReadService` recheck.

A `GONE` result is returned only when all of these conditions hold:

- the current routed channel has the Forum module enabled when a channel is present;
- the route owner provides its historical topic identity internally;
- `ForumTopicRouteTombstoneVisibilityService::can_disclose_public_gone` returns `true`;
- the deletion-time snapshot was publicly disclosable;
- a channel-restricted snapshot contains the current routed channel slug;
- the sealed channel count and digest remain valid.

Authentication never broadens this public tombstone decision. A formerly private, authenticated-only, hidden, or differently channel-scoped topic remains indistinguishable from a missing route.

The transports do not query the snapshot tables, alias table, category storage or audience relation storage directly. They receive only the boolean owner decision.

## Dual transport

`rustok-forum-storefront` keeps one selected-path facade:

```rust
resolve_storefront_topic_route(locale, short_id, slug)
```

SSR/hydrate uses the existing native server endpoint. Headless/CSR uses the new additive GraphQL decision field. There is no automatic fallback between paths.

The shared DTO enforces these semantic shapes at the host boundary:

- `CANONICAL` requires a canonical descriptor;
- `REDIRECT` requires a canonical descriptor;
- `GONE` forbids a canonical descriptor.

Malformed transport shapes fail closed as `503 Service Unavailable`.

## HTTP composition

The Rust storefront host maps the selected decision as follows:

- exact `CANONICAL`: render the existing Forum module page;
- `REDIRECT` or nonexact raw canonical path: private `308 Permanent Redirect`;
- authorized `GONE`: private `410 Gone`;
- missing, unauthorized, unsnapshotted or mismatched-channel route: private `404 Not Found`;
- malformed decision, snapshot integrity conflict or transport failure: private `503 Service Unavailable`.

Terminal and redirect responses retain `Cache-Control: private, no-store` through the existing host helper.

## Historical policy

Topics deleted before FORUM-24J have no deletion-time proof and remain hidden. No backfill is attempted.

Same-topic localized slug aliases may inherit the topic snapshot because their historical owner is the deleted topic. A cross-topic redirect whose target later disappears remains hidden unless a separate source-route proof exists.

## Verification handoff

No tests, verifiers, formatting, Cargo commands, migrations, workflows, registered-host runs or browser scenarios were executed while preparing this source slice.

Maintainers can run:

```bash
node scripts/verify/verify-forum-topic-route-storefront-graphql.mjs
node scripts/verify/verify-forum-topic-route-storefront-mount.mjs
node scripts/verify/verify-forum-topic-route-authorized-gone.mjs
cargo test -p rustok-forum --test topic_route_storefront_graphql_contract -- --nocapture
cargo test -p rustok-forum --test topic_route_authorized_gone_transport_contract -- --nocapture
cargo test -p rustok-forum graphql::topic_route_query::tests -- --nocapture
cargo test -p rustok-forum-storefront model::tests -- --nocapture
cargo test -p rustok-storefront forum_topic_route::tests -- --nocapture
cargo check -p rustok-forum --all-targets
cargo check -p rustok-forum-storefront --all-targets --features ssr
cargo check -p rustok-storefront --all-targets --features ssr
```

## Remaining FORUM-24 scope

- category route identity and historical aliases;
- canonical and hreflang document policy;
- Forum SEO composition across storefront hosts;
- maintainer migration, runtime and browser evidence.
