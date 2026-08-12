# Product attribute schema locale contract

Product attribute schemas remain Product-owned catalog-schema state. This
contract aligns their localized display names with the same platform locale
rules already applied to Product categories; it does not move schema ownership
into Taxonomy or Content.

## Write contract

- Every `product_attribute_schema_translations.locale` is stored as the canonical
  tag returned by `rustok_api::normalize_locale_tag`.
- Schema creation rejects invalid locale tags and duplicate-equivalent locales
  such as `en_us` and `en-US`.
- Schema translation names are trimmed and must contain 1..255 characters.
- Retained migration
  `m20260812_000014_normalize_product_attribute_schema_translation_locales`
  canonicalizes existing schema locales and fails closed on invalid locales,
  empty names, or normalized-locale collisions instead of choosing a winner.

## Read contract

`ProductCatalogSchemaService::list_schemas` normalizes the requested locale and
resolves one display name deterministically in this order:

1. requested locale;
2. platform fallback locale;
3. lexicographically smallest canonical available locale;
4. schema `code` only when the schema has no translation rows.

The final fallback is independent of database row order.

## Ownership boundary and follow-up

- Product owns schema identity, bindings, groups, attributes and localized
  schema labels.
- Taxonomy remains the shared vocabulary/route-key layer and does not acquire
  Product schema/category structure.
- Attribute/option and schema/category-group translation tables are audited in
  separate slices so each retained migration and write/read contract can be
  reviewed independently.
