#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-schema-readiness] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const sourcePath = 'crates/rustok-index/src/infrastructure/postgres/schema_readiness.rs';
const source = requireMarkers(sourcePath, [
  'pub const MAX_INDEX_SCHEMA_READINESS_SCHEMAS: usize = 64;',
  'pub struct IndexSchemaReadinessRequest',
  'pub struct IndexSchemaReadinessFailure',
  'pub reason: PersistedSchemaReadinessFailure',
  'PersistedSchemaReadinessFailure::Missing',
  'PersistedSchemaReadinessFailure::Inactive',
  'PersistedSchemaReadinessFailure::FingerprintMismatch',
  'PersistedSchemaReadinessFailure::ContractMismatch',
  'pub struct IndexSchemaReadinessReceipt',
  'pub struct PostgresIndexSchemaReadinessStore',
  'pub async fn require(',
  'let registered = registry',
  '.get(reference)',
  'SELECT module_name, entity_name, schema_version, schema_fingerprint, schema_json, status FROM index_schemas',
  'persisted.status != "active"',
  'persisted.fingerprint != expected_schema.fingerprint.to_string()',
  'persisted.schema_json != expected_schema.schema_json',
  'IndexSchemaReadinessError::NotReady { failures }',
]);

for (const forbidden of [
  'INSERT INTO index_schemas',
  'UPDATE index_schemas',
  'DELETE FROM index_schemas',
  'tokio::spawn',
  'loop {',
  'Product',
  'SalesChannel',
  'rustok_product',
  'rustok_channel',
]) {
  if (source.includes(forbidden)) {
    fail(`${sourcePath} contains forbidden write, runtime, or Product-domain coupling: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/schema_readiness_tests.rs', [
  'readiness_requires_the_complete_exact_tenant_schema_set',
  'readiness_reports_a_missing_exact_schema_without_partial_success',
  'readiness_rejects_inactive_or_contract_drifted_rows',
  'readiness_rejects_schema_json_drift_even_with_the_expected_fingerprint',
  'readiness_request_is_bounded_and_unambiguous',
  'readiness_rejects_refs_absent_from_the_runtime_registry_before_storage',
  'PersistedSchemaReadinessFailure::Missing',
  'PersistedSchemaReadinessFailure::Inactive',
  'PersistedSchemaReadinessFailure::FingerprintMismatch',
  'PersistedSchemaReadinessFailure::ContractMismatch',
]);

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod schema_readiness;',
  'mod schema_readiness_tests;',
  'IndexSchemaReadinessRequest',
  'PostgresIndexSchemaReadinessStore',
  'MAX_INDEX_SCHEMA_READINESS_SCHEMAS',
]);

requireMarkers('crates/rustok-index/src/lib.rs', [
  'IndexSchemaReadinessRequest',
  'IndexSchemaReadinessReceipt',
  'PostgresIndexSchemaReadinessStore',
  'MAX_INDEX_SCHEMA_READINESS_SCHEMAS',
]);

const doc = requireMarkers('crates/rustok-index/docs/m7-schema-readiness.md', [
  'Status: `source_complete_owner_execution_pending`',
  '`index_schemas`',
  '`schema_fingerprint`',
  '`schema_json`',
  'one current Product contract',
  'one current ProductVariant',
  'one current SalesChannel',
  'positive numeric schema key',
  'No tests, Node verifiers, Cargo checks',
]);
for (const legacy of ['product@1', 'product@2', 'product_variant@1', 'product_variant@2']) {
  if (doc.includes(legacy)) fail(`schema readiness doc retains legacy Product contract ${legacy}`);
}

const aggregate = read('scripts/verify/verify-index-query-contract.mjs');
if (!aggregate.includes("'verify-index-schema-readiness.mjs'")) {
  fail('Index aggregate verifier does not include the schema readiness guard');
}

console.log('[verify-index-schema-readiness] tenant schema readiness contract verified');
