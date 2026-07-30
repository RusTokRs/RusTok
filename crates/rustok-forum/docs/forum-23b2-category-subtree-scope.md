# FORUM-23B2A: Forum-owned Search category subtree scope

Date: 2026-07-30

Status: `source_complete_execution_pending`

Canonical roadmap: [`implementation-plan.md`](./implementation-plan.md)

## Purpose

`FORUM-23B1` extended the shared Search `category_ids` field with exact Forum category matching while intentionally leaving category-tree policy in Forum. This slice publishes the bounded Forum owner operation needed to resolve selected category roots into an already-authorized subtree scope.

The new service does not execute Search and does not import `rustok-search`. It returns identifiers that a host-composed Forum Search entrypoint can place into the existing internal `SearchQuery.category_ids` field.

## Owner contract

`ForumSearchCategoryScopeService::expand_visible_subtrees`:

- requires `forum_categories:list` through the existing RBAC owner boundary;
- accepts at most ten raw selected roots before deduplication, matching the existing public Search input bound;
- loads at most the canonical 512-node, depth-16 tenant category tree;
- validates duplicate identities, missing or foreign parents, cycles and depth overflow;
- reuses the current inherited public/authenticated visibility owner instead of copying that policy;
- excludes archived categories and prunes every excluded branch;
- reports missing, foreign, archived or viewer-hidden selected roots as `CategoryNotFound`;
- preserves first-occurrence root order and owner `(position, id)` child order;
- emits deterministic preorder identifiers and deduplicates overlapping roots.

A selected descendant beneath an excluded ancestor also fails as `CategoryNotFound`, even if invalid persistence state attempted to leave that descendant active.

## Search integration boundary

Search remains owner-neutral. It continues to implement exact matching for the identifiers supplied in `SearchQuery.category_ids`:

- product documents use normalized product-category relations;
- Forum category documents use their document identifier;
- Forum topic and approved-reply documents use the projected `facets.category_id` value.

Forum owns expansion and authorization. Search does not query Forum tables, reconstruct the category hierarchy or infer category visibility.

This slice does not yet compose expansion into a public Search transport. That follow-up belongs in a host or Forum-owned entrypoint that can call Forum first and Search second without creating a crate dependency cycle.

## Compatibility and degraded mode

No GraphQL field, REST route, Rust Search query shape, Forum projection shape, migration, dependency or `Cargo.lock` change is introduced. Exact-category Search remains available exactly as delivered in `FORUM-23B1`.

If the owner expansion service is not composed, callers retain exact-category behavior; they must not silently broaden a selected root by guessing descendants. Search-disabled behavior remains the canonical typed-unavailable or bounded SQL fallback policy from the Forum implementation plan.

The current visibility reuse covers the delivered inherited public/authenticated category floor. Complete role, trust, Channel, Groups and explicit-user audience composition remains open and is not claimed by this slice.

## Remaining work

- compose owner-expanded identifiers into a Forum Search entrypoint before Search execution;
- apply the complete richer Forum audience decision to category and topic Search scope;
- add exact author, tag, locale, date, solved, kind, channel/group and attachment-presence filters;
- complete owner-issued projection revision ordering, reconciliation and deletion/ACL cleanup;
- capture maintainer-executed PostgreSQL query/result evidence.

## Maintainer verification

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-forum category_search_scope -- --nocapture
node scripts/verify/verify-forum-search-category-subtree-scope.mjs
node scripts/verify/verify-forum-search-exact-category-filter.mjs
cargo xtask module validate forum
```
