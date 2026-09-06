# FORUM-23B2F3 exact Forum Search locale and date filters

## Status

`source_complete_execution_pending`

This slice locks the existing explicit Forum-only storefront Search execution to
one normalized effective locale and adds inclusive published-date filtering to
that same owner path. It deliberately does not create a second execution,
normalization, candidate scan, authorization or pagination implementation.
Runtime, PostgreSQL and reindex evidence remain maintainer-owned and are not
claimed here.

## Recheck result

The previous draft branch implemented the date window through parallel execution
and normalization modules. That shape duplicated the existing Forum Search owner
and could drift in validation, category scope, owner eligibility, totals, facets,
pagination and query-rule behavior. The continuation replaces that draft with one
execution owner:

```text
crates/rustok-search/src/forum_storefront_execution.rs
```

Locale, author, tag, solved and date predicates are represented by one bounded
`ForumStorefrontDocumentFilters` value and are evaluated in the existing stable
candidate scan.

## Exact locale boundary

The existing `SearchPreviewInput.locale` remains the only locale input. It is
normalized through the existing bounded locale rules; when absent, the tenant
fallback locale becomes the exact PostgreSQL FTS/typo locale and the post-scan
locale assertion. Missing, foreign-module or mismatched locale rows fail closed.

Locale-only execution preserves Forum categories, topics and replies and keeps
query-rule pins enabled. Existing legacy, author-only and tag/solved calls use the
same execution owner and retain their wire signatures.

## Owner date projection

Forum remains the owner of topic and reply creation time. Search evaluates only
Forum-projected values:

```text
forum_topic.payload.published_at
forum_reply.payload.published_at
```

Both values are derived from owner `created_at` and serialized as RFC3339. Legacy
topic or reply documents without this field fail closed while a date window is
active until a targeted or full Forum Search reindex repairs them.

## Input and evaluation

The GraphQL field accepts optional `publishedFrom` and `publishedTo` strings. The
additive native endpoint accepts `published_from` and `published_to`. Non-empty
values must be RFC3339 and are normalized to UTC. Bounds are inclusive; either may
be omitted, and a lower bound after the upper bound is rejected.

The stable raw 100-candidate cap is checked before date narrowing. Exact locale,
author, tag, solved and date predicates intersect before exact Forum topic/reply
owner eligibility, visible totals, facets, offset and limit. Categories do not
match an active date window. Ranking order is preserved and query-rule pins are
disabled only when a real document-narrowing filter is active.

## Transport compatibility

The existing GraphQL operations remain unchanged:

```text
ForumStorefrontSearch
ForumStorefrontSearchByAuthors
ForumStorefrontSearchByFilters
```

Date-window calls use the additive operation:

```text
ForumStorefrontSearchByDateWindow
```

The existing native endpoints also remain unchanged. Date-window calls use:

```text
search/forum-storefront-search-by-date-window
```

No shared Search input/DTO or neutral `SearchQuery` field is added.

## Compatibility and degraded mode

No database migration, dependency or `Cargo.lock` change is introduced. Topic and
approved-reply payloads gain `published_at`; legacy rows fail closed for date
queries until reindexed. Missing category-scope or result-eligibility owner
composition continues to fail closed. This slice does **not** add storefront UI
controls, kind/channel/group/attachment filters, durable projection ordering,
delete/ACL cleanup or runtime evidence.

## Maintainer verification

The implementation agent did not run these commands, as requested:

```bash
cargo test -p rustok-search forum_document_filters -- --nocapture
cargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture
cargo test -p rustok-search date_bounds_require_rfc3339 -- --nocapture
cargo test -p rustok-search-storefront transport::tests::only_explicit_forum_category_scope_selects_owner_path -- --nocapture
node scripts/verify/verify-forum-search-locale-date-filter.mjs
cargo check -p rustok-forum --all-targets
cargo check -p rustok-search --features graphql --all-targets
cargo check -p rustok-search-storefront --features ssr --all-targets
cargo xtask module validate forum
cargo xtask module validate search
```

PostgreSQL proof should cover requested and fallback locales, mismatched locale,
inclusive and one-sided bounds, reversed input, categories, legacy rows without
`published_at`, malformed timestamps, the raw candidate cap, owner eligibility,
totals, facets and pagination.
