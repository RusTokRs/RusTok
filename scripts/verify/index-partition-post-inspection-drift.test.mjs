import assert from 'node:assert/strict';
import {
  linkSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  utimesSync,
  writeFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  buildRetainedPartitionArchiveManifest,
  verifySavedRetainedPartitionArchiveManifest,
} from './index-partition-archive-manifest-core.mjs';
import { sha256Hex } from './index-partition-evidence-core.mjs';
import {
  inspectRetainedBundleDirectoryInventory,
  REVIEW_CONTRACT,
} from './index-partition-review-core.mjs';

const roles = [
  'baseline',
  'shadow',
  'query',
  'mutation',
  'maintenance',
  'cutover',
  'capture',
  'packet',
  'admission',
];

const identityOf = (stat) => `${stat.dev}:${stat.ino}`;
const fingerprintOf = (stat) => [
  identityOf(stat),
  stat.size,
  stat.mtimeNs,
  stat.ctimeNs,
].join(':');

const directorySnapshot = (root, relativePath = '') => {
  const filename = path.resolve(root, relativePath);
  const stat = lstatSync(filename, { bigint: true });
  return {
    path: relativePath,
    identity: identityOf(stat),
    fingerprint: fingerprintOf(stat),
    entries: readdirSync(filename).sort(),
  };
};

const buildContext = ({ nested = false } = {}) => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'index-partition-post-inspection-'));
  if (nested) mkdirSync(path.join(root, 'nested'));
  const files = roles.map((role, index) => {
    const relativePath = nested && role === 'query' ? 'nested/query.json' : `${role}.json`;
    const filename = path.join(root, relativePath);
    const bytes = Buffer.from(`${JSON.stringify({ role, index })}\n`, 'utf8');
    writeFileSync(filename, bytes);
    const stat = lstatSync(filename, { bigint: true });
    return {
      role,
      path: relativePath,
      bytes: bytes.length,
      sha256: sha256Hex(bytes),
      identity: identityOf(stat),
      fingerprint: fingerprintOf(stat),
    };
  });
  const rootDirectory = directorySnapshot(root);
  const inspection = {
    contract: REVIEW_CONTRACT,
    rootIdentity: rootDirectory.identity,
    rootFingerprint: rootDirectory.fingerprint,
    rootCanonical: root,
    directories: [
      rootDirectory,
      ...(nested ? [directorySnapshot(root, 'nested')] : []),
    ],
    packet: {
      database: {
        version: 'PostgreSQL 16.4',
        server_version_num: '160004',
        jit: 'off',
        system_identifier: '7432859712345678901',
        database_name: 'rustok_index_partition_fixture',
      },
    },
    admission: {
      outcome: 'admitted',
      evidence_id: 'post-inspection-drift-fixture',
      completed_at: '2026-07-28T12:00:00Z',
      packet_digest: 'a'.repeat(64),
      run_provenance: {
        repository: 'RusTokRs/RusTok',
        commit: '1'.repeat(40),
        run_key: 'post-inspection-drift-fixture',
        job: 'partition-evidence-fixture',
        runner_os: 'Linux',
        runner_arch: 'X64',
      },
    },
    files,
  };
  const manifestPath = path.join(path.dirname(root), `${path.basename(root)}.archive-manifest.json`);
  writeFileSync(
    manifestPath,
    `${JSON.stringify(buildRetainedPartitionArchiveManifest(inspection), null, 2)}\n`,
  );
  return { root, manifestPath, inspection, extraRoots: [] };
};

const cleanup = ({ root, manifestPath, extraRoots = [] }) => {
  rmSync(manifestPath, { force: true });
  rmSync(root, { recursive: true, force: true });
  for (const extraRoot of extraRoots) {
    rmSync(extraRoot, { recursive: true, force: true });
  }
};

test('rechecks the complete filesystem snapshot before publishing an archive verification receipt', () => {
  const context = buildContext();
  try {
    const manifestBefore = readFileSync(context.manifestPath);
    const savedManifest = JSON.parse(manifestBefore.toString('utf8'));
    const receipt = verifySavedRetainedPartitionArchiveManifest({
      inspection: context.inspection,
      root: context.root,
      manifestPath: context.manifestPath,
    });
    assert.equal(receipt.verified, true);
    assert.equal(receipt.retained_files_rechecked, true);
    assert.equal(receipt.file_count, 9);
    assert.equal(receipt.production_lifecycle_authorized, false);
    assert.equal(Object.hasOwn(savedManifest.files[0], 'identity'), false);
    assert.equal(Object.hasOwn(savedManifest.files[0], 'fingerprint'), false);
    assert.equal(Object.hasOwn(savedManifest, 'rootIdentity'), false);
    assert.equal(Object.hasOwn(savedManifest, 'rootFingerprint'), false);
    assert.equal(Object.hasOwn(savedManifest, 'directories'), false);
    assert.deepEqual(readFileSync(context.manifestPath), manifestBefore);
  } finally {
    cleanup(context);
  }
});

