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
const orderingPreflight = read('scripts/verify/check-index-storage-read-ordering.mjs');
const orderingFixture = read('scripts/verify/check-index-storage-read-ordering.test.mjs');
const orderingGuard = read('scripts/verify/verify-index-storage-read-ordering-contract.mjs');
const standaloneFixture = read('scripts/verify/index-storage-standalone-tools.test.mjs');
const standaloneGuard = read('scripts/verify/verify-index-storage-standalone-tools.mjs');
const databaseSettingsContract = read('scripts/verify/index-storage-database-settings-contract.mjs');
const renderer = read('scripts/verify/render-index-storage-adr.mjs');
const rendererFixture = read('scripts/verify/render-index-storage-adr.test.mjs');
const preparer = read('scripts/verify/prepare-index-storage-decision.mjs');
const finalizer = read('scripts/verify/finalize-index-storage-adr.mjs');
const verifier = read('scripts/verify/verify-index-storage-adr.mjs');
const fixture = read('scripts/verify/index-storage-decision-tooling.test.mjs');
const finalizerContractFixture = read('scripts/verify/finalize-index-storage-adr-decision-contract.test.mjs');
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
  "'verify-index-storage-read-ordering-contract.mjs'",
  "'verify-index-storage-standalone-tools.mjs'",
  "'verify-index-storage-adr-integrity.mjs'",
  "scriptPath('check-index-storage-read-ordering.test.mjs')",
  "scriptPath('index-storage-standalone-tools.test.mjs')",
  "scriptPath('finalize-index-storage-adr-decision-contract.test.mjs')",
  "runScript('check-index-storage-read-ordering.mjs', ['--input', packetRoot])",
  "runScript('check-index-storage-read-ordering.mjs', orderingArgs)",
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
  "test('packet runs terminal ordering preflight before the canonical validator'",
  "test('compare runs terminal ordering preflight before the canonical comparator'",
  "test('forwards decision preparation help without rewriting its arguments'",
  "test('forwards ADR finalization help without rewriting its arguments'",
  "test('forwards ADR verification help without rewriting its arguments'",
  "'verify-adr'",
]);

requireMarkers(orderingPreflight, 'terminal ordering preflight', [
  'const maskSqlText = (text) =>',
  'const executableSqlText = (sql, label) =>',
  "sql.startsWith('--', index)",
  "sql.startsWith('/*', index)",
  'unterminated block comment',
  'contains an unterminated ${kind}',
  'unterminated dollar-quoted string',
  'executableSql.trimEnd().endsWith(marker)',
  'must end with canonical ordering marker',
  'in executable SQL',
  'validatePacketReadOrdering',
]);
for (const forbidden of ['sql.includes(marker)', 'sql.trimEnd().endsWith(marker)']) {
  if (orderingPreflight.includes(forbidden)) {
    fail(`terminal ordering preflight restored unsafe raw-SQL validation: ${forbidden}`);
  }
}
requireMarkers(orderingFixture, 'terminal ordering fixture', [
  "test('accepts comment tokens inside strings and comments after executable ordering'",
  "test('rejects a source ordering marker that exists only in a nested query'",
  "test('rejects a candidate ordering marker that exists only in a block comment'",
  "test('rejects a terminal ordering marker hidden in a line comment'",
  "test('rejects an ordering marker hidden in a dollar-quoted string'",
  "test('rejects unterminated SQL comments before ordering validation'",
]);
requireMarkers(orderingGuard, 'terminal ordering guard', [
  "const preflight = read('scripts/verify/check-index-storage-read-ordering.mjs')",
  "const fixture = read('scripts/verify/check-index-storage-read-ordering.test.mjs')",
  "const standaloneGuard = read('scripts/verify/verify-index-storage-standalone-tools.mjs')",
  'executableSql.trimEnd().endsWith(marker)',
  "test('packet runs terminal ordering preflight before the canonical validator'",
  "test('compare runs terminal ordering preflight before the canonical comparator'",
  'standalone validator must preflight ordering before importing its core',
  'standalone comparator must preflight every input before importing its core',
  'packet terminal ordering preflight must run before the canonical validator',
  'comparison terminal ordering preflight must run before the canonical comparator',
  'scripts/verify/verify-index-storage-read-ordering-contract.mjs',
]);

