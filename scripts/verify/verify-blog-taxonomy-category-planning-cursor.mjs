#!/usr/bin/env node

import fs from 'node:fs';

const failures = [];
const read = (path) => fs.readFileSync(path, 'utf8');
const requireFile = (path) => {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
};
const requireAbsent = (path) => {
  if (fs.existsSync(path)) failures.push(`${path}: retired file must remain absent`);
};
const requireMarker = (source, marker, label = marker) => {
  if (!source.includes(marker)) failures.push(`missing ${label}`);
};
const rejectMarker = (source, marker, label = marker) => {
  if (source.includes(marker)) failures.push(`must not contain ${label}`);
};

const currentCursorPath = 'crates/rustok-blog/docs/implementation-plan-current.md';
const crateReadmePath = 'crates/rustok-blog/README.md';
const docsReadmePath = 'crates/rustok-blog/docs/README.md';
const entitiesPath = 'crates/rustok-blog/src/entities/mod.rs';

for (const path of [
  currentCursorPath,
  crateReadmePath,
  docsReadmePath,
  entitiesPath,
  'crates/rustok-blog/docs/implementation-plan-slice-98.md',
  'crates/rustok-blog/src/migrations/m20260824_000020_backfill_blog_categories_to_taxonomy.rs',
  'crates/rustok-blog/src/migrations/m20260828_000021_retire_blog_category_legacy_storage.rs',
  'crates/rustok-blog/src/entities/blog_category_translation.rs',
  'crates/rustok-blog/tests/category_taxonomy_translation_provider_retirement.rs',
]) {
  requireFile(path);
}

for (const path of [
  'crates/rustok-blog/src/translation_target.rs',
  'crates/rustok-blog/src/translation_target_tests.rs',
  'crates/rustok-blog/src/translation_evidence.rs',
  'crates/rustok-blog/src/entities/translation_change.rs',
  'crates/rustok-blog/tests/category_translation_target_postgres_test.rs',
  'crates/rustok-blog/contracts/evidence/blog-category-translation-postgres-source.json',
  'scripts/verify/verify-blog-category-translation-postgres-source.mjs',
]) {
  requireAbsent(path);
}

if (failures.length === 0) {
  const cursor = read(currentCursorPath);
  const crateReadme = read(crateReadmePath);
  const docsReadme = read(docsReadmePath);
  const entities = read(entitiesPath);

  requireMarker(
    cursor,
    'canonical_source_cursor_actualized_through_taxonomy_cat_12',
    'CAT-12 current cursor status',
  );
  requireMarker(cursor, 'blog_category_taxonomy_cutover = source_complete_through_cat12');
  requireMarker(cursor, 'blog_category_translation_provider = retired');
  requireMarker(
    cursor,
    'blog_category_translation_postgres_evidence = superseded_by_taxonomy_cutover',
  );
  requireMarker(cursor, 'There is **no** remaining execution item for the retired Blog Category');
  rejectMarker(
    cursor,
    'category_translation_postgres = source_ready_maintainer_execution_pending',
    'superseded Blog Category provider execution gate',
  );

  requireMarker(crateReadme, 'The former Blog-owned Category Translation target has been retired.');
  requireMarker(crateReadme, '`blog_categories` remains Blog-owned for module membership');
  requireMarker(crateReadme, '`blog_category_translations` and `blog_translation_changes`');
  rejectMarker(
    crateReadme,
    'Expose the owner-side `blog/category` Translation target',
    'retired provider responsibility',
  );

  requireMarker(docsReadme, '`BlogCategoryTranslationTargetProvider` is retired');
  requireMarker(docsReadme, 'migration `000021` fails closed');
  requireMarker(docsReadme, 'post `category_name` projection reads the canonical Taxonomy Category label');
  rejectMarker(
    docsReadme,
    '`blog_translation_changes` is Blog\'s append-only',
    'retired Blog owner journal contract',
  );

  requireMarker(
    entities,
    'pub(crate) mod blog_category_translation;',
    'crate-private historical donor entity',
  );
  rejectMarker(entities, 'pub mod blog_category_translation;', 'public donor entity exposure');
  rejectMarker(entities, 'translation_change', 'retired change entity registration');
}

if (failures.length > 0) {
  console.error('[blog-taxonomy-category-planning-cursor] verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  '[blog-taxonomy-category-planning-cursor] CAT-12 Blog cursor and retired provider boundary verified',
);