test('rejects unexpected retained bundle entries before inspection completes', () => {
  const context = buildContext();
  try {
    writeFileSync(path.join(context.root, 'unexpected.json'), '{}\n');
    assert.throws(
      () => inspectRetainedBundleDirectoryInventory({
        root: context.root,
        files: context.inspection.files.map((file) => path.join(context.root, file.path)),
      }),
      /unexpected retained bundle entry unexpected\.json/u,
    );
  } finally {
    cleanup(context);
  }
});

test('fails closed when nested retained bundle inventory changes after inspection', () => {
  const context = buildContext({ nested: true });
  try {
    const rootFingerprintBefore = fingerprintOf(lstatSync(context.root, { bigint: true }));
    writeFileSync(path.join(context.root, 'nested', 'unexpected.json'), '{}\n');
    assert.equal(fingerprintOf(lstatSync(context.root, { bigint: true })), rootFingerprintBefore);
    assert.throws(
      () => verifySavedRetainedPartitionArchiveManifest({
        inspection: context.inspection,
        root: context.root,
        manifestPath: context.manifestPath,
      }),
      /retained bundle directory nested (metadata|inventory) changed after inspection/u,
    );
  } finally {
    cleanup(context);
  }
});

test('fails closed when a retained file changes after inspection', () => {
  const context = buildContext();
  try {
    writeFileSync(path.join(context.root, 'query.json'), '{"role":"query","drift":true}\n');
    assert.throws(
      () => verifySavedRetainedPartitionArchiveManifest({
        inspection: context.inspection,
        root: context.root,
        manifestPath: context.manifestPath,
      }),
      /retained bundle file query changed after inspection/u,
    );
  } finally {
    cleanup(context);
  }
});

test('fails closed on a same-byte retained file identity replacement after inspection', () => {
  const context = buildContext();
  try {
    const target = path.join(context.root, 'query.json');
    const replacement = path.join(context.root, 'query.replacement');
    const bytes = readFileSync(target);
    const identityBefore = identityOf(lstatSync(target, { bigint: true }));
    writeFileSync(replacement, bytes);
    rmSync(target);
    renameSync(replacement, target);
    const identityAfter = identityOf(lstatSync(target, { bigint: true }));
    assert.notEqual(identityAfter, identityBefore);
    assert.throws(
      () => verifySavedRetainedPartitionArchiveManifest({
        inspection: context.inspection,
        root: context.root,
        manifestPath: context.manifestPath,
      }),
      /retained bundle (root changed|directory \. metadata changed|file query identity changed) after inspection/u,
    );
  } finally {
    cleanup(context);
  }
});

test('fails closed when retained metadata changes with the same inode and bytes', () => {
  const context = buildContext();
  try {
    const target = path.join(context.root, 'query.json');
    const before = lstatSync(target, { bigint: true });
    const identityBefore = identityOf(before);
    const fingerprintBefore = fingerprintOf(before);
    const atime = new Date(Number(before.atimeMs));
    const mtime = new Date(Number(before.mtimeMs) + 2_000);
    utimesSync(target, atime, mtime);
    const after = lstatSync(target, { bigint: true });
    assert.equal(identityOf(after), identityBefore);
    assert.notEqual(fingerprintOf(after), fingerprintBefore);
    assert.throws(
      () => verifySavedRetainedPartitionArchiveManifest({
        inspection: context.inspection,
        root: context.root,
        manifestPath: context.manifestPath,
      }),
      /retained bundle file query metadata changed after inspection/u,
    );
  } finally {
    cleanup(context);
  }
});

test('fails closed when retained bundle root metadata changes with the same inode', () => {
  const context = buildContext();
  try {
    const before = lstatSync(context.root, { bigint: true });
    const identityBefore = identityOf(before);
    const fingerprintBefore = fingerprintOf(before);
    const atime = new Date(Number(before.atimeMs));
    const mtime = new Date(Number(before.mtimeMs) + 2_000);
    utimesSync(context.root, atime, mtime);
    const after = lstatSync(context.root, { bigint: true });
    assert.equal(identityOf(after), identityBefore);
    assert.notEqual(fingerprintOf(after), fingerprintBefore);
    assert.throws(
      () => verifySavedRetainedPartitionArchiveManifest({
        inspection: context.inspection,
        root: context.root,
        manifestPath: context.manifestPath,
      }),
      /retained bundle root changed after inspection/u,
    );
  } finally {
    cleanup(context);
  }
});

test('fails closed when the retained bundle root is replaced after inspection', () => {
  const context = buildContext();
  const originalRoot = `${context.root}.original`;
  context.extraRoots.push(originalRoot);
  try {
    renameSync(context.root, originalRoot);
    mkdirSync(context.root);
    for (const role of roles) {
      linkSync(
        path.join(originalRoot, `${role}.json`),
        path.join(context.root, `${role}.json`),
      );
    }
    const rootIdentityAfter = identityOf(lstatSync(context.root, { bigint: true }));
    assert.notEqual(rootIdentityAfter, context.inspection.rootIdentity);
    assert.throws(
      () => verifySavedRetainedPartitionArchiveManifest({
        inspection: context.inspection,
        root: context.root,
        manifestPath: context.manifestPath,
      }),
      /retained bundle root changed after inspection/u,
    );
  } finally {
    cleanup(context);
  }
});
