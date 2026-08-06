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

## Why `GONE` was hidden

At the time of FORUM-24H, the tombstone ledger had no visibility snapshot, so returning a public `410 Gone` could disclose a formerly private or channel-restricted topic.

FORUM-24J later adds an immutable boolean-and-channel visibility snapshot owner for newly deleted topics. The FORUM-24H legacy field remains schema-compatible and still returns `null` for `ForumTopicRouteDisposition::Gone`.

FORUM-24K adds a separate additive `forumStorefrontTopicRouteDecision` field. Only that new field may expose `GONE`, and only after `ForumTopicRouteTombstoneVisibilityService::can_disclose_public_gone` admits the deletion-time public route for the current routed channel. The legacy response still does not expose historical topic IDs, alias IDs, snapshot payloads, audience selectors, or channel lists.

## Compatibility

The FORUM-24H field remains additive and unchanged. FORUM-24K does not make its enum wider or its canonical target nullable; it adds a separate decision type instead. This preserves existing generated clients while allowing the module-owned storefront package to opt into authorized terminal decisions.

## Verification handoff

No commands were executed while preparing or aligning this source slice. Maintainers can run:

```bash
node scripts/verify/verify-forum-topic-route-identity-owner.mjs
node scripts/verify/verify-forum-topic-route-storefront-graphql.mjs
node scripts/verify/verify-forum-topic-route-authorized-gone.mjs
cargo test -p rustok-forum graphql::topic_route_query::tests -- --nocapture
cargo test -p rustok-forum --test topic_route_storefront_graphql_contract -- --nocapture
cargo check -p rustok-forum --all-targets
```

## Remaining FORUM-24 scope after FORUM-24K

- add category route identity;
- define canonical and hreflang document policy;
- integrate SEO and capture runtime evidence.
