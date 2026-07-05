#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const root = process.env.PRODUCT_CATALOG_SCHEMA_ROOT
  ? pathToFileURL(`${resolve(process.env.PRODUCT_CATALOG_SCHEMA_ROOT)}/`)
  : new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const json = (path) => JSON.parse(read(path));
const fail = (message) => {
  console.error(`[verify-product-catalog-schema] ${message}`);
  process.exit(1);
};
const requireSource = (source, marker, label) => {
  if (!source.includes(marker)) fail(`${label} missing marker: ${marker}`);
};

const productMigrationPath =
  'crates/rustok-product/src/migrations/m20260701_000001_create_product_catalog_attributes.rs';
const indexMigrationPath =
  'crates/rustok-index/src/migrations/m20260701_000001_create_index_product_attribute_facets.rs';
const schemaServicePath = 'crates/rustok-product/src/services/catalog_schema_service.rs';
const schemaResolverPath = 'crates/rustok-product/src/services/catalog_schema.rs';
const catalogServicePath = 'crates/rustok-product/src/services/catalog.rs';
const indexerPath = 'crates/rustok-index/src/product/indexer.rs';

const productMigration = read(productMigrationPath);
const indexMigration = read(indexMigrationPath);
const schemaService = read(schemaServicePath);
const schemaResolver = read(schemaResolverPath);
const catalogService = read(catalogServicePath);
const indexer = read(indexerPath);
const plan = read('crates/rustok-product/docs/implementation-plan.md');
const docsReadme = read('crates/rustok-product/docs/README.md');
const databaseDocs = read('docs/architecture/database.md');
const packageJson = json('package.json');

for (const table of [
  'product_attributes',
  'product_attribute_translations',
  'product_attribute_options',
  'product_attribute_option_translations',
  'product_attribute_channel_settings',
  'catalog_categories',
  'catalog_category_translations',
  'catalog_category_closure',
  'product_attribute_schemas',
  'product_attribute_schema_translations',
  'product_attribute_schema_groups',
  'product_attribute_schema_group_translations',
  'product_attribute_schema_attributes',
  'category_attribute_schema_assignments',
  'category_attribute_groups',
  'category_attribute_group_translations',
  'category_attributes',
  'product_categories',
  'virtual_category_product_assignments',
  'product_attribute_values',
  'product_attribute_value_translations',
  'product_attribute_value_options',
  'product_variant_attribute_values',
  'product_variant_attribute_value_translations',
  'product_variant_attribute_value_options',
]) {
  requireSource(productMigration, `CREATE TABLE IF NOT EXISTS ${table} (`, productMigrationPath);
}

for (const marker of [
  'ALTER TABLE products',
  'ADD COLUMN IF NOT EXISTS primary_category_id UUID',
  'CONSTRAINT fk_products_primary_category',
  'REFERENCES catalog_categories(id)',
  'CONSTRAINT uq_product_attributes_tenant_code UNIQUE (tenant_id, code)',
  "value_type IN ('text', 'textarea', 'richtext', 'integer', 'decimal', 'boolean', 'date', 'datetime', 'select', 'multiselect', 'json')",
  "scope IN ('product', 'variant', 'both')",
  'locale VARCHAR(32) NOT NULL',
  "kind IN ('structural', 'collection', 'virtual')",
  'PRIMARY KEY (tenant_id, ancestor_id, descendant_id)',
  'CONSTRAINT chk_catalog_category_closure_depth CHECK (depth >= 0)',
  "mode IN ('inherit', 'use_schema', 'clone_from_category', 'custom')",
  "binding_kind IN ('addition', 'override', 'removal')",
  "assignment_kind IN ('primary', 'navigation', 'collection', 'virtual')",
  'snapshot JSONB NOT NULL DEFAULT',
  'detached_at TIMESTAMPTZ',
  'CONSTRAINT uq_product_attribute_values UNIQUE (tenant_id, product_id, attribute_id)',
  'CONSTRAINT uq_product_variant_attribute_values UNIQUE (tenant_id, variant_id, attribute_id)',
  'PRIMARY KEY (value_id, option_id)',
  'CREATE INDEX IF NOT EXISTS idx_products_primary_category',
  'CREATE INDEX IF NOT EXISTS idx_product_attributes_flags',
  'CREATE INDEX IF NOT EXISTS idx_catalog_category_closure_descendant',
  'CREATE INDEX IF NOT EXISTS idx_product_attribute_values_lookup',
  'CREATE INDEX IF NOT EXISTS idx_product_variant_attribute_values_lookup',
]) {
  requireSource(productMigration, marker, productMigrationPath);
}

const newCatalogLocaleColumns = productMigration.match(/locale VARCHAR\(32\) NOT NULL/g) ?? [];
if (newCatalogLocaleColumns.length < 8) {
  fail(`expected at least 8 VARCHAR(32) locale columns in native catalog migration, found ${newCatalogLocaleColumns.length}`);
}

