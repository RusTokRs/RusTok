#!/usr/bin/env node

import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const script = path.resolve('scripts/verify/index-storage-tooling.mjs');

const run = (...args) => spawnSync(process.execPath, [script, ...args], {
  encoding: 'utf8',
});

test('prints the stable Index storage tooling command surface', () => {
  const result = run('--help');
  assert.equal(result.status, 0, result.stderr);
  for (const command of ['contract', 'fixtures', 'packet', 'compare', 'hash', 'prepare', 'render', 'verify-adr']) {
    assert.match(result.stdout, new RegExp(`\\b${command}\\b`, 'u'));
  }
});

test('forwards hash help to the exact-byte comparison helper', () => {
  const result = run('hash', '--help');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /hash-index-storage-comparison\.mjs <comparison\.json>/u);
});

test('forwards comparator help without rewriting its arguments', () => {
  const result = run('compare', '--help');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /compare-index-storage-evidence\.mjs --input <dir>/u);
});

test('forwards decision preparation help without rewriting its arguments', () => {
  const result = run('prepare', '--help');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /prepare-index-storage-decision\.mjs/u);
  assert.match(result.stdout, /--selected <jsonb\|typed_eav\|hot_projection>/u);
});

test('forwards ADR finalization help without rewriting its arguments', () => {
  const result = run('render', '--help');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /finalize-index-storage-adr\.mjs/u);
  assert.match(result.stdout, /--comparison <comparison\.json>/u);
});

test('forwards ADR verification help without rewriting its arguments', () => {
  const result = run('verify-adr', '--help');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /verify-index-storage-adr\.mjs/u);
  assert.match(result.stdout, /--adr <adr\.md>/u);
});

test('rejects unsupported packet scales before invoking the validator', () => {
  const result = run('packet', '--scale', '10m');
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /packet --scale must be smoke, 100k, or 1m/u);
});

test('rejects arguments for aggregate commands', () => {
  const contract = run('contract', '--unexpected');
  assert.notEqual(contract.status, 0);
  assert.match(contract.stderr, /contract does not accept arguments/u);

  const fixtures = run('fixtures', '--unexpected');
  assert.notEqual(fixtures.status, 0);
  assert.match(fixtures.stderr, /fixtures does not accept arguments/u);
});

test('rejects unknown commands', () => {
  const result = run('publish');
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /unknown command: publish/u);
});
