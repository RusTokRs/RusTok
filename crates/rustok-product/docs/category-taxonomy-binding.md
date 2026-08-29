# Product Category → Taxonomy migration contract

Status: **source-complete Product Category SEO seam; canonical donor storage retirement pending**

TAXONOMY-CAT-23 introduced the tenant-safe Product-owned binding seam.
TAXONOMY-CAT-24 added the PostgreSQL-only monotonic backfill of existing Product
Categories. TAXONOMY-CAT-25 closed the post-backfill creation gap with atomic
Product → Taxonomy create synchronization. TAXONOMY-CAT-26 moved the bounded
PostgreSQL Category list projection to Taxonomy-owned canonical localized copy,
route slug and parent hierarchy while retaining Product-owned commerce fields.
TAXONOMY-CAT-27 now splits localized Product-only Category SEO into dedicated
Product storage so later canonical translation-donor retirement does not discard
`meta_title` / `meta_description`.

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

## CAT-26 compatibility history

CAT-26 closed with this bounded state:
Status: **source-complete PostgreSQL Taxonomy read projection; donor storage retirement pending**.
On PostgreSQL, `ProductCatalogSchemaService::list_categories` requires a same-ID typed
binding and consumes canonical localized `name`, canonical localized `slug` and
`parent_id` from `TaxonomyOwnerCategoryReader`. Product retains `code`, `kind`, `path`
and path ordering. Missing binding/owner/canonical localized state fails closed; the
PostgreSQL list path no longer reads `catalog_category_translations` as a hidden
canonical fallback. Other backends continue using the retained Product donor list path.

## Binding and backend boundary

`product_catalog_category_taxonomy_bindings` remains Product-owned relation storage
with tenant-safe composite foreign keys. The physical binding and deterministic
backfill are PostgreSQL-only because the retained Product `(tenant_id, id)` category
identity prerequisite is installed by the PostgreSQL-only tenant-consistency
migration. CAT-26 therefore switches Category list reads only when the Product
connection backend is PostgreSQL. Other backends continue using the retained Product
donor list path until they acquire an equivalent tenant-safe binding prerequisite.

## CAT-27 localized SEO seam

`catalog_category_translations` historically mixes two ownership classes:
canonical Category localized copy (`name` / `description`) and Product-only localized
SEO (`meta_title` / `meta_description`). Taxonomy owns the first class after CAT-26,
but Taxonomy intentionally has no Product SEO fields. CAT-27 therefore introduces
PostgreSQL-only `catalog_category_seo_translations` as dedicated Product-owned SEO
storage keyed by `(tenant_id, category_id, locale)`.

The CAT-27 migration is additive and deterministic:

- the table has a tenant-safe composite foreign key to
  `catalog_categories(tenant_id, id)` and cascades with Product Category lifecycle;
- a row must contain at least one non-null SEO value; empty SEO-only rows are not
  materialized;
- existing `meta_title` / `meta_description` values are copied from
  `catalog_category_translations` with the exact normalized locale already persisted
  by the Product locale migration;
- if the new SEO table already contains a row for the same tenant/category/locale with
  different SEO values, migration fails closed with an incompatible ownership error;
- compatible existing rows are retained and the backfill is monotonic via
  `ON CONFLICT (tenant_id, category_id, locale) DO NOTHING`;
- no canonical `name`, `description`, slug, route, hierarchy or Taxonomy copy is stored
  in the SEO table.

New PostgreSQL Product Category creates now write any localized Product SEO into
`catalog_category_seo_translations` inside the existing `ProductWriteTransaction`.
The compatibility `catalog_category_translations` write remains in CAT-27, then the SEO
write occurs, then the existing Taxonomy owner-sync/binding/event/commit sequence
continues. Any SEO insert or later Taxonomy failure rolls the whole Product create back.
Non-PostgreSQL creates do not write the new table because the seam is not installed on
those backends.

CAT-27 does **not** drop `catalog_category_translations`, does not stop its compatibility
writes, and does not claim canonical donor retirement. A later retirement slice must
first prove there is no remaining PostgreSQL runtime dependency on canonical Product
translation rows and must preserve the non-PostgreSQL donor boundary explicitly.

## Product-owned state retained after CAT-27

Product continues to own:

- Category `code`, `kind`, virtual-category `rule_config`, activation/soft-delete
  lifecycle and Product-specific metadata;
- Product `path` and closure storage used by current Product hierarchy/form and
  merchandising logic. `path` is a retained Product projection, not a replacement
  Taxonomy route authority;
- localized `meta_title` / `meta_description`, now additionally isolated in
  `catalog_category_seo_translations` on PostgreSQL;
- category-bound attribute/schema state;
- product/category membership, primary/navigation/collection/virtual assignment
  semantics and Product projections;
- compatibility `catalog_category_translations` storage until a later verified
  retirement slice removes only the ownership already superseded by Taxonomy.

## Locale and route behavior

Taxonomy owner reads use the same `rustok-content` deterministic locale policy already
used by the former Product list projection: requested locale, explicit/platform
fallback, then lexicographically smallest normalized available locale. CAT-26 therefore
changed canonical owner storage without changing fallback order.

Taxonomy route ownership remains module-scope-wide per locale. Product donor storage
still only guarantees base-slug uniqueness per parent. CAT-24/CAT-25 fail-closed route
checks remain required; CAT-26 consumes the Taxonomy canonical localized slug instead
of rebuilding or flattening Product `path` into a route key.

CAT-27 reuses the already-normalized Product Category locale for SEO identity. It does
not introduce a second fallback policy or a Product Category Translation provider.

## Translation ownership boundary

No `product/category` Translation provider is introduced. Canonical Category localized
copy is synchronized and read under the registered Taxonomy `taxonomy/term` provider on
PostgreSQL. `catalog_category_seo_translations` is Product SEO storage, not a Translation
provider and not canonical Category copy. The legacy Product translation rows remain
compatibility storage until a later verified retirement slice.