# `rustok-taxonomy` Documentation

`rustok-taxonomy` — shared vocabulary module of the platform. It owns the dictionary
layer for terms, translations and aliases, but does not take ownership of domain
attachment tables from blog/forum/product/profile and other modules.

## Purpose

- publish the canonical taxonomy dictionary contract;
- keep term identity, localized labels/slugs and scope rules inside the module;
- provide domain modules with shared vocabulary without reverting to polymorphic shared product storage.

## Scope

- `taxonomy_terms`, `taxonomy_term_translations`, `taxonomy_term_aliases`;
- tenant-scoped term identity and `canonical_key`;
- scope contract for `global` and `module` terms;
- alias-aware lookup and module integration helpers;
- no ownership over relation tables such as `blog_post_tags` or `forum_topic_tags`.

## Integration

- `rustok-blog`, `rustok-forum`, `rustok-product` and `rustok-profiles` use taxonomy as a shared dictionary;
- attachment ownership and public domain contracts remain inside owning modules;
- locale normalization and fallback must remain synchronized with the shared `rustok-content` contract;
- any new taxonomy consumers must enter through explicit module-owned relation tables.

## Verification

- `cargo xtask module validate taxonomy`
- `cargo xtask module test taxonomy`
- targeted tests for term CRUD, scope rules, alias lookup and consumer-module integration helpers

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)
