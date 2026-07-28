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
    'readdirSync',
    'inspectRetainedBundleDirectoryInventory',
    'collectRetainedBundleDirectoryInventory',
    'unexpected retained bundle entry',
    'retained bundle directory inventory changed during inspection',
    'retained bundle root changed during inspection',
    'fingerprint: fingerprintOf(descriptorStatAfter)',
    'capture artifact ${role} changed after it was read',
    'fingerprint = assembled.fingerprints.get(role)',
    'bytes: assembled.byteLengths.get(role)',
    'identity: file.identity',
    'fingerprint: file.fingerprint',
    'rootIdentity: rootSnapshot.identity',
    'rootFingerprint: rootSnapshot.fingerprint',
    'rootCanonical: rootSnapshot.canonical',
    'directories,',
  ], 'retained partition review core');

  requireMarkers(core, [
    'readStableRegularFile',
    'readRootSnapshot',
    'assertRootUnchanged',
    'assertSavedManifestUnchanged',
    'normalizeDirectories',
    'assertDirectoryInventoryUnchanged',
    'inspection.directories',
    'readdirSync',
    'openSync',
    'fstatSync',
    'closeSync',
    'fingerprintOf',
    'requireInspectionSnapshot',
    'must be a decimal device:inode identity',
    'must be a decimal device:inode:size:mtimeNs:ctimeNs fingerprint',
    'inspection.rootFingerprint',
    'current.identity !== expected.identity',
    'current.fingerprint !== expected.fingerprint',
    'current.identity !== file.identity',
    'current.fingerprint !== file.fingerprint',
    'identity changed after inspection',
    'metadata changed after inspection',
    'inventory changed after inspection',
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
    "Object.hasOwn(savedManifest, 'rootFingerprint')",
    "Object.hasOwn(savedManifest, 'directories')",
    'rejects unexpected retained bundle entries before inspection completes',
    'unexpected retained bundle entry unexpected\\.json',
    'fails closed when nested retained bundle inventory changes after inspection',
    'retained bundle directory nested (metadata|inventory) changed after inspection',
    'fails closed when a retained file changes after inspection',
    'retained bundle file query changed after inspection',
    'fails closed on a same-byte retained file identity replacement after inspection',
    'assert.notEqual(identityAfter, identityBefore)',
    'fails closed when retained metadata changes with the same inode and bytes',
    'assert.notEqual(fingerprintOf(after), fingerprintBefore)',
    'retained bundle file query metadata changed after inspection',
    'fails closed when retained bundle root metadata changes with the same inode',
    'retained bundle root changed after inspection',
    'fails closed when the retained bundle root is replaced after inspection',
  ], 'post-inspection drift fixture');

  requireMarkers(runbook, [
    'stable file descriptor',
    'internal filesystem snapshot',
    'exact recursive directory inventory',
    'unexpected file, directory, symbolic link, or special entry',
    'rereads all nine retained files',
    'nested directory inventory drift',
    'same-byte filesystem identity replacement',
    'same-inode metadata drift',
    'retained bundle root metadata drift',
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
