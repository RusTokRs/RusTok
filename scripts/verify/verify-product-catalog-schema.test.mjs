import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const repoRoot = process.cwd();
const script = path.join(repoRoot, 'scripts/verify/verify-product-catalog-schema.mjs');
const fixtureFiles = [
  'crates/rustok-product/src/migrations/m20260701_000001_create_product_catalog_attributes.rs',
  'crates/rustok-index/src/migrations/m20260701_000001_create_index_product_attribute_facets.rs',
  'crates/rustok-product/src/services/catalog_schema_service.rs',
  'crates/rustok-product/src/services/catalog_schema.rs',
  'crates/rustok-product/src/services/catalog.rs',
  'crates/rustok-index/src/product/indexer.rs',
  'crates/rustok-product/docs/implementation-plan.md',
  'crates/rustok-product/docs/README.md',
  'docs/architecture/database.md',
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
  localeWidthResult.stderr.includes('locale VARCHAR(32) NOT NULL'),
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
