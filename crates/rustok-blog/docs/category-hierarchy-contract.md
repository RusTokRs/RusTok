# Blog category hierarchy contract

Blog owns its category hierarchy. `rustok-taxonomy` remains the shared flat vocabulary for tags and does not own Blog parent/child edges, ordering, cycle policy, or materialized depth.

## Structural commands

Localized category updates own `name`, `slug`, `description`, and settings. `UpdateCategoryInput.position` is retained only for compatibility decoding and is rejected by `CategoryService::update`; moving or reordering an existing category is a distinct structural operation:

`POST /api/blog/categories/{id}/move`

with `MoveCategoryInput { parent_id, position }`.

`parent_id = null` means move to the root level. `position` is the zero-based position inside the destination sibling list. The command requires `blog_categories:manage`.

Category creation also treats `CreateCategoryInput.position` as a zero-based insertion index, not an arbitrary persisted scalar. Under the same tenant tree lock it validates the parent, rejects an index beyond the destination sibling count, canonicalizes existing sibling positions, and inserts the new category at the requested position. Creation refuses a 513th tenant category so every admitted tree remains operable by the bounded runtime hierarchy command.

Category deletion is leaf-only. The retained production hierarchy foreign key already uses `ON DELETE RESTRICT`; the owner service enforces the same rule explicitly on every backend before deleting a row, so SQLite cannot leave dangling children when its existing table cannot be retrofitted with that foreign key. After a leaf deletion, the service compacts remaining sibling positions before commit. A parent becomes deletable only after its children have been moved or deleted.

## Invariants

Structural create, move, and delete operations execute inside one database transaction and:

- keep the admitted tenant-local tree bounded to a maximum of 512 nodes;
- serialize PostgreSQL create/move/delete hierarchy writes with the same tenant-scoped transaction advisory lock; the entity insert hook also takes that lock before deriving child depth, while SQLite relies on its transaction writer serialization;
- reject a missing/cross-tenant parent, self-parenting, descendant-parent cycles, an already-invalid hierarchy, out-of-range insertion/destination positions, and deletion of a non-leaf category;
- canonicalize affected sibling positions after create, move, and leaf delete;
- recompute materialized `depth` from the complete post-move parent map and persist every descendant whose depth changes;
- publish the existing Blog-wide `ReindexRequested` event before a move or delete commits so search cannot observe a committed structural mutation without the corresponding reindex request.

The 512-node bound is an execution-safety limit, not a newly invented category-depth policy. The retained hierarchy migration remains the storage/bootstrap authority for tenant-parent foreign-key integrity and legacy cycle/depth validation. Runtime create/move/delete semantics are owner-service policy rather than Taxonomy policy.

`CategoryService::update` no longer writes `position`, even when the compatibility field is absent. That prevents a stale localized update from overwriting a concurrent structural move and leaves hierarchy placement with one owner-side write path for existing categories.

## Translation boundary

A hierarchy move or leaf delete does not move category hierarchy into Taxonomy. Structural moves do not rewrite localized category rows, choose a locale, or create a Taxonomy term. Translation CAS/revision evidence therefore remains owned by localized category mutations; the move command updates only `parent_id`, `position`, `depth`, and `updated_at`, while deletion retains its existing category translation lifecycle evidence before the owner row is removed.

The existing translation-target regression performs its source-copy update without a structural `position` mutation, proving translation revision advancement remains independent from hierarchy placement.

## Verification

- `node scripts/verify/verify-blog-category-hierarchy-command.mjs`
- `cargo test --locked -p rustok-blog --lib category_command::tests -- --nocapture`
- `cargo test --locked -p rustok-blog --test category_hierarchy -- --nocapture`
- `cargo test --locked -p rustok-blog --lib category_update_advances_exact_locale_and_owner_change_revisions -- --nocapture`
- `.github/workflows/blog-category-hierarchy-contract.yml`
