#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-adr-integrity] ${message}`);
  process.exit(1);
};

const router = read('scripts/verify/index-storage-tooling.mjs');
const routerFixture = read('scripts/verify/index-storage-tooling.test.mjs');
const preparer = read('scripts/verify/prepare-index-storage-decision.mjs');
const finalizer = read('scripts/verify/finalize-index-storage-adr.mjs');
const verifier = read('scripts/verify/verify-index-storage-adr.mjs');
const fixture = read('scripts/verify/index-storage-decision-tooling.test.mjs');
const guide = read('crates/rustok-index/docs/storage-decision.md');
const smokeWorkflow = read('.github/workflows/index-storage-smoke.yml');
const scaleWorkflow = read('.github/workflows/index-storage-scale-evidence.yml');

const requireMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (!content.includes(marker)) fail(`${label} is missing contract marker: ${marker}`);
  }
};
const forbidMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (content.includes(marker)) fail(`${label} contains forbidden marker: ${marker}`);
  }
};

requireMarkers(router, 'storage tooling router', [
  "'verify-index-storage-adr-integrity.mjs'",
  "case 'prepare':",
  "runScript('prepare-index-storage-decision.mjs', args)",
  "case 'render':",
  "runScript('finalize-index-storage-adr.mjs', args)",
  "case 'verify-adr':",
  "runScript('verify-index-storage-adr.mjs', args)",
  "scriptPath('index-storage-decision-tooling.test.mjs')",
]);
forbidMarkers(router, 'storage tooling router', [
  "runScript('render-index-storage-adr.mjs', args)",
  'shell: true',
  'execSync(',
]);

requireMarkers(routerFixture, 'storage tooling router fixture', [
  "test('forwards decision preparation help without rewriting its arguments'",
  "test('forwards ADR finalization help without rewriting its arguments'",
  "test('forwards ADR verification help without rewriting its arguments'",
  "'verify-adr'",
]);

requireMarkers(preparer, 'decision preparer', [
  "const placeholderPrefix = 'TODO(index-storage-decision):'",
  'comparison.methodology?.automatic_winner_selection !== false',
  'comparison must contain exactly the 100k and 1m scales',
  "createHash('sha256').update(bytes).digest('hex')",
  '--output must not overwrite the comparison input',
  'refusing to overwrite existing decision without --force',
  'comparison_commit: commit',
  'comparison_sha256: sha256',
  'const stagedOutput = `${args.output}.tmp-${process.pid}`',
  'renameSync(stagedOutput, args.output)',
  'if (existsSync(stagedOutput)) rmSync(stagedOutput, { force: true })',
]);
forbidMarkers(preparer, 'decision preparer', [
  "$schema: './storage-decision.schema.json'",
  'automatic_winner_selection: true',
  'shell: true',
]);

requireMarkers(finalizer, 'ADR finalizer', [
  'const requiredDecisionKeys = [',
  "const allowedDecisionKeys = new Set(['$schema', ...requiredDecisionKeys])",
  'decision is missing required field ${key}',
  'decision contains unsupported field ${key}',
  'decision.$schema must reference ./storage-decision.schema.json when present',
  'still contains a preparation placeholder',
  'writeFileSync(comparisonPath, comparison.bytes)',
  'writeFileSync(decisionPath, decision.bytes)',
  "path.join(scriptDirectory, 'render-index-storage-adr.mjs')",
  'Decision SHA-256:',
  'const stagedOutput = `${args.output}.tmp-${process.pid}`',
  'rmSync(temporaryRoot, { recursive: true, force: true })',
]);
forbidMarkers(finalizer, 'ADR finalizer', ['shell: true', 'execSync(', 'process.exit(']);

requireMarkers(verifier, 'saved ADR verifier', [
  "const prefix = '[verify-index-storage-adr]'",
  "createHash('sha256').update(bytes).digest('hex')",
  '\\x60([0-9a-f]{64})\\x60',
  'ADR must contain exactly one ${label} SHA-256 line',
  'ADR ${label} SHA-256 does not match the exact input bytes',
  'writeFileSync(comparisonPath, comparisonBytes)',
  'writeFileSync(decisionPath, decisionBytes)',
  "path.join(scriptDirectory, 'finalize-index-storage-adr.mjs')",
  'adrBytes.equals(rerendered)',
  'ADR bytes differ from deterministic finalization',
  'rmSync(temporaryRoot, { recursive: true, force: true })',
]);
forbidMarkers(verifier, 'saved ADR verifier', ['shell: true', 'execSync(', 'process.exit(']);

requireMarkers(fixture, 'decision tooling fixture', [
  "test('prepares an exact-comparison-bound manual decision draft'",
  "test('never overwrites the comparison input even with force'",
  "test('rejects unsupported fields in the decision envelope'",
  "test('finalizes an ADR bound to exact comparison and decision bytes'",
  "test('rejects a saved ADR changed after finalization'",
  "Object.hasOwn(decision, '$schema'), false",
  'String.fromCharCode(96)',
  'ADR bytes differ from deterministic finalization',
]);

requireMarkers(guide, 'storage decision guide', [
  'index-storage-tooling.mjs prepare',
  'index-storage-tooling.mjs render',
  'index-storage-tooling.mjs verify-adr',
  'Comparison SHA-256',
  'Decision SHA-256',
  'repeats deterministic finalization',
  'match the regenerated Markdown byte for byte',
  'Any manual edit, formatting change, stale decision, or replaced evidence file is rejected.',
]);
forbidMarkers(guide, 'storage decision guide', [
  'node scripts/verify/render-index-storage-adr.mjs',
  'Copy the printed 64-character digest into `comparison_sha256`',
]);

for (const [label, workflow] of [
  ['smoke workflow', smokeWorkflow],
  ['scale workflow', scaleWorkflow],
]) {
  requireMarkers(workflow, label, [
    'scripts/verify/verify-index-storage-adr.mjs',
    'node --check scripts/verify/verify-index-storage-adr.mjs',
    'scripts/verify/verify-index-storage-adr-integrity.mjs',
    'node --check scripts/verify/verify-index-storage-adr-integrity.mjs',
  ]);
}

console.log('[verify-index-storage-adr-integrity] atomic decision preparation, byte-bound finalization, saved ADR verification, fixtures, docs, and workflows are consistent');
