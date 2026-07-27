#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-hash-cli-contract] ${message}`);
  process.exit(1);
};

const helper = read('scripts/verify/hash-index-storage-comparison.mjs');
const fixture = read('scripts/verify/hash-index-storage-comparison-cli.test.mjs');
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

const soleHelp = helper.indexOf('if (args.length === 1 && helpArguments.has(args[0]))');
const mixedHelp = helper.indexOf('if (args.some((argument) => helpArguments.has(argument)))');
const arity = helper.indexOf("if (args.length !== 1) fail('exactly one comparison.json path is required')");
const evidenceRead = helper.indexOf('if (!existsSync(filename)) fail(`missing comparison file: ${filename}`)');
const digest = helper.indexOf("createHash('sha256').update(readFileSync(filename)).digest('hex')");
if ([soleHelp, mixedHelp, arity, evidenceRead, digest].some((index) => index < 0)
    || !(soleHelp < mixedHelp && mixedHelp < arity && arity < evidenceRead && evidenceRead < digest)) {
  fail('hash helper order must be sole help -> mixed-help rejection -> arity -> file access -> exact-byte digest');
}

requireMarkers(fixture, 'comparison hash CLI fixture', [
  "test('hash help aliases are valid only as the sole helper argument'",
  "test('mixed and repeated hash help fail before file access or digest output'",
  "test('hash helper requires exactly one comparison path'",
  "test('hash router digests exact comparison bytes without JSON normalization'",
  "['comparison.json', '--help']",
  "['--help', 'comparison.json']",
  "['comparison.json', '-h']",
  "['-h', 'comparison.json']",
  "['--help', '--help']",
  "['-h', '-h']",
  "Buffer.from('{\"scale\":\"100k\"}\\r\\n', 'utf8')",
  "createHash('sha256').update(bytes).digest('hex')",
  '/--help\\/-h must be the only argument/u',
  '/exactly one comparison\\.json path is required/u',
]);

requireMarkers(router, 'storage tooling router', [
  "'verify-index-storage-hash-cli-contract.mjs'",
  "scriptPath('hash-index-storage-comparison-cli.test.mjs')",
  "case 'hash':",
  "runScript('hash-index-storage-comparison.mjs', args)",
]);

requireMarkers(guide, 'storage decision guide', [
  'Hash help is accepted only as the sole argument.',
  'Mixed help/path invocations and zero or multiple comparison paths fail without producing a digest.',
  'The helper hashes the exact file bytes without JSON normalization.',
]);

console.log('[verify-index-storage-hash-cli-contract] comparison hash help, arity, exact-byte behavior, focused fixtures, router registration, and docs are cross-guarded');
