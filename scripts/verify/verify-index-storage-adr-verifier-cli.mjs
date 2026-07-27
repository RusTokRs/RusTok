#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-adr-verifier-cli] ${message}`);
  process.exit(1);
};

const verifier = read('scripts/verify/verify-index-storage-adr.mjs');
const fixture = read('scripts/verify/index-storage-tooling.test.mjs');
const router = read('scripts/verify/index-storage-tooling.mjs');
const guide = read('crates/rustok-index/docs/storage-decision.md');

const requireMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (!content.includes(marker)) fail(`${label} is missing contract marker: ${marker}`);
  }
};

requireMarkers(verifier, 'saved ADR verifier', [
  "const allowedArguments = new Set(['--comparison', '--decision', '--adr'])",
  "if (args.length !== 1) fail('help must be the only argument')",
  '!allowedArguments.has(argument)',
  'args[index + 1].startsWith(\'--\')',
  'if (values.has(argument)) fail(`${argument} was provided more than once`)',
  'const args = parseArgs();',
  "const comparisonBytes = readBytes(args.comparison, 'comparison')",
  "const decisionBytes = readBytes(args.decision, 'decision')",
  "const adrBytes = readBytes(args.adr, 'ADR')",
]);

if (verifier.includes("if (!argument.startsWith('--')")) {
  fail('saved ADR verifier must not accept arbitrary key/value options');
}

const parse = verifier.indexOf('const args = parseArgs();');
const comparisonRead = verifier.indexOf("const comparisonBytes = readBytes(args.comparison, 'comparison')");
const decisionRead = verifier.indexOf("const decisionBytes = readBytes(args.decision, 'decision')");
const adrRead = verifier.indexOf("const adrBytes = readBytes(args.adr, 'ADR')");
if ([parse, comparisonRead, decisionRead, adrRead].some((index) => index < 0)
    || !(parse < comparisonRead && comparisonRead < decisionRead && decisionRead < adrRead)) {
  fail('saved ADR verifier must validate its complete CLI before reading supplied files');
}

requireMarkers(fixture, 'storage tooling fixture', [
  "test('rejects unknown ADR verification options before reading inputs'",
  "'--format', 'markdown'",
  '/unknown or incomplete argument: --format/u',
  "test('rejects ADR verification help combined with other arguments'",
  "run('verify-adr', '--help', '--adr', 'adr.md')",
  '/help must be the only argument/u',
]);

requireMarkers(router, 'storage tooling router', [
  "'verify-index-storage-adr-verifier-cli.mjs'",
  "runScript('verify-index-storage-adr.mjs', args)",
]);

requireMarkers(guide, 'storage decision guide', [
  'The saved-ADR verifier accepts only `--comparison`, `--decision`, and `--adr`.',
  'Help is valid only as the sole argument; unknown, incomplete, mixed-help, or duplicate options fail before any supplied comparison, decision, or ADR file is read.',
]);

console.log('[verify-index-storage-adr-verifier-cli] strict saved-ADR verifier arguments, pre-read rejection, fixtures, router registration, and docs are cross-guarded');
