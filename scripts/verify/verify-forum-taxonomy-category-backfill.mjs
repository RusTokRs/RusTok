#!/usr/bin/env node

import fs from 'node:fs';

const read = (path) => fs.readFileSync(path, 'utf8');
const failures = [];
const requireMarker = (path, marker, label = marker) => {
  const source = read(path);
  if (!source.includes(marker)) failures.push(`${path}: missing ${label}`);
};
const rejectMarker = (path, marker, label = marker) => {
  const source = read(path);
  if (source.includes(marker)) failures.push(`${path}: must not contain ${label}`);
};

const migration = 'crates/rustok-forum/src/migrations/m20260823_000030_backfill_forum_categories_to_taxonomy.rs';
const registry = 'crates/rustok-forum/src/migrations/mod.rs';
const contracts = 'docs/migrations/backfill-contracts.json';
const entitiesRegistry = 'crates/rustok-forum/src/entities/mod.rs';
const legacyCategory = 'crates/rustok-forum/src/entities/forum_category.rs';
const legacyTranslation = 'crates/rustok-forum/src/entities/forum_category_translation.rs';
const forumServices = 'crates/rustok-forum/src/services/mod.rs';

for (const path of [
  migration,
  registry,
  contracts,
  entitiesRegistry,
  legacyCategory,
  legacyTranslation,
  forumServices,
]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  requireMarker(registry, 'm20260823_000030_backfill_forum_categories_to_taxonomy', 'registered CAT-5 backfill migration');
  requireMarker(registry, 'm20260822_000011_create_taxonomy_category_presentations', 'Taxonomy owner-storage dependency');
  requireMarker(registry, 'm20260823_000029_add_forum_taxonomy_category_binding', 'binding seam dependency');

  requireMarker(migration, 'const FORUM_SCOPE_VALUE: &str = "forum";', 'Forum module scope policy');
  requireMarker(migration, 'TaxonomyTermKind::Category', 'Category-only backfill');
  requireMarker(migration, 'TaxonomyScopeType::Module', 'module-scoped legacy migration');
  requireMarker(migration, 'id: Set(category.id)', 'Forum UUID preservation');
  requireMarker(migration, 'format!("forum-category-{category_id}")', 'locale-independent canonical key');
  requireMarker(migration, 'forum_category_translation::Entity::find()', 'historical localized copy backfill input');
  requireMarker(migration, 'legacy_category_route_alias::Entity::find()', 'legacy alias input');
  requireMarker(migration, 'taxonomy_term_route_key::ActiveModel', 'Taxonomy route ownership backfill');
  requireMarker(migration, 'translation_change::ActiveModel', 'Taxonomy Translation change evidence');
  requireMarker(migration, 'taxonomy_category_hierarchy::ActiveModel', 'Taxonomy hierarchy backfill');
  requireMarker(migration, 'taxonomy_category_presentation::ActiveModel', 'Taxonomy presentation backfill');
  requireMarker(migration, 'taxonomy_category_id: Set(category.id)', 'same-ID typed Forum binding');
  requireMarker(migration, 'The backfill is intentionally monotonic', 'data-preserving rollback contract');
  rejectMarker(migration, 'drop_table', 'legacy or Taxonomy table deletion');
  rejectMarker(migration, 'delete_many()', 'destructive row cleanup');

  requireMarker(contracts, 'forum-taxonomy-category-backfill', 'CAT-5 fixture declaration');
  requireMarker(contracts, '"migration": "m20260823_000030_backfill_forum_categories_to_taxonomy"', 'fixture migration registration');
  requireMarker(contracts, '"mode": "fixture"', 'runtime backfill fixture mode');

  requireMarker(
    entitiesRegistry,
    'pub(crate) mod forum_category_translation;',
    'migration-only legacy Category Translation entity visibility',
  );
  rejectMarker(
    entitiesRegistry,
    'pub mod forum_category_translation;',
    'public legacy Category Translation entity export',
  );
  requireMarker(legacyCategory, 'pub parent_id: Option<Uuid>', 'Forum-owned compatibility hierarchy projection retained');
  rejectMarker(
    legacyCategory,
    'forum_category_translation',
    'retired Forum Category Translation runtime relation',
  );
  requireMarker(
    legacyTranslation,
    'forum_category_translations',
    'historical backfill compatibility entity retained for the published migration',
  );
  rejectMarker(forumServices, 'ForumCategoryTranslationTargetProvider', 'retired duplicate Forum Translation provider');
}

if (failures.length > 0) {
  console.error('[forum-taxonomy-category-backfill] boundary verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[forum-taxonomy-category-backfill] deterministic staged backfill boundary verified');
