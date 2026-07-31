# FORUM-23B2E2 Product storefront Search channel visibility

## Status

`source_complete_execution_pending`

This slice completes the Product visibility half of the trusted storefront Search
channel boundary introduced by `FORUM-23B2E1`. Runtime and PostgreSQL evidence
remain maintainer-owned and are not claimed here.

## Owner boundary

Product remains the policy owner for channel availability. The canonical input is:

```text
products.metadata.channel_visibility.allowed_channel_slugs
```

Search copies only the normalized owner decision needed for retrieval into:

```text
search_documents.payload.channel_visibility.allowed_channel_slugs
```

Search does not import Product or Channel services and does not evaluate Product
business policy. The projection rules preserve the canonical Product compatibility
contract:

- absence of `channel_visibility` means a global Product and projects `[]`;
- a canonical array is preserved without inventing another allowlist;
- malformed explicit owner data projects a non-array value and therefore fails
  closed instead of silently becoming global.

## One storefront predicate

`storefront_product_channel_visibility.rs` owns the neutral Search predicate. It
receives the `TrustedStorefrontChannel` produced by `FORUM-23B2E1` and applies the
same rules in SQL and in the query-rule pin check:

- non-Product rows are unchanged;
- the projected Product allowlist must be a JSON array;
- an empty array is globally visible;
- a non-empty array requires the normalized trusted channel slug;
- a missing, malformed or unresolved restricted Product is hidden.

`PgSearchEngine::search_storefront` applies that predicate before ranking. Because
the filtered relation is shared by the ordinary FTS path, typo fallback, count and
facet queries, a restricted Product cannot survive in rows, totals or facets.
The existing `SearchEngine::search` path remains unchanged for admin preview and
other explicitly non-storefront callers.

## Secondary result paths

A query rule can load a pinned document after the ranked query. The storefront
query-rule path therefore rechecks Product payload visibility with the same owner
helper before inserting a pin.

Document suggestions use the same SQL predicate. Query-text suggestions are
aggregated search strings, do not expose a Product document or URL and are not
subject to Product allowlist filtering.

Ordinary GraphQL and native storefront Search, plus the explicit Forum-only
GraphQL/native execution path, retain the exact trusted channel throughout the
whole request. Forum-only bounded continuation pages cannot switch to the
unscoped engine path.

## Existing document repair

Older Product Search documents do not contain the projected allowlist. They are
already hidden by the fail-closed predicate. During Search bootstrap,
`SearchProjector` counts tenant Product documents whose projected value is missing
or is not an array and runs the existing product-scope rebuild when drift exists.
This is a Search-owned rebuild, not a database migration or a Product write.

## Compatibility and degraded mode

No database migration, public DTO, public `SearchQuery` field, dependency or
`Cargo.lock` change is introduced. The Search-owned Product payload gains the
channel visibility projection. Existing canonical Products are repaired through
the product-scope rebuild; no manual backfill is required.

If bootstrap repair has not run yet, old or malformed Product documents remain
hidden rather than becoming visible in every channel. Admin preview/global Search
keeps its previous operator semantics because it does not call the storefront-only
engine method.

This slice does **not** claim completion of remaining Forum filters, durable
non-Forum projection ordering, deletion/ACL cleanup, or runtime evidence.

## Maintainer verification

The implementation agent did not run these commands:

```bash
cargo test -p rustok-search storefront_product_channel_visibility -- --nocapture
cargo test -p rustok-search product_channel_visibility_drift_is_fail_closed -- --nocapture
node scripts/verify/verify-forum-search-product-channel-visibility.mjs
cargo check -p rustok-search --features graphql --all-targets
cargo check -p rustok-search-storefront --features ssr --all-targets
cargo check -p rustok-server --features mod-forum --all-targets
cargo xtask module validate forum
cargo xtask module validate search
```

PostgreSQL proof should include one globally visible Product, one Product allowed
in the resolved channel, one Product restricted to another channel and one legacy
or malformed projection. Rows, totals, facets, typo fallback, pinned rules and
document suggestions must all agree.
