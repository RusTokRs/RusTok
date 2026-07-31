# FORUM-23B2F4 exact Forum Search document kind filter

## Status

`source_complete_execution_pending`

This slice adds an exact bounded document-kind filter to the existing unified
Forum-only storefront Search execution owner. Runtime and PostgreSQL evidence
remain maintainer-owned and are not claimed here.

## Kind contract

The Forum GraphQL field accepts an optional `kinds` argument. The additive native
endpoint accepts the equivalent list. At most two values are accepted. Values are
trimmed, normalized to ASCII lowercase, deduplicated and must be one of:

```text
topic
reply
```

`topic` maps exactly to Search entity type `forum_topic`; `reply` maps exactly to
`forum_reply`. These values describe the already delivered Search document kinds.
They do not introduce or claim future Forum topic-policy kinds such as wiki,
announcement or Q&A.

The argument remains separate from neutral `SearchPreviewInput`, `SearchQuery` and
shared `SearchPreviewFilters` contracts.

## Evaluation order

The existing unified Forum execution owner resolves one stable raw Search snapshot.
The raw total must remain at or below 100 before kind narrowing. A broad query
cannot bypass the existing Forum owner-call bound because a later kind filter would
reduce the result set.

After the raw snapshot is complete and stable:

1. Search intersects exact locale, author, tag, solved, date and kind predicates.
2. Categories and non-Forum documents do not match an active kind filter.
3. Forum performs exact current topic and approved-reply eligibility for retained
   candidates.
4. Visible totals, facets, offset and limit are computed from the filtered and
   authorized intersection while preserving raw ranking order.

Query-rule pins remain disabled whenever the kind filter is active because the pin
loader does not carry the Forum-specific kind argument and must not reintroduce an
out-of-scope document.

## Transport compatibility

Existing GraphQL operations remain unchanged:

```text
ForumStorefrontSearch
ForumStorefrontSearchByAuthors
ForumStorefrontSearchByFilters
ForumStorefrontSearchByDateWindow
```

Kind-scoped calls use the additive operation:

```text
ForumStorefrontSearchByKinds
```

Existing native endpoint signatures remain unchanged. Kind-scoped calls use:

```text
search/forum-storefront-search-by-kinds
```

All paths delegate to the existing unified `execute_forum_storefront_search` owner.

## Compatibility and degraded mode

No database migration, Forum projection field, neutral Search query/input/DTO
field, dependency or `Cargo.lock` change is introduced. Empty `kinds` preserves
existing behavior. Invalid or unsupported values fail validation before Search.
Missing category-scope or result-eligibility owner composition continues to fail
closed exactly as before.

This slice does **not** add storefront UI controls, future Forum topic-policy kinds,
channel/group filters, attachment-presence filters, projection ordering,
delete/ACL cleanup or runtime evidence.

## Maintainer verification

The implementation agent did not run these commands:

```bash
cargo test -p rustok-search forum_document_filters -- --nocapture
cargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture
cargo test -p rustok-search-storefront transport::tests::only_explicit_forum_category_scope_selects_owner_path -- --nocapture
node scripts/verify/verify-forum-search-kind-filter.mjs
cargo check -p rustok-search --features graphql --all-targets
cargo check -p rustok-search-storefront --features ssr --all-targets
cargo xtask module validate forum
cargo xtask module validate search
```

PostgreSQL proof should cover topic-only, reply-only and both-kind requests,
intersection with author/tag/solved/date, categories, invalid values, the raw
candidate cap, exact owner eligibility, totals, facets and pagination.
