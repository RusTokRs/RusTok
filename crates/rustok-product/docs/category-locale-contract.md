# Product category locale contract

Product catalog categories remain a Product-owned commerce aggregate for policy,
closure, merchandising and SEO, while canonical shared Category copy is moving to
Taxonomy. TAXONOMY-CAT-24 added the monotonic Taxonomy copy, TAXONOMY-CAT-25
transactionally dual-writes new creates, and TAXONOMY-CAT-26 switches the PostgreSQL
Category list projection to Taxonomy-owned canonical localized copy and parent identity.

## Write contract

- Every `catalog_category_translation.locale` is stored as the canonical locale tag
  returned by `rustok_api::normalize_locale_tag`.
- Category creation rejects invalid locale tags and two input translations that
  normalize to the same locale (for example `en_us` and `en-US`).
- New Category translation names are trimmed and must contain 1..120 characters,
  matching the canonical Taxonomy Category owner-sync bound.
- New Category descriptions are trimmed; blank descriptions become `None`, and
  non-empty descriptions are limited to 2000 characters before either Product or
  Taxonomy persistence occurs.
- `meta_title` is bounded to 255 characters and `meta_description` to 500, matching
  retained Product storage columns. Those SEO fields remain Product-only and are not
  sent through the Taxonomy canonical-copy sync.
- The one Product base category slug must already equal Taxonomy's canonical route
  normalization, must fit the 120-byte Taxonomy route-key bound, and is mirrored
  unchanged into every synchronized locale.
- Category `position` must be non-negative because it is mirrored into Taxonomy
  hierarchy in the same create transaction.
- Retained migration `m20260812_000013_normalize_catalog_category_translation_locales`
  canonicalizes existing category locales. Invalid locales, empty names, or a
  normalized-locale collision block the migration instead of choosing a winner.

## CAT-25 compatibility history

TAXONOMY-CAT-25 now dual-writes new Category creates into Taxonomy before the Product
domain event and commit. At CAT-25 completion, `ProductCatalogSchemaService::list_categories`
still reads the Product donor; create dual-write is the prerequisite that prevents
post-backfill Categories from being missing during a later Taxonomy-backed read cutover.
The same-ID Product ↔ Taxonomy binding is inserted only after every locale succeeds.
The CAT-25 canonical text contract remains 1..120 characters for names, and blank
descriptions become `None`.

## CAT-26 PostgreSQL read contract

`ProductCatalogSchemaService::list_categories` still validates the requested locale
through `normalize_locale_tag`, but PostgreSQL canonical materialization is now split by
ownership:

- Product supplies the live Category set, stable UUID, `code`, `kind`, retained `path`
  and the Product-owned Taxonomy binding;
- every live row must have a same-ID binding to the expected Taxonomy Category;
- `TaxonomyOwnerCategoryReader` supplies canonical localized `name`, canonical localized
  `slug` and Taxonomy `parent_id` for module scope `product`;
- the owner reader receives the requested locale plus platform fallback and therefore
  preserves the existing deterministic resolution order: requested locale, platform
  fallback locale, then the lexicographically smallest normalized available locale;
- a missing Taxonomy owner row, incompatible canonical key/scope, or missing canonical
  localized copy fails closed rather than using `catalog_category_translations` as a
  hidden fallback;
- Product `path` continues to define list ordering in this bounded slice, so Product
  navigation/path projection is not silently reinterpreted as Taxonomy route identity.

On non-PostgreSQL backends the existing donor read remains active because the physical
Product ↔ Taxonomy binding/backfill seam is PostgreSQL-only. CAT-26 does not fabricate a
cross-backend ownership contract where the tenant-safe binding prerequisite does not
exist.

## CAT-24 Taxonomy projection history

At CAT-24 completion, Product categories remain a Product-owned **runtime** tree/closure
aggregate until a verified Taxonomy read/write cutover. That historical boundary is
retained as CAT-24 evidence even though CAT-26 now advances the PostgreSQL list read.

- Product stores one base category `slug`, not a localized slug. CAT-24 validates that
  route key canonically and projects the same one base Product category `slug` into
  every imported locale; it does not invent translated slugs.
- Product currently guarantees category slug uniqueness only within a parent, while
  Taxonomy route ownership is module-scope-wide per locale. CAT-24 does not flatten
  Product `path` into a replacement route key and does not rename a category
  automatically; an ambiguous projected Taxonomy route blocks the backfill for
  explicit donor remediation.
- CAT-24 backfills Product `parent_id` and `position` into the Taxonomy hierarchy only
  after every same-ID Category identity/localized route exists.
- Product translation UUIDs are retained in Taxonomy localized rows for that historical
  backfill; new owner-sync writes rely on canonical term+locale ownership rather than
  consumer translation-row identity.
- `meta_title` / `meta_description` stay Product-owned SEO data because Taxonomy Category
  localized rows do not have equivalent fields.
- CAT-24 does not make Product runtime consume it yet; that historical statement remains
  true for the CAT-24 slice even though CAT-26 now consumes the Taxonomy owner projection
  on PostgreSQL.

## Ownership boundary after CAT-26

- Taxonomy is canonical for shared Category identity, hierarchy parent, localized
  `name`/`slug`/`description` and route ownership on the PostgreSQL Product list path.
- Product continues to own `code`, `kind`, virtual-category `rule_config`, metadata,
  activation/deletion behavior, Product `path`, closure rows, category-bound
  attribute/schema state and product/category assignment semantics.
- Product translation rows are retained as compatibility/SEO storage, but PostgreSQL
  Category list display no longer reads them for canonical name or route slug.
- Product `path` remains a commerce/navigation projection rather than canonical route
  authority. A future explicit path/navigation slice must decide how Taxonomy slug or
  hierarchy edits propagate before donor closure/path storage can be retired.
- This slice does not change Product attribute/schema translation contracts; those
  remain a separate i18n audit surface.