requireMarkers(standaloneGuard, 'standalone evidence guard', [
  "const gitBlobSha = (bytes) => createHash('sha1')",
  "'dabc18d59360c300352ab3afb2510f0a0ff22796'",
  "'97ef0e8a216735e457c4c827d975462b84b009b3'",
  'validator wrapper must run terminal-ordering preflight before importing its core',
  'comparator wrapper must preflight every input before importing its core',
]);
requireMarkers(standaloneFixture, 'standalone evidence fixture', [
  "test('direct validator rejects non-executable terminal ordering before its core'",
  "test('direct comparator rejects nested-only terminal ordering before its core'",
  "test('direct validator reaches the byte-preserved core after a valid preflight'",
  "test('direct comparator forwards help to the byte-preserved core'",
]);

requireMarkers(databaseSettingsContract, 'database settings contract', [
  'export const comparableDatabaseFields = Object.freeze([',
  "'standard_conforming_strings'",
  "'timezone'",
  "'date_style'",
  "'extra_float_digits'",
  'export const databaseSettingsSource =',
  'export const requireComparisonDatabaseSettingsMethodology = (comparison, fail) =>',
]);

requireMarkers(renderer, 'ADR renderer', [
  "from './index-storage-database-settings-contract.mjs'",
  'requireComparisonDatabaseSettingsMethodology(comparison, fail);',
  'if (comparison.decision_ready !== true)',
  'decision.comparison_sha256 must match the exact comparison.json bytes',
  'renderer does not infer or rank a winning prototype',
]);
const rendererMethodologyGate = renderer.indexOf(
  'requireComparisonDatabaseSettingsMethodology(comparison, fail);',
);
const rendererDecisionReadyGate = renderer.indexOf('if (comparison.decision_ready !== true)');
if (rendererMethodologyGate < 0
    || rendererDecisionReadyGate < 0
    || rendererMethodologyGate > rendererDecisionReadyGate) {
  fail('ADR renderer must validate observed database-settings methodology before decision_ready');
}
requireMarkers(rendererFixture, 'ADR renderer fixture', [
  'comparable_database_fields: [...comparableDatabaseFields]',
  'database_settings_source: databaseSettingsSource',
  "test('rejects core-only comparison without observed database-settings methodology'",
  'comparable_database_fields must exactly match the canonical PostgreSQL database-settings contract',
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
  "const allowedArguments = new Set(['--comparison', '--decision', '--output'])",
  "if (args.length === 1 && (args[0] === '--help' || args[0] === '-h'))",
  "fail('--help/-h must be the only argument')",
  'if (!allowedArguments.has(argument)) fail(`unknown argument: ${argument}`)',
  'const requiredDecisionKeys = [',
  "const allowedDecisionKeys = new Set(['$schema', ...requiredDecisionKeys])",
  'decision is missing required field ${key}',
  'decision contains unsupported field ${key}',
  'decision.$schema must reference ./storage-decision.schema.json when present',
  'const requireAcceptedDecision = (decision) =>',
  'decision.status must be accepted before ADR finalization',
  'const leapYear = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);',
  'year === 0 || month < 1 || month > 12 || day < 1 || day > monthLengths[month - 1]',
  'decision.decision_date must be a real ISO calendar date',
  'requireAcceptedDecision(decision.value);',
  'still contains a preparation placeholder',
  'const stagedOutput = `${args.output}.tmp-${process.pid}`;',
  'ADR staging path must not overwrite the ${label} input',
  'rmSync(args.output, { force: true });',
  'rmSync(stagedOutput, { force: true });',
  'writeFileSync(comparisonPath, comparison.bytes)',
  'writeFileSync(decisionPath, decision.bytes)',
  "path.join(scriptDirectory, 'render-index-storage-adr.mjs')",
  'Decision SHA-256:',
  'rmSync(temporaryRoot, { recursive: true, force: true })',
]);
const finalizerOutputCollisionGate = finalizer.indexOf(
  'if (resolvedOutput === resolvedInput) fail(`--output must not overwrite the ${label} input`);',
);
const finalizerStagingCollisionGate = finalizer.indexOf(
  'if (resolvedStagedOutput === resolvedInput)',
);
const finalizerOutputRevocation = finalizer.indexOf('rmSync(args.output, { force: true });');
const finalizerStagingRevocation = finalizer.indexOf('rmSync(stagedOutput, { force: true });');
const finalizerEvidenceRead = finalizer.indexOf("const comparison = readJsonBytes(args.comparison, 'comparison');");
if (finalizerOutputCollisionGate < 0
    || finalizerStagingCollisionGate < 0
    || finalizerOutputRevocation < 0
    || finalizerStagingRevocation < 0
    || finalizerEvidenceRead < 0
    || finalizerOutputCollisionGate > finalizerStagingCollisionGate
    || finalizerStagingCollisionGate > finalizerOutputRevocation
    || finalizerOutputRevocation > finalizerStagingRevocation
    || finalizerStagingRevocation > finalizerEvidenceRead) {
  fail('ADR finalizer must check output and staging collisions, revoke stale publication paths, then read evidence');
}
forbidMarkers(finalizer, 'ADR finalizer', ['shell: true', 'execSync(', 'process.exit(']);

