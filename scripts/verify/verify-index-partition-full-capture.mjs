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
const requireExactlyOnce = (source, marker, label) => {
  const count = source.split(marker).length - 1;
  if (count !== 1) throw new Error(`${label} must contain ${marker} exactly once; found ${count}`);
};

try {
  const finalizer = read('ops/benches/src/index_storage/partition_capture.rs');
  const binary = read('ops/benches/src/bin/index_partition_capture_finalize.rs');
  const cargo = read('ops/benches/Cargo.toml');
  const module = read('ops/benches/src/index_storage/mod.rs');
  const orchestrator = read('scripts/verify/run-index-partition-evidence.mjs');
  const tooling = read('scripts/verify/index-storage-tooling.mjs');
  const runbook = read('crates/rustok-index/docs/partition-full-capture.md');
  const plan = read('crates/rustok-index/docs/implementation-plan.md');
  const m3Start = plan.indexOf('### M3 - PostgreSQL storage engine');
  const retainedStart = plan.indexOf('#### Retained repository contract wording');
  if (m3Start < 0 || retainedStart <= m3Start) {
    throw new Error('implementation plan must contain a bounded primary M3 checklist');
  }
  const primaryM3Checklist = plan.slice(m3Start, retainedStart);

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
    'M3 partition cutover rehearsal evidence runner: `complete`',
    'M3 retained packet owner orchestration: `complete`',
    'Real retained PostgreSQL packet execution: `open`',
    'INDEX_PARTITION_ALLOW_FULL_CAPTURE=1',
    'index-partition-capture-finalize',
    'forbidden before one retained admitted packet',
  ], 'full partition capture runbook');
  requireMarkers(plan, [
    'M3 partition cutover rehearsal evidence runner: `complete`',
    'M3 retained packet owner orchestration: `complete`',
    'Real retained PostgreSQL packet execution: `open`',
    '- [x] Add owner-operated PostgreSQL cutover/rollback rehearsal evidence capture.',
    '- [x] Add owner-operated full retained packet orchestration and capture finalization.',
    '12. The cutover rehearsal runner validates production and retained shadow identities,',
    '13. The full-capture orchestrator requires one explicit owner opt-in, one immutable',
    'one retained admitted packet, query adapter, and production partition',
  ], 'Index implementation plan');
  requireExactlyOnce(
    primaryM3Checklist,
    '- [ ] Execute one fresh full PostgreSQL capture and retain all six raw artifacts,',
    'primary M3 checklist',
  );
  requireExactlyOnce(
    primaryM3Checklist,
    '- [ ] Review and archive one complete admitted real packet before production lifecycle',
    'primary M3 checklist',
  );
  forbidMarkers(primaryM3Checklist, [
    'Execute and retain PostgreSQL baseline/shadow, query, mutation, and maintenance',
    'Execute retained PostgreSQL maintenance and cutover evidence.',
    'Execute retained PostgreSQL cutover evidence.',
    'Assemble and validate one complete retained real packet.',
  ], 'primary M3 checklist');

  console.log(`${prefix} contract satisfied`);
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
