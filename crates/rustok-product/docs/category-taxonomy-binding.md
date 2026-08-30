# Product Category → Taxonomy migration contract

Status: **source-complete PostgreSQL Product Category canonical/hierarchy donor retirement; non-PostgreSQL donor compatibility retained**
Current migration cursor: **TAXONOMY-CAT-34 PostgreSQL closure storage retirement; Product navigation projections and non-PostgreSQL closure compatibility retained**.

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
TAXONOMY-CAT-32 stops new PostgreSQL Product Category creates from materializing
the Product closure mirror after all PostgreSQL hierarchy consumers moved to Taxonomy.
TAXONOMY-CAT-33 retires the remaining PostgreSQL closure-parity commit invariant while
retaining cycle rejection and leaving physical closure storage retirement for a later slice.
TAXONOMY-CAT-34 physically retires the now-unread and unwritten PostgreSQL Product
closure storage while retaining the Product parent-cycle guard and non-PostgreSQL compatibility.

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

Historical CAT-29 record:
Status: **source-complete PostgreSQL legacy Category translation storage retirement; non-PostgreSQL donor compatibility retained**.

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

Historical CAT-30 boundary retained Product `path` and closure persistence as compatibility state outside canonical PostgreSQL hierarchy ownership.

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

Historical CAT-32 migration cursor: **TAXONOMY-CAT-32 PostgreSQL closure write retirement; Product navigation/path and non-PostgreSQL closure compatibility retained**.

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

## CAT-33 PostgreSQL closure invariant retirement

CAT-32 stopped PostgreSQL Category creates from writing the Product closure mirror, but
historical migration `m20260725_000002_enforce_catalog_category_tree_invariants` also
installed a deferred database assertion that required `catalog_category_closure` to be
an exact projection of `catalog_categories.parent_id`. Without a later migration, that
historical parity assertion can reject a valid post-CAT-32 Category transaction at
commit even though PostgreSQL runtime consumers already use Taxonomy hierarchy.

Migration `m20260829_000019_retire_product_category_closure_invariant` closes that gap.
On PostgreSQL its `up` path first runs the historical assertion as a fail-closed preflight,
then replaces `rustok_product_assert_category_tree()` with a **cycle-only** assertion.
The historical `trg_catalog_categories_validate_tree` and
`trg_catalog_category_closure_validate_tree` objects remain installed, but both now call
the cycle-only function. The closure trigger is therefore compatibility plumbing rather
than a closure-parity authority: changing or omitting closure rows no longer makes a
valid Category parent tree fail at deferred commit, while Product parent cycles remain
rejected.

CAT-33 is closure invariant retirement, not physical closure storage retirement. It does
not drop `catalog_category_closure`, does not delete historical closure rows in `up`, and
does not change PostgreSQL runtime hierarchy ownership already established by CAT-30/31.
Physical closure table/index/foreign-key/trigger retirement remains a separate later
slice after migration-history and rollback requirements are re-audited.

The migration keeps one-step rollback truthful. Its PostgreSQL `down` path runs the
cycle-only assertion, rebuilds `catalog_category_closure` exactly from the retained
Product `parent_id` projection using a recursive walk, restores the historical
closure-parity body of `rustok_product_assert_category_tree()`, and verifies the restored
invariant before commit. That allows rollback to the previous migration contract without
pretending that stale closure rows remained canonical during CAT-33.

Non-PostgreSQL backends are unchanged by CAT-33. Their active Product closure
write/read compatibility pair from CAT-32 remains intact.

## CAT-34 PostgreSQL closure storage retirement

Migration `m20260829_000020_retire_product_category_closure_storage` removes the final
physical PostgreSQL `catalog_category_closure` storage object after CAT-30 moved the last
PostgreSQL hierarchy consumer to Taxonomy, CAT-32 stopped PostgreSQL closure writes, and
CAT-33 removed closure parity from the deferred Product Category invariant.

The PostgreSQL `up` path is deliberately fail-closed:

