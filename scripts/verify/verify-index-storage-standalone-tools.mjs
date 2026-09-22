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
const sourceOracleGuard = readText('scripts/verify/verify-index-storage-source-oracle.mjs');

for (const [label, actual, expected] of [
  ['validator core', gitBlobSha(validatorCoreBytes), '6523312e0f760cc5f4f57a687f40c3dae1f07873'],
  ['comparator core', gitBlobSha(comparatorCoreBytes), '17baf03638426871acd9e908dc026b012b446424'],
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
const requireOrder = (content, label, markers) => {
  let previous = -1;
  for (const marker of markers) {
    const index = content.indexOf(marker);
    if (index < 0) fail(`${label} is missing ordered marker: ${marker}`);
    if (index <= previous) fail(`${label} has lifecycle markers out of order: ${marker}`);
    previous = index;
  }
};

requireMarkers(validator, 'validator wrapper', [
  "import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs'",
  "import { runValidatorCoreWithAtomicProvenance } from './run-index-storage-validator-core.mjs'",
  "import { validateRunnerResourceSnapshots } from './validate-index-storage-runner-resources.mjs'",
  "const supportedScales = new Set(['smoke', '100k', '1m'])",
  "const corePath = fileURLToPath(new URL('./validate-index-storage-evidence-core.mjs', import.meta.url))",
  'runValidatorCoreWithAtomicProvenance({ evidenceRoot, corePath })',
]);
requireOrder(validator, 'validator wrapper', [
  'invalidatePacketProvenance(evidenceRoot);',
  'validatePacketReadOrdering(evidenceRoot);',
  'validateRunnerResourceSnapshots(evidenceRoot);',
  'runValidatorCoreWithAtomicProvenance({ evidenceRoot, corePath })',
]);

requireMarkers(comparator, 'comparator wrapper', [
  "import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs'",
  "const corePath = fileURLToPath(new URL('./compare-index-storage-evidence-core.mjs', import.meta.url))",
  "if (argument === '--help' || argument === '-h')",
  "argument === '--input'",
  "argument === '--output'",
  'runComparatorCoreWithAtomicComparison(parsed)',
]);
requireOrder(comparator, 'comparator wrapper', [
  "rmSync(path.join(parsed.output, 'comparison.json'), { force: true });",
  'for (const input of parsed.inputs) validatePacketReadOrdering(input);',
  'runComparatorCoreWithAtomicComparison(parsed)',
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
  "await import('./validate-index-storage-evidence-core.mjs')",
]);
forbidMarkers(comparator, 'comparator wrapper', [
  'migrated to inspect the preserved core directly',
  'const resultDigestContract =',
  'shell: true',
  'execSync(',
  "await import('./compare-index-storage-evidence-core.mjs')",
]);

console.log('[verify-index-storage-standalone-tools] direct validator and comparator preserve ordering, lifecycle, and byte-preserved core execution');
