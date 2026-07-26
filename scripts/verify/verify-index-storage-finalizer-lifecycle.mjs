#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-finalizer-lifecycle] ${message}`);
  process.exit(1);
};

const finalizer = read('scripts/verify/finalize-index-storage-adr.mjs');
const fixture = read('scripts/verify/finalize-index-storage-adr-decision-contract.test.mjs');
const decisionFixture = read('scripts/verify/index-storage-decision-tooling.test.mjs');
const router = read('scripts/verify/index-storage-tooling.mjs');
const guide = read('crates/rustok-index/docs/storage-decision.md');

const requireMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (!content.includes(marker)) fail(`${label} is missing contract marker: ${marker}`);
  }
};

requireMarkers(finalizer, 'accepted ADR finalizer', [
  "const allowedArguments = new Set(['--comparison', '--decision', '--output'])",
  "if (args.length === 1 && (args[0] === '--help' || args[0] === '-h'))",
  "fail('--help/-h must be the only argument')",
  'if (!allowedArguments.has(argument)) fail(`unknown argument: ${argument}`)',
  'if (values.has(argument)) fail(`${argument} was provided more than once`)',
  'const requireAcceptedDecision = (decision) =>',
  'decision.status must be accepted before ADR finalization',
  'const leapYear = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);',
  'year === 0 || month < 1 || month > 12 || day < 1 || day > monthLengths[month - 1]',
  'const stagedOutput = `${args.output}.tmp-${process.pid}`;',
  'ADR staging path must not overwrite the ${label} input',
  'rmSync(args.output, { force: true });',
  'rmSync(stagedOutput, { force: true });',
  "const comparison = readJsonBytes(args.comparison, 'comparison');",
  'requireComparisonDatabaseSettingsMethodology(comparison.value, fail);',
  'requireAcceptedDecision(decision.value);',
  "requireIsoCalendarDate(decision.value.decision_date, 'decision.decision_date');",
  "path.join(scriptDirectory, 'render-index-storage-adr.mjs')",
  'writeFileSync(stagedOutput, markdown, \'utf8\')',
  'renameSync(stagedOutput, args.output)',
  'rmSync(temporaryRoot, { recursive: true, force: true })',
]);

for (const forbidden of ['shell: true', 'execSync(', 'process.exit(']) {
  if (finalizer.includes(forbidden)) fail(`accepted ADR finalizer contains forbidden behavior: ${forbidden}`);
}

const collision = finalizer.indexOf('if (resolvedOutput === resolvedInput) fail(`--output must not overwrite the ${label} input`);');
const stagingCollision = finalizer.indexOf('if (resolvedStagedOutput === resolvedInput)');
const revokeOutput = finalizer.indexOf('rmSync(args.output, { force: true });');
const revokeStaging = finalizer.indexOf('rmSync(stagedOutput, { force: true });');
const evidenceRead = finalizer.indexOf("const comparison = readJsonBytes(args.comparison, 'comparison');");
const methodology = finalizer.indexOf('requireComparisonDatabaseSettingsMethodology(comparison.value, fail);');
const accepted = finalizer.indexOf('requireAcceptedDecision(decision.value);');
const calendarDate = finalizer.indexOf("requireIsoCalendarDate(decision.value.decision_date, 'decision.decision_date');");
const renderer = finalizer.indexOf('const result = spawnSync(process.execPath, [');
const stageWrite = finalizer.indexOf("writeFileSync(stagedOutput, markdown, 'utf8')");
const publish = finalizer.indexOf('renameSync(stagedOutput, args.output)');
if ([collision, stagingCollision, revokeOutput, revokeStaging, evidenceRead, methodology,
  accepted, calendarDate, renderer, stageWrite, publish].some((index) => index < 0)
    || !(collision < stagingCollision
      && stagingCollision < revokeOutput
      && revokeOutput < revokeStaging
      && revokeStaging < evidenceRead
      && evidenceRead < methodology
      && methodology < accepted
      && accepted < calendarDate
      && calendarDate < renderer
      && renderer < stageWrite
      && stageWrite < publish)) {
  fail('finalizer order must be collision gates -> stale revocation -> evidence/methodology -> accepted decision/date -> renderer -> staged publication');
}

requireMarkers(fixture, 'accepted ADR finalizer fixture', [
  "test('finalizer accepts help only as the sole argument'",
  "test('finalizer rejects mixed help without changing an existing ADR'",
  "test('finalizer rejects unknown options without changing an existing ADR'",
  "test('finalizer output collision preserves the comparison bytes'",
  "test('finalizer staging collision preserves the comparison bytes'",
  "test('real finalization attempts revoke stale ADR and staging before evidence access'",
  "test('finalizer rejects impossible decision dates without leaving stale output'",
  "test('finalizer accepts a real leap date before renderer validation'",
  "test('finalizer rejects a proposed decision without leaving stale output'",
  "test('renderer failure leaves neither stale ADR nor staged output'",
  "'0000-01-01'",
  "decision({ decision_date: '2024-02-29' })",
  'assert.equal(existsSync(outputPath), false)',
  'assert.deepEqual(stagingEntries(root), [])',
]);

requireMarkers(decisionFixture, 'canonical decision fixture', [
  "source_oracle: 'normalized idx_bench_source workload result digests'",
  "result_digest: 'ordered_length_prefixed_json_v1'",
  "evidence_validation: 'fail closed on report shape, metrics, plans, effects, ordering, digest semantics, and cardinalities'",
  "first_run: 'first EXPLAIN ANALYZE repetition'",
  "warm_run: 'median after the first repetition; not a guaranteed OS cold-cache comparison'",
  "status: 'accepted'",
  "assert.equal(decision.status, 'proposed')",
  "decision.status = 'accepted'",
  'comparison methodology must contain exactly the canonical methodology fields',
]);

requireMarkers(router, 'Index storage router', [
  "'verify-index-storage-finalizer-lifecycle.mjs'",
  "scriptPath('finalize-index-storage-adr-decision-contract.test.mjs')",
  "scriptPath('index-storage-decision-tooling.test.mjs')",
  "runScript('finalize-index-storage-adr.mjs', args)",
]);

requireMarkers(guide, 'storage decision guide', [
  'The generated draft has `status: proposed`.',
  'The finalizer rejects a proposed decision',
  'The finalizer accepts only `--comparison`, `--decision`, and `--output`',
  'A valid replacement attempt revokes any existing ADR and process-specific staged output before evidence is read.',
  '- the decision status is `accepted`;',
  '- `decision_date` is a real ISO calendar date using Gregorian month and leap-year rules;',
  'the exact eight-field methodology envelope',
]);

console.log('[verify-index-storage-finalizer-lifecycle] accepted status, Gregorian dates, non-destructive CLI/collisions, stale-output revocation, staged publication, canonical fixtures, router registration, and docs are cross-guarded');
