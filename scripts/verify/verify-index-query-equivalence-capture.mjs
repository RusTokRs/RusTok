#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-query-equivalence-capture] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const capturePath = 'ops/benches/src/index_storage/query_equivalence_capture.rs';
const capture = requireMarkers(capturePath, [
  'index_query_equivalence_capture_v1',
  'INDEX_QUERY_EQUIVALENCE_ALLOW_CAPTURE',
  'RUSTOK_INDEX_TEST_DATABASE_URL',
  'INDEX_QUERY_EQUIVALENCE_OUTPUT_ROOT',
  'INDEX_QUERY_EQUIVALENCE_COMMIT',
  'INDEX_QUERY_EQUIVALENCE_RUN_KEY',
  'git_output(&config.workspace_root, ["rev-parse", "HEAD"])',
  'status", "--porcelain=v1", "--untracked-files=all',
  'postgres_query_port_matches_reference_fixture',
  '--test-threads=1',
  'skipping rustok-index PostgreSQL/reference equivalence',
  '1 passed; 0 failed',
  'pg_control_system()',
  'system_identifier',
  'scenario_contract_sha256',
  'stdout.log',
  'stderr.log',
  'equivalence.json',
  'create_new(true)',
  'fs::create_dir(&final_root)',
  'ensure_exact_files(&final_root, &[STDERR_FILE, STDOUT_FILE])',
  '&[DESCRIPTOR_FILE, STDERR_FILE, STDOUT_FILE]',
  'output.stdout.len() <= MAX_LOG_BYTES',
  'database_after == database_before',
  'verify_source_identity(config)?;',
]);
for (const forbidden of [
  'Command::new("sh")',
  'Command::new("bash")',
  'INSERT INTO index_schemas',
  'INSERT INTO index_entities',
  'INSERT INTO index_links',
  'serde_json::to_value(&config)',
  'fs::write(',
  'File::create(',
]) {
  if (capture.includes(forbidden)) fail(`${capturePath} contains forbidden marker ${forbidden}`);
}
const descriptorStart = capture.indexOf('struct QueryEquivalenceDescriptor');
const descriptorEnd = capture.indexOf('pub struct QueryEquivalenceCapture {');
if (descriptorStart < 0 || descriptorEnd <= descriptorStart) {
  fail(`${capturePath} has no inspectable descriptor contract`);
}
const descriptorContract = capture.slice(descriptorStart, descriptorEnd);
for (const forbidden of ['database_url', 'workspace_root', 'cargo_program']) {
  if (descriptorContract.includes(forbidden)) {
    fail(`${capturePath} serializes forbidden descriptor field ${forbidden}`);
  }
}

requireMarkers('ops/benches/src/bin/index_query_equivalence_capture.rs', [
  'QueryEquivalenceCaptureConfig::from_env()',
  'capture_query_equivalence(&config).await?',
  'index query equivalence capture complete',
]);
requireMarkers('ops/benches/src/index_storage/mod.rs', [
  'mod query_equivalence_capture;',
  'QueryEquivalenceCapture, QueryEquivalenceCaptureConfig, capture_query_equivalence',
]);
requireMarkers('ops/benches/Cargo.toml', [
  'hex = { workspace = true }',
  'name = "index-query-equivalence-capture"',
  'path = "src/bin/index_query_equivalence_capture.rs"',
]);
requireMarkers('crates/rustok-index/docs/m4-postgres-reference-equivalence.md', [
  'Status: `fixture_capture_and_admission_source_complete_owner_execution_pending`',
  '`index-query-equivalence-capture`',
  'descriptor-last no-clobber bundle',
  'does not retain the PostgreSQL URL',
  'Not run by the implementation agent',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [ ] Add plan/SQL snapshots and PostgreSQL/reference-engine equivalence tests.',
]);

console.log('[verify-index-query-equivalence-capture] OK');
