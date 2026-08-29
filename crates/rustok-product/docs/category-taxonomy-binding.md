# Product Category → Taxonomy migration contract

Status: **source-complete PostgreSQL legacy Category translation storage retirement; non-PostgreSQL donor compatibility retained**
Current migration cursor: **TAXONOMY-CAT-32 PostgreSQL closure write retirement; Product navigation/path and non-PostgreSQL closure compatibility retained**.

TAXONOMY-CAT-23 introduced the tenant-safe Product-owned binding seam.
TAXONOMY-CAT-24 added the PostgreSQL-only monotonic backfill of existing Product
Categories. TAXONOMY-CAT-25 closed the post-backfill creation gap with atomic
Product → Taxonomy create synchronization. TAXONOMY-CAT-26 moved the bounded
PostgreSQL Category list projection to Taxonomy-owned canonical localized copy,
route slug and parent hierarchy while retaining Product-owned commerce fields.
TAXONOMY-CAT-27 split localized Product-only Category SEO into dedicated Product
storage. TAXONOMY-CAT-28 stopped new PostgreSQL creates from materializing the
legacy canonical Product translation mirror. TAXONOMY-CAT-29 retired that legacy
table physically on PostgreSQL after fail-closed Taxonomy/SEO ownership preflight.
TAXONOMY-CAT-30 makes Product effective-form/schema inheritance and inherited
attribute-group label ancestry consume Taxonomy-owned hierarchy on PostgreSQL while
leaving Product schema/attribute policy, path/navigation projection and non-PostgreSQL
hierarchy compatibility in Product. TAXONOMY-CAT-31 makes the Product catalog
schema-directory ordering consume Taxonomy parent/position hierarchy order on
PostgreSQL without turning Product `path` into Taxonomy-owned navigation state.
TAXONOMY-CAT-32 now stops new PostgreSQL Product Category creates from materializing
the Product closure mirror after all PostgreSQL hierarchy consumers moved to Taxonomy.

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
CAT-26 requires every live Product Category to have a same-ID binding. It materializes
localized `name`, localized canonical `slug` and `parent_id` from the Taxonomy owner projection
and retains Product `code`, `kind` and `path` while preserving Product path ordering.
The PostgreSQL list path no longer reads `catalog_category_translations`; missing
binding/owner/canonical localized state fails closed instead of reviving donor truth.
Other backends continue using the retained Product donor list path.

The path-order statement above is retained as the exact CAT-26 historical boundary.
CAT-31 supersedes it only for the PostgreSQL catalog schema-directory result order; the
returned Product `path` value remains unchanged and Product-owned.

## Binding and backend boundary

`product_catalog_category_taxonomy_bindings` remains Product-owned relation storage
with tenant-safe composite foreign keys. The physical binding and deterministic
backfill are PostgreSQL-only because the retained Product `(tenant_id, id)` category
identity prerequisite is installed by the PostgreSQL-only tenant-consistency
migration. CAT-26 therefore switches Category list reads only when the Product
connection backend is PostgreSQL. Other backends continue using the retained Product
donor list path until they acquire an equivalent tenant-safe binding prerequisite.

## CAT-27 localized SEO seam

CAT-27 closed with this bounded state:
Status: **source-complete Product Category SEO seam; canonical donor storage retirement pending**.

`catalog_category_translations` historically mixes two ownership classes:
canonical Category localized copy (`name` / `description`) and Product-only localized
SEO (`meta_title` / `meta_description`). Taxonomy owns the first class after CAT-26,
but Taxonomy intentionally has no Product SEO fields. CAT-27 therefore introduces
PostgreSQL-only `catalog_category_seo_translations` as dedicated Product-owned SEO storage
keyed by `(tenant_id, category_id, locale)`.

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

At CAT-27 completion new PostgreSQL Product Category creates still wrote the compatibility
`catalog_category_translations` row, then wrote localized SEO, then performed the existing
Taxonomy owner-sync/binding/event/commit sequence. Any SEO insert or later Taxonomy failure rolls the whole Product create back.
CAT-27 does **not** drop `catalog_category_translations` and does not itself stop compatibility
writes; it makes that later write retirement safe for Product-only SEO.

## CAT-28 compatibility history

CAT-28 closed with this bounded state:
Status: **source-complete PostgreSQL legacy canonical write retirement; physical donor retirement pending**.

CAT-28 advances only the PostgreSQL create path. For each normalized Category locale:

- PostgreSQL does **not** insert a new `catalog_category_translations` row;
- Product-only SEO still writes to `catalog_category_seo_translations` when present;
- canonical `name` / `description`, route and hierarchy continue through the existing
  transaction-bound Taxonomy owner-sync;
