# Blog category GraphQL contract

Blog owns category hierarchy and localized category copy. `rustok-taxonomy` remains the shared flat vocabulary behind Blog tags and does not own Blog parent/child edges, sibling ordering, materialized depth, or category GraphQL operations.

## Owner read contract

`CategoryTreeService` is the sole GraphQL tree read authority. GraphQL resolvers do not query `blog_categories` or `blog_category_translations` directly.

The tree reader:

- requires `blog_categories:list` through the existing owner RBAC service;
- loads at most 512 tenant-local categories plus one sentinel row and fails closed if the bound is exceeded;
- resolves localized copy with the same shared Blog locale machinery used by category get/list;
- preserves deterministic sibling order (`position`, then category ID);
- rejects missing/foreign parents, cycles, disconnected hierarchies, negative stored depth, and any mismatch between materialized `depth` and the depth computed from the parent graph;
- returns `roots`, `total_nodes`, `max_depth`, and recursive nodes containing `parent_id`, `position`, `depth`, localized copy, available locales, and settings.

The 512-node bound is an execution-safety limit inherited from the Blog owner hierarchy contract. This GraphQL slice does not add a new depth policy.

## GraphQL queries

The merged Blog GraphQL root exposes:

- `blogCategory(id, locale)` for one localized category through `CategoryService::get`, requiring authenticated `blog_categories:read`;
- `blogCategoryTree(locale)` for the bounded owner tree through `CategoryTreeService::read`, requiring authenticated `blog_categories:list`.

The owner service can still represent a public-read security context for future storefront adapters, but this GraphQL category surface intentionally follows the existing authenticated Blog category adapter boundary. Queries use the current `TenantContext`; they do not expose a category `tenantId` override.

## GraphQL mutations

The merged Blog mutation root exposes:

- `createBlogCategory(input)` requiring `blog_categories:create` and returning the created category UUID;
- `updateBlogCategory(id, input)` requiring `blog_categories:update`;
- `moveBlogCategory(id, input)` requiring `blog_categories:manage`;
- `deleteBlogCategory(id)` requiring `blog_categories:delete`.

Create deliberately returns the domain-created UUID instead of performing a second `CategoryService::get` after commit. A principal that is allowed to create but not read therefore cannot receive a misleading mutation failure after the category has already been committed.

All category mutations require the authenticated actor tenant to equal the current request tenant. There is no category mutation `tenantId` override.

Localized copy and structural placement remain distinct contracts:

- `UpdateBlogCategoryInput` contains locale/name/slug/description/settings only;
- `MoveBlogCategoryInput` contains `parentId` and `position` only;
- converting `UpdateBlogCategoryInput` to the domain update always sets the compatibility `position` field to `None`;
- move-to-root remains unambiguous because `parentId = null` is represented only by the structural move input.

The GraphQL layer delegates all writes to `CategoryService` or `CategoryCommandService`; it does not write owner persistence directly.

## Consumer sequencing

This contract intentionally stops at the owner service and GraphQL boundary. Blog admin category management is a separate consumer slice and should consume these GraphQL operations rather than reimplement hierarchy persistence or bypassing the owner services.

## Verification

- `node scripts/verify/verify-blog-category-graphql-contract.mjs`
- `cargo test --locked -p rustok-blog --lib category_schema_keeps_localized_and_structural_commands_separate -- --nocapture`
- `cargo test --locked -p rustok-blog --lib category_types::tests -- --nocapture`
- `cargo test --locked -p rustok-blog --lib category_tree::tests -- --nocapture`
- `cargo test --locked -p rustok-blog --test category_tree -- --nocapture`
- `.github/workflows/blog-category-hierarchy-contract.yml`
