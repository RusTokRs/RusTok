import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const repoRoot = process.cwd();
const script = path.join(repoRoot, 'scripts/verify/verify-product-catalog-schema.mjs');
const fixtureFiles = [
  'crates/rustok-product/src/migrations/m20260701_000001_create_product_catalog_attributes.rs',
  'crates/rustok-product/src/migrations/mod.rs',
  'crates/rustok-product/src/migrations/m20260405_000007_expand_product_locale_storage_columns.rs',
  'crates/rustok-product/src/migrations/m20260701_000002_add_product_catalog_tenant_consistency_constraints.rs',
  'crates/rustok-product/src/migrations/m20250130_000012_create_commerce_products.rs',
  'crates/rustok-product/src/migrations/m20250130_000013_create_commerce_options.rs',
  'crates/rustok-product/src/migrations/m20250130_000014_create_commerce_variants.rs',
  'crates/rustok-index/src/migrations/m20260701_000001_create_index_product_attribute_facets.rs',
  'crates/rustok-product/src/services/catalog_schema_service.rs',
  'crates/rustok-product/src/services/catalog_schema.rs',
  'crates/rustok-product/src/services/catalog.rs',
  'crates/rustok-index/src/product/indexer.rs',
  'crates/rustok-commerce/tests/product_taxonomy_tags.rs',
  'crates/rustok-commerce/tests/graphql_runtime_parity_test/shipping.rs',
  'crates/rustok-commerce/tests/product_event_index_integration_test.rs',
  'crates/rustok-commerce/src/graphql/mutations/helpers.rs',
  'crates/rustok-commerce/src/graphql/mutations/catalog.rs',
  'crates/rustok-product/docs/implementation-plan.md',
  'crates/rustok-product/README.md',
  'crates/rustok-product/docs/README.md',
  'docs/architecture/database.md',
  'crates/rustok-commerce/README.md',
  'crates/rustok-commerce/docs/README.md',
  'crates/rustok-commerce/CRATE_API.md',
  'package.json',
];

function assert(condition, message) {
  if (!condition) {
    console.error(`[verify-product-catalog-schema.test] ${message}`);
    process.exit(1);
  }
}

function run(root = repoRoot) {
  return spawnSync(process.execPath, [script], {
    cwd: repoRoot,
    env: { ...process.env, PRODUCT_CATALOG_SCHEMA_ROOT: root },
    encoding: 'utf8',
  });
}

function copyFixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'product-catalog-schema-'));
  for (const file of fixtureFiles) {
    const target = path.join(root, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(repoRoot, file), target);
  }
  return root;
}

function replaceInFixture(root, file, search, replacement) {
  const fullPath = path.join(root, file);
  const original = fs.readFileSync(fullPath, 'utf8');
  assert(original.includes(search), `fixture source did not contain expected marker ${search}`);
  fs.writeFileSync(fullPath, original.replace(search, replacement));
}

const success = run();
assert(
  success.status === 0,
  `expected repository fixture to pass\nSTDOUT:\n${success.stdout}\nSTDERR:\n${success.stderr}`,
);

const missingValueOptions = copyFixture();
replaceInFixture(
  missingValueOptions,
  'crates/rustok-product/src/migrations/m20260701_000001_create_product_catalog_attributes.rs',
  'CREATE TABLE IF NOT EXISTS product_attribute_value_options',
  'CREATE TABLE IF NOT EXISTS product_attribute_value_options_drift',
);
const missingValueOptionsResult = run(missingValueOptions);
assert(missingValueOptionsResult.status !== 0, 'expected missing multiselect value option table to fail');
assert(
  missingValueOptionsResult.stderr.includes('product_attribute_value_options'),
  `expected value option table failure, got ${missingValueOptionsResult.stderr}`,
);

const localeWidthDrift = copyFixture();
replaceInFixture(
  localeWidthDrift,
  'crates/rustok-product/src/migrations/m20260701_000001_create_product_catalog_attributes.rs',
  'locale VARCHAR(32) NOT NULL',
  'locale VARCHAR(5) NOT NULL',
);
const localeWidthResult = run(localeWidthDrift);
assert(localeWidthResult.status !== 0, 'expected native catalog locale width drift to fail');
assert(
  localeWidthResult.stderr.includes('VARCHAR(32) locale columns'),
  `expected locale width failure, got ${localeWidthResult.stderr}`,
);

