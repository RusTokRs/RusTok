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
const productMigrationsModPath = 'crates/rustok-product/src/migrations/mod.rs';
const productLocaleExpansionMigrationPath =
  'crates/rustok-product/src/migrations/m20260405_000007_expand_product_locale_storage_columns.rs';
const productTenantConsistencyMigrationPath =
  'crates/rustok-product/src/migrations/m20260701_000002_add_product_catalog_tenant_consistency_constraints.rs';
const legacyProductMigrationPaths = [
  'crates/rustok-product/src/migrations/m20250130_000012_create_commerce_products.rs',
  'crates/rustok-product/src/migrations/m20250130_000013_create_commerce_options.rs',
  'crates/rustok-product/src/migrations/m20250130_000014_create_commerce_variants.rs',
];
const indexMigrationPath =
  'crates/rustok-index/src/migrations/m20260701_000001_create_index_product_attribute_facets.rs';
const schemaServicePath = 'crates/rustok-product/src/services/catalog_schema_service.rs';
const schemaResolverPath = 'crates/rustok-product/src/services/catalog_schema.rs';
const catalogServicePath = 'crates/rustok-product/src/services/catalog.rs';
const indexerPath = 'crates/rustok-index/src/product/indexer.rs';
const productTagsTestPath = 'crates/rustok-commerce/tests/product_taxonomy_tags.rs';
const shippingGraphqlTestPath = 'crates/rustok-commerce/tests/graphql_runtime_parity_test/shipping.rs';
const productEventTestPath = 'crates/rustok-commerce/tests/product_event_index_integration_test.rs';
const commerceMutationHelpersPath = 'crates/rustok-commerce/src/graphql/mutations/helpers.rs';
const commerceCatalogMutationPath = 'crates/rustok-commerce/src/graphql/mutations/catalog.rs';

const productMigration = read(productMigrationPath);
const productMigrationsMod = read(productMigrationsModPath);
const productLocaleExpansionMigration = read(productLocaleExpansionMigrationPath);
const productTenantConsistencyMigration = read(productTenantConsistencyMigrationPath);
const legacyProductMigrations = legacyProductMigrationPaths.map((path) => [path, read(path)]);
const indexMigration = read(indexMigrationPath);
const schemaService = read(schemaServicePath);
const schemaResolver = read(schemaResolverPath);
const catalogService = read(catalogServicePath);
const indexer = read(indexerPath);
const productTagsTest = read(productTagsTestPath);
const shippingGraphqlTest = read(shippingGraphqlTestPath);
const productEventTest = read(productEventTestPath);
const commerceMutationHelpers = read(commerceMutationHelpersPath);
const commerceCatalogMutation = read(commerceCatalogMutationPath);
const plan = read('crates/rustok-product/docs/implementation-plan.md');
const docsReadme = read('crates/rustok-product/docs/README.md');
const productReadme = read('crates/rustok-product/README.md');
const databaseDocs = read('docs/architecture/database.md');
const commerceReadme = read('crates/rustok-commerce/README.md');
const commerceDocsReadme = read('crates/rustok-commerce/docs/README.md');
const commerceCrateApi = read('crates/rustok-commerce/CRATE_API.md');
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
  'CREATE TABLE IF NOT EXISTS product_attribute_value_options (\n    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE',
  'CREATE TABLE IF NOT EXISTS product_variant_attribute_value_options (\n    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE',
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

