# FORUM-23B2C: explicit Forum storefront Search scope

Date: 2026-07-30

Status: `source_complete_execution_pending`

Canonical Forum roadmap: [`implementation-plan.md`](./implementation-plan.md)

Canonical Search roadmap: [`../../rustok-search/docs/implementation-plan.md`](../../rustok-search/docs/implementation-plan.md)

## Purpose

`FORUM-23B2B` made the bounded category-subtree scope equivalent to the delivered richer Forum category audience decision, but intentionally stopped before Search execution. This slice composes that owner result into a public storefront Search path without broadening product category semantics or introducing a direct Search-to-Forum dependency.

## Explicit selection boundary

The new execution path is selected only when the storefront request contains:

- exactly one normalized source module: `forum`;
- at least one category root;
- the tenant established by the current request.

GraphQL exposes `forumStorefrontSearch`. Native server-function transport exposes `search/forum-storefront-search`. The module-owned Search storefront transport chooses these endpoints only for the exact Forum/category shape above.

Unspecified, mixed, Product, Blog, Content, and Forum-without-category requests continue through the existing `storefrontSearch` GraphQL field or `search/storefront-search` native endpoint. Those paths and their exact category behavior are unchanged.

## Neutral owner composition

`rustok-search` publishes `StorefrontSearchCategoryScopePort`. The request carries bounded tenant, locale, exact source scope, category roots, optional authenticated context, request context, and transport identity. It contains no Forum persistence or audience types.

The server adapter is the only component that imports both owners. It:

- verifies that Forum is enabled for the request tenant;
- selects the authenticated Forum owner path only when the exact auth snapshot includes `forum_categories:list`;
- otherwise evaluates the public Forum audience scope;
- builds the existing exact tenant/user `PortContext` from trusted GraphQL or native request context;
- delegates to `ForumSearchCategoryAudienceScopeService`;
- returns only the already-authorized expanded category identifiers.

Missing owner composition, disabled Forum state, unresolved trust/Channel/Groups facts, denied roots, and owner failures fail closed. A denied, archived, missing, or foreign selected root remains non-oracular.

## Shared Search execution

GraphQL and native Forum-only endpoints call `execute_forum_storefront_search`. The owner:

- validates the explicit Forum-only source and non-empty root requirements before execution;
- preserves the existing storefront query, locale, filter, attribute, page, and query-length bounds;
- reuses Search dictionary transformation, tenant-effective filter presets, ranking resolution, PostgreSQL Search, query rules, canonical result URLs, and analytics;
- places the Forum-expanded identifiers into the existing `SearchQuery.category_ids` field;
- keeps `published_only = true`;
- retains the same Search result DTOs and selected-transport no-fallback behavior.

The GraphQL field also reuses the Search rate limiter and rejects a client tenant override. Native transport derives tenant, auth, locale, and request context from server extractors.

## Compatibility and degraded mode

No migration, backfill, Search document shape, Forum projection shape, dependency, or `Cargo.lock` change is introduced. The ordinary Search fields and endpoints are not modified. Product category filtering remains exact and cannot receive Forum descendants.

The new visibility-safe Forum-only field requires the neutral owner port. It does not silently fall back to exact-category Search when the owner is absent, because that would bypass richer category audience policy. Search-disabled behavior outside this explicit path is unchanged.

## Remaining work

- apply topic-local audience narrowing and exact reply authorization to Search result eligibility;
- derive trusted channel authority consistently for every storefront Search predicate;
- add author, tag, locale, date, solved, kind, channel/group, and attachment-presence filters;
- complete owner-issued projection revision ordering, reconciliation, and deletion/ACL cleanup;
- capture maintainer-executed PostgreSQL query/result evidence and `LINK-FORUM-03` runtime proof.

## Maintainer verification

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-search storefront_category_scope -- --nocapture
cargo test -p rustok-search-storefront transport::tests::only_explicit_forum_category_scope_selects_owner_path -- --nocapture
cargo test -p rustok-server --features mod-forum forum_search_category_scope -- --nocapture
node scripts/verify/verify-forum-search-storefront-scope.mjs
cargo check -p rustok-search --features graphql --all-targets
cargo check -p rustok-search-storefront --features ssr --all-targets
cargo check -p rustok-server --features mod-forum --all-targets
cargo xtask module validate forum
cargo xtask module validate search
```
