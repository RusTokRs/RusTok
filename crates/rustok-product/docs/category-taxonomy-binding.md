# Product Category → Taxonomy migration contract

Status: **source-complete monotonic backfill; Product runtime cutover pending**

TAXONOMY-CAT-23 introduced the tenant-safe Product-owned binding seam.
TAXONOMY-CAT-24 adds the next bounded step: a PostgreSQL-only monotonic copy of
Product Category canonical identity, localized copy, routes and hierarchy into
Taxonomy while Product remains the live runtime donor.

## Binding and backend boundary

`product_catalog_category_taxonomy_bindings` remains Product-owned relation
storage with tenant-safe composite foreign keys. The physical binding and
backfill are currently PostgreSQL-only because the retained Product
`(tenant_id, id)` category identity prerequisite is installed by the
PostgreSQL-only tenant-consistency migration. Other backends remain a no-op
until they have an equivalent tenant-safe prerequisite.

## Backfill contract

The backfill is deterministic and fail-closed:

- Product Category UUID is preserved as the Taxonomy Category UUID;
- Taxonomy uses `Category`, module scope `product`, and canonical key
  `product-category-{uuid}`;
- every Product category must have at least one canonical localized row;
- Product translation UUIDs are preserved for Taxonomy localized rows;
- Product stores one base category `slug`, not a localized slug. The same
  canonical base slug is therefore projected into every imported locale and
  reserved as that locale's Taxonomy route key; no localized slug is invented;
- Product donor storage only requires `slug` uniqueness per parent, while the
  Taxonomy route registry requires one owner for a route key across the whole
  module scope and locale. CAT-24 does not synthesize a `path`-derived slug or
  silently rename either category: if two Product categories project to the
  same Taxonomy route key, the migration blocks as an incompatible route
  collision and requires explicit donor remediation before cutover;
- localized `name` and `description` are copied exactly after canonical locale
  validation;
- every Category identity/localized route is created before hierarchy rows;
- Taxonomy hierarchy receives Product `parent_id` and `position` after all
  identities exist;
- the Product↔Taxonomy same-ID binding is populated only after identity,
  localized copy, routes and hierarchy succeed;
- incompatible Taxonomy UUID, canonical-key, localized-copy, translation UUID,
  route, hierarchy or binding ownership blocks the migration instead of
  choosing a winner;
- the copy runs inside one transaction and rollback is intentionally monotonic:
  copied Taxonomy truth is not deleted by `down()`.

## Retained Product-owned state

CAT-24 does **not** switch Product reads or writes and does **not** remove or
rewrite the live donor tables. Product continues to own and serve:

- `catalog_categories` and `catalog_category_translations` until runtime cutover;
- Product category closure and current mutation logic;
- `kind`, virtual-category `rule_config`, activation/soft-delete lifecycle and
  Product-specific metadata;
- `meta_title` / `meta_description`, which have no Taxonomy Category field in
  this slice and must remain truthful Product SEO data;
- category-bound attribute/schema state;
- product/category membership, primary/navigation/collection/virtual assignment
  semantics and Product projections.

Those retained fields must not be silently discarded or reinterpreted by the
backfill. Later cutover slices must make their ownership/lifecycle decisions
explicitly before retiring any donor storage.

## Translation ownership boundary

No `product/category` Translation provider is introduced. Canonical Category
localized copy is being prepared under the registered Taxonomy `taxonomy/term`
provider. Product remains the runtime source until a later verified read/write
cutover, so CAT-24 alone is not permission to delete Product translations or
change storefront/admin projections.
