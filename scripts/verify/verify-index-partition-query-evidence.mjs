#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-partition-query-evidence] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const runner = requireMarkers('ops/benches/src/index_storage/partition_query.rs', [
  'INDEX_PARTITION_ALLOW_QUERY_EVIDENCE',
  'normalized_partition_plan_v1',
  'partition query evidence requires PostgreSQL 16',
  'partition query evidence requires jit=off',
  'SET enable_partition_pruning = on',
  'SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY',
  'EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)',
  'manifest evidence_id does not match canonical manifest input',
  'query {} produced different baseline and shadow results',
  'had unstable {side} normalized plans across samples',
  'must prune entities to exactly one child when used',
  'must prune links to exactly one child when used',
  'cases.len() == manifest.repetitions.query',
  'baseline_p95_ms',
  'shadow_p95_ms',
  'baseline_plan_digest',
  'shadow_plan_digest',
  'baseline_explain_samples',
  'shadow_explain_samples',
  'fs::hard_link(&temporary, path)',
  'refusing to overwrite {path:?}',
]);

for (const forbidden of [
  'ALTER TABLE "index_entities"',
  'ALTER TABLE "index_links"',
  'DROP TABLE "index_entities"',
  'DROP TABLE "index_links"',
  'RENAME TO index_entities',
  'RENAME TO index_links',
  'INSERT INTO index_entities',
  'INSERT INTO index_links',
  'UPDATE index_entities',
  'UPDATE index_links',
  'DELETE FROM index_entities',
  'DELETE FROM index_links',
  'VACUUM FULL',
  'rustok_product',
  'rustok_content',
  'rustok_flex',
]) {
  if (runner.includes(forbidden)) fail(`query runner contains forbidden marker ${forbidden}`);
}

requireMarkers('ops/benches/src/bin/index_partition_query_evidence.rs', [
  'PartitionQueryConfig::from_env()',
  'capture_partition_query_evidence(&config).await?',
  'index partition query evidence complete',
]);

requireMarkers('ops/benches/src/index_storage/mod.rs', [
  'mod partition_query;',
  'PartitionQueryConfig',
  'capture_partition_query_evidence',
]);

requireMarkers('ops/benches/Cargo.toml', [
  'name = "index-partition-query-evidence"',
  'path = "src/bin/index_partition_query_evidence.rs"',
]);

requireMarkers('ops/benches/README.md', [
  'Index partition query evidence',
  'INDEX_PARTITION_ALLOW_QUERY_EVIDENCE=1',
  'INDEX_PARTITION_QUERY_SAMPLES=7',
  'index-partition-query-evidence',
  'read-only repeatable-read transaction',
  'retains full JSON',
  '`EXPLAIN (ANALYZE, BUFFERS, WAL)` samples',
]);

requireMarkers('crates/rustok-index/docs/partition-evidence-runbook.md', [
  'index-partition-query-evidence',
  'normalized_partition_plan_v1',
  'query.json',
  'exactly one entity or link child partition',
  'result digest parity',
]);

requireMarkers('crates/rustok-index/docs/README.md', [
  'M3 partition query evidence runner: `complete`',
  'Real mutation, maintenance, and cutover evidence remain',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- M3 partition query evidence runner: `complete`',
  '- [x] Add owner-operated PostgreSQL baseline/shadow query evidence capture.',
  '- [ ] Execute retained PostgreSQL mutation, maintenance, and cutover evidence.',
  'The ninth M3 slice adds owner-operated baseline/shadow query evidence.',
]);

requireMarkers('scripts/verify/index-storage-tooling.mjs', [
  "'verify-index-partition-query-evidence.mjs'",
]);

requireMarkers('.github/workflows/index-storage-smoke.yml', [
  'scripts/verify/verify-index-partition-query-evidence.mjs',
  'node --check scripts/verify/verify-index-partition-query-evidence.mjs',
  '--bin index-partition-query-evidence',
]);

console.log('[verify-index-partition-query-evidence] OK');