- the same-ID binding, `CatalogCategoryCreated`, operation receipt and transaction commit
  retain their existing ordering and rollback behavior;
- non-PostgreSQL backends still write `catalog_category_translations` because their list
  path remains donor-backed and the PostgreSQL-only Taxonomy binding/SEO seams are absent.

This is write retirement, not physical storage retirement. Existing PostgreSQL legacy
rows remain untouched in the CAT-28 slice. Its recorded next step is the separate
PostgreSQL-only fail-closed physical donor retirement implemented by CAT-29.

## CAT-29 PostgreSQL physical donor retirement

Migration `m20260829_000018_retire_product_category_legacy_translations` is PostgreSQL-only.
Other database backends return without changing schema because they still rely on the
legacy donor read/write path.

On PostgreSQL the migration opens one transaction and refuses to drop the table unless
all ownership proofs succeed:

- every Product Category, including historical/soft-deleted Product rows, has a typed
  Product → Taxonomy binding for the same UUID;
- the bound Taxonomy term exists in the same tenant, has kind `Category`, module scope
  `product`, and canonical key `product-category-{uuid}`;
- every historical legacy locale still has a same-tenant, same-ID Taxonomy localized row for that exact locale;
- every legacy locale row containing `meta_title` or `meta_description` has an exact
  `(tenant_id, category_id, locale)` row in `catalog_category_seo_translations` with
  identical Product-owned SEO values; missing rows or `IS DISTINCT FROM` differences
  block retirement;
- legacy `name` / `description` are deliberately **not** compared byte-for-byte with
  current Taxonomy copy, because Taxonomy became canonical before retirement and may
  have legitimately evolved after the historical donor snapshot. Locale coverage proves
  that the canonical owner still has the donor locale without requiring stale byte equality.

Only after those checks succeed does the same transaction execute
`DROP TABLE IF EXISTS catalog_category_translations` and commit. Any preflight or drop
failure rolls the transaction back and leaves the donor table intact.

The migration is intentionally irreversible. Recreating an empty donor table in `down`
would falsely suggest that the retired canonical localized copy and historical Product
translation UUIDs can be reconstructed. Fresh PostgreSQL installs still run the old
creation/backfill/SEO migrations in order and then retire the donor at CAT-29, while
non-PostgreSQL installs keep it because CAT-27 through CAT-29 storage migrations are
backend-bounded.

## CAT-30 PostgreSQL hierarchy-consumer cutover

CAT-30 does not move Product attribute/schema business policy into Taxonomy. It changes
only the canonical hierarchy source used by Product consumers that inherit along a
Category ancestor chain.

On PostgreSQL:

- effective Product form/schema resolution still reads Product-owned category schema
  assignments and attribute bindings, but each `CatalogCategorySchema.parent_category_id`
  is composed from the same-ID Taxonomy `Category` owner projection rather than
  `catalog_categories.parent_id`;
- inherited category attribute-group labels traverse the Taxonomy root-to-leaf parent
  chain instead of `catalog_category_closure`;
- every live Product Category must retain a same-ID Product → Taxonomy binding, module
  scope `product`, and canonical key `product-category-{uuid}`; missing or mismatched
  owner state fails closed rather than silently falling back to Product hierarchy;
- the Taxonomy owner reader exposes a generic `ConnectionTrait` entry point so Product
  transaction-bound reads can reuse the host connection without reading Taxonomy
  persistence entities directly;
- no schema migration is required and no Product schema/attribute assignment,
  membership, lifecycle, merchandising or virtual-rule ownership changes.

On non-PostgreSQL backends the existing Product `parent_id`/closure hierarchy consumers
remain active because those backends do not yet have the tenant-safe Taxonomy binding
prerequisite.

## CAT-31 PostgreSQL schema-directory ordering

CAT-31 narrows one remaining read seam. `ProductCatalogSchemaReadPort::list_categories`
is a Product catalog schema-directory operation rather than a storefront navigation
projection. On PostgreSQL, its final result order now follows the same Taxonomy-owned
Category hierarchy already used for canonical `parent_id`: parent before visible child,
with siblings ordered by Taxonomy `position`, then canonical key and UUID for deterministic
ties.

The existing SQL may retain Product `path` ordering as a stable input order for historical
CAT-26 compatibility, but CAT-31 explicitly reorders the composed PostgreSQL result from
the Taxonomy owner projection before it crosses the schema-directory port. A negative
Taxonomy position or cyclic projected hierarchy fails closed. When a Taxonomy parent is
outside the live Product subset, the live child is treated as a visible-root boundary;
Product lifecycle filtering is therefore preserved without rewriting canonical parent
identity.

