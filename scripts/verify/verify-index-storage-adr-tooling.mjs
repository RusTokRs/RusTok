#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-adr-tooling] ${message}`);
  process.exit(1);
};

const renderer = read('scripts/verify/render-index-storage-adr.mjs');
const fixture = read('scripts/verify/render-index-storage-adr.test.mjs');
const guide = read('crates/rustok-index/docs/storage-decision.md');
const schema = JSON.parse(read('crates/rustok-index/docs/storage-decision.schema.json'));
const example = JSON.parse(read('crates/rustok-index/docs/storage-decision.example.json'));

for (const marker of [
  "const prototypes = ['jsonb', 'typed_eav', 'hot_projection']",
  "'same_result_digest_contract'",
  "comparison.methodology?.automatic_winner_selection !== false",
  'comparison must contain exactly one ${scale} evidence entry',
  'comparison decision contract ${field} is not satisfied',
  'decision.comparison_commit must match the evidence comparison commit',
  'decision.rejection_rationales must contain exactly',
  'comparison.cross_scale_ratios',
  'cross-scale prototype order',
  'read workload order differs across scales',
  'mutation workload order differs across scales',
  'const render = (comparison, decision) =>',
  'renderer does not infer or rank a winning prototype',
]) {
  if (!renderer.includes(marker)) fail(`ADR renderer is missing contract marker: ${marker}`);
}

for (const forbidden of [
  'Comparison input:',
  'comparisonPath',
  'automatic_winner_selection: true',
  'selected_prototype =',
  'sort((left, right) => left.',
]) {
  if (renderer.includes(forbidden)) fail(`ADR renderer contains forbidden behavior: ${forbidden}`);
}

for (const marker of [
  "test('renders a manual same-commit storage ADR'",
  "test('rejects evidence that is not decision-ready'",
  "test('rejects a decision tied to another commit'",
  "test('requires rationale for every rejected alternative'",
  "test('rejects an unsatisfied evidence decision flag'",
]) {
  if (!fixture.includes(marker)) fail(`ADR renderer fixture is missing scenario: ${marker}`);
}

if (schema.$schema !== 'https://json-schema.org/draft/2020-12/schema') {
  fail('storage decision schema must use JSON Schema draft 2020-12');
}
if (schema.properties?.$schema?.const !== './storage-decision.schema.json') {
  fail('storage decision schema must allow the colocated schema reference used by the example');
}
if (!schema.properties?.selected_prototype?.enum
    || JSON.stringify(schema.properties.selected_prototype.enum)
      !== JSON.stringify(['jsonb', 'typed_eav', 'hot_projection'])) {
  fail('storage decision schema prototype enum drifted');
}
if (!Array.isArray(schema.allOf) || schema.allOf.length !== 3) {
  fail('storage decision schema must define one rejection contract per selected prototype');
}
for (const field of [
  'status',
  'decision_date',
  'owner',
  'comparison_commit',
  'selected_prototype',
  'selection_rationale',
  'rejection_rationales',
  'operational_tradeoffs',
  'migration_strategy',
  'rollback_strategy',
]) {
  if (!schema.required?.includes(field)) fail(`storage decision schema is missing required field ${field}`);
}

if (example.$schema !== './storage-decision.schema.json') {
  fail('storage decision example must reference the colocated schema');
}
if (!schema.properties.selected_prototype.enum.includes(example.selected_prototype)) {
  fail('storage decision example selected prototype is invalid');
}
const rejected = schema.properties.selected_prototype.enum
  .filter((prototype) => prototype !== example.selected_prototype)
  .sort();
if (JSON.stringify(Object.keys(example.rejection_rationales ?? {}).sort()) !== JSON.stringify(rejected)) {
  fail('storage decision example must reject exactly the unselected prototypes');
}

for (const marker of [
  'render-index-storage-adr.mjs',
  'storage-decision.schema.json',
  'decision_ready: true',
  'The renderer fails closed unless:',
  'It never infers or ranks a winner.',
  'not on the filesystem path used to invoke the renderer',
]) {
  if (!guide.includes(marker)) fail(`storage decision guide is missing marker: ${marker}`);
}

console.log('[verify-index-storage-adr-tooling] ADR renderer, schema, example, fixtures and guide are consistent');
