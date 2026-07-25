#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const readBytes = (filename) => readFileSync(new URL(filename, root));
const readText = (filename) => readBytes(filename).toString('utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-standalone-tools] ${message}`);
  process.exit(1);
};

const gitBlobSha = (bytes) => createHash('sha1')
  .update(`blob ${bytes.length}\0`)
  .update(bytes)
  .digest('hex');

const validatorPath = 'scripts/verify/validate-index-storage-evidence.mjs';
const validatorCorePath = 'scripts/verify/validate-index-storage-evidence-core.mjs';
const comparatorPath = 'scripts/verify/compare-index-storage-evidence.mjs';
const comparatorCorePath = 'scripts/verify/compare-index-storage-evidence-core.mjs';

const validator = readText(validatorPath);
const validatorCoreBytes = readBytes(validatorCorePath);
const comparator = readText(comparatorPath);
const comparatorCoreBytes = readBytes(comparatorCorePath);
const standaloneFixture = readText('scripts/verify/index-storage-standalone-tools.test.mjs');
const sourceOracleGuard = readText('scripts/verify/verify-index-storage-source-oracle.mjs');

for (const [label, actual, expected] of [
  ['validator core', gitBlobSha(validatorCoreBytes), 'dabc18d59360c300352ab3afb2510f0a0ff22796'],
  ['comparator core', gitBlobSha(comparatorCoreBytes), '97ef0e8a216735e457c4c827d975462b84b009b3'],
]) {
  if (actual !== expected) fail(`${label} is no longer the byte-preserved implementation: ${actual}`);
}

const requireMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (!content.includes(marker)) fail(`${label} is missing marker: ${marker}`);
  }
};
const forbidMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (content.includes(marker)) fail(`${label} contains forbidden marker: ${marker}`);
  }
};

requireMarkers(validator, 'validator wrapper', [
  "import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs'",
  "const supportedScales = new Set(['smoke', '100k', '1m'])",
  'validatePacketReadOrdering(evidenceRoot);',
  "await import('./validate-index-storage-evidence-core.mjs')",
]);
const validatorPreflight = validator.indexOf('validatePacketReadOrdering(evidenceRoot);');
const validatorCore = validator.indexOf("await import('./validate-index-storage-evidence-core.mjs')");
if (validatorPreflight < 0 || validatorCore < 0 || validatorPreflight > validatorCore) {
  fail('validator wrapper must run terminal-ordering preflight before importing its core');
}

requireMarkers(comparator, 'comparator wrapper', [
  "import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs'",
  "import { spawnSync } from 'node:child_process'",
  "const corePath = fileURLToPath(new URL('./compare-index-storage-evidence-core.mjs', import.meta.url))",
  "if (argument === '--help' || argument === '-h')",
  "if (args.length !== 1) throw new Error('help must be the only argument')",
  'let outputProvided = false;',
  "if (outputProvided) throw new Error('--output was provided more than once')",
  "argument === '--input'",
  "argument === '--output'",
  "rmSync(path.join(parsed.output, 'comparison.json'), { force: true });",
  'for (const input of parsed.inputs) validatePacketReadOrdering(input);',
  'export const runComparatorCoreWithAtomicComparison = ({',
  "const stagingRoot = mkdtempSync(path.join(output, '.comparison-staging-'));",
  'spawn(process.execPath, [corePath, ...args]',
  'finalizeComparison({ inputs, output: stagingRoot });',
  'renameSync(stagedMarkdown, outputMarkdown);',
  'renameSync(stagedJson, outputJson);',
  'rmSync(stagingRoot, { recursive: true, force: true });',
  'const status = runComparatorCoreWithAtomicComparison(parsed);',
]);
const comparatorHelpGate = comparator.indexOf(
  "if (args.length !== 1) throw new Error('help must be the only argument')",
);
const comparatorOutputGate = comparator.indexOf(
  "if (outputProvided) throw new Error('--output was provided more than once')",
);
const comparatorRevoke = comparator.indexOf(
  "rmSync(path.join(parsed.output, 'comparison.json'), { force: true });",
);
const comparatorPreflight = comparator.indexOf(
  'for (const input of parsed.inputs) validatePacketReadOrdering(input);',
);
const comparatorAtomicRun = comparator.indexOf('const status = runComparatorCoreWithAtomicComparison(parsed);');
if (comparatorRevoke < 0
    || comparatorPreflight < 0
    || comparatorAtomicRun < 0
    || comparatorRevoke > comparatorPreflight
    || comparatorPreflight > comparatorAtomicRun) {
  fail('comparator wrapper must revoke stale comparison before preflight and stage core output afterward');
}
if (comparatorHelpGate < 0
    || comparatorOutputGate < 0
    || comparatorHelpGate > comparatorRevoke
    || comparatorOutputGate > comparatorRevoke) {
  fail('comparator wrapper must reject ambiguous control arguments before changing output state');
}
const markdownPublish = comparator.indexOf('renameSync(stagedMarkdown, outputMarkdown);');
const jsonPublish = comparator.indexOf('renameSync(stagedJson, outputJson);');
if (markdownPublish < 0 || jsonPublish < 0 || markdownPublish > jsonPublish) {
  fail('comparator wrapper must publish comparison.json last as the decision-input success marker');
}

requireMarkers(standaloneFixture, 'standalone evidence fixture', [
  "test('direct comparator revokes stale comparison before ordering preflight'",
  "test('comparator publishes finalized JSON last and removes staging'",
  "test('comparator core failure cannot publish a partial decision input'",
  "test('comparison post-processing failure leaves no decision input'",
  'assert.deepEqual(stagingEntries(output), [])',
]);

requireMarkers(sourceOracleGuard, 'source-oracle guard', [
  "read('scripts/verify/validate-index-storage-evidence-core.mjs')",
  "read('scripts/verify/compare-index-storage-evidence-core.mjs')",
  'packet validator core missing strict guard',
  'evidence comparator core missing contract guard',
  'temporary compatibility marker shim',
]);

forbidMarkers(validator, 'validator wrapper', [
  'migrated to inspect the preserved core directly',
  'const resultDigestContract =',
  'shell: true',
  'execSync(',
  'spawnSync(',
]);
forbidMarkers(comparator, 'comparator wrapper', [
  'migrated to inspect the preserved core directly',
  'const resultDigestContract =',
  'shell: true',
  'execSync(',
  "await import('./compare-index-storage-evidence-core.mjs')",
]);

console.log('[verify-index-storage-standalone-tools] validator ordering and comparator argument, staging, cleanup, and JSON-last publication contracts are cross-guarded');