const missingPartialFacetIndex = copyFixture();
replaceInFixture(
  missingPartialFacetIndex,
  'crates/rustok-index/src/migrations/m20260701_000001_create_index_product_attribute_facets.rs',
  'WHERE is_filterable = TRUE AND is_detached = FALSE',
  'WHERE is_filterable = TRUE',
);
const missingPartialFacetIndexResult = run(missingPartialFacetIndex);
assert(missingPartialFacetIndexResult.status !== 0, 'expected missing detached facet partial index guard to fail');
assert(
  missingPartialFacetIndexResult.stderr.includes('WHERE is_filterable = TRUE AND is_detached = FALSE'),
  `expected partial facet index failure, got ${missingPartialFacetIndexResult.stderr}`,
);

const legacyLocaleDrift = copyFixture();
replaceInFixture(
  legacyLocaleDrift,
  'crates/rustok-product/src/migrations/m20250130_000012_create_commerce_products.rs',
  '.string_len(32)',
  '.string_len(5)',
);
const legacyLocaleDriftResult = run(legacyLocaleDrift);
assert(legacyLocaleDriftResult.status !== 0, 'expected legacy locale width drift to fail');
assert(
  legacyLocaleDriftResult.stderr.includes('still contains locale string_len(5)'),
  `expected legacy locale width failure, got ${legacyLocaleDriftResult.stderr}`,
);

const missingLocaleExpansionMigration = copyFixture();
replaceInFixture(
  missingLocaleExpansionMigration,
  'crates/rustok-product/src/migrations/mod.rs',
  'mod m20260405_000007_expand_product_locale_storage_columns;',
  '// locale expansion migration drift',
);
const missingLocaleExpansionMigrationResult = run(missingLocaleExpansionMigration);
assert(missingLocaleExpansionMigrationResult.status !== 0, 'expected missing locale expansion migration wiring to fail');
assert(
  missingLocaleExpansionMigrationResult.stderr.includes('m20260405_000007_expand_product_locale_storage_columns'),
  `expected locale expansion migration wiring failure, got ${missingLocaleExpansionMigrationResult.stderr}`,
);

const missingTenantConsistencyMigration = copyFixture();
replaceInFixture(
  missingTenantConsistencyMigration,
  'crates/rustok-product/src/migrations/mod.rs',
  'mod m20260701_000002_add_product_catalog_tenant_consistency_constraints;',
  '// tenant consistency migration drift',
);
const missingTenantConsistencyMigrationResult = run(missingTenantConsistencyMigration);
assert(missingTenantConsistencyMigrationResult.status !== 0, 'expected missing tenant consistency migration wiring to fail');
assert(
  missingTenantConsistencyMigrationResult.stderr.includes('m20260701_000002_add_product_catalog_tenant_consistency_constraints'),
  `expected tenant consistency migration wiring failure, got ${missingTenantConsistencyMigrationResult.stderr}`,
);

const missingTenantAwareValueOptionInsert = copyFixture();
replaceInFixture(
  missingTenantAwareValueOptionInsert,
  'crates/rustok-product/src/services/catalog_schema_service.rs',
  'INSERT INTO product_attribute_value_options (tenant_id, value_id, option_id)',
  'INSERT INTO product_attribute_value_options (value_id, option_id)',
);
const missingTenantAwareValueOptionInsertResult = run(missingTenantAwareValueOptionInsert);
assert(missingTenantAwareValueOptionInsertResult.status !== 0, 'expected missing tenant-aware value option insert to fail');
assert(
  missingTenantAwareValueOptionInsertResult.stderr.includes('tenant_id, value_id, option_id'),
  `expected tenant-aware value option insert failure, got ${missingTenantAwareValueOptionInsertResult.stderr}`,
);

const missingDetachedReadMarker = copyFixture();
replaceInFixture(
  missingDetachedReadMarker,
  'crates/rustok-product/src/services/catalog_schema_service.rs',
  'record.detached = detached_attribute_ids.contains(&record.attribute_id);',
  'record.detached = row.detached;',
);
const missingDetachedReadMarkerResult = run(missingDetachedReadMarker);
assert(missingDetachedReadMarkerResult.status !== 0, 'expected missing read-time detached marker to fail');
assert(
  missingDetachedReadMarkerResult.stderr.includes('record.detached = detached_attribute_ids.contains'),
  `expected detached read marker failure, got ${missingDetachedReadMarkerResult.stderr}`,
);

const missingTagMetadataTest = copyFixture();
replaceInFixture(
  missingTagMetadataTest,
  'crates/rustok-commerce/tests/product_taxonomy_tags.rs',
  'product_tags_are_synced_into_product_tags_without_metadata_mirror',
  'product_tags_drifted',
);
const missingTagMetadataTestResult = run(missingTagMetadataTest);
assert(missingTagMetadataTestResult.status !== 0, 'expected missing taxonomy tag edge-case test to fail');
assert(
  missingTagMetadataTestResult.stderr.includes('product_tags_are_synced_into_product_tags_without_metadata_mirror'),
  `expected taxonomy tag edge-case failure, got ${missingTagMetadataTestResult.stderr}`,
);

