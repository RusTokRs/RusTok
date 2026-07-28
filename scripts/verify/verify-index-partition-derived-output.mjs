#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const prefix = '[verify-index-partition-derived-output]';
const read = (filename) => readFileSync(filename, 'utf8');
const requireMarkers = (source, markers, label) => {
  for (const marker of markers) {
    if (!source.includes(marker)) throw new Error(`${label} is missing ${marker}`);
  }
};

try {
  const core = read('scripts/verify/index-partition-derived-output-core.mjs');
  const manifestCli = read('scripts/verify/render-index-partition-archive-manifest.mjs');
  const verifyCli = read('scripts/verify/verify-index-partition-archive-manifest.mjs');
  const fixture = read('scripts/verify/index-partition-derived-output.test.mjs');
  const tooling = read('scripts/verify/index-storage-tooling.mjs');
  const runbook = read('crates/rustok-index/docs/partition-full-capture.md');

  requireMarkers(core, [
    'randomUUID',
    "openSync(temporaryPath, 'wx', 0o600)",
    'writeFileSync(descriptor, bytes)',
    'fsyncSync(descriptor)',
    'linkSync(temporaryPath, resolvedOutputPath)',
    'already exists; refusing to overwrite',
    'ensureOutsideRoot(canonicalRoot, canonicalOutput, label)',
    'removePublishedIdentity',
    'removeTemporaryFile(temporaryPath)',
  ], 'derived output core');
  requireMarkers(manifestCli, [
    "'--output'",
    'outputPath:',
    'publishDerivedJsonOutsideRetainedBundle',
    'renderDerivedJson',
    'options.outputPath === undefined',
    "label: 'archive manifest output'",
    'process.stdout.write',
    'refuses overwrite',
  ], 'archive manifest CLI');
  requireMarkers(verifyCli, [
    "'--output'",
    'outputPath:',
    'publishDerivedJsonOutsideRetainedBundle',
    'renderDerivedJson',
    'options.outputPath === undefined',
    "label: 'archive verification receipt output'",
    'process.stdout.write',
    'refuses overwrite',
  ], 'archive verification CLI');
  requireMarkers(fixture, [
    'atomically publishes deterministic derived JSON outside the retained bundle',
    'refuses to overwrite an existing derived output',
    'rejects derived output inside the retained bundle without creating a file',
    'rejects an external symlink parent that resolves into the retained bundle',
    'already exists; refusing to overwrite',
    'must stay outside the retained bundle root',
  ], 'derived output fixture');
  requireMarkers(tooling, [
    'verify-index-partition-derived-output.mjs',
    'index-partition-derived-output.test.mjs',
    'partition-archive-manifest',
    'partition-archive-verify',
    '[--output <archive-manifest.json>]',
    '[--output <verification-receipt.json>]',
  ], 'Index storage tooling router');
  requireMarkers(runbook, [
    '--output evidence/index-partition/retained-run.archive-manifest.json',
    '--output evidence/index-partition/retained-run.archive-verification.json',
    'no-clobber hard link',
    'refuses to overwrite',
    'shell can truncate an existing target',
    'Without `--output`, stdout mode remains available',
    'production_lifecycle_authorized: false',
  ], 'full partition capture runbook');

  console.log(`${prefix} contract satisfied`);
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
