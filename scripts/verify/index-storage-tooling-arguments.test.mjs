#!/usr/bin/env node

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';

const script = path.resolve('scripts/verify/index-storage-tooling.mjs');
const run = (...args) => spawnSync(process.execPath, [script, ...args], {
  encoding: 'utf8',
});

const assertPreflightFailure = (result, pattern) => {
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, pattern);
  assert.doesNotMatch(result.stderr, /check-index-storage-read-ordering/u);
  assert.doesNotMatch(result.stderr, /validate-index-storage-evidence/u);
};

test('prints usage for an empty command line', () => {
  const result = run();
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Usage:/u);
});

test('accepts global help only as the sole argument', () => {
  for (const help of ['--help', '-h']) {
    const result = run(help);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Usage:/u);
  }

  for (const help of ['--help', '-h']) {
    const mixed = run(help, 'packet');
    assertPreflightFailure(mixed, /help must be the only argument/u);
    assert.equal(mixed.stdout, '');
  }
});

test('rejects duplicate packet scale before invoking evidence tooling', () => {
  const result = run('packet', '--scale', 'smoke', '--scale', '1m');
  assertPreflightFailure(result, /--scale was provided more than once/u);
});

test('rejects duplicate packet root before invoking evidence tooling', () => {
  const result = run(
    'packet',
    '--scale', 'smoke',
    '--root', 'first-evidence-root',
    '--root', 'second-evidence-root',
  );
  assertPreflightFailure(result, /--root was provided more than once/u);
});

test('duplicate packet options fail before later value validation', () => {
  const duplicateScale = run('packet', '--scale', '10m', '--scale', '100k');
  assertPreflightFailure(duplicateScale, /--scale was provided more than once/u);
  assert.doesNotMatch(duplicateScale.stderr, /packet --scale must be/u);

  const duplicateRoot = run(
    'packet',
    '--root', 'first-evidence-root',
    '--root', 'second-evidence-root',
    '--scale', '100k',
  );
  assertPreflightFailure(duplicateRoot, /--root was provided more than once/u);
});
