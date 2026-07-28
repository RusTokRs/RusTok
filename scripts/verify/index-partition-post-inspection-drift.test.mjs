import assert from 'node:assert/strict';
import {
  lstatSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
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
import { REVIEW_CONTRACT } from './index-partition-review-core.mjs';

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

const buildContext = () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'index-partition-post-inspection-'));
  const files = roles.map((role, index) => {
    const relativePath = `${role}.json`;
    const filename = path.join(root, relativePath);
    const bytes = Buffer.from(`${JSON.stringify({ role, index })}\n`, 'utf8');
    writeFileSync(filename, bytes);
    return {
      role,
      path: relativePath,
      bytes: bytes.length,
      sha256: sha256Hex(bytes),
      identity: identityOf(lstatSync(filename, { bigint: true })),
    };
  });
  const inspection = {
    contract: REVIEW_CONTRACT,
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
  return { root, manifestPath, inspection };
};

const cleanup = ({ root, manifestPath }) => {
  rmSync(manifestPath, { force: true });
  rmSync(root, { recursive: true, force: true });
};

test('rechecks all retained files before publishing an archive verification receipt', () => {
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
    assert.deepEqual(readFileSync(context.manifestPath), manifestBefore);
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
      /retained bundle file query identity changed after inspection/u,
    );
  } finally {
    cleanup(context);
  }
});
