#!/usr/bin/env node

import fs from 'node:fs';

const migrationPath =
  'crates/rustok-product/src/migrations/m20260829_000016_backfill_product_categories_to_taxonomy.rs';
const registryPath = 'crates/rustok-product/src/migrations/mod.rs';
const contractPath = 'crates/rustok-product/docs/category-taxonomy-binding.md';
const localeContractPath = 'crates/rustok-product/docs/category-locale-contract.md';
const bindingPath =
  'crates/rustok-product/src/migrations/m20260828_000015_add_product_taxonomy_category_binding.rs';

const failures = [];
const need = (source, marker, label = marker) => {
  if (!source.includes(marker)) failures.push(`missing ${label}: ${marker}`);
};
const forbid = (source, marker, label = marker) => {
  if (source.includes(marker)) failures.push(`forbidden ${label}: ${marker}`);
};
const normalizeWhitespace = (source) => source.replace(/\s+/g, ' ').trim();

for (const path of [migrationPath, registryPath, contractPath, localeContractPath, bindingPath]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  const migration = fs.readFileSync(migrationPath, 'utf8');
  const registry = fs.readFileSync(registryPath, 'utf8');
  const contract = normalizeWhitespace(fs.readFileSync(contractPath, 'utf8'));
  const localeContract = normalizeWhitespace(fs.readFileSync(localeContractPath, 'utf8'));
  const binding = fs.readFileSync(bindingPath, 'utf8');

  for (const [marker, label] of [
    ['manager.get_database_backend() != DatabaseBackend::Postgres', 'PostgreSQL-only backfill boundary'],
    ['manager.get_connection().begin().await?', 'transactional backfill start'],
    ['txn.commit().await?', 'transactional backfill commit'],
    ['FROM catalog_categories', 'Product category donor read'],
    ['FROM catalog_category_translations', 'Product localized donor read'],
    ['const PRODUCT_SCOPE_VALUE: &str = "product";', 'Product Taxonomy module scope'],
    ['TaxonomyTermKind::Category', 'Taxonomy Category identity'],
    ['TaxonomyScopeType::Module', 'module-scoped Taxonomy identity'],
    ['format!("product-category-{category_id}")', 'same-ID Product canonical key'],
    ['taxonomy_term_translation::Entity::find_by_id(translation.id)', 'translation UUID collision guard'],
    ['slug: Set(route_key.clone())', 'base slug projected into every locale'],
    ['normalize_term_locale', 'canonical locale validation'],
    ['normalize_term_route_key', 'canonical route validation'],
    ['taxonomy_term_route_key::Entity::find()', 'route ownership collision guard'],
    ['translation_change::ActiveModel', 'Taxonomy Translation change evidence'],
    ['taxonomy_category_hierarchy::Entity::find_by_id', 'Taxonomy hierarchy collision guard'],
    ['ensure_product_binding(txn, category).await?', 'same-ID Product binding population'],
    ['INSERT INTO product_catalog_category_taxonomy_bindings', 'binding insert'],
    ['VALUES ($1, $2, $2, $3)', 'same-ID binding insert'],
    ['translations.is_empty()', 'missing localized copy fail-closed'],
    ['Every Taxonomy identity must exist before parent references are copied', 'identity-before-hierarchy ordering'],
    ['Bind only after identity/localized copy/routes/hierarchy are complete', 'binding-last ordering'],
    ['intentionally monotonic copy', 'monotonic rollback contract'],
  ]) {
    need(migration, marker, label);
  }

  for (const [marker, label] of [
    ['DROP TABLE catalog_categories', 'destructive Product category retirement'],
    ['DROP TABLE catalog_category_translations', 'destructive Product translation retirement'],
    ['DELETE FROM catalog_categories', 'Product category deletion'],
    ['UPDATE catalog_categories', 'Product runtime donor rewrite'],
    ['meta_title', 'premature Product SEO migration'],
    ['meta_description', 'premature Product SEO migration'],
    ['rule_config', 'premature virtual-category migration'],
    ['product/category', 'duplicate Product Translation provider'],
  ]) {
    forbid(migration, marker, label);
  }

  for (const [marker, label] of [
    ['mod m20260829_000016_backfill_product_categories_to_taxonomy;', 'migration module registration'],
    ['Box::new(m20260829_000016_backfill_product_categories_to_taxonomy::Migration)', 'migration execution registration'],
    ['"m20260829_000016_backfill_product_categories_to_taxonomy"', 'migration dependency descriptor'],
    ['"m20260822_000010_create_taxonomy_category_hierarchy"', 'Taxonomy Category storage dependency'],
    ['"m20260828_000015_add_product_taxonomy_category_binding"', 'Product binding dependency'],
  ]) {
    need(registry, marker, label);
  }

  need(binding, 'product_catalog_category_taxonomy_bindings', 'CAT-23 binding prerequisite');
  need(
    binding,
    'manager.get_database_backend() != DatabaseBackend::Postgres',
    'CAT-23 PostgreSQL binding boundary',
  );

  for (const [marker, label] of [
    ['Status: **source-complete monotonic backfill; Product runtime cutover pending**', 'CAT-24 bounded status'],
    ['same canonical base slug is therefore projected into every imported locale', 'explicit Product base-slug projection rule'],
    ['no localized slug is invented', 'no fabricated localized route data'],
    ['meta_title / meta_description', 'retained Product SEO ownership'],
    ['activation/soft-delete lifecycle', 'retained Product lifecycle ownership'],
    ['does **not** switch Product reads or writes', 'no runtime cutover boundary'],
    ['No `product/category` Translation provider is introduced', 'no duplicate Translation provider'],
    ['registered Taxonomy `taxonomy/term` provider', 'canonical Translation owner'],
    ['incompatible Taxonomy UUID, canonical-key, localized-copy, translation UUID, route, hierarchy or binding ownership blocks the migration', 'fail-closed collision contract'],
  ]) {
    need(contract, marker, label);
  }

  for (const [marker, label] of [
    ['Product-owned **runtime** tree/closure aggregate until a verified Taxonomy read/write cutover', 'runtime-vs-target hierarchy boundary'],
    ['CAT-24 backfills Product `parent_id` and `position` into the Taxonomy hierarchy', 'Taxonomy hierarchy shadow copy'],
    ['projects the same one base Product category `slug` into every imported locale', 'locale contract base-slug projection'],
    ['`meta_title` / `meta_description` stay Product-owned SEO data', 'locale contract retained SEO ownership'],
    ['does not make Product runtime consume it yet', 'locale contract no runtime cutover'],
  ]) {
    need(localeContract, marker, label);
  }
}

if (failures.length > 0) {
  console.error('[product-taxonomy-category-backfill] verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[product-taxonomy-category-backfill] monotonic Product Category backfill contract verified');
