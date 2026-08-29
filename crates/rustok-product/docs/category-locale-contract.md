# Product category locale contract

Product catalog categories remain a Product-owned **runtime** tree/closure
aggregate until a verified Taxonomy read/write cutover. TAXONOMY-CAT-24 adds a
monotonic Taxonomy shadow copy of canonical Category identity, localized copy,
route ownership and parent/order hierarchy; it does not switch Product runtime
reads or writes and does not retire Product closure storage.

## Write contract

- Every `catalog_category_translation.locale` is stored as the canonical locale
  tag returned by `rustok_api::normalize_locale_tag`.
- Category creation rejects invalid locale tags and two input translations that
  normalize to the same locale (for example `en_us` and `en-US`).
- Category translation names are trimmed and must contain 1..255 characters.
- `meta_title` is bounded to 255 characters and `meta_description` to 500,
  matching retained Product storage columns.
- Retained migration `m20260812_000013_normalize_catalog_category_translation_locales`
  canonicalizes existing category locales. Invalid locales, empty names, or a
  normalized-locale collision block the migration instead of choosing a winner.

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
other localized read models.

## CAT-24 Taxonomy projection

- Product stores one base category `slug`, not a localized slug. CAT-24 validates
  that route key canonically and projects the same one base Product category
  `slug` into every imported locale; it does not invent translated slugs.
- CAT-24 backfills Product `parent_id` and `position` into the Taxonomy hierarchy
  only after every same-ID Category identity/localized route exists.
- Product translation UUIDs are retained in Taxonomy localized rows; incompatible
  UUID, route, localized-copy or hierarchy ownership blocks the migration.
- `meta_title` / `meta_description` stay Product-owned SEO data because Taxonomy
  Category localized rows do not have equivalent fields in this slice.

## Ownership boundary

- Product currently serves runtime parent/child relations, closure rows, moves,
  deletion/activation behavior, structural/category-form behavior and all
  Product-specific category projections until read/write cutover is verified.
- Taxonomy is the accepted canonical target for shared Category identity,
  hierarchy, localized `name`/`slug`/`description` and route ownership. CAT-24
  establishes that target copy but does not make Product runtime consume it yet.
- Product-specific `kind`, virtual-category `rule_config`, metadata,
  category-bound attribute/schema state and product/category assignment semantics
  do not move into Taxonomy through this backfill.
- This slice does not change Product attribute/schema translation contracts;
  those remain a separate i18n audit surface.
