# Blog Category hierarchy contract

Taxonomy owns canonical Blog Category hierarchy placement. Blog owns the
`blog_categories` membership/settings/revision row and the Blog command policy
that is allowed to mutate Taxonomy's module-scoped hierarchy for Blog categories.

There is no second Blog hierarchy table and no materialized Blog-owned
`parent_id`, `position`, or `depth` persistence.

## Structural commands

Localized Category updates own localized copy through Taxonomy plus Blog
membership settings. `UpdateCategoryInput.position` is retained only for
compatibility decoding and is rejected by `CategoryService::update`.

Moving or reordering an existing category is the explicit command:

`POST /api/blog/categories/{id}/move`

with `MoveCategoryInput { parent_id, position }`.

`parent_id = null` means root. `position` is a zero-based insertion index in
the destination sibling list. The command requires `blog_categories:manage`.

Creation uses the same tenant-scoped tree lock, validates the Blog membership
parent, shifts canonical Taxonomy sibling positions and writes the same-ID
Taxonomy Category/hierarchy in the owner transaction. A tenant is bounded to 512
Blog categories for this structural command path.

Delete is leaf-only. Taxonomy owns the outer canonical delete transaction; the
Blog cleanup removes Blog membership, removes the Blog-scoped hierarchy
placement, compacts the remaining canonical Taxonomy sibling positions, emits
the Blog reindex signal, and then delegates host-composed capability cleanup.
Any cleanup failure rolls the whole transaction back.

## Invariants

Structural create, move, and delete:

- serialize PostgreSQL hierarchy writes with the same
  `blog-category-tree:{tenant_id}` transaction advisory lock;
- reject missing/cross-tenant parents, self-parenting, descendant cycles,
  invalid existing trees and out-of-range sibling positions;
- keep sibling positions dense and deterministic with category id as a stable
  tie-breaker;
- keep the tree bounded to 512 Blog memberships;
- write canonical placement only in
  `taxonomy_category_hierarchy`;
- recompute response `depth` from the canonical Taxonomy parent map; depth is
  not duplicated in Blog persistence;
- keep localized copy and structural placement separate.

The move command is projection-neutral because Blog Search does not index
parent/position/depth. Create/update copy changes and delete retain their
appropriate Blog reindex behavior.

## Ownership consequences

`blog_categories` contains Blog membership/settings/revision only. Direct
hierarchy fields must not be reintroduced there.

Taxonomy owns canonical localized Category identity, route history and hierarchy
placement. Blog remains the command/orchestration owner for Blog-specific
membership semantics and RBAC, but it does not become a second storage owner.

Structural moves do not rewrite localized category rows or invent a locale.

## Verification

- `node scripts/verify/verify-blog-category-hierarchy-command.mjs`
- `cargo test --locked -p rustok-blog --test category_hierarchy -- --nocapture`
- `cargo test --locked -p rustok-blog --test category_taxonomy_delete_lifecycle -- --nocapture`
- `.github/workflows/blog-category-hierarchy-contract.yml`
