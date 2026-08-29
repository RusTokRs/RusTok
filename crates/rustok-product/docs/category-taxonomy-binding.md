# Product Category → Taxonomy migration contract

Status: **source-complete create dual-write; Product read cutover and donor retirement pending**

TAXONOMY-CAT-23 introduced the tenant-safe Product-owned binding seam.
TAXONOMY-CAT-24 added the PostgreSQL-only monotonic backfill of existing Product
Categories. TAXONOMY-CAT-25 closes the post-backfill creation gap: every new
Product Category create now mirrors canonical identity, localized copy, route
ownership and hierarchy into Taxonomy inside the same Product transaction before
the Product domain event and commit.

## CAT-23 compatibility history

CAT-23 closed with this bounded state:
Status: **source-complete additive seam; backfill and runtime cutover pending**.
That slice does **not** backfill the binding and does **not** switch Product reads or writes.
Its recorded next-step intent was preserving Product category UUIDs where possible.
The physical binding table is currently created only on PostgreSQL because its
tenant-safe Product prerequisite is PostgreSQL-only. No `product/category`
Translation provider should be introduced by CAT-23 or later Category migration
slices; the registered Taxonomy `taxonomy/term` provider remains canonical.

## CAT-24 compatibility history

CAT-24 closed with this bounded state:
Status: **source-complete monotonic backfill; Product runtime cutover pending**.
CAT-24 does **not** switch Product reads or writes and does **not** remove or rewrite
the live donor tables. Its backfill preserves existing Product category and translation
UUIDs where compatible, copies hierarchy after identity/localized routes, and populates
the same-ID binding last. The historical backfill remains monotonic; CAT-25 does not
re-run or reinterpret it.

For retained CAT-24 source evidence, the exact migration rules remain: the same
canonical base slug is therefore projected into every imported locale; no localized
slug is invented; the migration does not synthesize a `path`-derived slug; if two
Product categories conflict under Taxonomy route ownership, the migration blocks as
an incompatible route collision. Product `meta_title` / `meta_description`,
activation/soft-delete lifecycle and other Product policy stay donor-owned. An
incompatible Taxonomy UUID, canonical-key, localized-copy, translation UUID, route,
hierarchy or binding ownership blocks the migration instead of choosing a winner.
No `product/category` Translation provider is introduced; canonical Category copy is
served by the registered Taxonomy `taxonomy/term` provider.

## Binding and backend boundary

`product_catalog_category_taxonomy_bindings` remains Product-owned relation
storage with tenant-safe composite foreign keys. The physical binding, backfill
and Product Category canonical create path are currently PostgreSQL-only because
the retained Product `(tenant_id, id)` category identity prerequisite is installed
by the PostgreSQL-only tenant-consistency migration. Other backends remain a no-op
for this migration seam until they have an equivalent tenant-safe prerequisite.

## CAT-25 transactional create contract

A successful Product Category create now establishes both sides atomically:

- Product chooses the stable Category UUID and writes its donor category, closure
  and localized rows inside `ProductWriteTransaction`;
- every canonical normalized Product locale is synchronized through Taxonomy's
  transaction-bound `sync_module_category_in_tx` owner port with module scope
  `product`, canonical key `product-category-{uuid}`, the same Product base slug,
  localized `name`/`description`, `parent_id` and `position`;
- Product stores one base category slug, so that exact canonical route key is sent
  for every locale; no localized slug or `path`-derived replacement is invented;
- after every locale succeeds, Product creates the same-ID
  `product_catalog_category_taxonomy_bindings` row;
- only then may `CatalogCategoryCreated`, the Product operation receipt and the
  shared transaction commit succeed;
- any Taxonomy tenant/scope/identity/route/hierarchy conflict or binding failure
  rolls back the Product donor inserts as part of that same transaction.

CAT-25 also aligns new Product Category canonical text with the existing Taxonomy
owner-sync boundary before persistence: localized names are trimmed and limited to
120 characters; descriptions are trimmed, empty text becomes `None`, and canonical
description length is limited to 2000 characters. Product-only `meta_title` and
`meta_description` remain unchanged and are not sent to Taxonomy.

## Route compatibility

Product donor storage only requires `slug` uniqueness per parent, while the
Taxonomy route registry requires one owner for a route key across the whole module
scope and locale. CAT-24 and CAT-25 do not synthesize a `path`-derived slug or
silently rename either category. If a Product category route is not already the
canonical Taxonomy route form, exceeds the Taxonomy route-key bound, or collides
with another owner, creation/backfill fails closed and requires explicit donor or
input remediation.

## Current runtime boundary

CAT-25 does **not** switch Product reads. `ProductCatalogSchemaService::list_categories`
still materializes `code`, `kind`, `path`, hierarchy and localized display name from
the Product donor. This is deliberate: the dual-write prerequisite must exist before
a later read projection can rely on Taxonomy for every post-backfill Category.

Product continues to own and serve:

- `catalog_categories`, `catalog_category_translations` and Product closure until
  later verified read/write retirement slices;
- Product `kind`, virtual-category `rule_config`, activation/soft-delete lifecycle
  and Product-specific metadata;
- `meta_title` / `meta_description`, which have no Taxonomy Category field and
  remain truthful Product SEO data;
- category-bound attribute/schema state;
- product/category membership, primary/navigation/collection/virtual assignment
  semantics and Product projections.

Those retained fields must not be silently discarded or reinterpreted. A later
read cutover must consume the existing `TaxonomyOwnerCategoryReader` projection
while continuing to compose the Product-specific fields above.

## Translation ownership boundary

No `product/category` Translation provider is introduced. Canonical Category
localized copy is synchronized under the registered Taxonomy `taxonomy/term`
provider. Product's translation rows remain compatibility/runtime donor storage
until a later verified read/Translation retirement slice; CAT-25 alone is not
permission to delete them or change storefront/admin read projections.
