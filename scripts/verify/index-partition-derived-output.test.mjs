import assert from 'node:assert/strict';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  publishDerivedJsonOutsideRetainedBundle,
  renderDerivedJson,
} from './index-partition-derived-output-core.mjs';

const buildPaths = () => {
  const parent = mkdtempSync(path.join(os.tmpdir(), 'index-partition-derived-output-'));
  const root = path.join(parent, 'retained-run');
  const output = path.join(parent, 'retained-run.archive-manifest.json');
  return { parent, root, output };
};

const cleanup = (parent) => rmSync(parent, { recursive: true, force: true });

test('atomically publishes deterministic derived JSON outside the retained bundle', () => {
  const paths = buildPaths();
  try {
    rmSync(paths.root, { recursive: true, force: true });
    const rootParent = path.dirname(paths.root);
    const rootName = path.basename(paths.root);
    mkdirSync(path.join(rootParent, rootName));
    const value = { contract: 'derived_fixture_v1', verified: true };
    const result = publishDerivedJsonOutsideRetainedBundle({
      root: paths.root,
      outputPath: paths.output,
      value,
      label: 'fixture output',
    });
    assert.equal(result.path, path.resolve(paths.output));
    assert.equal(result.bytes, Buffer.byteLength(renderDerivedJson(value)));
    assert.equal(readFileSync(paths.output, 'utf8'), renderDerivedJson(value));
    assert.deepEqual(
      readdirSync(paths.parent).sort(),
      ['retained-run', 'retained-run.archive-manifest.json'],
    );
  } finally {
    cleanup(paths.parent);
  }
});

test('refuses to overwrite an existing derived output', () => {
  const paths = buildPaths();
  try {
    mkdirSync(paths.root);
    writeFileSync(paths.output, 'sentinel');
    assert.throws(
      () => publishDerivedJsonOutsideRetainedBundle({
        root: paths.root,
        outputPath: paths.output,
        value: { verified: true },
        label: 'fixture output',
      }),
      /already exists; refusing to overwrite/u,
    );
    assert.equal(readFileSync(paths.output, 'utf8'), 'sentinel');
    assert.deepEqual(
      readdirSync(paths.parent).sort(),
      ['retained-run', 'retained-run.archive-manifest.json'],
    );
  } finally {
    cleanup(paths.parent);
  }
});

test('rejects derived output inside the retained bundle without creating a file', () => {
  const paths = buildPaths();
  try {
    mkdirSync(paths.root);
    const output = path.join(paths.root, 'verification-receipt.json');
    assert.throws(
      () => publishDerivedJsonOutsideRetainedBundle({
        root: paths.root,
        outputPath: output,
        value: { verified: true },
        label: 'fixture output',
      }),
      /must stay outside the retained bundle root/u,
    );
    assert.equal(existsSync(output), false);
  } finally {
    cleanup(paths.parent);
  }
});

test('rejects an external symlink parent that resolves into the retained bundle', (t) => {
  const paths = buildPaths();
  try {
    mkdirSync(paths.root);
    const linkedParent = path.join(paths.parent, 'linked-root');
    try {
      symlinkSync(paths.root, linkedParent, 'dir');
    } catch (error) {
      if (process.platform === 'win32' && (error.code === 'EPERM' || error.code === 'EACCES')) {
        t?.skip?.('directory symlinks require elevated permissions on Windows');
        return;
      }
      throw error;
    }
    const output = path.join(linkedParent, 'verification-receipt.json');
    assert.throws(
      () => publishDerivedJsonOutsideRetainedBundle({
        root: paths.root,
        outputPath: output,
        value: { verified: true },
        label: 'fixture output',
      }),
      /parent must be a regular non-symlink directory/u,
    );
    assert.equal(existsSync(path.join(paths.root, 'verification-receipt.json')), false);
  } finally {
    cleanup(paths.parent);
  }
});
