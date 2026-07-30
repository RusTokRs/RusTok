#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-partition-snapshot-capture] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const runner = requireMarkers('ops/benches/src/index_storage/partition_snapshot.rs', [
  'INDEX_PARTITION_ALLOW_SHADOW_COPY',
  'index_partition_query_audit_v1',
  'index_partition_relation_digest_v1',
  'partition evidence requires PostgreSQL 16',
  'partition evidence requires jit=off',
  'must remain an ordinary unpartitioned table',
  'pg_advisory_lock(hashtextextended($1, 0))',
  'SET TRANSACTION ISOLATION LEVEL REPEATABLE READ',
  'CREATE TABLE {} (LIKE {} INCLUDING DEFAULTS INCLUDING GENERATED INCLUDING IDENTITY INCLUDING STORAGE INCLUDING COMMENTS) PARTITION BY HASH (tenant_id)',
  'INSERT INTO {} SELECT * FROM {}',
  'CREATE UNIQUE INDEX {} ON {}',
  'ADD CONSTRAINT {} FOREIGN KEY',
  'row_to_json(row_data)::text',
  'shadow relation byte count overflow',
  'shadow source foreign key is not validated',
  'entity shadow snapshot diverged from the repeatable-read baseline',
  'link shadow snapshot diverged from the repeatable-read baseline',
  'fs::hard_link(&baseline_temp, baseline_path)',
  'fs::hard_link(&shadow_temp, shadow_path)',
  'refusing to overwrite {baseline_path:?}',
  'refusing to overwrite {shadow_path:?}',
  'deterministic_manifest_and_bootstrap_remain_shadow_only',
  'assert!(!sql.contains(forbidden), "bootstrap contains {forbidden}")',
]);

const productionRunner = runner.split('\n#[cfg(test)]', 1)[0];
for (const forbidden of [
  'ALTER TABLE "index_entities"',
  'ALTER TABLE "index_links"',
  'DROP TABLE "index_entities"',
  'DROP TABLE "index_links"',
  'RENAME TO index_entities',
  'RENAME TO index_links',
  'VACUUM FULL',
  'rustok_product',
  'rustok_content',
  'rustok_flex',
]) {
  if (productionRunner.includes(forbidden)) {
    fail(`snapshot runner production code contains forbidden marker ${forbidden}`);
  }
}

requireMarkers('ops/benches/src/bin/index_partition_snapshot_capture.rs', [
  'PartitionSnapshotConfig::from_env()',
  'capture_partition_snapshot(&config).await?',
  'index partition snapshot capture complete',
]);

requireMarkers('ops/benches/src/index_storage/mod.rs', [
  'mod partition_snapshot;',
  'PartitionSnapshotConfig',
  'capture_partition_snapshot',
]);

requireMarkers('ops/benches/Cargo.toml', [
  'sha2 = { workspace = true }',
  'name = "index-partition-snapshot-capture"',
  'path = "src/bin/index_partition_snapshot_capture.rs"',
]);

requireMarkers('ops/benches/README.md', [
  'Index partition snapshot capture',
  'INDEX_PARTITION_ALLOW_SHADOW_COPY=1',
  'INDEX_PARTITION_QUERY_AUDIT',
  'index-partition-snapshot-capture',
  'does not rename, drop, or alter the canonical production relations',
]);

requireMarkers('crates/rustok-index/docs/partition-evidence-runbook.md', [
  'index_partition_query_audit_v1',
  'index-partition-snapshot-capture',
  'baseline.json',
  'shadow.json',
  'repeatable-read snapshot',
  'Query, mutation, maintenance, and cutover artifacts remain separate owner-run',
  'measurements. A failed attempt may leave partial shadow state for inspection',
]);

requireMarkers('crates/rustok-index/docs/README.md', [
  'M3 partition baseline/shadow snapshot runner: `complete`',
  'The real query, mutation,',
  'maintenance, and cutover measurements remain open.',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- M3 partition baseline/shadow snapshot runner: `complete`',
  '- [x] Add owner-operated PostgreSQL baseline/shadow snapshot capture.',
  '- [ ] Execute retained PostgreSQL query, mutation, maintenance, and cutover evidence.',
  '8. The snapshot runner creates evidence-bound shadow parents/children, copies one',
  'repeatable-read baseline, attaches shadow integrity, records parity/size/catch-up,',
]);

requireMarkers('scripts/verify/index-storage-tooling.mjs', [
  "'verify-index-partition-snapshot-capture.mjs'",
]);

requireMarkers('.github/workflows/index-storage-smoke.yml', [
  'scripts/verify/verify-index-partition-snapshot-capture.mjs',
  'node --check scripts/verify/verify-index-partition-snapshot-capture.mjs',
  '--bin index-partition-snapshot-capture',
]);

console.log('[verify-index-partition-snapshot-capture] OK');
