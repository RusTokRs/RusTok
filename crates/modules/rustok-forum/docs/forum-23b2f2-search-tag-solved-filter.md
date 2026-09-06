# FORUM-23B2F2 exact Forum Search tag and solved filters

## Status

`source_complete_execution_pending`

This slice adds exact bounded tag and solved-state filters to the explicit
Forum-only storefront Search path. Runtime, PostgreSQL and reindex evidence remain
maintainer-owned and are not claimed here.

## Owner projection boundary

Forum remains the owner of topic tags and solution state. Search evaluates only
Forum-projected values:

```text
forum_topic.payload.tags
forum_topic.payload.solution_reply_id
forum_reply.payload.topic_tags
forum_reply.payload.is_solution
```

Topic tags are already projected. This slice additionally copies the parent
topic's localized tags into each approved reply projection as `topic_tags`, so a
reply can participate in the same exact tag scope without Search reading Forum
tables or importing Forum policy.

Legacy reply documents without `topic_tags` fail closed while a tag filter is
active. A targeted or full Forum Search reindex is required before those replies
can match tag-scoped queries. Searches without tag filters preserve their previous
behavior.

## Input contract

The Forum GraphQL field accepts optional `tags` and `solved` arguments. The
additive native filter endpoint accepts equivalent values. At most ten tag values
are accepted, each up to 64 characters after trimming.

Tag matching is exact, case-sensitive and uses AND semantics: every requested tag
must occur in the projected tag list. This follows the Forum owner, which trims
and deduplicates tags without lowercasing them. A missing tag list or an array
containing any non-string value fails closed while tag scope is active.

`solved` is a nullable boolean:

- for topics, `true` requires a valid UUID string in `solution_reply_id`, while
  `false` requires an explicit JSON null;
- for replies, `true` means the reply is the exact current solution and `false`
  means its projected `is_solution` value is false;
- a missing, wrongly typed or otherwise malformed projected solved marker fails
  closed.

The arguments remain separate from neutral `SearchPreviewInput`, `SearchQuery`
and `SearchPreviewFilters` contracts.

## Evaluation order

The existing Forum-only execution owner first resolves its stable raw Search
snapshot. The raw total must remain at or below 100 before tag or solved narrowing.
A broad query cannot bypass the existing Forum owner-call bound merely because a
later filter would reduce the result set.

After the raw snapshot is complete and stable:

1. Search intersects active author, tag and solved predicates.
2. Categories and non-Forum documents are excluded whenever any document filter
   is active.
3. Forum performs exact current topic and approved-reply eligibility for the
   retained candidates.
4. Visible totals, facets, offset and limit are computed from the filtered and
   authorized intersection while preserving raw ranking order.

Query-rule pins remain disabled whenever any Forum document filter is active,
because the pin loader does not carry those filters and must not reintroduce an
out-of-scope result.

## Transport compatibility

The existing `ForumStorefrontSearch` and `ForumStorefrontSearchByAuthors` GraphQL
operations remain unchanged. Author/tag/solved calls use the additive
`ForumStorefrontSearchByFilters` operation.

The existing native endpoints remain unchanged:

```text
search/forum-storefront-search
search/forum-storefront-search-by-authors
```

Filtered calls use the additive endpoint:

```text
search/forum-storefront-search-by-filters
```

All operations delegate to the same Search execution owner.

## Compatibility and degraded mode

No database migration, public shared input/DTO field, neutral `SearchQuery` field,
dependency or `Cargo.lock` change is introduced. The reply projection payload gains
`topic_tags`; legacy reply rows require reindex for positive tag matches and fail
closed until repaired. Existing unfiltered and author-only wire operations retain
their previous shape for rolling deployments.

Missing category-scope or result-eligibility owner composition continues to fail
closed exactly as before. This slice does **not** add storefront UI controls,
locale/date/kind/channel/group/attachment filters, durable non-Forum projection
ordering, deletion or ACL cleanup, or runtime evidence.

## Maintainer verification

The implementation agent did not run these commands:

```bash
cargo test -p rustok-search forum_document_filters -- --nocapture
cargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture
cargo test -p rustok-search-storefront transport::tests::only_explicit_forum_category_scope_selects_owner_path -- --nocapture
node scripts/verify/verify-forum-search-tag-solved-filter.mjs
cargo check -p rustok-forum --all-targets
cargo check -p rustok-search --features graphql --all-targets
cargo check -p rustok-search-storefront --features ssr --all-targets
cargo xtask module validate forum
cargo xtask module validate search
```

PostgreSQL proof should cover tagged solved and unsolved topics, solution and
ordinary approved replies, a reply reindexed with parent-topic tags, one legacy
reply without `topic_tags`, categories, malformed tag/solution projections,
mismatched case, multiple-tag AND semantics, totals, facets and pagination after
exact owner eligibility.
