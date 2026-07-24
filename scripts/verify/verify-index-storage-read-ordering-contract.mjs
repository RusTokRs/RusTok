#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-read-ordering-contract] ${message}`);
  process.exit(1);
};

const preflight = read('scripts/verify/check-index-storage-read-ordering.mjs');
const fixture = read('scripts/verify/check-index-storage-read-ordering.test.mjs');
const router = read('scripts/verify/index-storage-tooling.mjs');
const smokeWorkflow = read('.github/workflows/index-storage-smoke.yml');
const scaleWorkflow = read('.github/workflows/index-storage-scale-evidence.yml');
const scaleRunWorkflow = read('.github/workflows/index-storage-scale-run.yml');

const requireMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (!content.includes(marker)) fail(`${label} is missing contract marker: ${marker}`);
  }
};

requireMarkers(preflight, 'read ordering preflight', [
  "const canonicalPrototypes = ['jsonb', 'typed_eav', 'hot_projection']",
  "'status_equality'",
  "'price_range_sort'",
  "'multi_value_tag'",
  "'two_hop_channel_filter'",
  "'keyset_page'",
  "'exact_count'",
  'sql.trimEnd().endsWith(marker)',
  'must end with canonical ordering marker',
  'validatePacketReadOrdering',
  'source workload order',
  'prototype order',
]);
if (preflight.includes('sql.includes(marker)')) {
  fail('read ordering preflight restored substring-only ordering validation');
}

requireMarkers(fixture, 'read ordering fixture', [
  "test('accepts canonical terminal ordering with trailing whitespace'",
  "test('rejects a source ordering marker that exists only in a nested query'",
  "test('rejects a candidate ordering marker that exists only in a comment'",
  "test('rejects workload order drift before checking SQL text'",
]);

requireMarkers(router, 'storage tooling router', [
  "'verify-index-storage-read-ordering-contract.mjs'",
  "scriptPath('check-index-storage-read-ordering.test.mjs')",
  "runScript('check-index-storage-read-ordering.mjs', ['--input', packetRoot])",
  "runScript('check-index-storage-read-ordering.mjs', orderingArgs)",
  "runScript('validate-index-storage-evidence.mjs', [], environment)",
  "runScript('compare-index-storage-evidence.mjs', args)",
]);
const packetOrdering = router.indexOf("runScript('check-index-storage-read-ordering.mjs', ['--input', packetRoot])");
const packetValidator = router.indexOf("runScript('validate-index-storage-evidence.mjs', [], environment)");
if (packetOrdering < 0 || packetValidator < 0 || packetOrdering > packetValidator) {
  fail('packet terminal ordering preflight must run before the canonical validator');
}
const compareOrdering = router.indexOf("runScript('check-index-storage-read-ordering.mjs', orderingArgs)");
const comparator = router.indexOf("runScript('compare-index-storage-evidence.mjs', args)");
if (compareOrdering < 0 || comparator < 0 || compareOrdering > comparator) {
  fail('comparison terminal ordering preflight must run before the canonical comparator');
}

for (const [label, workflow] of [
  ['smoke workflow', smokeWorkflow],
  ['scale workflow', scaleWorkflow],
]) {
  requireMarkers(workflow, label, [
    'scripts/verify/check-index-storage-read-ordering.mjs',
    'scripts/verify/check-index-storage-read-ordering.test.mjs',
    'scripts/verify/verify-index-storage-read-ordering-contract.mjs',
    'node --check scripts/verify/check-index-storage-read-ordering.mjs',
    'node --check scripts/verify/check-index-storage-read-ordering.test.mjs',
  ]);
}
requireMarkers(scaleRunWorkflow, 'scale run workflow', [
  'node scripts/verify/index-storage-tooling.mjs packet',
]);

console.log('[verify-index-storage-read-ordering-contract] terminal ordering preflight, fixtures, router, and workflows are consistent');
