# rustok-taxonomy implementation plan (`rustok-taxonomy`)

## Scope and objective

This document is the source of truth for the initial rollout of `rustok-taxonomy`.

Primary objective: establish a shared taxonomy dictionary that supports both platform-wide
and module-local terms without reintroducing polymorphic shared product storage.

## Phase 1 - foundation dictionary

- [x] Create module crate and runtime metadata.
- [x] Add module-owned migrations for:
  - `taxonomy_terms`
  - `taxonomy_term_translations`
  - `taxonomy_term_aliases`
- [x] Implement tenant-scoped term dictionary with:
  - `kind = tag`
  - `scope_type = global | module`
  - stable `canonical_key`
  - `status = active | deprecated`
- [x] Reuse shared locale normalization and fallback helpers from `rustok-content`.
- [x] Add CRUD/list/lookup service APIs with alias-aware slug resolution.
- [x] Add targeted service tests for uniqueness, locale fallback, and scope-aware lookup.

## Planned next slices

- [x] Add `forum_topic_tags` and migrate forum write-paths off raw `forum_topic.tags`
  JSON as the primary source of truth.
  - [x] `rustok-forum` now owns typed relation table `forum_topic_tags`.
  - [x] Forum topic create/update flows resolve or create module-scoped taxonomy
    terms transactionally through `rustok-taxonomy`.
  - [x] Existing external forum `tags: Vec<String>` contract stays unchanged.
- [x] Integrate `rustok-blog` vocabulary with shared taxonomy terms.
  - [x] `rustok-blog` now keeps `blog_post_tags` as the module-owned relation table.
  - [x] Blog tag CRUD/list flows resolve through `rustok-taxonomy` instead of blog-local tag tables.
  - [x] Post create/update flows auto-create blog-scoped terms and reuse matching global terms.
- [x] Add product-facing `product_tags` relation instead of metadata-only product tags.
  - [x] `rustok-product` now owns typed relation table `product_tags`.
  - [x] Product create/update flows sync first-class `tags` into shared taxonomy terms.
  - [x] Product transport surfaces expose first-class `tags`, and legacy `metadata.tags`
    is retired.
- [x] Add profile tags through `profile_tags`.
  - [x] `rustok-profiles` now depends on `rustok-taxonomy`.
  - [x] Profile upsert/read paths sync and resolve taxonomy-backed tags.
  - [x] Public profile DTO / GraphQL surfaces expose `tags: Vec<String>`.
