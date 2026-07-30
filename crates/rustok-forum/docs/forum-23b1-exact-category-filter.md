# FORUM-23B1: exact category Search filter

Date: 2026-07-30

Status: `source_complete_execution_pending`

## Purpose

Forum Search already projects public category, topic, and approved-reply documents. Topic and reply documents carry the exact public category identifier in their bounded JSONB facets, while category documents retain their category identifier as the Search document identifier.

The shared Search query contract already exposes a bounded `category_ids` input for product category filtering. B1 extends that existing contract rather than adding a Forum-only transport field.

## Query contract

`SearchPreviewInput.category_ids` remains the public GraphQL input. Existing normalization still:

- accepts at most ten values;
- parses every value as a UUID before query execution;
- rejects invalid UUIDs;
- applies the same input to preview, admin global Search, and public storefront Search;
- preserves the storefront `published_only` policy.

Callers that require only Forum documents should pair `category_ids` with `source_modules: ["forum"]`. A mixed-source query may intentionally return matching product and Forum documents because `category_ids` is a shared owner-neutral Search field.

## PostgreSQL filter behavior

`PgSearchEngine::build_filter_clause` retains the existing product branch based on `index_product_categories`. It adds a separate `source_module = 'forum'` branch:

- `forum_category` documents match when `document_id` equals one of the requested category identifiers;
- `forum_topic` and `forum_reply` documents match when `facets.category_id` equals one of the requested identifiers.

UUID owner identifiers and their JSON string representations are inserted only as bound parameters. The query does not interpolate caller values into SQL.

Both the full-text and typo-tolerant ranked CTEs now carry `search_documents.facets`. Total counting, paged result loading, and facet aggregation continue to use the same generated filter clause.

## Projection authority

Forum remains the source of category placement. Search reads only the already-authorized public projection:

- category documents come from public category discovery;
- topics come from public topic and category discovery;
- replies require approved status and reauthorize their topic and category;
- pending, hidden, denied, deleted, or otherwise non-public owner state is not made searchable by this filter.

B1 does not copy category-tree visibility or placement policy into Search.

## Scope boundary

This is an exact-category foundation, not category-subtree completion. It does not resolve descendants, read the Forum category tree during a Search request, add category bucket labels, or implement the remaining author, tag, locale, date, solved, kind, channel/group, and attachment-presence filters.

A bounded, visibility-authorized subtree expansion remains `FORUM-23B2`.

## Compatibility

No GraphQL field, Rust `SearchQuery` field, Search document shape, Forum projection shape, database migration, dependency, or `Cargo.lock` change is introduced. Existing product category filtering remains active.

## Maintainer verification

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
node scripts/verify/verify-forum-search-exact-category-filter.mjs
cargo test -p rustok-search category_filter_preserves_product_and_adds_exact_forum_scope -- --nocapture
cargo test -p rustok-search pg_engine -- --nocapture
cargo check -p rustok-search --all-targets
cargo xtask module validate search
cargo xtask module validate forum
```
