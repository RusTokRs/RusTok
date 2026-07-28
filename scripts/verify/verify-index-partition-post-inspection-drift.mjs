#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const prefix = '[verify-index-partition-post-inspection-drift]';
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
  const core = read('scripts/verify/index-partition-archive-manifest-core.mjs');
  const fixture = read('scripts/verify/index-partition-post-inspection-drift.test.mjs');
  const runbook = read('crates/rustok-index/docs/partition-full-capture.md');

  requireMarkers(core, [
    'readStableRegularFile',
    'openSync',
    'fstatSync',
    'closeSync',
    'fingerprintOf',
    'assertRetainedFilesUnchanged',
    'changed before it could be read',
    'changed while it was being read',
    'changed after inspection',
    'retained_files_rechecked: true',
    'saved archive manifest aliases a retained bundle file',
    'production_lifecycle_authorized: false',
  ], 'archive verifier core');
  forbidMarkers(core, [
    'writeFileSync',
    'mkdirSync',
    'renameSync',
    'rmSync',
    'spawnSync',
    'DATABASE_URL',
    'INDEX_PARTITION_ALLOW',
  ], 'archive verifier core');

  requireMarkers(fixture, [
    'rechecks all retained files before publishing an archive verification receipt',
    'retained_files_rechecked',
    'fails closed when a retained file changes after inspection',
    'retained bundle file query changed after inspection',
  ], 'post-inspection drift fixture');

  requireMarkers(runbook, [
    'rereads all nine retained files',
    'post-inspection exact-byte drift',
    '`retained_files_rechecked: true`',
  ], 'full partition capture runbook');

  console.log(`${prefix} contract satisfied`);
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
