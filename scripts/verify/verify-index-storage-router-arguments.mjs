#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-router-arguments] ${message}`);
  process.exit(1);
};

const router = read('scripts/verify/index-storage-tooling.mjs');
const fixture = read('scripts/verify/index-storage-tooling-arguments.test.mjs');
const guide = read('crates/rustok-index/docs/storage-decision.md');

const requireMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (!content.includes(marker)) fail(`${label} is missing contract marker: ${marker}`);
  }
};

requireMarkers(router, 'storage tooling router', [
  'const [command, ...args] = process.argv.slice(2)',
  'if (!command) {',
  "if (command === '--help' || command === '-h') {",
  "if (args.length !== 0) fail('help must be the only argument')",
  "if (scale !== null) fail('--scale was provided more than once')",
  "if (root !== null) fail('--root was provided more than once')",
  "'verify-index-storage-router-arguments.mjs'",
  "scriptPath('index-storage-tooling-arguments.test.mjs')",
]);

for (const forbidden of [
  "if (!command || command === '--help' || command === '-h')",
  "scale = args[++index];\n    } else if (argument === '--root'",
]) {
  if (router.includes(forbidden)) fail(`storage tooling router contains ambiguous behavior: ${forbidden}`);
}

const commandParse = router.indexOf('const [command, ...args] = process.argv.slice(2)');
const missingCommand = router.indexOf('if (!command) {');
const helpGate = router.indexOf("if (command === '--help' || command === '-h') {");
const switchDispatch = router.indexOf('switch (command) {');
if ([commandParse, missingCommand, helpGate, switchDispatch].some((index) => index < 0)
    || !(commandParse < missingCommand && missingCommand < helpGate && helpGate < switchDispatch)) {
  fail('global router arguments must be validated before command dispatch');
}

const scaleDuplicate = router.indexOf("if (scale !== null) fail('--scale was provided more than once')");
const scaleAssign = router.indexOf('scale = args[++index]');
const rootDuplicate = router.indexOf("if (root !== null) fail('--root was provided more than once')");
const rootAssign = router.indexOf('root = args[++index]');
const scaleValidation = router.indexOf("if (!['smoke', '100k', '1m'].includes(scale))");
if ([scaleDuplicate, scaleAssign, rootDuplicate, rootAssign, scaleValidation].some((index) => index < 0)
    || !(scaleDuplicate < scaleAssign
      && rootDuplicate < rootAssign
      && scaleAssign < scaleValidation
      && rootAssign < scaleValidation)) {
  fail('duplicate packet options must fail before assignment and scale validation');
}

requireMarkers(fixture, 'router argument fixture', [
  "test('prints usage for an empty command line'",
  "test('accepts global help only as the sole argument'",
  "for (const help of ['--help', '-h'])",
  "test('rejects duplicate packet scale before invoking evidence tooling'",
  "test('rejects duplicate packet root before invoking evidence tooling'",
  "test('duplicate packet options fail before later value validation'",
  'assert.doesNotMatch(result.stderr, /check-index-storage-read-ordering/u)',
  'assert.doesNotMatch(result.stderr, /validate-index-storage-evidence/u)',
]);

requireMarkers(guide, 'storage decision guide', [
  'Global `--help` and `-h` are accepted only as the sole router argument.',
  'The `packet` command rejects repeated `--scale` or `--root` options before invoking ordering checks or evidence validation.',
]);

console.log('[verify-index-storage-router-arguments] global help and packet value options fail closed before dispatch, with fixture, router, and documentation coverage');
