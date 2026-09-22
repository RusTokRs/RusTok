# ADR: `rustok-taxonomy` as a shared scope-aware vocabulary module

## Status

Accepted

## Context

RusToK already has multiple incompatible tag shapes:

- `rustok-blog` owns typed tag tables and post-tag relations;
- `rustok-forum` stores topic tags as a JSON array on `forum_topics`;
- `rustok-product` / `rustok-commerce` currently only expose free-form metadata;
- `rustok-profiles` does not yet have a first-class tag/interest surface;
- legacy generic tags still exist in `rustok-content`, but `rustok-content` is no longer the
  canonical storage owner for blog/forum/pages-style product domains after the content split.

We need a shared vocabulary layer that supports:

- global terms reused across modules;
- module-local terms that stay attachable only inside one module;
- locale-aware names/slugs/aliases;
- future faceting/search/recommendation uses;
- strict separation between shared dictionary ownership and module-owned domain attachments.

## Decision

Introduce a new optional module: `rustok-taxonomy`.

`rustok-taxonomy` owns the shared dictionary only:

- `taxonomy_terms`
- `taxonomy_term_translations`
- `taxonomy_term_aliases`

The initial contract is scope-aware:

- `scope_type = global` means a term can be attached from any module;
- `scope_type = module` means a term can be attached only from the module named in `scope_value`.

The initial term kind is:

- `kind = tag`

Canonical identity is locale-independent:

- each term has a stable `canonical_key` inside `(tenant, kind, scope)`;
- localized `name`, `slug`, and aliases live in translation/alias tables.

Module-owned entity attachments remain outside `rustok-taxonomy`.

Target relation examples:

- `blog_post_terms`
- `forum_topic_terms`
- `product_terms`
- `profile_terms`

`rustok-taxonomy` must not become a polymorphic shared attachment backend.
Generic `target_type/target_id` attachment tables are explicitly not the target architecture.

## Consequences

Positive:

- one shared vocabulary layer can serve blog/forum/product/profile surfaces;
- global and module-local terms use one coherent model instead of duplicated tag systems;
- locale normalization and fallback can stay aligned with the shared content contract;
- later search facets, aliases, curated topics, and recommendations can build on the same term IDs.

Tradeoffs:

- domain modules still need their own migration slices for relation tables;
- current blog/forum tag paths need staged migration into the new vocabulary owner;
- product/profile integrations remain future slices until relation tables are added.

## Phase 1 rollout

Phase 1 for `rustok-taxonomy` includes:

- module crate and runtime metadata;
- scope-aware term dictionary with `global | module`;
- localized translations and aliases;
- CRUD/list/lookup services;
- module-aware resolution that prefers module-local terms before global terms.

It does not yet include:

- hierarchy / broader-narrower term relations;
- subscriptions/interests;
- domain relation tables in blog/forum/product/profile modules.
