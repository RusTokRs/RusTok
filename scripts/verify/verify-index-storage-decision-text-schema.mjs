#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-decision-text-schema] ${message}`);
  process.exit(1);
};

const schema = JSON.parse(read('crates/rustok-index/docs/storage-decision.schema.json'));
const fixture = read('scripts/verify/storage-decision-schema-text.test.mjs');
const router = read('scripts/verify/index-storage-tooling.mjs');
const guide = read('crates/rustok-index/docs/storage-decision.md');
const prototypes = ['jsonb', 'typed_eav', 'hot_projection'];
const topLevelText = [
  'owner',
  'selection_rationale',
  'operational_tradeoffs',
  'migration_strategy',
  'rollback_strategy',
];

const requireTextContract = (definition, label) => {
  if (definition?.type !== 'string') fail(`${label} must be a string`);
  if (definition?.minLength !== 1) fail(`${label} must retain minLength 1`);
  if (definition?.pattern !== '\\S') {
    fail(`${label} must require at least one non-whitespace character`);
  }
};

for (const field of topLevelText) {
  requireTextContract(schema.properties?.[field], `storage decision schema ${field}`);
}

if (!Array.isArray(schema.allOf) || schema.allOf.length !== prototypes.length) {
  fail('storage decision schema must retain one conditional rejection branch per prototype');
}
for (const selected of prototypes) {
  const branch = schema.allOf.find(
    (entry) => entry?.if?.properties?.selected_prototype?.const === selected,
  );
  if (!branch) fail(`storage decision schema is missing the ${selected} branch`);
  const rejection = branch.then?.properties?.rejection_rationales;
  const expected = prototypes.filter((prototype) => prototype !== selected);
  if (rejection?.additionalProperties !== false) {
    fail(`${selected} rejection rationales must reject additional properties`);
  }
  if (JSON.stringify(rejection?.required) !== JSON.stringify(expected)) {
    fail(`${selected} rejection rationale requirements drifted`);
  }
  if (JSON.stringify(Object.keys(rejection?.properties ?? {})) !== JSON.stringify(expected)) {
    fail(`${selected} rejection rationale properties drifted`);
  }
  for (const rejected of expected) {
    requireTextContract(
      rejection.properties[rejected],
      `storage decision schema ${selected}.rejection_rationales.${rejected}`,
    );
  }
}

for (const marker of [
  "test('requires non-whitespace text for every top-level decision narrative'",
  "test('requires non-whitespace text for every conditional rejection rationale'",
  "test('rejects empty and whitespace-only values while accepting reviewed text'",
  "assert.equal(definition?.pattern, '\\\\S'",
  "for (const value of ['', ' ', '   ', '\\n', '\\t', ' \\n\\t '])",
  "for (const value of ['reviewed', ' reviewed ', '\\nreviewed\\t'])",
]) {
  if (!fixture.includes(marker)) fail(`decision text schema fixture is missing marker: ${marker}`);
}

for (const marker of [
  "'verify-index-storage-decision-text-schema.mjs'",
  "scriptPath('storage-decision-schema-text.test.mjs')",
]) {
  if (!router.includes(marker)) fail(`storage tooling router is missing marker: ${marker}`);
}

for (const marker of [
  'The decision schema requires at least one non-whitespace character in the owner and every selection, rejection, operational, migration, and rollback narrative.',
  'Whitespace-only text is rejected before a decision is treated as schema-valid.',
]) {
  if (!guide.includes(marker)) fail(`storage decision guide is missing marker: ${marker}`);
}

console.log('[verify-index-storage-decision-text-schema] top-level and conditional decision narratives require non-whitespace text, with fixture, router, and documentation coverage');
