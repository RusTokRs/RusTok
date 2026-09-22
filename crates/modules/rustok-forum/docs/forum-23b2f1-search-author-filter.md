# FORUM-23B2F1 exact Forum Search author filter

## Status

`source_complete_execution_pending`

This slice adds an exact bounded author filter to the explicit Forum-only
storefront Search path. Runtime and PostgreSQL evidence remain maintainer-owned
and are not claimed here.

## Owner boundary

Forum remains the owner of public author presentation. Topic and approved-reply
Search projections already contain the public `ProfileSummary` under:

```text
payload.author.user_id
```

Search evaluates only that public projected identifier. It does not read the raw
Forum author relation, import Profiles policy or reconstruct an identity after
Forum omitted, denied or redacted the public author summary.

A missing, null, malformed, denied or redacted public author therefore does not
match an active author filter.

## Input contract

The explicit GraphQL field accepts optional `authorIds`; the additive native
server function accepts the equivalent `author_ids`. Values are UUID strings and
the existing filter bound limits the list to ten values.

The author argument is intentionally separate from the shared
`SearchPreviewInput`, `SearchQuery` and `SearchPreviewFilters` contracts. Ordinary
storefront Search, mixed sources, Product Search, admin preview and admin global
Search do not gain Forum-specific semantics.

## Evaluation order

The existing Forum-only execution owner still performs its stable raw Search
scan from offset zero in bounded pages. The raw candidate total must remain at or
below 100 before author filtering; a broad query is not admitted merely because
an author filter would later narrow it.

After the raw snapshot is complete and stable:

1. Search retains only Forum topics and replies whose public projected author ID
   exactly matches one requested UUID.
2. Forum categories and every non-Forum document are excluded while the filter is
   active.
3. The existing Forum owner evaluates topic-local and approved-reply visibility
   only for the retained candidates.
4. Visible totals, facets, offset and limit are computed from the intersection of
   author scope and owner authorization while preserving raw ranking order.

Query-rule pins are disabled while an author filter is active. The existing pin
loader has no Forum author argument, so applying pins afterwards could reintroduce
a document outside the requested author scope.

## Transport parity

The GraphQL and native adapters carry author IDs as a Forum-specific argument to
the same Search execution owner. The existing GraphQL operation
`ForumStorefrontSearch` stays byte-for-byte free of the new argument for rolling
compatibility, while author-scoped calls use the additive
`ForumStorefrontSearchByAuthors` operation. Native keeps the existing
`search/forum-storefront-search` endpoint and its wire shape unchanged, while
author-scoped calls use the additive
`search/forum-storefront-search-by-authors` endpoint.

The storefront transport facade exposes `fetch_forum_search_by_authors` without
changing the general `fetch_search` contract. Existing explicit Forum Search calls
continue through the old GraphQL operation or native endpoint with the previous
behavior.

## Compatibility and degraded mode

No database migration, projection backfill, public `SearchPreviewInput` field,
shared storefront filter DTO, `SearchQuery` field, dependency or `Cargo.lock`
change is introduced. Existing GraphQL and native wire operations also retain
their previous shape for rolling-deploy compatibility.

The filter relies on public author data already present in current Forum topic and
reply projections. Documents whose author summary is absent remain searchable
without an author filter but fail closed when an author scope is requested.
Missing Forum category-scope or result-eligibility owner composition continues to
fail closed exactly as before.

This slice does **not** add a storefront UI control, tag/solved/date/kind filters,
durable non-Forum projection ordering, deletion/ACL cleanup or runtime evidence.

## Maintainer verification

The implementation agent did not run these commands:

```bash
cargo test -p rustok-search forum_document_filters -- --nocapture
cargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture
cargo test -p rustok-search-storefront transport::tests::only_explicit_forum_category_scope_selects_owner_path -- --nocapture
node scripts/verify/verify-forum-search-author-filter.mjs
cargo check -p rustok-search --features graphql --all-targets
cargo check -p rustok-search-storefront --features ssr --all-targets
cargo xtask module validate forum
cargo xtask module validate search
```

PostgreSQL proof should cover one topic and one approved reply by the requested
public author, a visible document by another author, a category, and a document
whose public author summary is absent. Totals, facets and pagination must include
only the authorized exact-author intersection.
