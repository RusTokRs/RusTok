#!/usr/bin/env node

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

import { runComparatorCoreWithAtomicComparison } from './compare-index-storage-evidence.mjs';

const comparator = path.resolve('scripts/verify/compare-index-storage-evidence.mjs');
const runComparator = (...args) => spawnSync(process.execPath, [comparator, ...args], {
  encoding: 'utf8',
});
const quietStream = { write: () => true };
const stagingEntries = (output) => readdirSync(output)
  .filter((entry) => entry.startsWith('.comparison-staging-'));
const stagedOutputFrom = (args) => {
  const outputIndex = args.lastIndexOf('--output');
  assert.notEqual(outputIndex, -1);
  return args[outputIndex + 1];
};

const withOutput = (prefix, callback) => {
  const root = mkdtempSync(path.join(tmpdir(), prefix));
  const output = path.join(root, 'comparison');
  mkdirSync(output);
  try {
    callback({ root, output });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

test('direct comparator help is valid only as the sole argument', () => {
  for (const help of ['--help', '-h']) {
    const result = runComparator(help);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /compare-index-storage-evidence\.mjs --input <dir>/u);
  }

  const mixed = runComparator('--help', '--output', 'comparison');
  assert.notEqual(mixed.status, 0);
  assert.match(mixed.stderr, /help must be the only argument/u);
  assert.equal(mixed.stdout, '');
});

test('direct comparator rejects duplicate output before evidence access', () => {
  const result = runComparator(
    '--input', 'missing-evidence',
    '--output', 'first-comparison',
    '--output', 'second-comparison',
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /--output was provided more than once/u);
  assert.doesNotMatch(result.stderr, /missing evidence file/u);
});

test('comparator publishes finalized markdown before JSON and removes staging', () => {
  withOutput('rustok-index-comparator-publish-', ({ output }) => {
    writeFileSync(path.join(output, 'comparison.json'), '{"stale":true}\n', 'utf8');
    writeFileSync(path.join(output, 'comparison.md'), 'stale markdown\n', 'utf8');
    const publication = [];

    const spawn = (_executable, args) => {
      const staged = stagedOutputFrom(args);
      writeFileSync(path.join(staged, 'comparison.json'), '{"core":true}\n', 'utf8');
      writeFileSync(path.join(staged, 'comparison.md'), 'core markdown\n', 'utf8');
      return { status: 0, stdout: '', stderr: '' };
    };
    const finalizeComparison = ({ output: staged }) => {
      writeFileSync(path.join(staged, 'comparison.json'), '{"finalized":true}\n', 'utf8');
      writeFileSync(path.join(staged, 'comparison.md'), 'finalized markdown\n', 'utf8');
    };
    const fsRename = (source, destination) => {
      publication.push(path.basename(destination));
      const bytes = readFileSync(source);
      writeFileSync(destination, bytes);
      rmSync(source, { force: true });
    };

    const status = runComparatorCoreWithAtomicComparison({
      inputs: ['packet-100k', 'packet-1m'],
      output,
      spawn,
      finalizeComparison,
      rename: fsRename,
      stdout: quietStream,
      stderr: quietStream,
    });

    assert.equal(status, 0);
    assert.deepEqual(publication, ['comparison.md', 'comparison.json']);
    assert.equal(readFileSync(path.join(output, 'comparison.json'), 'utf8'), '{"finalized":true}\n');
    assert.equal(readFileSync(path.join(output, 'comparison.md'), 'utf8'), 'finalized markdown\n');
    assert.deepEqual(stagingEntries(output), []);
  });
});

test('comparator core failure cannot publish a partial decision input', () => {
  withOutput('rustok-index-comparator-core-failure-', ({ output }) => {
    writeFileSync(path.join(output, 'comparison.json'), '{"stale":true}\n', 'utf8');
    const spawn = (_executable, args) => {
      const staged = stagedOutputFrom(args);
      writeFileSync(path.join(staged, 'comparison.json'), '{"partial":', 'utf8');
      return { status: 1, stdout: '', stderr: 'core failed\n' };
    };

    const status = runComparatorCoreWithAtomicComparison({
      inputs: ['packet-100k', 'packet-1m'],
      output,
      spawn,
      stdout: quietStream,
      stderr: quietStream,
    });

    assert.equal(status, 1);
    assert.equal(existsSync(path.join(output, 'comparison.json')), false);
    assert.deepEqual(stagingEntries(output), []);
  });
});

test('post-processing failure leaves no decision input or staging residue', () => {
  withOutput('rustok-index-comparator-finalize-failure-', ({ output }) => {
    writeFileSync(path.join(output, 'comparison.json'), '{"stale":true}\n', 'utf8');
    const spawn = (_executable, args) => {
      const staged = stagedOutputFrom(args);
      writeFileSync(path.join(staged, 'comparison.json'), '{"core":true}\n', 'utf8');
      writeFileSync(path.join(staged, 'comparison.md'), 'core markdown\n', 'utf8');
      return { status: 0, stdout: '', stderr: '' };
    };

    assert.throws(
      () => runComparatorCoreWithAtomicComparison({
        inputs: ['packet-100k', 'packet-1m'],
        output,
        spawn,
        finalizeComparison: () => {
          throw new Error('methodology finalization failed');
        },
        stdout: quietStream,
        stderr: quietStream,
      }),
      /methodology finalization failed/u,
    );
    assert.equal(existsSync(path.join(output, 'comparison.json')), false);
    assert.deepEqual(stagingEntries(output), []);
  });
});

test('missing staged output after successful core leaves no decision input', () => {
  withOutput('rustok-index-comparator-missing-output-', ({ output }) => {
    const spawn = () => ({ status: 0, stdout: '', stderr: '' });
    assert.throws(
      () => runComparatorCoreWithAtomicComparison({
        inputs: ['packet-100k', 'packet-1m'],
        output,
        spawn,
        finalizeComparison: () => {},
        stdout: quietStream,
        stderr: quietStream,
      }),
      /without complete comparison outputs/u,
    );
    assert.equal(existsSync(path.join(output, 'comparison.json')), false);
    assert.deepEqual(stagingEntries(output), []);
  });
});
