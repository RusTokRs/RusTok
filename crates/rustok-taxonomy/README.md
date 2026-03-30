# rustok-taxonomy

## Purpose

`rustok-taxonomy` owns the scope-aware taxonomy dictionary for RusToK.

## Responsibilities

- Provide tenant-scoped taxonomy terms that can be either shared across modules or limited to one module.
- Keep canonical term identity separate from localized names and slugs.
- Own taxonomy storage (`taxonomy_terms`, `taxonomy_term_translations`, `taxonomy_term_aliases`) and migrations.
- Expose CRUD/list/lookup services for shared and module-local taxonomy terms.
- Provide transaction-aware helpers for domain modules that need to resolve or create module-local terms inside their own write transactions.
- Reuse the platform multilingual locale/fallback contract so blog/forum/pages-style locale handling stays consistent.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Reuses locale normalization and fallback helpers from `rustok-content`.
- Already backs forum topic tags through forum-owned `forum_topic_tags`.
- Already backs blog tags through blog-owned `blog_post_tags`.
- Already backs product tags through product-owned `product_tags`.
- Already backs profile tags through profile-owned `profile_tags`.
- Is the shared vocabulary layer for `blog`, `forum`, `product`, `profiles`, and future modules
  while leaving entity-term relation tables module-owned.

## Entry points

- `TaxonomyModule`
- `TaxonomyService`
- `dto::*`
- `entities::*`
- `migrations::*`

See also `docs/README.md`.