requireMarkers(finalizerContractFixture, 'ADR finalizer decision-contract fixture', [
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
  '--help/-h must be the only argument',
  'unknown argument: --format',
  "source_oracle: 'normalized idx_bench_source workload result digests'",
  "result_digest: 'ordered_length_prefixed_json_v1'",
  'comparable_database_fields: [...comparableDatabaseFields]',
  'database_settings_source: databaseSettingsSource',
  "'0000-01-01'",
  "decision({ decision_date: '2024-02-29' })",
  'decision.status must be accepted before ADR finalization',
  'decision.decision_date must be a real ISO calendar date',
  'assert.equal(existsSync(outputPath), false)',
  'assert.deepEqual(stagingEntries(root), [])',
]);

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
  "assert.equal(decision.status, 'proposed')",
  "status: 'accepted'",
  "decision.status = 'accepted';",
  "Object.hasOwn(decision, '$schema'), false",
  'String.fromCharCode(96)',
  'ADR bytes differ from deterministic finalization',
]);

requireMarkers(guide, 'storage decision guide', [
  'index-storage-tooling.mjs prepare',
  'index-storage-tooling.mjs render',
  'index-storage-tooling.mjs verify-adr',
  'Only `comparison.json` emitted through the official comparator wrapper is valid decision input.',
  'Direct output from `compare-index-storage-evidence-core.mjs` is intentionally incomplete',
  'exact ordered `comparable_database_fields` contract',
  'The comparator rejects intra-packet or cross-scale drift in any field',
  'The generated draft has `status: proposed`.',
  'Change it to `accepted` only after the evidence and rationales have been reviewed.',
  'a year from `0001` through `9999`',
  'Malformed command lines and output paths that collide with either input are non-destructive.',
  'A valid replacement attempt revokes any existing ADR and process-specific staged output before evidence is read.',
  'The standalone renderer enforces the same methodology contract even when invoked directly.',
  'Recomputing `comparison_sha256` after removing or changing the methodology does not make the input acceptable.',
  'Comparison SHA-256',
  'Decision SHA-256',
  'repeats deterministic finalization including the observed database-settings gate',
  'match the regenerated Markdown byte for byte',
  'Any manual edit, formatting change, stale decision, replaced evidence file, or methodology drift is rejected.',
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
    'scripts/verify/check-index-storage-read-ordering.mjs',
    'scripts/verify/check-index-storage-read-ordering.test.mjs',
    'scripts/verify/verify-index-storage-read-ordering-contract.mjs',
    'scripts/verify/validate-index-storage-evidence-core.mjs',
    'scripts/verify/compare-index-storage-evidence-core.mjs',
    'scripts/verify/index-storage-standalone-tools.test.mjs',
    'scripts/verify/verify-index-storage-standalone-tools.mjs',
    'node --check scripts/verify/check-index-storage-read-ordering.mjs',
    'node --check scripts/verify/check-index-storage-read-ordering.test.mjs',
    'node --check scripts/verify/verify-index-storage-read-ordering-contract.mjs',
    'node --check scripts/verify/index-storage-standalone-tools.test.mjs',
    'node --check scripts/verify/verify-index-storage-standalone-tools.mjs',
    'scripts/verify/verify-index-storage-adr.mjs',
    'node --check scripts/verify/verify-index-storage-adr.mjs',
    'scripts/verify/verify-index-storage-adr-integrity.mjs',
    'node --check scripts/verify/verify-index-storage-adr-integrity.mjs',
  ]);
}

console.log('[verify-index-storage-adr-integrity] executable SQL ordering, observed database-settings methodology, standalone evidence entrypoints, atomic decision preparation, strict finalizer CLI and replacement lifecycle, accepted byte-bound finalization, saved ADR verification, fixtures, docs, and workflows are cross-guarded');
