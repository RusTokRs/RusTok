# Product Category → Taxonomy migration contract

Status: **source-complete PostgreSQL Taxonomy read projection; donor storage retirement pending**

TAXONOMY-CAT-23 introduced the tenant-safe Product-owned binding seam.
TAXONOMY-CAT-24 added the PostgreSQL-only monotonic backfill of existing Product
Categories. TAXONOMY-CAT-25 closed the post-backfill creation gap with atomic
Product → Taxonomy create synchronization. TAXONOMY-CAT-26 now moves the bounded
PostgreSQL Category list projection to Taxonomy-owned canonical localized copy,
route slug and parent hierarchy while retaining Product-owned commerce fields.

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
the same-ID binding last. The historical backfill remains monotonic; later slices do not
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

## CAT-25 compatibility history

CAT-25 closed with this bounded state:
Status: **source-complete create dual-write; Product read cutover and donor retirement pending**.
At that point CAT-25 does **not** switch Product reads: its purpose was to guarantee
that every new Product Category create now mirrors canonical identity, localized copy,
route ownership and hierarchy into Taxonomy inside `ProductWriteTransaction`. Every
locale is synchronized first, the same-ID binding is inserted after all locale syncs,
and only then may `CatalogCategoryCreated` and the shared transaction commit succeed.
Any Taxonomy owner conflict rolls back the Product donor inserts as part of that same
transaction. CAT-25 deliberately identified `TaxonomyOwnerCategoryReader` as the next
read projection rather than bypassing Taxonomy persistence ownership.

## Binding and backend boundary

`product_catalog_category_taxonomy_bindings` remains Product-owned relation storage
with tenant-safe composite foreign keys. The physical binding and deterministic
backfill are PostgreSQL-only because the retained Product `(tenant_id, id)` category
identity prerequisite is installed by the PostgreSQL-only tenant-consistency
migration. CAT-26 therefore switches Category list reads only when the Product
connection backend is PostgreSQL. Other backends continue using the retained Product
donor list path until they acquire an equivalent tenant-safe binding prerequisite.

## CAT-26 PostgreSQL read projection

`ProductCatalogSchemaService::list_categories` now has an explicit backend boundary.
On PostgreSQL it:

- reads the live Product Category set only for Product-owned composition fields:
  Category UUID, `code`, `kind`, `path` and the typed Taxonomy binding;
- requires every live Product Category to have a same-ID binding; a missing or
  incompatible binding fails closed rather than falling back to donor translations;
- calls `TaxonomyOwnerCategoryReader` with kind `Category`, module scope `product`,
  the exact bound Category IDs, requested locale and platform fallback locale;
- requires every requested ID to resolve to the expected canonical key
  `product-category-{uuid}` and to have canonical localized copy;
- materializes localized `name`, localized canonical `slug` and `parent_id` from the
  Taxonomy owner projection;
- retains Product `code`, `kind` and `path` in the public Product DTO and preserves
  Product path ordering for this slice.

The PostgreSQL list path no longer reads `catalog_category_translations` for display
copy and no longer treats Product `parent_id` or Product base `slug` as canonical read
authority. Missing Taxonomy state is migration drift and therefore an error, not a
reason to silently resurrect the donor as canonical truth.

## Product-owned state retained after CAT-26

CAT-26 is a read-projection slice, not donor retirement. Product continues to own:

- Category `code`, `kind`, virtual-category `rule_config`, activation/soft-delete
  lifecycle and Product-specific metadata;
- Product `path` and closure storage used by current Product hierarchy/form and
  merchandising logic. `path` is a retained Product projection, not a replacement
  Taxonomy route authority, so it may intentionally differ from a later canonical
  Taxonomy slug change until a dedicated path/navigation slice updates that contract;
- `meta_title` / `meta_description`, which have no Taxonomy Category field and remain
  truthful Product SEO data;
- category-bound attribute/schema state;
- product/category membership, primary/navigation/collection/virtual assignment
  semantics and Product projections;
- compatibility `catalog_category_translations` writes needed by retained Product SEO
  and future retirement work, even though PostgreSQL Category list display no longer
  reads their canonical name copy.

No Category create/write behavior changes in CAT-26; the CAT-25 transactional owner
sync remains the prerequisite keeping post-backfill Categories present in Taxonomy.

## Locale and route behavior

Taxonomy owner reads use the same `rustok-content` deterministic locale policy already
used by the former Product list projection: requested locale, explicit/platform
fallback, then lexicographically smallest normalized available locale. CAT-26 therefore
changes canonical owner storage without changing fallback order.

Taxonomy route ownership remains module-scope-wide per locale. Product donor storage
still only guarantees base-slug uniqueness per parent. CAT-24/CAT-25 fail-closed route
checks remain required; CAT-26 consumes the Taxonomy canonical localized slug instead
of rebuilding or flattening Product `path` into a route key.

## Translation ownership boundary

No `product/category` Translation provider is introduced. Canonical Category localized
copy is synchronized and now read under the registered Taxonomy `taxonomy/term`
provider on PostgreSQL. Product translation rows remain compatibility/SEO donor storage
until later verified retirement slices; CAT-26 is not permission to delete SEO fields,
closure state or Product merchandising policy.
