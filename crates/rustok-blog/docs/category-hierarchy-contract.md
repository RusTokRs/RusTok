# Blog category hierarchy contract

Blog owns its category hierarchy. `rustok-taxonomy` remains the shared flat vocabulary for tags and does not own Blog parent/child edges, ordering, cycle policy, or materialized depth.

## Structural command

Existing localized category updates continue to own `name`, `slug`, `description`, `position` compatibility, and settings. Moving an existing category is a distinct structural operation:

`POST /api/blog/categories/{id}/move`

with `MoveCategoryInput { parent_id, position }`.

`parent_id = null` means move to the root level. `position` is the zero-based position inside the destination sibling list. The command requires `blog_categories:manage`.

## Invariants

The owner command executes in one database transaction and:

- loads a bounded tenant-local tree (maximum 512 nodes, maximum depth 16);
- serializes PostgreSQL tree moves with a tenant-scoped transaction advisory lock; SQLite relies on its transaction writer serialization;
- rejects a missing/cross-tenant parent, self-parenting, descendant-parent cycles, excessive depth, and out-of-range destination positions;
- canonicalizes source and destination sibling positions;
- recomputes materialized `depth` from the complete post-move parent map and persists every descendant whose depth changes;
- publishes the existing Blog-wide `ReindexRequested` event before commit so search cannot observe a committed hierarchy move without the corresponding reindex request.

The retained hierarchy migration remains the storage/bootstrap authority for tenant-parent foreign-key integrity and legacy cycle/depth validation. Runtime reparent semantics are owner-service policy rather than Taxonomy policy.

## Translation boundary

A hierarchy move changes structural placement only. It does not rewrite localized category rows, choose a locale, or create a Taxonomy term. Translation CAS/revision evidence therefore remains owned by localized category mutations; the move command updates only `parent_id`, `position`, `depth`, and `updated_at`.

## Verification

- `node scripts/verify/verify-blog-category-hierarchy-command.mjs`
- `cargo test --locked -p rustok-blog --lib category_command::tests -- --nocapture`
- `cargo test --locked -p rustok-blog --test category_hierarchy -- --nocapture`
- `.github/workflows/blog-category-hierarchy-contract.yml`
