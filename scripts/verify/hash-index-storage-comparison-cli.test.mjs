#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { test } from 'node:test';

const router = path.resolve('scripts/verify/index-storage-tooling.mjs');
const run = (...args) => spawnSync(process.execPath, [router, 'hash', ...args], {
  encoding: 'utf8',
});

test('hash help aliases are valid only as the sole helper argument', () => {
  for (const argument of ['--help', '-h']) {
    const result = run(argument);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /hash-index-storage-comparison\.mjs <comparison\.json>/u);
    assert.equal(result.stderr, '');
  }
});

test('mixed and repeated hash help fail before file access or digest output', () => {
  for (const args of [
    ['comparison.json', '--help'],
    ['--help', 'comparison.json'],
    ['comparison.json', '-h'],
    ['-h', 'comparison.json'],
    ['--help', '--help'],
    ['-h', '-h'],
  ]) {
    const result = run(...args);
    assert.notEqual(result.status, 0, args.join(' '));
    assert.match(result.stderr, /--help\/-h must be the only argument/u);
    assert.doesNotMatch(result.stderr, /missing comparison file/u);
    assert.equal(result.stdout, '');
  }
});

test('hash helper requires exactly one comparison path', () => {
  for (const args of [[], ['left.json', 'right.json']]) {
    const result = run(...args);
    assert.notEqual(result.status, 0, args.join(' '));
    assert.match(result.stderr, /exactly one comparison\.json path is required/u);
    assert.equal(result.stdout, '');
  }
});

test('hash router digests exact comparison bytes without JSON normalization', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-hash-cli-'));
  try {
    const comparisonPath = path.join(root, 'comparison.json');
    const bytes = Buffer.from('{"scale":"100k"}\r\n', 'utf8');
    writeFileSync(comparisonPath, bytes);
    const result = run(comparisonPath);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout, `${createHash('sha256').update(bytes).digest('hex')}\n`);
    assert.equal(result.stderr, '');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
