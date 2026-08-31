#!/usr/bin/env node

import fs from 'node:fs';

const migrationPath =
  'crates/rustok-product/src/migrations/m20260828_000015_add_product_taxonomy_category_binding.rs';
const registryPath = 'crates/rustok-product/src/migrations/mod.rs';
const contractPath = 'crates/rustok-product/docs/category-taxonomy-binding.md';
const tenantConstraintPath =
  'crates/rustok-product/src/migrations/m20260701_000002_add_product_catalog_tenant_consistency_constraints.rs';

const failures = [];
const need = (source, marker, label = marker) => {
  if (!source.includes(marker)) failures.push(`missing ${label}: ${marker}`);
};
const forbid = (source, marker, label = marker) => {
  if (source.includes(marker)) failures.push(`forbidden ${label}: ${marker}`);
};
const occurrences = (source, marker) => source.split(marker).length - 1;
const normalizeWhitespace = (source) => source.replace(/\s+/g, ' ').trim();

for (const path of [migrationPath, registryPath, contractPath, tenantConstraintPath]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  const migration = fs.readFileSync(migrationPath, 'utf8');
  const registry = fs.readFileSync(registryPath, 'utf8');
  const contract = fs.readFileSync(contractPath, 'utf8');
  const normalizedContract = normalizeWhitespace(contract);
  const tenantConstraints = fs.readFileSync(tenantConstraintPath, 'utf8');
  const postgresGuard = 'manager.get_database_backend() != DatabaseBackend::Postgres';

  for (const [marker, label] of [
    ['product_catalog_category_taxonomy_bindings', 'binding table'],
    ['CatalogCategoryId', 'Product catalog category identity'],
    ['TaxonomyCategoryId', 'Taxonomy category identity'],
    ['CatalogCategories::TenantId, CatalogCategories::Id', 'tenant-safe Product target'],
    ['TaxonomyTerms::TenantId, TaxonomyTerms::Id', 'tenant-safe Taxonomy target'],
    ['fk_product_catalog_category_taxonomy_binding_product', 'Product composite FK'],
    ['fk_product_catalog_category_taxonomy_binding_taxonomy', 'Taxonomy composite FK'],
    ['uq_product_catalog_category_taxonomy_binding_taxonomy', 'one-to-one Taxonomy binding'],
    ['ForeignKeyAction::Restrict', 'Taxonomy delete protection'],
    ['ForeignKeyAction::Cascade', 'Product binding lifecycle'],
    ['use sea_orm_migration::sea_orm::DatabaseBackend;', 'explicit database backend boundary'],
  ]) {
    need(migration, marker, label);
  }

  if (occurrences(migration, postgresGuard) < 2) {
    failures.push('binding migration must guard both up and down as PostgreSQL-only');
  }

  for (const [marker, label] of [
    ['DROP TABLE catalog_categories', 'destructive Product Category cutover'],
    ['DROP TABLE catalog_category_translations', 'destructive Product translation cutover'],
    ['DELETE FROM catalog_categories', 'Product Category data deletion'],
    ['INSERT INTO taxonomy_terms', 'premature Taxonomy backfill'],
  ]) {
    forbid(migration, marker, label);
  }

  need(
    registry,
    'mod m20260828_000015_add_product_taxonomy_category_binding;',
    'migration module registration',
  );
  need(
    registry,
    'Box::new(m20260828_000015_add_product_taxonomy_category_binding::Migration)',
    'migration execution registration',
  );
  need(
    registry,
    '"m20260828_000015_add_product_taxonomy_category_binding"',
    'migration dependency descriptor',
  );
  need(
    registry,
    '"m20260701_000002_add_product_catalog_tenant_consistency_constraints"',
    'Product tenant-consistency dependency',
  );
  need(
    registry,
    '"m20260711_000001_add_tenant_identity_key"',
    'Taxonomy tenant-identity dependency',
  );
  need(
    tenantConstraints,
    'uq_catalog_categories_tenant_id UNIQUE (tenant_id, id)',
    'Product composite category identity prerequisite',
  );
  need(
    tenantConstraints,
    postgresGuard,
    'Product composite category identity prerequisite PostgreSQL boundary',
  );

  for (const [marker, label] of [
    ['Status: **source-complete additive seam; backfill and runtime cutover pending**', 'bounded seam status'],
    ['does **not** backfill the binding and does **not** switch Product reads or writes', 'no-runtime-cutover boundary'],
    ['preserving Product category UUIDs where possible', 'same-ID backfill intent'],
    ['No `product/category` Translation provider should be introduced', 'no duplicate Translation provider'],
    ['registered Taxonomy `taxonomy/term` provider', 'canonical Translation owner'],
    ['physical binding table is currently created only on PostgreSQL', 'documented PostgreSQL storage boundary'],
  ]) {
    need(normalizedContract, marker, label);
  }
}

if (failures.length > 0) {
  console.error('[product-taxonomy-category-binding] verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[product-taxonomy-category-binding] additive tenant-safe PostgreSQL binding seam verified');
