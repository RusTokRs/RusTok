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

const migration =
  'crates/rustok-blog/src/migrations/m20260824_000019_add_blog_taxonomy_category_binding.rs';
const migrationRegistry = 'crates/rustok-blog/src/migrations/mod.rs';
const backfillContracts = 'docs/migrations/backfill-contracts.json';
const relation = 'crates/rustok-blog/src/entities/blog_category_taxonomy_binding.rs';
const runtimeTest = 'crates/rustok-blog/tests/category_taxonomy_binding.rs';
const entities = 'crates/rustok-blog/src/entities/mod.rs';
const legacyCategory = 'crates/rustok-blog/src/entities/blog_category.rs';
const legacyTranslation = 'crates/rustok-blog/src/entities/blog_category_translation.rs';

for (const path of [
  migration,
  migrationRegistry,
  backfillContracts,
  relation,
  runtimeTest,
  entities,
  legacyCategory,
  legacyTranslation,
]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  requireMarker(migration, 'blog_category_taxonomy_bindings', 'typed Blog→Taxonomy binding table');
  requireMarker(migration, 'fk_blog_category_taxonomy_binding_blog', 'Blog composite foreign key');
  requireMarker(
    migration,
    'fk_blog_category_taxonomy_binding_taxonomy',
    'Taxonomy composite foreign key',
  );
  requireMarker(
    migration,
    'uq_blog_category_taxonomy_binding_taxonomy',
    'one-to-one tenant binding index',
  );
  requireMarker(
    migration,
    '(TaxonomyTerms::TenantId, TaxonomyTerms::Id)',
    'tenant-safe Taxonomy identity target',
  );
  requireMarker(
    migrationRegistry,
    'm20260824_000019_add_blog_taxonomy_category_binding',
    'registered CAT-6 migration',
  );
  requireMarker(
    migrationRegistry,
    'm20260711_000001_add_tenant_identity_key',
    'Taxonomy tenant identity dependency',
  );
  requireMarker(
    migrationRegistry,
    'm20260812_000017_enforce_blog_category_hierarchy',
    'Blog tenant identity dependency',
  );
  requireMarker(
    backfillContracts,
    'blog-taxonomy-category-binding-bootstrap',
    'CAT-6 backfill declaration',
  );
  requireMarker(
    backfillContracts,
    '"migration": "m20260824_000019_add_blog_taxonomy_category_binding"',
    'CAT-6 migration backfill registration',
  );
  requireMarker(backfillContracts, '"mode": "none"', 'empty binding-table backfill mode');

  requireMarker(relation, 'BlogCategoryTaxonomyBindingService', 'bounded binding service');
  requireMarker(relation, 'taxonomy_term_identity_exists', 'Taxonomy owner identity validation');
  requireMarker(relation, 'TaxonomyTermKind::Category', 'Category-only owner validation');
  requireMarker(relation, 'same-tenant Taxonomy Category', 'cross-tenant fail-closed contract');
  requireMarker(
    relation,
    'already bound to a different Taxonomy Category',
    'no implicit rebind contract',
  );
  requireMarker(
    relation,
    'already bound to another Blog category',
    'one-to-one duplicate guard',
  );
  requireMarker(entities, 'pub mod blog_category_taxonomy_binding;', 'binding entity registration');

  requireMarker(
    runtimeTest,
    'blog_category_binding_is_category_only_tenant_bounded_and_one_to_one',
    'runtime binding contract',
  );
  requireMarker(
    runtimeTest,
    'repeating the same binding should be idempotent',
    'idempotent bind proof',
  );
  requireMarker(
    runtimeTest,
    'Taxonomy Tags must not masquerade as Categories',
    'wrong-kind runtime proof',
  );
  requireMarker(
    runtimeTest,
    'foreign-tenant Taxonomy Categories must fail closed',
    'foreign-tenant runtime proof',
  );
  requireMarker(
    runtimeTest,
    'stale Taxonomy Category identities must fail closed',
    'stale identity runtime proof',
  );

  rejectMarker(
    legacyCategory,
    'taxonomy_category_id',
    'binding state embedded in legacy Blog category row',
  );
  requireMarker(
    legacyCategory,
    'pub parent_id: Option<Uuid>',
    'legacy hierarchy retained during staged cutover',
  );
  requireMarker(
    legacyCategory,
    'pub settings: Json',
    'Blog-specific settings retained during staged cutover',
  );
  requireMarker(
    legacyCategory,
    'pub post_count: i32',
    'Blog-specific counters retained during staged cutover',
  );
  requireMarker(
    legacyTranslation,
    'blog_category_translations',
    'legacy localized copy retained until deterministic backfill',
  );
}

if (failures.length > 0) {
  console.error('[blog-taxonomy-category-binding] boundary verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[blog-taxonomy-category-binding] typed staged binding boundary verified');
