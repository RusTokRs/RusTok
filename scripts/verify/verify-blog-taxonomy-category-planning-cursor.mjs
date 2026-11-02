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
/// The cursor keeps superseded phrases verbatim in its marked archive section (see the
/// `## Superseded historical cursor phrases` heading), and the canonical-plan-current guard
/// enforces that they appear *only* there. The retirement checks below are about live text,
/// so they run against the document with that section removed. A missing section leaves the
/// document untouched, which keeps the rejection fail-closed.
const liveCursorText = (source) => {
  const start = source.indexOf('## Superseded historical cursor phrases');
  if (start < 0) return source;
  const end = source.indexOf('## Validation boundary', start);
  // Both boundaries are required: an unterminated archive section would otherwise hide
  // everything after it from the retirement checks.
  if (end < 0) return source;
  return source.slice(0, start) + source.slice(end);
};
const requireMarker = (source, marker, label = marker) => {
  if (!source.includes(marker)) failures.push(`missing ${label}`);
};
const rejectMarker = (source, marker, label = marker) => {
  if (source.includes(marker)) failures.push(`must not contain ${label}`);
};

const currentCursorPath = 'crates/modules/rustok-blog/docs/implementation-plan-current.md';
const crateReadmePath = 'crates/modules/rustok-blog/README.md';
const docsReadmePath = 'crates/modules/rustok-blog/docs/README.md';
const entitiesPath = 'crates/modules/rustok-blog/src/entities/mod.rs';

for (const path of [
  currentCursorPath,
  crateReadmePath,
  docsReadmePath,
  entitiesPath,
  'crates/modules/rustok-blog/docs/implementation-plan-slice-98.md',
  'crates/modules/rustok-blog/src/migrations/m20261029_000001_create_blog_schema.rs',
  'crates/modules/rustok-blog/tests/category_taxonomy_translation_provider_retirement.rs',
]) {
  requireFile(path);
}

for (const path of [
  'crates/modules/rustok-blog/src/translation_target.rs',
  'crates/modules/rustok-blog/src/translation_target_tests.rs',
  'crates/modules/rustok-blog/src/translation_evidence.rs',
  'crates/modules/rustok-blog/src/entities/translation_change.rs',
  'crates/modules/rustok-blog/tests/category_translation_target_postgres_test.rs',
  'crates/modules/rustok-blog/contracts/evidence/blog-category-translation-postgres-source.json',
  'scripts/verify/verify-blog-category-translation-postgres-source.mjs',
]) {
  requireAbsent(path);
}

if (failures.length === 0) {
  const cursor = read(currentCursorPath);
  const liveCursor = liveCursorText(cursor);
  const crateReadme = read(crateReadmePath);
  const docsReadme = read(docsReadmePath);
  const entities = read(entitiesPath);

  requireMarker(
    cursor,
    'canonical_reference_v1_source_certified_execution_evidence_pending',
    'current canonical reference cursor status',
  );
  requireMarker(cursor, 'blog_category_taxonomy_cutover = source_complete_through_cat12');
  requireMarker(
    cursor,
    'blog_category_documentation_cursor = owner_scoped_actualized_through_cat17',
  );
  for (const marker of [
    '- CAT-13: actualize the canonical Blog planning cursor',
    '- CAT-14: actualize cross-owner Taxonomy/Flex planning',
    '- CAT-15: actualize central/module Translation plans',
    '- CAT-16: actualize the central module registry',
    '- CAT-17: align the long-form Blog plan',
  ]) {
    requireMarker(cursor, marker);
  }
  requireMarker(cursor, 'blog_category_translation_provider = retired');
  requireMarker(
    cursor,
    'blog_category_translation_postgres_evidence = superseded_by_taxonomy_cutover',
  );
  requireMarker(cursor, 'There is **no** remaining execution item for the retired Blog Category');
  requireMarker(
    cursor,
    'There is no predeclared Blog Category Translation cleanup slice after CAT-17.',
  );
  rejectMarker(
    liveCursor,
    'canonical_source_cursor_actualized_through_taxonomy_cat_12`.',
    'stale CAT-12-only cursor status',
  );
  rejectMarker(
    liveCursor,
    'The next bounded source task is a cross-cutting documentation actualization:',
    'completed cross-cutting documentation next cursor',
  );
  rejectMarker(
    liveCursor,
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

  rejectMarker(entities, 'blog_category_translation', 'retired donor entity exposure');
  rejectMarker(entities, 'translation_change', 'retired change entity registration');
}

if (failures.length > 0) {
  console.error('[blog-taxonomy-category-planning-cursor] verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  '[blog-taxonomy-category-planning-cursor] CAT-17 docs cursor and CAT-12 source boundary verified',
);
