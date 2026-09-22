# FORUM-23B2E1 trusted storefront Search channel authority

Canonical task status and remaining scope live in
[`implementation-plan.md`](implementation-plan.md). This note records the stable
source boundary delivered by `FORUM-23B2E1`; it is not a second roadmap.

## Result

Public storefront Search no longer chooses its effective channel from caller
input. `rustok-search` owns a small neutral authority helper that accepts the
middleware-owned `RequestContext` plus the legacy optional `channel_id` field.
The helper returns the trusted channel ID and slug only when:

- the request-context tenant matches the Search tenant;
- channel ID and slug are both present or both absent;
- a present channel ID is non-nil;
- a present slug is bounded, non-empty and free of control characters;
- a caller-supplied `channel_id`, when present, exactly matches the trusted ID.

The public field remains for compatibility, but it is only an assertion. It can
never select a different channel or create a channel context when middleware did
not resolve one.

## Surfaces

The same authority is used by:

- GraphQL `storefrontSearch`;
- native `search/storefront-search`;
- GraphQL `forumStorefrontSearch`;
- native `search/forum-storefront-search`.

The shared Forum-only execution owner validates the trusted context again, so a
future transport cannot bypass the boundary by constructing
`ForumStorefrontSearchRequest` directly. Search preview and admin global Search
retain their existing administrator-selected channel behavior.

## Compatibility and degraded mode

No public DTO field, `SearchQuery` field, migration, backfill, dependency or
`Cargo.lock` entry changes in this slice. Existing clients that send the same
channel ID already resolved by middleware remain compatible. Missing caller
input uses the trusted context. Invalid, foreign-tenant, incomplete or
mismatched context fails closed with a bounded validation error.

This slice intentionally does **not** claim product visibility completion.
Product Search documents still need the canonical
`metadata.channel_visibility.allowed_channel_slugs` projection and the
PostgreSQL engine still needs one channel predicate for base rows, totals,
facets, typo fallback, suggestions and query-rule materialization.

## Maintainer verification

The implementation agent did not run these commands:

```bash
cargo test -p rustok-search storefront_channel_authority -- --nocapture
cargo test -p rustok-search --features graphql --lib -- --nocapture
cargo check -p rustok-search --features graphql --all-targets
cargo check -p rustok-search-storefront --features ssr --all-targets
node scripts/verify/verify-forum-search-trusted-channel-authority.mjs
cargo xtask module validate forum
cargo xtask module validate search
```

Runtime evidence remains pending until the maintainer executes the focused
commands and exercises both storefront transports with absent, matching and
mismatched caller assertions.
