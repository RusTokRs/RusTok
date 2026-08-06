# FORUM-24H visibility-safe storefront topic route GraphQL transport

Status: **source-ready / maintainer execution pending**

## Scope

FORUM-24H adds one additive GraphQL query for resolving an incoming localized Forum topic route:

```graphql
forumStorefrontTopicRoute(
  tenantId: UUID
  locale: String!
  shortId: String!
  slug: String!
): GqlForumStorefrontTopicRouteResolution
```

The query composes the existing `ForumTopicRouteService` and then rechecks the returned canonical topic through the same `TopicService` storefront read contract used by `forumStorefrontTopic`.

## Visibility contract

The resolver:

- requires the Forum module to be enabled;
- treats `tenantId` only as an assertion against the routed tenant;
- checks the public channel module toggle for anonymous requests;
- resolves canonical and redirect history through `ForumTopicRouteService`;
- reopens the owner-provided canonical topic through `TopicService::get_with_locale_fallback`;
- uses the trusted permission snapshot for authenticated requests and `SecurityContext::public_read()` for anonymous requests;
- requires an anonymous canonical topic to remain open and visible in the routed public channel;
- returns `null` when the route or canonical topic is missing, deleted, or not storefront-visible.

The transport does not read the alias table, calculate short identities, normalize route segments, or recompute redirect targets.

## Public response

The response contains only:

- the normalized requested locale, short identity, and slug returned by the owner;
- `CANONICAL` or `REDIRECT`;
- the authorized canonical topic route descriptor.

It intentionally does not expose:

- the historical topic UUID;
- the immutable alias UUID;
- alias reasons or persistence metadata;
- a public `GONE` disposition.

## Why `GONE` remains hidden

The existing tombstone ledger proves route history but does not retain the visibility policy that applied before deletion. Returning a public `410 Gone` for every stored tombstone could therefore disclose a formerly private or channel-restricted topic.

FORUM-24H fails closed and returns `null` for `ForumTopicRouteDisposition::Gone`. A later slice must define a visibility-authorized tombstone snapshot or another owner-approved disclosure policy before storefront hosts may emit `410`.

## Compatibility

This is an additive GraphQL query. It changes no owner method, mutation, database schema, migration, event, admin workflow, storefront host route, canonical link, hreflang, or SEO policy.

The existing query-string storefront links remain unchanged in this slice. The new transport is the prerequisite for a later canonical route mount and redirect composition.

## Verification handoff

No commands were executed while preparing this source slice. Maintainers can run:

```bash
node scripts/verify/verify-forum-topic-route-identity-owner.mjs
node scripts/verify/verify-forum-topic-route-storefront-graphql.mjs
cargo test -p rustok-forum graphql::topic_route_query::tests -- --nocapture
cargo test -p rustok-forum --test topic_route_storefront_graphql_contract -- --nocapture
cargo check -p rustok-forum --all-targets
```

## Remaining FORUM-24 scope

- mount canonical topic routes in storefront hosts;
- compose HTTP redirect behavior;
- define visibility-authorized deleted-route handling;
- add category route identity;
- define canonical and hreflang policy;
- integrate SEO and capture runtime evidence.