Product `path` remains a Product-owned navigation projection and is still returned in
`CatalogCategoryListRecord`; CAT-31 does not make Taxonomy own Product navigation URLs,
merchandising placement or path lifecycle. Non-PostgreSQL donor reads retain Product
`path` ordering unchanged.

## CAT-32 PostgreSQL closure write retirement

CAT-32 advances only the Product create compatibility mirror after CAT-30 removed its
last PostgreSQL runtime consumer. PostgreSQL no longer materializes new
`catalog_category_closure` rows when a Product Category is created. Canonical ancestry
continues to be synchronized through the same transaction-bound Taxonomy owner write,
and the existing Product Category row, same-ID binding, event and commit ordering are
unchanged.

This is write retirement, not physical storage retirement. CAT-32 does **not** drop
`catalog_category_closure`, delete historical rows or reinterpret older backfill state.
`parent_id`, `path` and `level` remain Product-owned projections for Product navigation,
lifecycle compatibility and later migration analysis. Product `path` is still returned
by the schema-directory contract even though it is no longer the canonical PostgreSQL
hierarchy/order source.

Non-PostgreSQL backends continue to materialize Product closure rows on create and their
effective-form group-label ancestry still reads `catalog_category_closure`. Those
backends therefore retain the existing Product closure write/read compatibility pair
until they gain an equivalent tenant-safe Taxonomy hierarchy cutover.

## Product-owned state retained after CAT-32

Product continues to own:

- Category `code`, `kind`, virtual-category `rule_config`, activation/soft-delete
  lifecycle and Product-specific metadata;
- Product `parent_id`, `path` and `level` projections. PostgreSQL no longer treats those
  fields or Product closure rows as canonical Category ancestry; `path` remains the
  Product navigation projection;
- historical PostgreSQL `catalog_category_closure` storage pending a separate physical
  retirement proof, while non-PostgreSQL backends retain active closure writes/reads;
- localized `meta_title` / `meta_description` in
  `catalog_category_seo_translations` on PostgreSQL;
- category-bound attribute/schema definitions, assignments, inheritance semantics and
  labels. CAT-30 changes their hierarchy source, not their Product ownership;
- product/category membership, primary/navigation/collection/virtual assignment
  semantics and Product projections;
- non-PostgreSQL `catalog_category_translations` donor storage until those backends gain
  an equivalent verified Taxonomy cutover.

PostgreSQL no longer owns a Product-local canonical Category translation table after
CAT-29. CAT-30 stops Product effective-form/group-label consumers from treating
Product-local parent/closure state as canonical ancestry, CAT-31 stops Product path
from being the externally visible PostgreSQL schema-directory ordering authority, and
CAT-32 stops creating new PostgreSQL closure mirror rows. Taxonomy remains the canonical
Category identity/localized-copy/route/hierarchy owner; Product relation/policy/SEO/
navigation state stays Product-owned.

## Locale and route behavior

Taxonomy owner reads use the same `rustok-content` deterministic locale policy already
used by the former Product list projection: requested locale, explicit/platform
fallback, then lexicographically smallest normalized available locale. CAT-26 therefore
changed canonical owner storage without changing fallback order.

Taxonomy route ownership remains module-scope-wide per locale. Product donor storage
still only guarantees base-slug uniqueness per parent. CAT-24/CAT-25 fail-closed route
checks remain required; CAT-26 consumes the Taxonomy canonical localized slug instead
of rebuilding or flattening Product `path` into a route key.

CAT-27 reuses the already-normalized Product Category locale for SEO identity. CAT-28
reuses that same normalized input for Taxonomy and Product SEO without creating a second
localized canonical copy on PostgreSQL. CAT-29 removes only the superseded PostgreSQL
donor table; CAT-30 changes hierarchy consumers, CAT-31 changes schema-directory
ordering, and CAT-32 retires only PostgreSQL closure writes. None changes Taxonomy locale
resolution, Product route semantics or Product SEO locale identity.

## Translation ownership boundary

No `product/category` Translation provider is introduced. Canonical Category localized
copy is synchronized and read under the registered Taxonomy `taxonomy/term` provider on
PostgreSQL. `catalog_category_seo_translations` is Product SEO storage, not a Translation
provider and not canonical Category copy. After CAT-29, legacy Product Category
translation rows exist only on backends that still use the explicit non-PostgreSQL donor
compatibility path.