for (const table of ['index_product_categories', 'index_product_attribute_values']) {
  requireSource(indexMigration, `CREATE TABLE IF NOT EXISTS ${table} (`, indexMigrationPath);
}
for (const marker of [
  'PRIMARY KEY (tenant_id, product_id, category_id, locale)',
  'locale VARCHAR(32) NOT NULL',
  'channel_id UUID',
  'attribute_code VARCHAR(128) NOT NULL',
  'facet_bucket_key VARCHAR(255)',
  'is_detached BOOLEAN NOT NULL DEFAULT FALSE',
  'CONSTRAINT uq_index_product_attribute_values UNIQUE',
  'WHERE is_filterable = TRUE AND is_detached = FALSE',
  'WHERE is_sortable = TRUE AND is_detached = FALSE',
  'WHERE is_searchable = TRUE AND is_detached = FALSE',
]) {
  requireSource(indexMigration, marker, indexMigrationPath);
}

for (const marker of [
  'pub enum CatalogCategoryKind',
  'Structural',
  'Collection',
  'Virtual',
  'pub enum CategorySchemaMode',
  'Inherit',
  'UseSchema',
  'CloneFromCategory',
  'Custom',
  'pub enum CategoryAttributeBindingKind',
  'Addition',
  'Override',
  'Removal',
  'resolve_effective_product_form',
  'SchemaResolutionError::NonStructuralPrimaryCategory',
  'resolve_category_attributes(parent_id, categories, schemas, visiting)?',
  'clone_snapshot',
  'apply_local_category_bindings',
  'binding.is_disabled = true',
  'detached_attribute_ids',
]) {
  requireSource(schemaResolver, marker, schemaResolverPath);
}

for (const marker of [
  'parse_virtual_category_rule_v1(&input.rule_config)',
  'validate_virtual_category_rule_references',
  'ensure_structural_category(&txn, tenant_id, input.category_id)',
  'load_effective_form_for_category(tenant_id, source_category_id, &[])',
  'serde_json::to_value(form.attributes)',
  'ProductAttributeValuesChanged',
  'load_effective_product_form_from_storage',
  'product must have a primary structural category before attribute values can be saved',
  'attribute {} is outside the product effective schema',
  'option {} does not belong to attribute {} or is archived',
  'ProductAttributeValuePatchValue::Multiselect(option_ids) if option_ids.is_empty()',
  'DELETE FROM product_attribute_values WHERE tenant_id = $1 AND product_id = $2 AND attribute_id = $3',
  'INSERT INTO product_attribute_value_translations',
  'INSERT INTO product_attribute_value_options',
  'attribute {} is not detached for this product',
]) {
  requireSource(schemaService, marker, schemaServicePath);
}

for (const marker of [
  'Primary category must be structural',
  'DomainEvent::ProductPrimaryCategoryChanged',
  'validate_product_publish_requirements',
]) {
  requireSource(catalogService, marker, catalogServicePath);
}

for (const marker of [
  'load_effective_product_form_from_storage',
  'refresh_virtual_category_assignments',
  'virtual_category_rule_matches',
  'virtual_category_product_assignments',
  'index_product_categories',
  'index_product_attribute_values',
  'is_detached',
  'is_filterable',
  'is_searchable',
  'is_sortable',
  'channel_id',
]) {
  requireSource(indexer, marker, indexerPath);
}

for (const marker of [
  '`product_attributes` является единым справочником',
  '`catalog_categories` хранит structural, collection и virtual',
  '`product_attribute_schemas` являются опциональными reusable templates',
  'localized labels и text-like values вынесены в translation tables',
  'detached values сохраняются и возвращаются отдельным маркером',
  '`rustok-index` при индексации товара материализует tenant/locale-scoped строки категорий',
]) {
  requireSource(docsReadme, marker, 'crates/rustok-product/docs/README.md');
}
for (const marker of [
  '`product_attributes`, `product_attribute_translations`, `product_attribute_options`',
  '`catalog_categories`, `catalog_category_translations`, `catalog_category_closure`',
  '`product_attribute_values`, `product_variant_attribute_values`',
  '`index_product_attribute_values`',
  'не индексирует detached values',
]) {
  requireSource(databaseDocs, marker, 'docs/architecture/database.md');
}

for (const marker of [
  'DB-level tenant consistency audit',
  'Нормализовать оставшиеся legacy product locale columns до `VARCHAR(32)`',
  'detached-value marker contract',
  'быстрый no-compile schema guardrail',
]) {
  requireSource(plan, marker, 'crates/rustok-product/docs/implementation-plan.md');
}

const scripts = packageJson.scripts ?? {};
if (scripts['verify:product:catalog-schema'] !== 'node scripts/verify/verify-product-catalog-schema.mjs') {
  fail('package.json product catalog schema verify script drift');
}
if (scripts['test:verify:product:catalog-schema'] !== 'node scripts/verify/verify-product-catalog-schema.test.mjs') {
  fail('package.json product catalog schema fixture test script drift');
}
if (!scripts['verify:ecommerce:fba']?.includes('npm run verify:product:catalog-schema')) {
  fail('package.json ecommerce FBA verify aggregate lacks product catalog schema guardrail');
}
if (!scripts['test:verify:ecommerce:fba']?.includes('npm run test:verify:product:catalog-schema')) {
  fail('package.json ecommerce FBA fixture aggregate lacks product catalog schema guardrail test');
}

console.log('[verify-product-catalog-schema] Product catalog schema/write-read model invariants are source-locked');
