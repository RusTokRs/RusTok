#!/usr/bin/env node

import fs from 'node:fs';

const failures = [];
const read = (path) => fs.readFileSync(path, 'utf8');
const normalizeWhitespace = (source) => source.replace(/\s+/g, ' ').trim();
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
const taxonomyPlanPath = 'crates/rustok-taxonomy/docs/implementation-plan.md';

for (const path of [platformPlanPath, databasePath, adrPath, blogCursorPath, taxonomyPlanPath]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  const platformPlan = read(platformPlanPath);
  const normalizedPlatformPlan = normalizeWhitespace(platformPlan);
  const database = read(databasePath);
  const adr = read(adrPath);
  const blogCursor = read(blogCursorPath);
  const taxonomyPlan = read(taxonomyPlanPath);
  const normalizedTaxonomyPlan = normalizeWhitespace(taxonomyPlan);

  requireMarker(
    platformPlan,
    'The Blog migration is complete through TAXONOMY-CAT-12',
    'completed Blog consumer migration status',
  );
  requireMarker(
    normalizedPlatformPlan,
    'The former Blog Category Translation provider, live donor mirror/journal, and their runtime source files are retired.',
    'retired Blog provider boundary',
  );
  requireMarker(
    platformPlan,
    'Blog has completed this consumer migration through\nTAXONOMY-CAT-12.',
    'Phase D Blog completion',
  );
  requireMarker(
    normalizedPlatformPlan,
    'Product PostgreSQL follows the same ownership model and is source-complete through TAXONOMY-CAT-34.',
    'Product PostgreSQL CAT-34 architecture cursor',
  );
  requireMarker(
    normalizedPlatformPlan,
    'No TAXONOMY-CAT-35 Product slice or next Category consumer is currently accepted by this plan.',
    'no invented Product CAT-35 or later-consumer cursor',
  );
  requireMarker(
    normalizedPlatformPlan,
    'Forum established the first consumer migration precedent and its backend ownership/storage cutover is complete.',
    'Forum backend completion architecture status',
  );
  requireMarker(
    normalizedPlatformPlan,
    '`taxonomy.category`: active explicit Flex donor after TAXONOMY-CAT-4.',
    'Taxonomy Category active Flex donor architecture status',
  );
  rejectMarker(
    platformPlan,
    'Product and Blog follow with their own domain-specific binding semantics',
    'stale Blog-pending migration wording',
  );
  rejectMarker(
    normalizedPlatformPlan,
    'Product remains a separate consumer migration with its own domain-specific binding semantics.',
    'stale Product-pending architecture wording',
  );
  rejectMarker(
    normalizedPlatformPlan,
    'Product follows with its own domain-specific binding semantics rather than a blind table rename.',
    'stale Product-future Phase D wording',
  );
  rejectMarker(
    normalizedPlatformPlan,
    '`taxonomy.category`: add after the Taxonomy Category owner exists.',
    'stale future Taxonomy Category Flex donor wording',
  );

  requireMarker(
    normalizedTaxonomyPlan,
    '### Blog consumer cutover — COMPLETE',
    'Taxonomy live plan completed Blog consumer cutover heading',
  );
  requireMarker(
    normalizedTaxonomyPlan,
    'The Blog Category source/storage cutover is complete through TAXONOMY-CAT-12',
    'Taxonomy live plan Blog source/storage completion',
  );
  requireMarker(
    normalizedTaxonomyPlan,
    'The owner-scoped Blog documentation cursor is actualized through TAXONOMY-CAT-17',
    'Taxonomy live plan Blog documentation cursor',
  );
  requireMarker(
    normalizedTaxonomyPlan,
    '### TAXONOMY-CAT-6 — Product and later consumers — IN PROGRESS',
    'Taxonomy live plan remaining consumer cursor',
  );
  requireMarker(
    normalizedTaxonomyPlan,
    '**Current Product cursor: TAXONOMY-CAT-34.**',
    'Taxonomy live plan current Product cursor',
  );
  rejectMarker(
    normalizedTaxonomyPlan,
    '### TAXONOMY-CAT-6 — Blog/Product and later consumers — PLANNED',
    'stale Blog-pending Taxonomy CAT-6 heading',
  );
  rejectMarker(
    normalizedTaxonomyPlan,
    'Product and Blog follow Forum',
    'stale Blog-pending Taxonomy consumer wording',
  );
  rejectMarker(
    normalizedTaxonomyPlan,
    'Product remains the next Category consumer migration',
    'stale Product-next Taxonomy consumer wording',
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
  '[blog-taxonomy-category-cross-owner-docs] platform, Taxonomy live plan and database summary match completed Category ownership',
);
