# Product category locale contract

Product catalog categories remain a Product-owned **runtime** tree/closure
aggregate until a verified Taxonomy read/write cutover. TAXONOMY-CAT-24 added a
monotonic Taxonomy shadow copy of canonical Category identity, localized copy,
route ownership and parent/order hierarchy. TAXONOMY-CAT-25 now dual-writes new
Category creates into that Taxonomy owner inside the same Product transaction,
while Product list/read projections and Product-specific category state remain
on the donor side.

## Write contract

- Every `catalog_category_translation.locale` is stored as the canonical locale
  tag returned by `rustok_api::normalize_locale_tag`.
- Category creation rejects invalid locale tags and two input translations that
  normalize to the same locale (for example `en_us` and `en-US`).
- New Category translation names are trimmed and must contain 1..120 characters,
  matching the canonical Taxonomy Category owner-sync bound.
- New Category descriptions are trimmed; blank descriptions become `None`, and
  non-empty descriptions are limited to 2000 characters before either Product
  or Taxonomy persistence occurs.
- `meta_title` is bounded to 255 characters and `meta_description` to 500,
  matching retained Product storage columns. Those SEO fields remain Product-only
  and are not sent through the Taxonomy canonical-copy sync.
- The one Product base category slug must already equal Taxonomy's canonical route
  normalization, must fit the 120-byte Taxonomy route-key bound, and is mirrored
  unchanged into every synchronized locale.
- Category `position` must be non-negative because it is mirrored into Taxonomy
  hierarchy in the same create transaction.
- Retained migration `m20260812_000013_normalize_catalog_category_translation_locales`
  canonicalizes existing category locales. Invalid locales, empty names, or a
  normalized-locale collision block the migration instead of choosing a winner.

## CAT-25 create atomicity

`ProductCatalogSchemaService::create_category` first writes Product donor identity,
closure and normalized localized rows inside `ProductWriteTransaction`. Before the
Product `CatalogCategoryCreated` event or commit, each normalized locale is passed
to Taxonomy's transaction-bound Category owner-sync with the same Category UUID,
module scope `product`, canonical key `product-category-{uuid}`, base slug,
localized name/description and Product `parent_id`/`position`. The same-ID Product
↔ Taxonomy binding is inserted only after every locale succeeds.

Any Taxonomy identity, scope, route, hierarchy or localized-copy conflict aborts
the shared transaction, so a failed canonical sync cannot leave a newly-created
Product donor Category committed without its Taxonomy owner state.

## Read contract

`ProductCatalogSchemaService::list_categories` still reads the Product donor
until a later verified runtime cutover. It normalizes the requested locale and
resolves one display name deterministically in this order:

1. requested locale;
2. platform fallback locale;
3. lexicographically smallest canonical available locale;
4. category `code` only when the category has no translation rows.

The final fallback is intentionally independent of database row order, matching
the owner-neutral locale policy used by Blog, Forum, Taxonomy, Comments and
other localized read models. CAT-25 deliberately leaves this read path unchanged;
create dual-write is the prerequisite that prevents post-backfill Categories from
being missing when a later Taxonomy-backed read projection is introduced.

## CAT-24 Taxonomy projection

- Product stores one base category `slug`, not a localized slug. CAT-24 validates
  that route key canonically and projects the same one base Product category
  `slug` into every imported locale; it does not invent translated slugs.
- Product currently guarantees category slug uniqueness only within a parent,
  while Taxonomy route ownership is module-scope-wide per locale. CAT-24 does
  not flatten Product `path` into a replacement route key and does not rename a
  category automatically; an ambiguous projected Taxonomy route blocks the
  backfill for explicit donor remediation.
- CAT-24 backfills Product `parent_id` and `position` into the Taxonomy hierarchy
  only after every same-ID Category identity/localized route exists.
- Product translation UUIDs are retained in Taxonomy localized rows for that
  historical backfill; new CAT-25 owner-sync writes rely on canonical term+locale
  ownership rather than consumer translation-row identity.
- `meta_title` / `meta_description` stay Product-owned SEO data because Taxonomy
  Category localized rows do not have equivalent fields in this slice.
- CAT-24 does not make Product runtime consume it yet; CAT-25 changes create
  synchronization only and still does not switch Product list/read materialization.

## Ownership boundary

- Product currently serves runtime parent/child relations, closure rows,
  deletion/activation behavior, structural/category-form behavior and all
  Product-specific category projections until later read/write cutover slices.
- Taxonomy is the canonical target and now the transactional create mirror for
  shared Category identity, hierarchy, localized `name`/`slug`/`description` and
  route ownership.
- Product-specific `kind`, virtual-category `rule_config`, metadata,
  category-bound attribute/schema state and product/category assignment semantics
  do not move into Taxonomy through CAT-25.
- This slice does not change Product attribute/schema translation contracts;
  those remain a separate i18n audit surface.
