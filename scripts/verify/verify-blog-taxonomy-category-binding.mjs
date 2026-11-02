#!/usr/bin/env node

// Successor contract for `verify-blog-taxonomy-category-binding.mjs`.
//
// CAT-6 introduced the typed Blog -> Taxonomy Category binding table
// (`m20260824_000019_add_blog_taxonomy_category_binding`) as an intermediate cutover step.
// `m20260916_000022_clean_blog_category_canonical_taxonomy` then retired that table and the
// Blog-owned hierarchy columns: `rustok-taxonomy` owns canonical Blog Category identity,
// localized copy, route history and hierarchy, while `blog_categories` keeps module
// membership and Blog-owned settings. The binding relation, its entity and its runtime
// harness are gone, so the guard now proves the retirement instead of the intermediate
// design: the typed binding must stay absent, the cleanup migration must stay registered,
// the canonical Taxonomy identity path must be the only remaining join, and the crate's
// retirement harness must keep executing those claims.

import fs from 'node:fs';

const failures = [];
const read = (path) => fs.readFileSync(path, 'utf8');
const requireFile = (path, label = path) => {
  if (!fs.existsSync(path)) failures.push(`${path}: required ${label}`);
};
const requireAbsent = (path, label = path) => {
  if (fs.existsSync(path)) failures.push(`${path}: retired ${label} must remain absent`);
};
const requireMarker = (path, marker, label = marker) => {
  if (!read(path).includes(marker)) failures.push(`${path}: missing ${label}`);
};
const rejectMarker = (path, marker, label = marker) => {
  if (read(path).includes(marker)) failures.push(`${path}: must not contain ${label}`);
};

const consolidatedMigration =
  'crates/modules/rustok-blog/src/migrations/m20261029_000001_create_blog_schema.rs';
const isConsolidated = fs.existsSync(consolidatedMigration);

const bindingMigration =
  'crates/modules/rustok-blog/src/migrations/m20260824_000019_add_blog_taxonomy_category_binding.rs';
const cleanupMigration =
  'crates/modules/rustok-blog/src/migrations/m20260916_000022_clean_blog_category_canonical_taxonomy.rs';
const migrationRegistry = 'crates/modules/rustok-blog/src/migrations/mod.rs';
const backfillContracts = 'docs/migrations/backfill-contracts.json';
const entities = 'crates/modules/rustok-blog/src/entities/mod.rs';
const legacyCategory = 'crates/modules/rustok-blog/src/entities/blog_category.rs';
const owner = 'crates/modules/rustok-blog/src/services/category_owner.rs';
const sync = 'crates/modules/rustok-blog/src/services/category_taxonomy_sync.rs';
const retirementTest = 'crates/modules/rustok-blog/tests/category_taxonomy_legacy_storage_retirement.rs';

const requiredFiles = isConsolidated
  ? [consolidatedMigration, migrationRegistry, entities, legacyCategory, owner, sync, retirementTest]
  : [bindingMigration, cleanupMigration, migrationRegistry, backfillContracts, entities, legacyCategory, owner, sync, retirementTest];

for (const path of requiredFiles) {
  requireFile(path);
}
requireAbsent(
  'crates/modules/rustok-blog/src/entities/blog_category_taxonomy_binding.rs',
  'typed Blog -> Taxonomy binding entity',
);
requireAbsent(
  'crates/modules/rustok-blog/tests/category_taxonomy_binding.rs',
  'typed binding runtime harness',
);
requireAbsent(
  'crates/modules/rustok-blog/src/entities/blog_category_translation.rs',
  'donor Blog Category translation entity',
);

if (failures.length === 0) {
  if (isConsolidated) {
    requireMarker(
      migrationRegistry,
      'm20261029_000001_create_blog_schema',
      'registered consolidated migration',
    );
    rejectMarker(
      consolidatedMigration,
      'BlogCategoryTaxonomyBindings',
      'retired binding table must not exist in consolidated schema',
    );
    rejectMarker(
      consolidatedMigration,
      'blog_category_taxonomy_bindings',
      'retired binding table name must not exist in consolidated schema',
    );
  } else {
    requireMarker(
      bindingMigration,
      'BlogCategoryTaxonomyBindings::Table',
      'historical CAT-6 binding table creator',
    );
    requireMarker(
      migrationRegistry,
      'm20260824_000019_add_blog_taxonomy_category_binding',
      'registered historical CAT-6 migration',
    );
    requireMarker(
      backfillContracts,
      '"migration": "m20260824_000019_add_blog_taxonomy_category_binding"',
      'CAT-6 backfill declaration',
    );
    requireMarker(backfillContracts, '"mode": "none"', 'empty binding-table backfill mode');

    requireMarker(
      cleanupMigration,
      'BlogCategoryTaxonomyBindings::Table',
      'retired binding table target',
    );
    requireMarker(cleanupMigration, 'drop_table(', 'binding table drop');
    requireMarker(
      migrationRegistry,
      'm20260916_000022_clean_blog_category_canonical_taxonomy',
      'registered canonical cleanup migration',
    );
    for (const column of ['BlogCategories::ParentId', 'BlogCategories::Position', 'BlogCategories::Depth']) {
      requireMarker(cleanupMigration, column, `retired Blog-owned hierarchy column ${column}`);
    }
  }

  requireMarker(legacyCategory, 'pub settings: Json', 'Blog-owned settings retained');
  for (const retired of ['pub parent_id', 'pub position', 'pub post_count']) {
    rejectMarker(legacyCategory, retired, `retired Blog-owned ${retired}`);
  }
  rejectMarker(entities, 'blog_category_taxonomy_binding', 'retired binding entity registration');

  requireMarker(sync, 'sync_module_category_with_owned_aliases_in_tx', 'canonical Taxonomy sync delegate');
  requireMarker(sync, 'canonical_key_for_blog_category', 'deterministic canonical Category key');
  requireMarker(owner, 'TaxonomyOwnerCategoryReader', 'canonical Taxonomy owner read');
  requireMarker(owner, 'delete_module_category_with_cleanup(', 'Taxonomy-owned delete lifecycle');

  const retirementMarkers = isConsolidated
    ? [
        'legacy_category_storage_is_retired_in_canonical_schema',
        'retired donor storage must not remain on the Blog entity surface',
      ]
    : [
        'legacy_category_storage_retires_only_after_taxonomy_identity_cutover',
        'blog_category_taxonomy_binding::Entity::find()',
        'Intentionally irreversible',
        'retired donor storage must not remain on the Blog entity surface',
      ];
  for (const marker of retirementMarkers) {
    requireMarker(retirementTest, marker, `retirement proof ${marker}`);
  }
}

if (failures.length > 0) {
  console.error('[blog-taxonomy-category-binding] retirement boundary verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  '[blog-taxonomy-category-binding] typed binding retired; canonical Taxonomy Category identity verified',
);
