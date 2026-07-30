# FORUM-23B2B: richer category audience scope for Search

Date: 2026-07-30

Status: `source_complete_execution_pending`

Canonical roadmap: [`implementation-plan.md`](./implementation-plan.md)

## Purpose

`FORUM-23B2A` introduced a bounded Forum-owned category-subtree expansion, but intentionally covered only the inherited public/authenticated floor. Forum already owns richer inherited category audience layers for roles, trust, Channel membership, Groups membership, and explicit user allow/deny rules.

This slice adds the owner operation that applies those delivered audience rules before a category subtree may be handed to Search. It prevents a future host Search composition from broadening an authenticated viewer beyond the exact category visibility used by Forum reads.

## Owner contract

`ForumSearchCategoryAudienceScopeService` exposes separate public and authenticated entrypoints.

The public entrypoint:

- uses `SecurityContext::public_read()` and the public Forum audience viewer;
- excludes authenticated-floor categories, archived categories, and every richer layer that requires an authenticated selector;
- never invokes an optional trust, Channel, or Groups owner capability for an anonymous viewer.

The authenticated entrypoint:

- requires the existing `forum_categories:list` owner permission;
- requires an exact user `SecurityContext` and matching tenant/user `PortContext`;
- reuses `ForumCategoryAudienceVisibilityService`, including inherited root-to-category conjunction semantics;
- lets local explicit deny, explicit allow, and role decisions remain locally decidable;
- invokes the shared owner facts capability only when unresolved trust, Channel, or Groups selectors require it;
- fails closed with the existing typed capability error when required owner facts are unavailable.

Both entrypoints:

- accept at most ten raw selected roots before deduplication;
- reuse the canonical 512-node, depth-16 category tree;
- exclude archived categories;
- prune a denied ancestor together with all descendants;
- return a denied, archived, missing, or foreign selected root as `CategoryNotFound`;
- preserve selected-root first occurrence and canonical child order;
- emit deterministic preorder IDs and deduplicate overlapping roots.

## Search integration boundary

This service still does not execute Search and does not import `rustok-search`. It returns `ForumSearchCategoryScope`, the same owner result shape introduced in B2A.

A later host-composed Forum-only Search entrypoint can call this owner first and then place `expanded_category_ids` into the existing Search `category_ids` field. The host must keep that expansion restricted to an explicit Forum-only source scope so product category semantics are not broadened.

Topic-local audience narrowing and exact reply authorization are not category-tree properties. They remain separate Search result eligibility work and are not claimed here.

## Compatibility and degraded mode

No GraphQL or REST field, Search query shape, Forum projection shape, migration, dependency, or `Cargo.lock` change is introduced. The B2A public/authenticated-floor service remains available for internal callers that explicitly require only that delivered baseline.

Public evaluation does not require optional audience facts. Authenticated evaluation remains operational for locally decidable rules when the facts provider is absent. A category layer that still requires trust, Channel, or Groups facts fails closed rather than appearing in Search.

## Remaining work

- compose this richer owner result into an explicit Forum-only Search entrypoint;
- apply exact topic-local narrowing and reply authorization to Search result eligibility;
- add author, tag, locale, date, solved, kind, channel/group, and attachment-presence filters;
- complete owner-issued projection revision ordering, reconciliation, and deletion/ACL cleanup;
- capture maintainer-executed PostgreSQL query and result evidence.

## Maintainer verification

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-forum category_search_audience_scope -- --nocapture
node scripts/verify/verify-forum-search-category-audience-scope.mjs
node scripts/verify/verify-forum-search-category-subtree-scope.mjs
cargo xtask module validate forum
```