const missingShippingProfileTest = copyFixture();
replaceInFixture(
  missingShippingProfileTest,
  'crates/rustok-commerce/tests/graphql_runtime_parity_test/shipping.rs',
  'admin_graphql_rejects_unknown_shipping_profile_references',
  'admin_graphql_shipping_profile_drift',
);
const missingShippingProfileTestResult = run(missingShippingProfileTest);
assert(missingShippingProfileTestResult.status !== 0, 'expected missing shipping profile edge-case test to fail');
assert(
  missingShippingProfileTestResult.stderr.includes('admin_graphql_rejects_unknown_shipping_profile_references'),
  `expected shipping profile edge-case failure, got ${missingShippingProfileTestResult.stderr}`,
);

const missingConsumerDocsMarker = copyFixture();
replaceInFixture(
  missingConsumerDocsMarker,
  'crates/rustok-commerce/CRATE_API.md',
  'Product create/update/list/detail contracts now expose first-class `tags`',
  'Product create/update/list/detail contracts drifted',
);
const missingConsumerDocsMarkerResult = run(missingConsumerDocsMarker);
assert(missingConsumerDocsMarkerResult.status !== 0, 'expected missing commerce consumer docs marker to fail');
assert(
  missingConsumerDocsMarkerResult.stderr.includes('Product create/update/list/detail contracts now expose first-class `tags`'),
  `expected commerce consumer docs marker failure, got ${missingConsumerDocsMarkerResult.stderr}`,
);

const missingSchemaValidation = copyFixture();
replaceInFixture(
  missingSchemaValidation,
  'crates/rustok-product/src/services/catalog_schema_service.rs',
  'attribute {} is outside the product effective schema',
  'attribute outside schema drift',
);
const missingSchemaValidationResult = run(missingSchemaValidation);
assert(missingSchemaValidationResult.status !== 0, 'expected missing effective schema validation to fail');
assert(
  missingSchemaValidationResult.stderr.includes('attribute {} is outside the product effective schema'),
  `expected effective schema validation failure, got ${missingSchemaValidationResult.stderr}`,
);

const missingPlanBacklog = copyFixture();
replaceInFixture(
  missingPlanBacklog,
  'crates/rustok-product/docs/implementation-plan.md',
  'DB-level tenant consistency audit',
  'tenant consistency drift',
);
const missingPlanBacklogResult = run(missingPlanBacklog);
assert(missingPlanBacklogResult.status !== 0, 'expected missing hardening backlog marker to fail');
assert(
  missingPlanBacklogResult.stderr.includes('DB-level tenant consistency audit'),
  `expected plan backlog failure, got ${missingPlanBacklogResult.stderr}`,
);

const missingCatalogFilterUiPlan = copyFixture();
replaceInFixture(
  missingCatalogFilterUiPlan,
  'crates/rustok-product/docs/implementation-plan.md',
  '[x] Connect storefront/admin UI controls to optional catalog filters/sorts.',
  '[ ] Connect storefront/admin UI controls to optional catalog filters/sorts.',
);
const missingCatalogFilterUiPlanResult = run(missingCatalogFilterUiPlan);
assert(missingCatalogFilterUiPlanResult.status !== 0, 'expected missing catalog filter UI plan marker to fail');
assert(
  missingCatalogFilterUiPlanResult.stderr.includes('Connect storefront/admin UI controls to optional catalog filters/sorts'),
  `expected catalog filter UI plan marker failure, got ${missingCatalogFilterUiPlanResult.stderr}`,
);

const missingPackageAggregate = copyFixture();
const packagePath = path.join(missingPackageAggregate, 'package.json');
const packageJson = JSON.parse(fs.readFileSync(packagePath, 'utf8'));
packageJson.scripts['verify:ecommerce:fba'] = packageJson.scripts['verify:ecommerce:fba'].replace(
  ' && npm run verify:product:catalog-schema',
  '',
);
fs.writeFileSync(packagePath, `${JSON.stringify(packageJson, null, 2)}\n`);
const missingPackageAggregateResult = run(missingPackageAggregate);
assert(missingPackageAggregateResult.status !== 0, 'expected missing package aggregate marker to fail');
assert(
  missingPackageAggregateResult.stderr.includes('package.json ecommerce FBA verify aggregate lacks product catalog schema guardrail'),
  `expected package aggregate failure, got ${missingPackageAggregateResult.stderr}`,
);

console.log('[verify-product-catalog-schema.test] fixture coverage passed');
