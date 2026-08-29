# Product category locale contract

Product catalog categories remain a Product-owned commerce aggregate for policy,
closure, merchandising and SEO, while canonical shared Category copy is Taxonomy-owned
on PostgreSQL. TAXONOMY-CAT-24 added the monotonic Taxonomy copy, TAXONOMY-CAT-25
transactionally dual-writes new creates, TAXONOMY-CAT-26 switches the PostgreSQL
Category list projection to Taxonomy-owned canonical localized copy and parent identity,
TAXONOMY-CAT-27 isolates Product-only localized SEO into dedicated Product storage,
TAXONOMY-CAT-28 stops new PostgreSQL creates from writing the legacy canonical translation
mirror, and TAXONOMY-CAT-29 physically retires that mirror on PostgreSQL after fail-closed
ownership checks.

## Write contract

- Every normalized Category input locale uses the canonical locale tag returned by
  `rustok_api::normalize_locale_tag`.
- Category creation rejects invalid locale tags and two input translations that
  normalize to the same locale (for example `en_us` and `en-US`).
- New Category translation names are trimmed and must contain 1..120 characters,
  matching the canonical Taxonomy Category owner-sync bound.
- New Category descriptions are trimmed; blank descriptions become `None`, and
  non-empty descriptions are limited to 2000 characters before persistence occurs.
- `meta_title` is bounded to 255 characters and `meta_description` to 500. They remain
  Product-only SEO and are never sent through Taxonomy canonical-copy sync.
- The one Product base category slug must already equal Taxonomy's canonical route
  normalization, must fit the 120-byte Taxonomy route-key bound, and is mirrored
  unchanged into every synchronized locale.
- Category `position` must be non-negative because it is mirrored into Taxonomy
  hierarchy in the same create transaction.
- Retained migration `m20260812_000013_normalize_catalog_category_translation_locales`
  canonicalizes existing legacy category locales before the historical backfill and
  retirement sequence. Invalid locales, empty names, or a normalized-locale collision
  block that migration instead of choosing a winner.

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
through `normalize_locale_tag`, but PostgreSQL canonical materialization is split by
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

## CAT-27 localized SEO contract

TAXONOMY-CAT-27 isolates Product-only localized SEO. On PostgreSQL,
`catalog_category_seo_translations` is the Product-owned localized SEO store. Its locale
key is the same canonical locale already produced by Product Category input normalization;
CAT-27 does not create a separate SEO locale normalizer or fallback policy.

The migration copies only rows where at least one of `meta_title` or `meta_description`
is non-null. It preserves the exact persisted locale and SEO values, and fails closed if
an already-present SEO row for the same tenant/category/locale disagrees with the legacy
Product translation row. Compatible rows remain monotonic and no blank SEO-only row is
created.

At CAT-27 completion, each new PostgreSQL locale still wrote the compatibility
`catalog_category_translations` row. If that locale had SEO, the same
`ProductWriteTransaction` also inserted `(tenant_id, category_id, locale, meta_title,
meta_description)` into `catalog_category_seo_translations` before the existing Taxonomy
owner-sync. Therefore Product SEO and canonical Taxonomy copy could not partially commit.
CAT-27 does **not** drop `catalog_category_translations` and does not stop compatibility
writes; that historical state remains the retained CAT-27 evidence.

## CAT-28 compatibility history

TAXONOMY-CAT-28 stops new PostgreSQL creates from writing the legacy canonical translation mirror.
Its bounded write contract remains:

- `should_write_legacy_category_translation(DatabaseBackend::Postgres)` is false;
- PostgreSQL therefore does not insert a new `catalog_category_translations` row;
- each normalized locale still writes Product SEO to `catalog_category_seo_translations`
  when SEO exists, before the existing Taxonomy owner-sync;
- canonical normalized `name` and `description` are still sent to Taxonomy for every
  locale; no localized canonical content is discarded;
- non-PostgreSQL backends continue to insert the legacy Product translation row because
  their `list_categories` path still reads that donor and they do not have the
  PostgreSQL-only Taxonomy/SEO storage boundary;
- event and commit ordering remain unchanged, so Taxonomy/SEO failure still rolls back
  the Product Category create atomically.

CAT-28 does not delete or rewrite historical legacy rows. It retires new PostgreSQL
canonical mirror writes so a later PostgreSQL-only physical retirement migration can
preflight and drop the table without racing newly-created donor rows.

## CAT-29 PostgreSQL locale-storage retirement

CAT-29 removes `catalog_category_translations` only on PostgreSQL. Before the drop, one
transaction proves that every Product Category still has the expected same-ID Taxonomy
owner in tenant/module scope and that all Product-owned SEO from historical locale rows
was copied exactly into `catalog_category_seo_translations`.

The SEO check resolves tenant identity through `catalog_categories` because the legacy
translation row itself has no `tenant_id`. It joins SEO by the exact normalized
`(tenant_id, category_id, locale)` identity and uses `IS DISTINCT FROM` for both SEO
fields. A missing SEO row or different `meta_title` / `meta_description` blocks the
migration. Legacy rows with no SEO do not require an empty SEO-only row.

The retirement intentionally does not compare historical legacy `name` or `description`
bytes with current Taxonomy copy. Taxonomy is already canonical and may legitimately
have been edited after CAT-24/CAT-25; requiring stale donor equality would turn a valid
canonical edit into a migration blocker. Same-ID tenant/kind/scope/canonical-key
ownership is the fail-closed proof for canonical copy retirement.

On successful PostgreSQL preflight, `catalog_category_translations` is dropped in the
same transaction. The migration is irreversible and `down` does not recreate an empty
localized donor. On SQLite/MySQL and other non-PostgreSQL backends CAT-29 is a no-op;
their legacy translation table and donor list/write behavior remain live.

## CAT-24 Taxonomy projection history

At CAT-24 completion, Product categories remain a Product-owned **runtime** tree/closure
aggregate until a verified Taxonomy read/write cutover. That historical boundary is
retained as CAT-24 evidence even though CAT-26 advances the PostgreSQL list read.

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

## Ownership boundary after CAT-29

- Taxonomy is canonical for shared Category identity, hierarchy parent, localized
  `name`/`slug`/`description` and route ownership on the PostgreSQL Product path.
- Product continues to own `code`, `kind`, virtual-category `rule_config`, metadata,
  activation/deletion behavior, Product `path`, closure rows, category-bound
  attribute/schema state and product/category assignment semantics.
- Product localized `meta_title` / `meta_description` are Product-only SEO and live in
  `catalog_category_seo_translations` on PostgreSQL.
- PostgreSQL no longer has `catalog_category_translations` after CAT-29 migration
  completion; new canonical writes had already stopped in CAT-28.
- Non-PostgreSQL Product translation rows remain live donor storage and their reads/writes
  are intentionally unchanged.
- Product `path` remains a commerce/navigation projection rather than canonical route
  authority. A future explicit path/navigation slice must decide how Taxonomy slug or
  hierarchy edits propagate before donor closure/path storage can be retired.
- This slice does not change Product attribute/schema translation contracts; those
  remain a separate i18n audit surface.