for (const [path, source] of legacyProductMigrations) {
  if (source.includes('string_len(5)')) fail(`${path} still contains locale string_len(5)`);
}
for (const marker of [
  'mod m20260405_000007_expand_product_locale_storage_columns;',
  'Box::new(m20260405_000007_expand_product_locale_storage_columns::Migration)',
  'mod m20260701_000002_add_product_catalog_tenant_consistency_constraints;',
  'Box::new(m20260701_000002_add_product_catalog_tenant_consistency_constraints::Migration)',
]) {
  requireSource(productMigrationsMod, marker, productMigrationsModPath);
}
for (const marker of [
  'ALTER TABLE product_translations',
  'ALTER TABLE product_image_translations',
  'ALTER TABLE product_option_translations',
  'ALTER TABLE product_option_value_translations',
  'ALTER TABLE product_variant_translations',
  'ALTER COLUMN locale TYPE VARCHAR(32)',
  'shrinking locale columns can truncate valid BCP47-like tags',
]) {
  requireSource(productLocaleExpansionMigration, marker, productLocaleExpansionMigrationPath);
}
for (const marker of [
  'UPDATE product_variants pv',
  'ALTER TABLE product_variants',
  'ALTER COLUMN tenant_id SET NOT NULL',
  'ALTER TABLE product_attribute_value_options',
  'ADD COLUMN IF NOT EXISTS tenant_id UUID',
  'UPDATE product_attribute_value_options pavo',
  'ALTER TABLE product_variant_attribute_value_options',
  'UPDATE product_variant_attribute_value_options pvavo',
  'uq_products_tenant_id',
  'uq_product_variants_tenant_id',
  'uq_product_attributes_tenant_id',
  'uq_product_attribute_options_tenant_id',
  'uq_catalog_categories_tenant_id',
  'uq_product_attribute_schemas_tenant_id',
  'uq_product_attribute_schema_groups_tenant_id',
  'uq_category_attribute_groups_tenant_id',
  'uq_product_attribute_values_tenant_id',
  'uq_product_variant_attribute_values_tenant_id',
  'fk_products_primary_category_tenant',
  'ON DELETE SET NULL (primary_category_id)',
  'fk_product_variants_product_tenant',
  'fk_product_attribute_options_attribute_tenant',
  'fk_catalog_categories_parent_tenant',
  'ON DELETE SET NULL (parent_id)',
  'fk_catalog_category_closure_ancestor_tenant',
  'fk_catalog_category_closure_descendant_tenant',
  'fk_product_attribute_schema_attributes_attribute_tenant',
  'ON DELETE SET NULL (group_id)',
  'fk_category_attributes_attribute_tenant',
  'fk_product_categories_product_tenant',
  'fk_product_categories_category_tenant',
  'fk_virtual_category_product_assignments_product_tenant',
  'fk_product_attribute_values_product_tenant',
  'fk_product_attribute_value_options_option_tenant',
  'fk_product_variant_attribute_values_variant_tenant',
  'fk_product_variant_attribute_value_options_option_tenant',
  'ON DELETE SET NULL (schema_id)',
  'ON DELETE SET NULL (cloned_from_category_id)',
  'ON DELETE SET NULL (inherited_from_group_id)',
]) {
  requireSource(productTenantConsistencyMigration, marker, productTenantConsistencyMigrationPath);
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
  'let detached_attribute_ids = existing_value_attribute_ids',
  '.filter(|attribute_id| !effective_ids.contains(attribute_id))',
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
  'record.detached = detached_attribute_ids.contains(&record.attribute_id);',
  'product must have a primary structural category before attribute values can be saved',
  'attribute {} is outside the product effective schema',
  'option {} does not belong to attribute {} or is archived',
  'ProductAttributeValuePatchValue::Multiselect(option_ids) if option_ids.is_empty()',
  'DELETE FROM product_attribute_values WHERE tenant_id = $1 AND product_id = $2 AND attribute_id = $3',
  'detached_at = NULL',
  'INSERT INTO product_attribute_value_translations',
  'INSERT INTO product_attribute_value_options',
  'INSERT INTO product_attribute_value_options (tenant_id, value_id, option_id)',
  'attribute {} is not detached for this product',
]) {
  requireSource(schemaService, marker, schemaServicePath);
}

for (const marker of [
  'Primary category must be structural',
  'DomainEvent::ProductPrimaryCategoryChanged',
  'validate_product_publish_requirements',
  'validate_new_product_publish_requirements',
  'DomainEvent::ProductPublished { product_id }',
  'publish_in_tx',
]) {
  requireSource(catalogService, marker, catalogServicePath);
}

