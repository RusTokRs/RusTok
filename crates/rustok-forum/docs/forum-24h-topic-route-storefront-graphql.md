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

The query composes the existing `ForumTopicRouteService` and then rechecks the returned canonical topic through `ForumTopicAudienceReadService`, the same exact category/topic audience owner used by the selected storefront topic transport.

## Visibility contract

The resolver:

- requires the Forum module to be enabled;
- treats `tenantId` only as an assertion against the routed tenant;
- checks the public channel module toggle for anonymous requests;
- resolves canonical and redirect history through `ForumTopicRouteService`;
- reopens the owner-provided canonical topic through `ForumTopicAudienceReadService`;
- supplies the trusted permission snapshot and exact GraphQL audience port context for authenticated requests;
- uses the owner public storefront path for anonymous requests;
- retains category audience, topic audience, open-status and routed-channel visibility enforcement inside the exact owner;
- returns `null` when the route or canonical topic is missing, deleted, or not storefront-visible.

The transport does not read the alias table, calculate short identities, normalize route segments, recompute redirect targets, or reproduce category/topic audience policy.

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

FORUM-24H fails closed and returns `null` for `ForumTopicRouteDisposition::Gone`. A later owner slice must define a visibility-authorized tombstone snapshot or another approved disclosure policy before storefront hosts may emit `410`.

## Compatibility

This is an additive GraphQL query. It changes no route owner method, mutation, database schema, migration, event or admin workflow. FORUM-24I later composes the query and equivalent native owner path into the Rust storefront host.

## Verification handoff

No commands were executed while preparing or aligning this source slice. Maintainers can run:

```bash
node scripts/verify/verify-forum-topic-route-identity-owner.mjs
node scripts/verify/verify-forum-topic-route-storefront-graphql.mjs
cargo test -p rustok-forum graphql::topic_route_query::tests -- --nocapture
cargo test -p rustok-forum --test topic_route_storefront_graphql_contract -- --nocapture
cargo check -p rustok-forum --all-targets
```

## Remaining FORUM-24 scope after FORUM-24I

- define visibility-authorized deleted-route handling;
- add category route identity;
- define canonical and hreflang document policy;
- integrate SEO and capture runtime evidence.
