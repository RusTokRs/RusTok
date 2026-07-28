#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const prefix = '[verify-index-partition-full-capture]';
const read = (filename) => readFileSync(filename, 'utf8');
const requireMarkers = (source, markers, label) => {
  for (const marker of markers) {
    if (!source.includes(marker)) throw new Error(`${label} is missing ${marker}`);
  }
};
const forbidMarkers = (source, markers, label) => {
  for (const marker of markers) {
    if (source.includes(marker)) throw new Error(`${label} must not contain ${marker}`);
  }
};

try {
  const finalizer = read('ops/benches/src/index_storage/partition_capture.rs');
  const binary = read('ops/benches/src/bin/index_partition_capture_finalize.rs');
  const cargo = read('ops/benches/Cargo.toml');
  const module = read('ops/benches/src/index_storage/mod.rs');
  const orchestrator = read('scripts/verify/run-index-partition-evidence.mjs');
  const tooling = read('scripts/verify/index-storage-tooling.mjs');
  const runbook = read('crates/rustok-index/docs/partition-evidence-runbook.md');
  const plan = read('crates/rustok-index/docs/implementation-plan.md');

  requireMarkers(finalizer, [
    'INDEX_PARTITION_ALLOW_CAPTURE_FINALIZE',
    'index_partition_capture_v1',
    'pg_control_system()',
    'system_identifier',
    'capture.json',
    'baseline.json',
    'shadow.json',
    'query.json',
    'mutation.json',
    'maintenance.json',
    'cutover.json',
    'create_new(true)',
    'fs::hard_link',
    'refusing to overwrite',
  ], 'capture finalizer');
  requireMarkers(binary, [
    'PartitionCaptureFinalizeConfig::from_env()',
    'finalize_partition_capture',
  ], 'capture finalizer binary');
  requireMarkers(cargo, [
    'name = "index-partition-capture-finalize"',
    'path = "src/bin/index_partition_capture_finalize.rs"',
  ], 'benchmark Cargo targets');
  requireMarkers(module, [
    'mod partition_capture;',
    'PartitionCaptureFinalizeConfig',
    'finalize_partition_capture',
  ], 'index storage module exports');
  requireMarkers(orchestrator, [
    'INDEX_PARTITION_ALLOW_FULL_CAPTURE',
    'index-partition-snapshot-capture',
    'index-partition-query-evidence',
    'index-partition-mutation-evidence',
    'index-partition-maintenance-evidence',
    'index-partition-cutover-evidence',
    'index-partition-capture-finalize',
    'assemble-index-partition-evidence.mjs',
    'validate-index-partition-evidence.mjs',
    'refusing to reuse partial partition evidence output',
  ], 'full capture orchestrator');
  forbidMarkers(`${finalizer}\n${orchestrator}`, [
    'DROP TABLE',
    'TRUNCATE TABLE',
    'ALTER TABLE index_entities',
    'ALTER TABLE index_links',
    'dual-write',
  ], 'full capture tooling');
  requireMarkers(tooling, [
    'partition-capture',
    'run-index-partition-evidence.mjs',
    'verify-index-partition-full-capture.mjs',
  ], 'index storage tooling router');
  requireMarkers(runbook, [
    'partition-capture',
    'INDEX_PARTITION_ALLOW_FULL_CAPTURE=1',
    'index-partition-capture-finalize',
  ], 'partition evidence runbook');
  requireMarkers(plan, [
    'M3 partition cutover rehearsal evidence runner: `complete`',
    'M3 retained packet owner orchestration: `complete`',
  ], 'Index implementation plan');

  console.log(`${prefix} contract satisfied`);
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
