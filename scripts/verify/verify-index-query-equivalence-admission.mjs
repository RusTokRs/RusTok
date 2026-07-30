#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-query-equivalence-admission] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const admissionPath = 'ops/benches/src/index_storage/query_equivalence_admission.rs';
const admission = requireMarkers(admissionPath, [
  'index_query_equivalence_admission_v1',
  'index_query_equivalence_capture_v1',
  'INDEX_QUERY_EQUIVALENCE_ALLOW_ADMISSION',
  'INDEX_QUERY_EQUIVALENCE_BUNDLE',
  'INDEX_QUERY_EQUIVALENCE_EXPECTED_REPOSITORY',
  'INDEX_QUERY_EQUIVALENCE_EXPECTED_COMMIT',
  'INDEX_QUERY_EQUIVALENCE_EXPECTED_RUN_KEY',
  '#[serde(deny_unknown_fields)]',
  'const INVENTORY: [&str; 3] = [DESCRIPTOR_FILE, STDERR_FILE, STDOUT_FILE];',
  'postgres_query_port_matches_reference_fixture',
  '--test-threads=1',
  'scenario_contract_sha256 == sha256_json(&SCENARIOS)?',
  'descriptor.sha256 == sha256_bytes(bytes)',
  'combined.contains("test result: ok.")',
  'combined.contains("1 passed; 0 failed")',
  '!combined.contains(SKIP_MARKER)',
  'inventory_after == inventory_before',
  'query equivalence descriptor changed during admission review',
  'query equivalence stdout changed during admission review',
  'query equivalence stderr changed during admission review',
  'production_lifecycle_authorized: false',
  'query equivalence admission receipt must be outside the immutable bundle',
  'query equivalence admission receipt parent must already exist',
  '.create_new(true)',
  'file.sync_all()',
]);

for (const forbidden of [
  'DATABASE_URL',
  'database_url',
  'Command::new(',
  'INSERT INTO index_schemas',
  'INSERT INTO index_entities',
  'INSERT INTO index_links',
  'fs::write(',
  'File::create(',
  'remove_dir_all',
  'remove_file',
]) {
  if (admission.includes(forbidden)) {
    fail(`${admissionPath} contains forbidden marker ${forbidden}`);
  }
}

requireMarkers('ops/benches/src/bin/index_query_equivalence_admission.rs', [
  'QueryEquivalenceAdmissionConfig::from_env()',
  'admit_query_equivalence_bundle(&config)?',
  'index query equivalence admission complete',
]);
requireMarkers('ops/benches/src/index_storage/mod.rs', [
  'mod query_equivalence_admission;',
  'QueryEquivalenceAdmission, QueryEquivalenceAdmissionConfig, admit_query_equivalence_bundle',
]);
requireMarkers('ops/benches/Cargo.toml', [
  'name = "index-query-equivalence-admission"',
  'path = "src/bin/index_query_equivalence_admission.rs"',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-query-equivalence-admission.mjs'",
]);
requireMarkers('crates/rustok-index/docs/m4-postgres-reference-equivalence.md', [
  'Status: `fixture_capture_and_admission_source_complete_owner_execution_pending`',
  '`index-query-equivalence-admission`',
  '`production_lifecycle_authorized: false`',
  'receipt parent must already exist',
  'Not run by the implementation agent',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [x] Add retained v4 plan/SQL snapshots and synchronized source guards.',
  '- [ ] Execute PostgreSQL/reference-engine equivalence capture and admit retained live evidence.',
]);

console.log('[verify-index-query-equivalence-admission] OK');