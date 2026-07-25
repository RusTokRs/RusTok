#!/usr/bin/env node

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';

const script = path.resolve('scripts/verify/index-storage-tooling.mjs');
const run = (...args) => spawnSync(process.execPath, [script, ...args], {
  encoding: 'utf8',
});

test('rejects global help combined with other arguments', () => {
  const result = run('--help', 'packet');
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /help must be the only argument/u);
  assert.equal(result.stdout, '');
});

test('rejects duplicate packet scale before invoking evidence tooling', () => {
  const result = run('packet', '--scale', 'smoke', '--scale', '1m');
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /--scale was provided more than once/u);
  assert.doesNotMatch(result.stderr, /check-index-storage-read-ordering/u);
});

test('rejects duplicate packet root before invoking evidence tooling', () => {
  const result = run(
    'packet',
    '--scale', 'smoke',
    '--root', 'first-evidence-root',
    '--root', 'second-evidence-root',
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /--root was provided more than once/u);
  assert.doesNotMatch(result.stderr, /check-index-storage-read-ordering/u);
});
