# FORUM-23B2D — storefront Search result eligibility

## Result

The explicit Forum-only storefront Search path now reauthorizes every topic and
approved-reply candidate through the current Forum owner before visible
pagination, totals, facets, query-rule ordering, or transport mapping.

Search owns the neutral `StorefrontSearchResultEligibilityPort`. The server is
the only adapter that imports both Search and Forum contracts. Forum owns the
actual decision through `ForumSearchResultEligibilityService`, which reuses
`ForumTopicAudienceVisibilityService` rather than copying audience policy into
Search.

## Execution order

1. The existing `FORUM-23B2C` path expands selected category roots through the
   richer Forum category-audience owner.
2. Search executes the existing dictionary, preset, ranking, PostgreSQL FTS or
   typo-fallback query from offset zero in bounded pages.
3. A raw result set larger than 512 rows is rejected and asks the caller to
   narrow the query or category scope.
4. Category rows remain authorized by the previously resolved Forum category
   scope. Topic and reply rows become typed neutral candidates.
5. Forum batch-loads current approved reply ownership, deduplicates parent topic
   IDs, and evaluates each topic through the exact storefront topic audience
   owner.
6. Missing, stale, closed, channel-denied, inherited-category-denied, or
   topic-local-denied candidates are omitted without revealing which predicate
   rejected them.
7. Search preserves the raw ranking order of allowed rows, then computes visible
   totals, facets, offset, and limit. Query rules run only on that authorized
   page.

## Authorization and privacy

Public evaluation requires no optional audience-facts provider. Authenticated
evaluation is selected only when the trusted auth snapshot has the same
`forum_categories:list` admission already used by the category-scope path. The
server constructs a read-only `PortContext` from trusted tenant, actor,
permissions, locale, session, deadline, and route channel state.

Richer role, Forum trust, Channel, Groups, explicit allow, and explicit deny
layers remain Forum policy. When a still-required external owner fact cannot be
resolved, the request fails closed. A missing result-eligibility port, disabled
Forum module, invalid owner subset, or provider failure cannot degrade to raw
Search output.

Replies are eligible only when the exact tenant-owned reply is currently
`approved` and its parent topic is visible to the same viewer. Search never
trusts a projected `topic_id` payload as owner authorization.

## Bounds and degraded mode

The raw Search candidate set and owner request are capped at 512 rows. Search
reads raw candidates in pages of 50 and requires the total to remain unchanged
and every continuation page to advance. A wider or unstable set returns a typed
failure instead of a partial visible page.

No migration, backfill, dependency, projection-shape, public DTO, or
`Cargo.lock` change is introduced. The ordinary mixed, unspecified, Product,
Blog, Content, and Forum-without-category Search paths remain unchanged. The
explicit visibility-safe Forum path requires both the category-scope and
result-eligibility ports.

## Source-ready verification

The machine contract is
`crates/rustok-forum/contracts/forum-search-result-eligibility.json`. The static
guardrail is
`scripts/verify/verify-forum-search-result-eligibility.mjs`.

Maintainers should run the focused verification set before claiming executable
evidence:

```bash
cargo test -p rustok-search storefront_result_eligibility -- --nocapture
cargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture
cargo test -p rustok-server --features mod-forum forum_search_result_eligibility -- --nocapture
node scripts/verify/verify-forum-search-result-eligibility.mjs
cargo check -p rustok-search --features graphql --all-targets
cargo check -p rustok-search-storefront --features ssr --all-targets
cargo check -p rustok-server --features mod-forum --all-targets
cargo xtask module validate forum
cargo xtask module validate search
```

No command above was run while publishing this source-ready slice.

## Remaining work

Trusted channel authority still needs one consistent predicate across every
storefront Search result family, especially Product. Remaining Forum filters,
owner-issued projection revisions, durable ordering, reconciliation, ACL or
deletion cleanup, PostgreSQL evidence, and `LINK-FORUM-03` remain separate
slices.
