#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-methodology-envelope] ${message}`);
  process.exit(1);
};

const contract = read('scripts/verify/index-storage-database-settings-contract.mjs');
const fixture = read('scripts/verify/comparison-methodology-envelope.test.mjs');
const preparer = read('scripts/verify/prepare-index-storage-decision.mjs');
const renderer = read('scripts/verify/render-index-storage-adr-core.mjs');
const router = read('scripts/verify/index-storage-tooling.mjs');

const requireMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (!content.includes(marker)) fail(`${label} is missing contract marker: ${marker}`);
  }
};

requireMarkers(contract, 'comparison methodology contract', [
  'export const comparisonMethodologyKeys = Object.freeze([',
  "'source_oracle'",
  "'result_digest'",
  "'evidence_validation'",
  "'first_run'",
  "'warm_run'",
  "'automatic_winner_selection'",
  "'comparable_database_fields'",
  "'database_settings_source'",
  'const actualKeys = Object.keys(methodology).sort()',
  'const expectedKeys = [...comparisonMethodologyKeys].sort()',
  "fail('comparison methodology must contain exactly the canonical methodology fields')",
  'methodology.comparable_database_fields',
  'methodology.database_settings_source',
]);

const keysStart = contract.indexOf('export const comparisonMethodologyKeys = Object.freeze([');
const shapeGate = contract.indexOf('const actualKeys = Object.keys(methodology).sort()');
const databaseFieldsGate = contract.indexOf('if (!sameJson(methodology.comparable_database_fields');
const sourceGate = contract.indexOf('if (methodology.database_settings_source !== databaseSettingsSource)');
if ([keysStart, shapeGate, databaseFieldsGate, sourceGate].some((index) => index < 0)
    || !(keysStart < shapeGate && shapeGate < databaseFieldsGate && databaseFieldsGate < sourceGate)) {
  fail('comparison methodology validation must check exact keys before PostgreSQL field and source values');
}

requireMarkers(preparer, 'decision preparer', [
  "from './index-storage-database-settings-contract.mjs'",
  'requireComparisonDatabaseSettingsMethodology(comparison, fail)',
]);
requireMarkers(renderer, 'ADR renderer core', [
  "from './index-storage-database-settings-contract.mjs'",
  'requireComparisonDatabaseSettingsMethodology(comparison, fail)',
]);

requireMarkers(fixture, 'comparison methodology fixture', [
  "test('accepts exactly the canonical comparison methodology envelope'",
  "test('rejects a missing comparison methodology field'",
  "test('rejects a renamed comparison methodology field'",
  "test('rejects an additional comparison methodology field'",
  "test('decision preparation rejects methodology drift before publishing a draft'",
  "test('direct ADR rendering rejects methodology drift before publishing output'",
  'assert.equal(existsSync(decisionPath), false)',
  'assert.equal(existsSync(outputPath), false)',
]);

requireMarkers(router, 'storage tooling router', [
  "'verify-index-storage-methodology-envelope.mjs'",
  "scriptPath('comparison-methodology-envelope.test.mjs')",
]);

console.log('[verify-index-storage-methodology-envelope] exact eight-field comparison methodology shape is enforced before decision preparation and ADR rendering, with focused fixtures and router coverage');
