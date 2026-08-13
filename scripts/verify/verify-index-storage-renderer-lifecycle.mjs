#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-renderer-lifecycle] ${message}`);
  process.exit(1);
};

const wrapper = read('scripts/verify/render-index-storage-adr.mjs');
const core = read('scripts/verify/render-index-storage-adr-core.mjs');
const fixture = read('scripts/verify/render-index-storage-adr.test.mjs');
const router = read('scripts/verify/index-storage-tooling.mjs');
const guide = read('crates/rustok-index/docs/storage-decision.md');

const requireMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (!content.includes(marker)) fail(`${label} is missing contract marker: ${marker}`);
  }
};

requireMarkers(wrapper, 'renderer lifecycle wrapper', [
  "const allowedArguments = new Set(['--comparison', '--decision', '--output'])",
  "if (args.length !== 1) fail('help must be the only argument')",
  'if (values.has(argument)) fail(`${argument} was provided more than once`)',
  '--output must not overwrite the ${label} input',
  'const delegatedCoreContractMarkers = Object.freeze([',
  'const requireDelegatedCoreContract = () =>',
  "const core = readFileSync(corePath, 'utf8')",
  'renderer core is missing contract marker: ${marker}',
  "const stagingRoot = mkdtempSync(path.join(parent || '.', `.${path.basename(args.output)}.tmp-`))",
  'const stagedOutput = path.join(stagingRoot, path.basename(args.output))',
  'rmSync(args.output, { force: true })',
  'const status = runCore(args, stagedOutput)',
  "if (!existsSync(stagedOutput)) fail('renderer core succeeded without producing staged ADR output')",
  'renameSync(stagedOutput, args.output)',
  'rmSync(stagingRoot, { recursive: true, force: true })',
]);
for (const forbidden of ['writeFileSync(args.output', 'randomUUID()', 'shell: true', 'execSync(']) {
  if (wrapper.includes(forbidden)) fail(`renderer lifecycle wrapper contains forbidden behavior: ${forbidden}`);
}

const outputCollision = wrapper.indexOf('const resolvedOutput = path.resolve(args.output)');
const coreContract = wrapper.indexOf('requireDelegatedCoreContract();');
const stagingCreate = wrapper.indexOf('const stagingRoot = mkdtempSync');
const outputRevoke = wrapper.indexOf('rmSync(args.output, { force: true })');
const coreRun = wrapper.indexOf('const status = runCore(args, stagedOutput)');
const publish = wrapper.indexOf('renameSync(stagedOutput, args.output)');
if ([outputCollision, coreContract, stagingCreate, outputRevoke, coreRun, publish].some((index) => index < 0)
    || !(outputCollision < coreContract
      && coreContract < stagingCreate
      && stagingCreate < outputRevoke
      && outputRevoke < coreRun
      && coreRun < publish)) {
  fail('renderer lifecycle order must be collision checks -> core contract -> unique staging -> stale revocation -> core render -> atomic publication');
}

requireMarkers(core, 'renderer core', [
  "import { createHash } from 'node:crypto'",
  "from './index-storage-database-settings-contract.mjs'",
  'requireComparisonDatabaseSettingsMethodology(comparison, fail);',
  'if (comparison.decision_ready !== true)',
  'decision.comparison_sha256 must match the exact comparison.json bytes',
  'const render = (comparison, decision, comparisonSha256) =>',
  'renderer does not infer or rank a winning prototype',
  'writeFileSync(args.output, markdown)',
]);

requireMarkers(fixture, 'renderer lifecycle fixture', [
  "source_oracle: 'normalized idx_bench_source workload result digests'",
  "result_digest: 'ordered_length_prefixed_json_v1'",
  "evidence_validation: 'fail closed on report shape, metrics, plans, effects, ordering, digest semantics, and cardinalities'",
  "first_run: 'first EXPLAIN ANALYZE repetition'",
  "warm_run: 'median after the first repetition; not a guaranteed OS cold-cache comparison'",
  "test('renders a manual same-commit storage ADR'",
  "test('never overwrites the comparison input'",
  "test('never overwrites the decision input'",
  "test('help is successful only as the sole argument'",
  "test('mixed help preserves an existing ADR output'",
  "test('unknown and duplicate options fail before changing output'",
  "test('real render attempts revoke stale ADR output before core validation'",
  "entry.startsWith('.adr.md.tmp-')",
  'assert.deepEqual(stagingEntries(root), [])',
]);

requireMarkers(router, 'storage tooling router', [
  "'verify-index-storage-renderer-lifecycle.mjs'",
  "scriptPath('render-index-storage-adr.test.mjs')",
]);

requireMarkers(guide, 'storage decision guide', [
  'The directly invocable renderer accepts help only as a sole argument and rejects unknown, incomplete, or duplicate options before changing files.',
  'A real render attempt withdraws any stale output before evidence validation, writes into a unique same-directory staging location, and publishes the completed Markdown with one rename.',
  'Failure leaves neither an old final ADR nor staging residue.',
]);

console.log('[verify-index-storage-renderer-lifecycle] strict direct-renderer CLI, stale-output revocation, unique staging, delegated core validation, atomic publication, fixtures, and docs are cross-guarded');
