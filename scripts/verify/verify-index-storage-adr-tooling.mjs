#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-adr-tooling] ${message}`);
  process.exit(1);
};

const router = read('scripts/verify/index-storage-tooling.mjs');
const routerFixture = read('scripts/verify/index-storage-tooling.test.mjs');
const renderer = read('scripts/verify/render-index-storage-adr.mjs');
const digestHelper = read('scripts/verify/hash-index-storage-comparison.mjs');
const fixture = read('scripts/verify/render-index-storage-adr.test.mjs');
const guide = read('crates/rustok-index/docs/storage-decision.md');
const schema = JSON.parse(read('crates/rustok-index/docs/storage-decision.schema.json'));
const example = JSON.parse(read('crates/rustok-index/docs/storage-decision.example.json'));

for (const marker of [
  "const prefix = '[index-storage-tooling]'",
  'const runNode = (args, label, environment = process.env) =>',
  "spawnSync(process.execPath, args",
  "'verify-index-fba.mjs'",
  "'verify-index-storage-source-oracle.mjs'",
  "'verify-index-storage-adr-tooling.mjs'",
  "'--test'",
  "scriptPath('compare-index-storage-evidence.test.mjs')",
  "scriptPath('render-index-storage-adr.test.mjs')",
  'INDEX_BENCH_SCALE: scale',
  'environment.INDEX_BENCH_EVIDENCE_ROOT = root',
  "case 'contract':",
  "case 'fixtures':",
  "case 'packet':",
  "case 'compare':",
  "case 'hash':",
  "case 'render':",
]) {
  if (!router.includes(marker)) fail(`storage tooling router is missing contract marker: ${marker}`);
}
for (const forbidden of [
  'shell: true',
  'execSync(',
  'execFileSync(',
  'spawnSync(command',
]) {
  if (router.includes(forbidden)) fail(`storage tooling router contains forbidden dispatch: ${forbidden}`);
}

for (const marker of [
  "test('prints the stable Index storage tooling command surface'",
  "test('forwards hash help to the exact-byte comparison helper'",
  "test('forwards comparator help without rewriting its arguments'",
  "test('forwards renderer help without rewriting its arguments'",
  "test('rejects unsupported packet scales before invoking the validator'",
  "test('rejects arguments for aggregate commands'",
  "test('rejects unknown commands'",
]) {
  if (!routerFixture.includes(marker)) fail(`storage tooling router fixture is missing scenario: ${marker}`);
}

for (const marker of [
  "import { createHash } from 'node:crypto'",
  "const prototypes = ['jsonb', 'typed_eav', 'hot_projection']",
  "'same_result_digest_contract'",
  'const readComparison = (filename) =>',
  "createHash('sha256').update(bytes).digest('hex')",
  'comparison.methodology?.automatic_winner_selection !== false',
  'comparison must contain exactly one ${scale} evidence entry',
  'comparison decision contract ${field} is not satisfied',
  'decision.comparison_commit must match the evidence comparison commit',
  'decision.comparison_sha256 must be a SHA-256 digest',
  'decision.comparison_sha256 must match the exact comparison.json bytes',
  'decision.rejection_rationales must contain exactly',
  'comparison.cross_scale_ratios',
  'cross-scale prototype order',
  'read workload order differs across scales',
  'mutation workload order differs across scales',
  'const render = (comparison, decision, comparisonSha256) =>',
  'Comparison SHA-256:',
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
  "createHash('sha256')",
  'exactly one comparison.json path is required',
  'readFileSync(filename)',
  'process.stdout.write(`${digest}\\n`)',
]) {
  if (!digestHelper.includes(marker)) fail(`comparison digest helper is missing marker: ${marker}`);
}

for (const marker of [
  "test('renders a manual same-commit storage ADR'",
  "test('rejects evidence that is not decision-ready'",
  "test('rejects a decision tied to another commit'",
  "test('rejects comparison bytes changed after the decision'",
  "test('requires rationale for every rejected alternative'",
  "test('rejects an unsatisfied evidence decision flag'",
  'comparison_sha256: sha256Json(comparison)',
]) {
  if (!fixture.includes(marker)) fail(`ADR renderer fixture is missing scenario: ${marker}`);
}

if (schema.$schema !== 'https://json-schema.org/draft/2020-12/schema') {
  fail('storage decision schema must use JSON Schema draft 2020-12');
}
if (schema.$id !== 'https://rustok.dev/schemas/index-storage-decision-v2.json') {
  fail('storage decision schema must use the exact-binding v2 identifier');
}
if (schema.properties?.$schema?.const !== './storage-decision.schema.json') {
  fail('storage decision schema must allow the colocated schema reference used by the example');
}
if (schema.properties?.comparison_sha256?.pattern !== '^[0-9a-fA-F]{64}$') {
  fail('storage decision schema must require a SHA-256 comparison digest');
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
  'comparison_sha256',
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
if (!/^[0-9a-f]{64}$/u.test(example.comparison_sha256 ?? '')) {
  fail('storage decision example must contain a SHA-256 comparison digest');
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
  'index-storage-tooling.mjs contract',
  'index-storage-tooling.mjs fixtures',
  'index-storage-tooling.mjs packet',
  'index-storage-tooling.mjs compare',
  'index-storage-tooling.mjs hash',
  'index-storage-tooling.mjs render',
  'without shell evaluation',
  'hash-index-storage-comparison.mjs',
  'comparison_sha256',
  'exact comparison-file bytes',
  'render-index-storage-adr.mjs',
  'storage-decision.schema.json',
  'decision_ready: true',
  'The renderer fails closed unless:',
  'It never infers or ranks a winner.',
  'not on the filesystem path used to invoke the renderer',
]) {
  if (!guide.includes(marker)) fail(`storage decision guide is missing marker: ${marker}`);
}

console.log('[verify-index-storage-adr-tooling] command router, ADR renderer, digest binding, schema, examples, fixtures and guide are consistent');
