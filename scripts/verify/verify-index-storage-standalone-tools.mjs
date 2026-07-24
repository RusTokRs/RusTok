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
  'migrated to inspect the preserved core directly',
]);
const validatorPreflight = validator.indexOf('validatePacketReadOrdering(evidenceRoot);');
const validatorCore = validator.indexOf("await import('./validate-index-storage-evidence-core.mjs')");
if (validatorPreflight < 0 || validatorCore < 0 || validatorPreflight > validatorCore) {
  fail('validator wrapper must run terminal-ordering preflight before importing its core');
}

requireMarkers(comparator, 'comparator wrapper', [
  "import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs'",
  "if (argument === '--help' || argument === '-h') return null",
  "argument === '--input'",
  "argument === '--output'",
  'for (const input of inputs) validatePacketReadOrdering(input);',
  "await import('./compare-index-storage-evidence-core.mjs')",
  'migrated to inspect the preserved core directly',
]);
const comparatorPreflight = comparator.indexOf('for (const input of inputs) validatePacketReadOrdering(input);');
const comparatorCore = comparator.indexOf("await import('./compare-index-storage-evidence-core.mjs')");
if (comparatorPreflight < 0 || comparatorCore < 0 || comparatorPreflight > comparatorCore) {
  fail('comparator wrapper must preflight every input before importing its core');
}

forbidMarkers(validator, 'validator wrapper', ['shell: true', 'execSync(', 'spawnSync(']);
forbidMarkers(comparator, 'comparator wrapper', ['shell: true', 'execSync(', 'spawnSync(']);

console.log('[verify-index-storage-standalone-tools] direct validator and comparator enforce executable SQL ordering before byte-preserved core execution');
