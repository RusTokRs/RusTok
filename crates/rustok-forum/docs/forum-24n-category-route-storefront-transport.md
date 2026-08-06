# FORUM-24N visibility-safe category route transport

Status: **source-ready / maintainer execution pending**

## Scope

FORUM-24N exposes the localized category route owner from FORUM-24L/M through two storefront transports:

- GraphQL field `forumStorefrontCategoryRoute`;
- native Leptos server function endpoint `forum/storefront-category-route`.

Both return the same public decision shape for:

```text
/{locale}/forum/c/{slug}
```

Machine contract:

```text
crates/rustok-forum/contracts/forum-category-route-storefront-transport.json
```

## Public response

A disclosed route contains:

- normalized requested locale;
- normalized requested slug;
- `CANONICAL` or `REDIRECT`;
- the owner-provided canonical category descriptor:
  - category ID;
  - effective locale;
  - current slug;
  - canonical path.

The response never includes the immutable alias ID, alias reason, storage timestamps, category policy layers, viewer facts or denial reason.

There is no category `GONE` decision. Missing, archived, disabled or unauthorized category history remains absent.

## Resolution and locale policy

Both transports call `ForumCategoryRouteService::resolve` with:

1. the requested route locale;
2. the routed tenant default locale as the explicit fallback;
3. the existing platform fallback `en`;
4. the existing unambiguous first-available behavior.

FORUM-24M alias precedence is preserved. An exact-locale historical alias cannot be shadowed by a fallback-locale current category route.

The transport does not infer a canonical path or redirect locally. It maps only the descriptor returned by the route owner.

## Visibility boundary

Route identity is evaluated first, but no route is disclosed until the resolved canonical category passes the exact existing category read owner.

For anonymous requests, both transports use:

```text
ForumCategoryAudienceReadService::get_public_storefront_visible_with_locale_fallback
```

For authenticated requests, both transports:

1. derive `SecurityContext` from the trusted permission snapshot;
2. build `category_read_audience_port_context` from routed tenant, authenticated user, request channel and canonical locale;
3. use operation `SelectedCategory` and the transport-specific label;
4. call `get_authenticated_storefront_visible_with_audience_context`.

This rechecks the inherited public/authenticated floor and every richer category audience layer. An alias row never authorizes disclosure.

The routed channel module gate is checked before route identity is returned. A disabled channel receives no category route result.

## GraphQL behavior

The additive field is:

```graphql
forumStorefrontCategoryRoute(
  tenantId: UUID
  locale: String!
  slug: String!
): GqlForumStorefrontCategoryRouteResolution
```

The optional tenant argument must equal `TenantContext.id`. A mismatch remains a permission error.

Missing, archived, channel-disabled or audience-hidden routes return `null`. Persistence conflicts and owner failures return an internal GraphQL error rather than pretending that inconsistent route history is absent.

## Native behavior

The native server function reads `HostRuntimeContext`, `TenantContext`, `OptionalAuthContext` and `RequestContext` from trusted server context. The request DTO carries only locale and slug.

Missing, archived, channel-disabled or audience-hidden routes return `None`. Persistence conflicts and owner failures become transport errors. A native failure never falls back to GraphQL.

## Compatibility

This slice does not:

- mount a category URL in `apps/storefront`;
- alter category-card links;
- add HTTP `308`, `404` or `410` mapping;
- add category tombstones;
- change category commands or alias storage;
- change existing category list/detail GraphQL fields;
- change REST;
- change topic routes;
- add canonical document metadata, hreflang, schema.org or other SEO composition.

The new `resolve_storefront_category_route` export is transport capability only. No host router or UI invokes it in this slice.

## Verification handoff

No tests, verifiers, formatting, Cargo commands, migrations, workflows, HTTP scenarios, browser scenarios or CI were executed while preparing this slice.

Maintainers can run:

```bash
node scripts/verify/verify-forum-category-route-storefront-transport.mjs
cargo test -p rustok-forum graphql::category_route_query::tests -- --nocapture
cargo test -p rustok-forum --test category_route_storefront_transport_contract -- --nocapture
cargo test -p rustok-forum-storefront model::tests::category_route_payload_uses_graphql_enum_and_field_names -- --nocapture
cargo check -p rustok-forum --all-targets
cargo check -p rustok-forum-storefront --all-targets
```

## Remaining FORUM-24 scope after FORUM-24N

- Rust storefront category route mount and category-link cutover;
- private no-store redirect/not-found HTTP policy;
- canonical and hreflang document policy;
- Forum-specific SEO composition and matching schema.org semantics;
- Next storefront parity;
- maintainer SQLite, PostgreSQL, HTTP and browser evidence.

The canonical implementation plan remains the single roadmap. Its FORUM-24 ledger entry is not updated by this slice because the connected complete-file writer cannot safely retrieve and replace the full plan losslessly; this document records only the stable FORUM-24N contract and does not create a second backlog.
