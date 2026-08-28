#!/usr/bin/env node

import fs from 'node:fs';

const failures = [];
const read = (path) => fs.readFileSync(path, 'utf8');
const requireMarker = (source, marker, label = marker) => {
  if (!source.includes(marker)) failures.push(`missing ${label}`);
};
const rejectMarker = (source, marker, label = marker) => {
  if (source.includes(marker)) failures.push(`must not contain ${label}`);
};

const platformPlanPath = 'docs/architecture/taxonomy-flex-category-platform-plan.md';
const databasePath = 'docs/architecture/database.md';
const adrPath = 'DECISIONS/2026-08-22-taxonomy-category-flex-ownership.md';
const blogCursorPath = 'crates/rustok-blog/docs/implementation-plan-current.md';

for (const path of [platformPlanPath, databasePath, adrPath, blogCursorPath]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  const platformPlan = read(platformPlanPath);
  const database = read(databasePath);
  const adr = read(adrPath);
  const blogCursor = read(blogCursorPath);

  requireMarker(
    platformPlan,
    'The Blog migration is complete through TAXONOMY-CAT-12',
    'completed Blog consumer migration status',
  );
  requireMarker(
    platformPlan,
    'The former Blog Category Translation provider, live donor mirror/journal, and their runtime source files are retired.',
    'retired Blog provider boundary',
  );
  requireMarker(
    platformPlan,
    'Blog has completed this consumer migration through\nTAXONOMY-CAT-12.',
    'Phase D Blog completion',
  );
  rejectMarker(
    platformPlan,
    'Product and Blog follow with their own domain-specific binding semantics',
    'stale Blog-pending migration wording',
  );

  requireMarker(
    database,
    'm20260824_000020_backfill_blog_categories_to_taxonomy',
    'historical Blog Category Taxonomy backfill',
  );
  requireMarker(
    database,
    'm20260828_000021_retire_blog_category_legacy_storage',
    'Blog Category donor storage retirement migration',
  );
  requireMarker(
    database,
    'The former Blog `blog/category` Translation provider and Blog Category change\njournal are retired.',
    'retired Blog Translation provider storage boundary',
  );
  requireMarker(
    database,
    'canonical Category localized `name`, `slug`, `description`, route aliases and\n  hierarchy projection live in Taxonomy-owned Category storage',
    'Taxonomy-owned canonical Category storage',
  );
  rejectMarker(
    database,
    '`blog_translation_changes` is Blog\'s append-only, content-free owner change',
    'retired Blog change-journal contract',
  );
  rejectMarker(
    database,
    '`blog_category_translations` owns exact `name`',
    'retired Blog Category translation source-of-truth claim',
  );

  requireMarker(
    adr,
    'Do not register a duplicate `forum/category`, `blog/category` or `product/category` Translation',
    'accepted no-duplicate-provider ADR',
  );
  requireMarker(blogCursor, 'blog_category_translation_provider = retired');
}

if (failures.length > 0) {
  console.error('[blog-taxonomy-category-cross-owner-docs] verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  '[blog-taxonomy-category-cross-owner-docs] platform plan and database summary match CAT-12 ownership',
);