- it runs the retained cycle-only `rustok_product_assert_category_tree()` before changing
  schema and requires the CAT-33 closure compatibility trigger to exist;
- it drops `trg_catalog_category_closure_validate_tree`, then executes
  `DROP TABLE catalog_category_closure` **without `CASCADE`**, so an unexpected external
  dependency aborts the migration instead of being silently destroyed;
- the table-owned primary key, depth check, foreign keys and descendant index retire with
  the table, while `trg_catalog_categories_validate_tree`,
  `rustok_product_validate_category_tree_trigger()` and the cycle-only assertion function
  remain live;
- it invokes the cycle-only assertion again after the drop, proving that the retained
  Product parent-tree guard has no hidden closure-table dependency.

CAT-34 keeps one-step rollback truthful rather than recreating an empty compatibility
shell. Its PostgreSQL `down` path recreates the historical closure table shape, including
the tenant-safe composite ancestor/descendant foreign keys added by
`m20260701_000002_add_product_catalog_tenant_consistency_constraints`, restores
`idx_catalog_category_closure_descendant`, recursively reconstructs exact closure rows
from the retained Product `parent_id` projection, proves row/depth parity directly, and
recreates the deferred closure trigger. The CAT-33 cycle-only assertion remains unchanged;
a subsequent rollback of CAT-33 is the step that restores closure parity as a commit
authority. This preserves truthful CAT-34 → CAT-33 → CAT-32 rollback sequencing.

Non-PostgreSQL backends keep their existing `catalog_category_closure` table, create
writes and effective-form ancestry reads unchanged. CAT-34 is therefore physical
PostgreSQL storage retirement only; it does not remove the compatibility seam from
SQLite/MySQL or change Product navigation projections.

## Product-owned state retained after CAT-34

Product continues to own:

- Category `code`, `kind`, virtual-category `rule_config`, activation/soft-delete
  lifecycle and Product-specific metadata;
- Product `parent_id`, `path` and `level` projections. PostgreSQL no longer treats those
  fields as canonical Category ancestry; `path` remains the Product navigation projection;
- localized `meta_title` / `meta_description` in
  `catalog_category_seo_translations` on PostgreSQL;
- category-bound attribute/schema definitions, assignments, inheritance semantics and
  labels. CAT-30 changes their hierarchy source, not their Product ownership;
- product/category membership, primary/navigation/collection/virtual assignment
  semantics and Product projections;
- non-PostgreSQL `catalog_category_translations` donor storage and
  `catalog_category_closure` hierarchy compatibility until those backends gain an
  equivalent verified Taxonomy cutover.

PostgreSQL no longer owns a Product-local canonical Category translation table after
CAT-29. CAT-30 stops Product effective-form/group-label consumers from treating
Product-local parent/closure state as canonical ancestry, CAT-31 stops Product path
from being the externally visible PostgreSQL schema-directory ordering authority,
CAT-32 stops creating new PostgreSQL closure mirror rows, CAT-33 stops the historical
closure-parity function from reviving that mirror as a deferred commit requirement, and
CAT-34 removes the retired PostgreSQL closure table/trigger/index/foreign-key storage.
Taxonomy remains the canonical Category identity/localized-copy/route/hierarchy owner;
Product relation/policy/SEO/navigation state stays Product-owned.

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
ordering, CAT-32 retires PostgreSQL closure writes, CAT-33 retires only the PostgreSQL
closure-parity commit invariant, and CAT-34 retires only the physical PostgreSQL closure
storage. None changes Taxonomy locale resolution, Product route semantics or Product SEO
locale identity.

## Translation ownership boundary

No `product/category` Translation provider is introduced. Canonical Category localized
copy is synchronized and read under the registered Taxonomy `taxonomy/term` provider on
PostgreSQL. `catalog_category_seo_translations` is Product SEO storage, not a Translation
provider and not canonical Category copy. After CAT-29, legacy Product Category
translation rows exist only on backends that still use the explicit non-PostgreSQL donor
compatibility path.