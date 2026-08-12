# Blog implementation slice 106 — category hierarchy integrity

## Decision

Blog category hierarchy is Blog-owned domain state. Taxonomy continues to own
shared vocabulary identities and localized route-key semantics; it does not own
Blog `parent_id`, ordering, depth, cycle policy, or category metadata.

## Invariants

- A Blog category root has `depth = 0`.
- A child stores `depth = parent.depth + 1`.
- Parent and child must belong to the same tenant.
- A parent must already exist when a child is inserted.
- Retained data must form an acyclic forest: orphan, cross-tenant, and cyclic
  edges block migration rather than being silently repaired.
- Existing stored `depth` is derived state and is recomputed from parent edges
  during migration.
- Production databases that support adding the retained-table constraint enforce
  the parent edge with `(tenant_id, parent_id) -> (tenant_id, id)` and
  `ON DELETE RESTRICT`, so a parent cannot be deleted while children reference it.
- SQLite cannot add a foreign key to an existing table without a table rebuild.
  The entity insert hook and migration preflight still enforce parent/depth
  correctness there; late storage-level delete restriction remains a backend
  capability gap rather than being hidden behind a trigger trick.

## Implementation

`blog_category::ActiveModelBehavior::before_save` owns insert-time depth
materialization. It ignores caller-supplied depth, loads the parent inside the
same connection/transaction and rejects missing/foreign parents, negative
legacy parent depth, or `i32` depth exhaustion.

Migration `m20260812_000017_enforce_blog_category_hierarchy` loads the retained
category graph, computes depth independently from stored values, rejects invalid
edges/cycles, backfills only changed depths, creates the composite tenant/id
identity index, and then installs the composite self foreign key on databases
that support adding the constraint to an existing table.

## Verification

- Unit coverage for child-depth derivation and overflow/negative-depth rejection.
- Migration graph coverage for valid multi-level depth, orphan rejection,
  cross-tenant parent rejection, and cycle rejection.
- Existing Blog category CRUD remains the public write surface; no generic
  Taxonomy hierarchy or shared content category owner is introduced.

## Follow-up

`rustok-content::CategoryService` is a legacy generic category CRUD surface that
conflicts with the current `rustok-content/CRATE_API.md` ownership statement.
Audit all external consumers and retire/privatize that surface in a separate
slice rather than strengthening it into a second category owner.
