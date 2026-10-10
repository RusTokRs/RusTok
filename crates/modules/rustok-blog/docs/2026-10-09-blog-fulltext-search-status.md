# H-3: Full-text Search — Status: Already Integrated via `rustok-search`

- **Date**: 2026-10-09
- **Status**: ✅ Already implemented
- **Owner**: `rustok-search` (projector), `rustok-blog` (events)

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

## What Could Be Improved (Follow-up)

1. **Admin scoped search**: Add `search` parameter to admin `posts` GraphQL query for convenient admin panel search (currently must use global search)
2. **Search suggestions**: Blog post titles could feed into the search suggestions API
3. **Search analytics**: Track which blog searches are popular to improve content strategy

These are convenience improvements, not gaps. The core search functionality is complete.
