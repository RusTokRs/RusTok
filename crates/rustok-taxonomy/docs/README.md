# rustok-taxonomy docs

## Current role

`rustok-taxonomy` is the shared vocabulary module for RusToK.

At the current phase it owns:

- taxonomy term dictionary;
- localized names/slugs/descriptions;
- localized aliases;
- scope contract for `global` and `module` terms.

It does **not** own domain entity attachments. Relation tables such as `blog_post_terms`,
`forum_topic_terms`, `product_terms`, or `profile_terms` stay module-owned.

## Core invariants

- Term identity is tenant-scoped and locale-independent.
- `canonical_key` is the stable key inside a `(tenant, kind, scope)` boundary.
- `scope_type = global` means the term is attachable from any module.
- `scope_type = module` means the term is attachable only from the named module in `scope_value`.
- Localized `slug` and aliases must be unique inside the same `(tenant, kind, scope, locale)` boundary.
- Locale normalization and fallback follow the shared `rustok-content` contract.

## Phase 1 surface

- `kind = tag`
- `scope_type = global | module`
- CRUD for terms
- localized translation upsert
- localized alias replacement
- list and lookup APIs
- transaction-aware module integration helpers for module-owned relation tables

## Current integrations

- `rustok-forum` now uses `rustok-taxonomy` as the shared dictionary behind
  forum-owned `forum_topic_tags`.
- Forum keeps the attachment table locally, while taxonomy remains responsible
  only for term identity, translations, aliases, and scope rules.
- `rustok-blog` uses taxonomy-backed `blog_post_tags` while keeping blog write
  flows and public `tags: Vec<String>` contract module-owned.
- `rustok-product` uses taxonomy-backed `product_tags`; first-class product
  `tags` are the public contract surface and legacy `metadata.tags` is no longer
  mirrored.
- `rustok-profiles` uses taxonomy-backed `profile_tags` for profile labels /
  interests without moving profile ownership into the taxonomy module.
