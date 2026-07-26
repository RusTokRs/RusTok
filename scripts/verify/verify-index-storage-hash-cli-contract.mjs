#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-hash-cli-contract] ${message}`);
  process.exit(1);
};

const helper = read('scripts/verify/hash-index-storage-comparison.mjs');
const fixture = read('scripts/verify/index-storage-tooling.test.mjs');
const router = read('scripts/verify/index-storage-tooling.mjs');
const guide = read('crates/rustok-index/docs/storage-decision.md');

const requireMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (!content.includes(marker)) fail(`${label} is missing contract marker: ${marker}`);
  }
};

requireMarkers(helper, 'comparison hash helper', [
  "const helpArguments = new Set(['--help', '-h'])",
  'if (args.length === 1 && helpArguments.has(args[0]))',
  'if (args.some((argument) => helpArguments.has(argument)))',
  "fail('--help/-h must be the only argument')",
  "if (args.length !== 1) fail('exactly one comparison.json path is required')",
  "createHash('sha256').update(readFileSync(filename)).digest('hex')",
  'process.stdout.write(`${digest}\\n`)',
]);
for (const forbidden of ["args.includes('--help')", "args.includes('-h')"]) {
  if (helper.includes(forbidden)) fail(`comparison hash helper restored ambiguous help detection: ${forbidden}`);
}

requireMarkers(fixture, 'storage tooling fixture', [
  "test('forwards hash help to the exact-byte comparison helper'",
  "test('forwards short hash help to the exact-byte comparison helper'",
  "test('rejects mixed hash help in every ordering'",
  "test('rejects missing or multiple comparison hash paths'",
  "test('hashes the exact comparison bytes through the router'",
  "['comparison.json', '--help']",
  "['--help', 'comparison.json']",
  "['comparison.json', '-h']",
  "['-h', 'comparison.json']",
  "['--help', '--help']",
  "Buffer.from('{\"scale\":\"100k\"}\\r\\n', 'utf8')",
  "createHash('sha256').update(bytes).digest('hex')",
  '/--help\\/-h must be the only argument/u',
  '/exactly one comparison\\.json path is required/u',
]);

requireMarkers(router, 'storage tooling router', [
  "'verify-index-storage-hash-cli-contract.mjs'",
  "case 'hash':",
  "runScript('hash-index-storage-comparison.mjs', args)",
]);

requireMarkers(guide, 'storage decision guide', [
  'Hash help is accepted only as the sole argument.',
  'Mixed help/path invocations and zero or multiple comparison paths fail without producing a digest.',
  'The helper hashes the exact file bytes without JSON normalization.',
]);

console.log('[verify-index-storage-hash-cli-contract] comparison hash help, arity, and exact-byte behavior are cross-guarded');