for (const marker of [
  'product_tags_are_synced_into_product_tags_without_metadata_mirror',
  'product_tag_sync_reuses_existing_global_taxonomy_term',
  'update_product_tags_resyncs_product_tag_relations_without_metadata_mirror',
  'update_product_tags_only_preserves_existing_non_tag_metadata',
  'legacy_metadata_tags_are_used_as_read_fallback_but_not_exposed_publicly',
  'assert!(product.metadata.get("tags").is_none())',
  'assert!(updated.metadata.get("tags").is_none())',
  '"tags": ["legacy", "sale", "legacy"]',
]) {
  requireSource(productTagsTest, marker, productTagsTestPath);
}
for (const marker of [
  'admin_graphql_rejects_unknown_shipping_profile_references',
  'storefront_graphql_shipping_options_filter_incompatible_shipping_profiles',
  'storefront_graphql_update_cart_context_rejects_incompatible_shipping_profile_option',
  'Unknown shipping profile slug: missing-profile',
  'allowed_shipping_profile_slugs: Some(vec!["default".to_string()])',
  'allowed_shipping_profile_slugs: Some(vec!["bulky".to_string()])',
  'shipping_profile_slug: Some("bulky".to_string())',
]) {
  requireSource(shippingGraphqlTest, marker, shippingGraphqlTestPath);
}
for (const marker of [
  'test_product_publishing_triggers_event',
  'publish_product(tenant_id, actor_id, product.id)',
  'SysEvents::find()',
  'Column::EventType.eq(event_type)',
  '"product.published"',
]) {
  requireSource(productEventTest, marker, productEventTestPath);
}
for (const marker of [
  'validate_product_shipping_profile_input',
  '.ensure_shipping_profile_slug_exists(tenant_id, &slug)',
  'effective_shipping_profile_slug(',
  'product_model.shipping_profile_slug.as_deref()',
  'variant.shipping_profile_slug.as_deref()',
]) {
  requireSource(commerceMutationHelpers, marker, commerceMutationHelpersPath);
}
for (const marker of [
  'validate_product_shipping_profile_input(',
  'input.shipping_profile_slug.as_deref()',
  'catalog.publish_product(tenant_id, user_id, id).await?',
]) {
  requireSource(commerceCatalogMutation, marker, commerceCatalogMutationPath);
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
  '`product_attributes`',
  '`catalog_categories`',
  '`product_attribute_schemas`',
  'reusable templates',
  'translation tables',
  'detached values',
  '`rustok-index`',
  'tenant/locale-scoped',
]) {
  requireSource(docsReadme, marker, 'crates/rustok-product/docs/README.md');
}
for (const marker of [
  'Product-owned relation storage for taxonomy-backed tags (`product_tags`)',
  'first-class `tags` contract fields',
  'first-class `shipping_profile_slug`',
  'nullable `seller_id`',
  'canonical marketplace',
  '`adminPricingProduct` /',
  '`storefrontPricingProduct`',
  'Product-owned catalog search metadata',
]) {
  requireSource(productReadme, marker, 'crates/rustok-product/README.md');
}
for (const marker of [
  '`rustok-taxonomy` + `product_tags`',
  'first-class',
  'legacy `metadata.tags`',
  '`variant.shipping_profile_slug -> product.shipping_profile_slug -> default`',
  'Transport-level validation for `shipping_profile_slug`',
  'active shipping profiles from the typed',
]) {
  requireSource(docsReadme, marker, 'crates/rustok-product/docs/README.md');
}
for (const marker of [
  'Expose first-class `shipping_profile_slug` on product and variant create/update/read contracts',
  'Resolve the effective shipping profile as `variant -> product -> default`',
  'cart/order line-item snapshots',
  'deliverability-aware cart and checkout contracts',
  'nullable `seller_id` as the canonical marketplace identity key',
]) {
  requireSource(commerceReadme, marker, 'crates/rustok-commerce/README.md');
}
for (const marker of [
  'For shipping profiles, the metadata-backed baseline is no longer the sole source of truth',
  '`products.shipping_profile_slug` and `product_variants.shipping_profile_slug` now live as typed persistence',
  'admin/storefront write-path now validates these references against the active typed shipping-profile registry',
  'line items, `cart_shipping_selections`, order line items and fulfillment metadata store canonical language-agnostic seller identity (`seller_id`)',
  'Generic catalog roots `product` / `storefrontProduct` should now be treated only as a catalog-authoritative surface',
]) {
  requireSource(commerceDocsReadme, marker, 'crates/rustok-commerce/docs/README.md');
}
for (const marker of [
  'Product create/update/list/detail contracts now expose first-class `tags`',
  '`metadata.tags` is no longer part of the supported public contract',
  'Owner service, DTO and entity contracts are imported directly from `rustok-product`',
]) {
  requireSource(commerceCrateApi, marker, 'crates/rustok-commerce/CRATE_API.md');
}
for (const marker of [
  '`product_attributes`, `product_attribute_translations`, `product_attribute_options`',
  '`catalog_categories`, `catalog_category_translations`, `catalog_category_closure`',
  '`product_attribute_values`, `product_variant_attribute_values`',
  '`index_product_attribute_values`',
  'detached values',
]) {
  requireSource(databaseDocs, marker, 'docs/architecture/database.md');
}

for (const marker of [
  'DB-level tenant consistency audit',
  '`VARCHAR(32)` locale storage',
  'optional catalog filters/sorts',
  '`VARCHAR(32)`',
  'detached-value marker contract',
  'no-compile schema guardrail',
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
