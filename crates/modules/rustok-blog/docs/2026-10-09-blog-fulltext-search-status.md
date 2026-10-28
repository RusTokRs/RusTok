# H-3: Full-text Search — Status: Already Integrated via `rustok-search`

- **Date**: 2026-10-09
- **Status**: superseded by the 2026-10-25 projection-driver cutover
- **Owner**: `rustok-search` (storage driver), `rustok-blog` (documents and events)
- **Superseded by**: `DECISIONS/2026-10-25-search-projection-driver-ownership.md` and the
  "Blog projection cutover" section of `rustok-search/docs/implementation-plan.md`

> The architecture described below is the state of 2026-10-09 and is kept as history.
> `BlogSearchProjector` no longer exists: `rustok-blog` owns the documents through
> `BlogSearchProjectionSource`, Search stores them through `SearchProjectionDriver`, the
> author name comes from the Profiles port instead of a `users` join, and the indexed
> corpus is the public Blog corpus (`published` and stamped).

## Overview

Full-text search for blog posts is **already fully integrated** through the
`rustok-search` module. The audit flagged this as a gap, but the implementation
exists and is production-ready.

## Architecture

```
Blog Module                          Search Module
─────────────                        ─────────────
BlogPostCreated     ──event──→  ingestion.rs → BlogSearchProjector.upsert_post()
BlogPostPublished   ──event──→  ingestion.rs → BlogSearchProjector.upsert_post()
BlogPostUnpublished ──event──→  ingestion.rs → BlogSearchProjector.upsert_post()
BlogPostUpdated     ──event──→  ingestion.rs → BlogSearchProjector.upsert_post()
BlogPostArchived    ──event──→  ingestion.rs → BlogSearchProjector.upsert_post()
BlogPostDeleted     ──event──→  ingestion.rs → BlogSearchProjector.delete_post()
UserUpdated         ──event──→  ingestion.rs → BlogSearchProjector.refresh_author_projection()
TenantModuleToggled ──event──→  ingestion.rs → BlogSearchProjector.rebuild_tenant()
```

## Indexed Fields

The `BlogSearchProjector` indexes each post into `search_documents` with:

| Field | Source | Weight |
|-------|--------|--------|
| `title` | `blog_post_translations.title` | A (highest) |
| `subtitle` | Category name (localized) | B |
| `body` | Excerpt + RichText article plain text | C |
| `keywords_text` | Category + author name + SEO title/description + tags | D |
| `facets` | `has_category`, `has_tags`, `has_channels`, `channel_slugs` | — |
| `payload` | Full JSON with slug, excerpt, SEO, author, tags, channels, comment_count, version | — |

## Search API

### GraphQL (via `rustok-search`)

```graphql
query {
  searchStorefront(
    query: "rust async"
    entityTypes: ["blog_post"]
    sourceModules: ["blog"]
    publishedOnly: true
    limit: 20
  ) {
    items {
      id
      title
      snippet
      score
      url
      payload
    }
    total
    tookMs
  }
}
```

### Result URL Resolution

Search results for blog posts automatically resolve to:
```
/modules/blog?slug={slug}
```

via `canonical_search_result_url()` in `rustok-search::engine`.

## What Works

- ✅ Full-text search across title, excerpt, article body, tags, category, author name
- ✅ Locale-aware indexing (one document per translation)
- ✅ Channel visibility filtering
- ✅ Author name projection refresh on user updates
- ✅ Automatic reindexing on all post lifecycle events
- ✅ Tenant-scoped search
- ✅ Published-only filtering
- ✅ Ranking with configurable profiles
- ✅ Faceted search (by category, tags, channels)
- ✅ Owner-side `SearchProjectionSource` SPI: `BlogSearchProjectionSourceFactory` registered in `BlogModule::register_runtime_extensions` providing domain-owned `SearchProjectionDocument` emission, decoupled from search internals.

## Projection Architecture Evolution

1. **Owner SPI (`rustok-blog::search_projection`):**
   - Implements `SearchProjectionSourceFactory` and `SearchProjectionSource`.
   - `list_public_documents()` uses keyset cursor pagination (`PublishedPostCursor`), taxonomy category projection (`TaxonomyOwnerCategoryReader`), canonical tags (`blog_post_tags` + taxonomy dictionary), and profile summary resolution.
   - Decoupled from `rustok-search` database schemas; registered in module runtime extensions.
2. **Ingestion Runner (`rustok-search`):**
   - `SearchIngestionHandler::with_projection_sources` accepts both `blog_source` and `forum_source`.
   - `BlogSearchProjector` retained as backward-compatible fallback and schema validation harness.

## What Could Be Improved (Follow-up)

1. **Admin scoped search**: Add `search` parameter to admin `posts` GraphQL query for convenient admin panel search (currently must use global search)
2. **Search suggestions**: Blog post titles could feed into the search suggestions API
3. **Search analytics**: Track which blog searches are popular to improve content strategy

These are convenience improvements, not gaps. The core search functionality is complete.

## 2026-10-28 audit clarification (implementation in PR #4658)

The **backend Search projection** is real and owned by the canonical Search
storage driver, with Blog supplying its own public documents and events. The
backend alone is **not equivalent to working full-text search on the /blog
listing**: Next and Leptos still apply `q` as a substring filter over the
already fetched page of `publicBlogPosts`. This loses matching documents on
later pages and makes their Blog keyset cursors unrelated to full-text results.
Those storefront UIs must switch to the Search owner transport and Search's
offset pagination; they are not declared fixed by this backend audit.

The audit found that Blog's category UUID existed in `payload.category_id`
but Search's category filtering SQL reads `facets.category_id`. The facet
was added for future projections; **existing documents require a Blog scope
reindex** (the canonical `ReindexRequested(target_type="blog", target_id=None)`
event) to gain it. Do not attempt a second projection writer or a direct
mutation of Search tables from Blog.

The host's former public Search corpus declaration for Blog skipped live
Channel-module visibility. Blog is now registered as a gated
`SearchAudiencePolicyPort` through host composition; its bulk eligibility
lookup checks publication, tenant, post allowlist and Channel active/module
binding against the Blog owner. Search and autocomplete approvals are keyed by
`(source_module, document_id)` so independent modules with the same UUID do
not share visibility. Public Blog projections remain absent for drafts and
archived posts.

The generic Search gated-audience scan currently has a **1,000 candidate
limit** per request. Broad Blog searches may require narrower text/filters;
this is a known performance/capability constraint, not infinite pagination.
Search exposes category UUID filters, but not a combined exact Blog-tag filter;
full-text plus tag filtering in the Blog listing requires explicit support,
not local filtering of a single Search page.

All regressions are added as source tests but have **not been executed**.
