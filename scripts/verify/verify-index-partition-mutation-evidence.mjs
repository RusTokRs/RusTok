#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-partition-mutation-evidence] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};
const requireNormalizedMarkers = (relative, markers) => {
  const source = read(relative).replace(/\s+/gu, ' ');
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const runner = requireMarkers('ops/benches/src/index_storage/partition_mutation.rs', [
  'INDEX_PARTITION_ALLOW_MUTATION_EVIDENCE',
  'partition mutation evidence requires PostgreSQL 16',
  'partition mutation evidence requires jit=off',
  'SET synchronous_commit = on',
  'SET TRANSACTION ISOLATION LEVEL REPEATABLE READ',
  'SAVEPOINT {SAMPLE_SAVEPOINT}',
  'ROLLBACK TO SAVEPOINT {SAMPLE_SAVEPOINT}',
  'transaction.rollback().await',
  'EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)',
  'maximum_node_wal_bytes',
  'baseline_p95_ms',
  'shadow_p95_ms',
  'baseline_wal_bytes',
  'shadow_wal_bytes',
  'cases.len() == manifest.repetitions.mutation',
  'partition mutation relation count parity failed',
  'to_jsonb(c) = to_jsonb(s)',
  'did not affect exactly one matching row on both sides',
  'must prune its target to exactly one child partition',
  'fs::hard_link(&temporary, path)',
  'refusing to overwrite {path:?}',
]);

for (const forbidden of [
  'COMMIT;',
  '.commit().await',
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
  if (runner.includes(forbidden)) fail(`mutation runner contains forbidden marker ${forbidden}`);
}

requireMarkers('ops/benches/src/bin/index_partition_mutation_evidence.rs', [
  'PartitionMutationConfig::from_env()',
  'capture_partition_mutation_evidence(&config).await?',
  'index partition mutation evidence complete',
]);

requireMarkers('ops/benches/src/index_storage/mod.rs', [
  'mod partition_mutation;',
  'PartitionMutationConfig',
  'capture_partition_mutation_evidence',
]);

requireMarkers('ops/benches/Cargo.toml', [
  'name = "index-partition-mutation-evidence"',
  'path = "src/bin/index_partition_mutation_evidence.rs"',
]);

requireNormalizedMarkers('ops/benches/README.md', [
  'Index partition mutation/WAL evidence',
  'INDEX_PARTITION_ALLOW_MUTATION_EVIDENCE=1',
  'INDEX_PARTITION_MUTATION_SAMPLES=7',
  'index-partition-mutation-evidence',
  'rollback-only repeatable-read transaction',
  'maximum per-sample plan-node WAL bytes',
]);

requireNormalizedMarkers('crates/rustok-index/docs/partition-evidence-runbook.md', [
  'index-partition-mutation-evidence',
  'mutation.json',
  'maximum per-sample plan-node WAL bytes',
  'Every validation and EXPLAIN mutation is rolled back to a savepoint',
]);

requireNormalizedMarkers('crates/rustok-index/docs/README.md', [
  'M3 partition mutation/WAL evidence runner: `complete`',
  'maintenance and cutover evidence remain open',
]);

requireNormalizedMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- M3 partition mutation/WAL evidence runner: `complete`',
  '- [x] Add owner-operated PostgreSQL mutation/WAL evidence capture.',
  '- [ ] Execute retained PostgreSQL mutation, maintenance, and cutover evidence.',
  '10. The mutation/WAL runner validates the same manifest and catalog, requires count parity and matching generic anchors, executes rollback-only mutation samples,',
]);

requireMarkers('scripts/verify/index-storage-tooling.mjs', [
  "'verify-index-partition-mutation-evidence.mjs'",
]);

requireMarkers('.github/workflows/index-storage-smoke.yml', [
  'scripts/verify/verify-index-partition-mutation-evidence.mjs',
  'node --check scripts/verify/verify-index-partition-mutation-evidence.mjs',
  '--bin index-partition-mutation-evidence',
]);

console.log('[verify-index-partition-mutation-evidence] OK');
