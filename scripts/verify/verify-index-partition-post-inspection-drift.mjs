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
  const assembly = read('scripts/verify/index-partition-evidence-assembly-core.mjs');
  const review = read('scripts/verify/index-partition-review-core.mjs');
  const core = read('scripts/verify/index-partition-archive-manifest-core.mjs');
  const fixture = read('scripts/verify/index-partition-post-inspection-drift.test.mjs');
  const runbook = read('crates/rustok-index/docs/partition-full-capture.md');

  requireMarkers(assembly, [
    'readStableRegularFile',
    'openSync',
    'fstatSync',
    'closeSync',
    'fingerprintOf',
    'fingerprint: fingerprintOf(descriptorStatAfter)',
    'changed before it could be read',
    'changed while it was being read',
    'byteLengths.set(role, file.bytes.length)',
    'identities.set(role, file.identity)',
    'fingerprints.set(role, file.fingerprint)',
  ], 'partition evidence assembly core');

  requireMarkers(review, [
    'readStableRegularFile',
    'readRootSnapshot',
    'assertRootSnapshot',
    'retained bundle root changed during inspection',
    'fingerprint: fingerprintOf(descriptorStatAfter)',
    'capture artifact ${role} changed after it was read',
    'fingerprint = assembled.fingerprints.get(role)',
    'bytes: assembled.byteLengths.get(role)',
    'identity: file.identity',
    'fingerprint: file.fingerprint',
    'rootIdentity: rootSnapshot.identity',
    'rootCanonical: rootSnapshot.canonical',
  ], 'retained partition review core');

  requireMarkers(core, [
    'readStableRegularFile',
    'readRootSnapshot',
    'assertRootUnchanged',
    'assertSavedManifestUnchanged',
    'openSync',
    'fstatSync',
    'closeSync',
    'fingerprintOf',
    'requireInspectionSnapshot',
    'must be a decimal device:inode identity',
    'must be a decimal device:inode:size:mtimeNs:ctimeNs fingerprint',
    'current.identity !== file.identity',
    'current.fingerprint !== file.fingerprint',
    'identity changed after inspection',
    'metadata changed after inspection',
    'changed after it was verified',
    'retained bundle root changed after inspection',
    'changed before it could be read',
    'changed while it was being read',
    'changed after inspection',
    'retained_files_rechecked: true',
    'saved archive manifest aliases a retained bundle file',
    'production_lifecycle_authorized: false',
  ], 'archive verifier core');
  forbidMarkers(`${assembly}\n${review}\n${core}`, [
    'writeFileSync',
    'mkdirSync',
    'renameSync',
    'rmSync',
    'spawnSync',
    'DATABASE_URL',
    'INDEX_PARTITION_ALLOW',
  ], 'partition inspection and archive verifier cores');

  requireMarkers(fixture, [
    'rechecks the complete filesystem snapshot before publishing an archive verification receipt',
    'retained_files_rechecked',
    "Object.hasOwn(savedManifest.files[0], 'identity')",
    "Object.hasOwn(savedManifest.files[0], 'fingerprint')",
    "Object.hasOwn(savedManifest, 'rootIdentity')",
    'fails closed when a retained file changes after inspection',
    'retained bundle file query changed after inspection',
    'fails closed on a same-byte retained file identity replacement after inspection',
    'assert.notEqual(identityAfter, identityBefore)',
    'retained bundle (root changed|file query identity changed) after inspection',
    'fails closed when retained metadata changes with the same inode and bytes',
    'assert.notEqual(fingerprintOf(after), fingerprintBefore)',
    'retained bundle file query metadata changed after inspection',
    'fails closed when the retained bundle root is replaced after inspection',
    'retained bundle root changed after inspection',
  ], 'post-inspection drift fixture');

  requireMarkers(runbook, [
    'stable file descriptor',
    'internal filesystem snapshot',
    'rereads all nine retained files',
    'same-byte filesystem identity replacement',
    'same-inode metadata drift',
    'retained bundle root replacement',
    'rereads the saved archive manifest',
    'post-inspection exact-byte drift',
    '`retained_files_rechecked: true`',
  ], 'full partition capture runbook');

  console.log(`${prefix} contract satisfied`);
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
