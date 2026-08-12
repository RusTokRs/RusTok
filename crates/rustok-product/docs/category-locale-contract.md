# Product category locale contract

Product catalog categories remain a Product-owned tree/closure aggregate. This
contract only aligns their localized labels with the platform locale rules; it
does not move category hierarchy into Taxonomy.

## Write contract

- Every `catalog_category_translation.locale` is stored as the canonical locale
  tag returned by `rustok_api::normalize_locale_tag`.
- Category creation rejects invalid locale tags and two input translations that
  normalize to the same locale (for example `en_us` and `en-US`).
- Category translation names are trimmed and must contain 1..255 characters.
- `meta_title` is bounded to 255 characters and `meta_description` to 500,
  matching retained storage columns.
- Retained migration `m20260812_000013_normalize_catalog_category_translation_locales`
  canonicalizes existing category locales. Invalid locales, empty names, or a
  normalized-locale collision block the migration instead of choosing a winner.

## Read contract

`ProductCatalogSchemaService::list_categories` normalizes the requested locale
and resolves one display name deterministically in this order:

1. requested locale;
2. platform fallback locale;
3. lexicographically smallest canonical available locale;
4. category `code` only when the category has no translation rows.

The final fallback is intentionally independent of database row order, matching
the owner-neutral locale policy used by Blog, Forum, Taxonomy, Comments and
other localized read models.

## Ownership boundary

- Product owns category parent/child relations, closure rows, moves, deletion
  rules and structural/category-form behavior.
- Taxonomy owns shared vocabulary identities and localized taxonomy route keys;
  it does not acquire Product category `parent_id` or closure storage.
- This slice does not change Product attribute/schema translation contracts;
  those remain a separate i18n audit surface.
