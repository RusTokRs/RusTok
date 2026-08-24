#!/usr/bin/env node

import fs from 'node:fs';

const migration =
  'crates/rustok-blog/src/migrations/m20260824_000020_backfill_blog_categories_to_taxonomy.rs';
const registry = 'crates/rustok-blog/src/migrations/mod.rs';
const contracts = 'docs/migrations/backfill-contracts.json';
const source = fs.readFileSync(migration, 'utf8');
const registrySource = fs.readFileSync(registry, 'utf8');
const contractSource = fs.readFileSync(contracts, 'utf8');
const failures = [];

const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`missing ${label}: ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`forbidden ${label}: ${marker}`);
};

for (const [marker, label] of [
  ['const BLOG_SCOPE_VALUE: &str = "blog"', 'module/blog scope'],
  ['TaxonomyTermKind::Category', 'Category kind'],
  ['canonical_key_for_blog_category', 'deterministic canonical key'],
  ['id: Set(category.id)', 'same UUID Taxonomy identity'],
  ['blog_category_translation::Entity::find()', 'legacy localized copy donor'],
  ['normalize_term_locale', 'canonical locale validation'],
  ['normalize_term_route_key', 'canonical route validation'],
  ['taxonomy_term_route_key::ActiveModel', 'Taxonomy route ownership'],
  ['taxonomy_category_hierarchy::ActiveModel', 'Taxonomy hierarchy ownership'],
  ['parent_term_id: Set(category.parent_id)', 'parent preservation'],
  ['position: Set(category.position)', 'position preservation'],
  ['blog_category_taxonomy_binding::ActiveModel', 'same-ID binding'],
  ['taxonomy_term_translation::Entity::find_by_id(translation.id)', 'translation UUID collision guard'],
  ['already owned by an incompatible Taxonomy term', 'identity collision fail-closed guard'],
  ['has no localized copy', 'missing-copy fail-closed guard'],
  ['route {locale}/{route_key} is already owned', 'route collision fail-closed guard'],
  ['async fn down(&self, _manager: &SchemaManager)', 'monotonic rollback boundary'],
]) {
  need(source, marker, label);
}

for (const [marker, label] of [
  ['category.settings', 'Blog-specific settings transfer'],
  ['category.post_count', 'Blog-specific counter transfer'],
  ['taxonomy_category_presentation', 'invented presentation transfer'],
  ['taxonomy_term_alias', 'fabricated alias history'],
]) {
  forbid(source, marker, label);
}

need(
  registrySource,
  'm20260824_000020_backfill_blog_categories_to_taxonomy',
  'registered CAT-6 backfill migration',
);
need(
  registrySource,
  'm20260822_000010_create_taxonomy_category_hierarchy',
  'Taxonomy hierarchy dependency',
);
need(
  registrySource,
  'm20260824_000019_add_blog_taxonomy_category_binding',
  'Blog binding dependency',
);
need(
  contractSource,
  'blog-taxonomy-category-backfill',
  'fixture backfill contract',
);
need(
  contractSource,
  '"migration": "m20260824_000020_backfill_blog_categories_to_taxonomy"',
  'backfill contract migration registration',
);

if (failures.length) {
  console.error('[blog-taxonomy-category-backfill] verification failed');
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

console.log('[blog-taxonomy-category-backfill] deterministic staged backfill verified');
