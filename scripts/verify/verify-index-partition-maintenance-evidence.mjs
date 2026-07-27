#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-partition-maintenance-evidence] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const runner = requireMarkers('ops/benches/src/index_storage/partition_maintenance.rs', [
  'INDEX_PARTITION_ALLOW_MAINTENANCE_EVIDENCE',
  'partition maintenance evidence requires PostgreSQL 16',
  'partition maintenance evidence requires jit=off',
  'SET vacuum_cost_delay = 0',
  'SET synchronous_commit = on',
  'index_pe_maintenance_',
  'CREATE SCHEMA',
  'autovacuum_enabled = false',
  'manifest.repetitions.maintenance',
  'failed to commit partition maintenance evidence churn',
  'VACUUM (ANALYZE)',
  'pg_stat_force_next_flush',
  'n_dead_tup::bigint AS n_dead_tup',
  'baseline_vacuum_ms',
  'shadow_vacuum_ms',
  'baseline_dead_tuples',
  'shadow_dead_tuples',
  'ordinary baseline VACUUM did not clear estimated dead tuples',
  'ordinary shadow VACUUM did not clear estimated dead tuples',
  'canonical or retained snapshot-shadow relations changed during maintenance evidence',
  'baseline/shadow maintenance entity clones diverged',
  'baseline/shadow maintenance link clones diverged',
  'fs::hard_link(&temporary, path)',
  'refusing to overwrite {path:?}',
]);

for (const forbidden of [
  'VACUUM FULL',
  'DROP SCHEMA',
  'DROP TABLE',
  'RENAME TO',
  'UPDATE index_entities',
  'UPDATE index_links',
  'DELETE FROM index_entities',
  'DELETE FROM index_links',
  'ALTER TABLE "index_entities"',
  'ALTER TABLE "index_links"',
  'rustok_product',
  'rustok_content',
  'rustok_flex',
]) {
  if (runner.includes(forbidden)) fail(`maintenance runner contains forbidden marker ${forbidden}`);
}

requireMarkers('ops/benches/src/bin/index_partition_maintenance_evidence.rs', [
  'PartitionMaintenanceConfig::from_env()',
  'capture_partition_maintenance_evidence(&config).await?',
  'index partition maintenance evidence complete',
]);

requireMarkers('ops/benches/src/index_storage/mod.rs', [
  'mod partition_maintenance;',
  'PartitionMaintenanceConfig',
  'capture_partition_maintenance_evidence',
]);

requireMarkers('ops/benches/Cargo.toml', [
  'name = "index-partition-maintenance-evidence"',
  'path = "src/bin/index_partition_maintenance_evidence.rs"',
]);

requireMarkers('ops/benches/README.md', [
  'Index partition maintenance evidence',
  'INDEX_PARTITION_ALLOW_MAINTENANCE_EVIDENCE=1',
  'INDEX_PARTITION_MAINTENANCE_CYCLES=3',
  'index-partition-maintenance-evidence',
  'evidence-only baseline and partitioned clones',
  'ordinary `VACUUM (ANALYZE)`',
]);

requireMarkers('crates/rustok-index/docs/partition-evidence-runbook.md', [
  'index-partition-maintenance-evidence',
  'maintenance.json',
  'evidence-only maintenance schema',
  'ordinary `VACUUM (ANALYZE)`',
  'canonical and retained snapshot-shadow relations remain unchanged',
]);

requireMarkers('crates/rustok-index/docs/README.md', [
  'M3 partition maintenance evidence runner: `complete`',
  'cutover evidence remains open',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- M3 partition maintenance evidence runner: `complete`',
  '- [x] Add owner-operated PostgreSQL baseline/shadow ordinary-VACUUM maintenance evidence capture.',
  '- [ ] Execute retained PostgreSQL cutover evidence.',
  'The eleventh M3 slice adds owner-operated ordinary-VACUUM maintenance evidence.',
]);

requireMarkers('scripts/verify/index-storage-tooling.mjs', [
  "'verify-index-partition-maintenance-evidence.mjs'",
]);

requireMarkers('.github/workflows/index-storage-smoke.yml', [
  'scripts/verify/verify-index-partition-maintenance-evidence.mjs',
  'node --check scripts/verify/verify-index-partition-maintenance-evidence.mjs',
  '--bin index-partition-maintenance-evidence',
]);

console.log('[verify-index-partition-maintenance-evidence] OK');
