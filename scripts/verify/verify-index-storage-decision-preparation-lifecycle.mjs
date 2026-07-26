#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-decision-preparation-lifecycle] ${message}`);
  process.exit(1);
};

const preparation = read('scripts/verify/prepare-index-storage-decision.mjs');
const fixture = read('scripts/verify/prepare-index-storage-decision-lifecycle.test.mjs');
const router = read('scripts/verify/index-storage-tooling.mjs');
const guide = read('crates/rustok-index/docs/storage-decision.md');

const requireMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (!content.includes(marker)) fail(`${label} is missing contract marker: ${marker}`);
  }
};

requireMarkers(preparation, 'decision preparation lifecycle', [
  'const allowedArguments = new Set([',
  "'--comparison'",
  "'--selected'",
  "'--owner'",
  "'--date'",
  "'--output'",
  "if (args.length !== 1) fail('help must be the only argument')",
  "if (force) fail('--force was provided more than once')",
  '!allowedArguments.has(argument)',
  'if (values.has(argument)) fail(`${argument} was provided more than once`)',
  'const main = () =>',
  'if (args === null) return 0',
  '--output must not overwrite the comparison input',
  'refusing to overwrite existing decision without --force',
  "const stagingRoot = mkdtempSync(path.join(parent || '.', `.${path.basename(args.output)}.tmp-`))",
  'const stagedOutput = path.join(stagingRoot, path.basename(args.output))',
  'if (args.force) rmSync(args.output, { force: true })',
  'const { comparison, sha256 } = readComparison(args.comparison)',
  "writeFileSync(stagedOutput, `${JSON.stringify(decision, null, 2)}\\n`, 'utf8')",
  'renameSync(stagedOutput, args.output)',
  'rmSync(stagingRoot, { recursive: true, force: true })',
  'process.exitCode = main()',
]);

for (const forbidden of [
  'process.exit(1)',
  'const stagedOutput = `${args.output}.tmp-${process.pid}`',
  "if (!argument.startsWith('--')",
]) {
  if (preparation.includes(forbidden)) {
    fail(`decision preparation lifecycle contains forbidden behavior: ${forbidden}`);
  }
}

const parse = preparation.indexOf('const args = parseArgs();');
const outputCollision = preparation.indexOf('const resolvedOutput = path.resolve(args.output)');
const overwriteGate = preparation.indexOf('if (existsSync(args.output) && !args.force)');
const stagingCreate = preparation.indexOf('const stagingRoot = mkdtempSync');
const staleRevoke = preparation.indexOf('if (args.force) rmSync(args.output, { force: true })');
const comparisonRead = preparation.indexOf('const { comparison, sha256 } = readComparison(args.comparison)');
const stagedWrite = preparation.indexOf('writeFileSync(stagedOutput');
const publish = preparation.indexOf('renameSync(stagedOutput, args.output)');
const cleanup = preparation.indexOf('rmSync(stagingRoot, { recursive: true, force: true })');
if ([
  parse,
  outputCollision,
  overwriteGate,
  stagingCreate,
  staleRevoke,
  comparisonRead,
  stagedWrite,
  publish,
  cleanup,
].some((index) => index < 0)
    || !(parse < outputCollision
      && outputCollision < overwriteGate
      && overwriteGate < stagingCreate
      && stagingCreate < staleRevoke
      && staleRevoke < comparisonRead
      && comparisonRead < stagedWrite
      && stagedWrite < publish
      && publish < cleanup)) {
  fail('decision preparation lifecycle order must be parse -> collision/overwrite gates -> unique staging -> forced stale revocation -> comparison validation -> staged write -> atomic publication -> cleanup');
}

requireMarkers(fixture, 'decision preparation lifecycle fixture', [
  "test('help is successful only as the sole argument'",
  "test('unknown forced arguments preserve an existing decision'",
  "test('duplicate arguments preserve an existing decision'",
  "test('output collision is non-destructive'",
  "test('non-forced preparation preserves an existing decision'",
  "test('forced preparation revokes stale output before comparison validation'",
  "test('forced preparation revokes stale output before comparison access'",
  "test('successful forced preparation publishes one fresh draft atomically'",
  "entry.startsWith('.decision.json.tmp-')",
  'assert.deepEqual(stagingEntries(root), [])',
]);

requireMarkers(router, 'storage tooling router', [
  "'verify-index-storage-decision-preparation-lifecycle.mjs'",
  "scriptPath('prepare-index-storage-decision-lifecycle.test.mjs')",
  "runScript('prepare-index-storage-decision.mjs', args)",
]);

requireMarkers(guide, 'storage decision guide', [
  'The preparation command accepts only `--comparison`, `--selected`, `--owner`, `--date`, and `--output`, plus one standalone `--force` flag.',
  'A valid forced replacement creates a unique same-directory staging location before withdrawing the stale draft, then validates the comparison and publishes the complete replacement with one rename.',
  'Failure after stale-draft withdrawal leaves no decision file and no staging residue.',
]);

console.log('[verify-index-storage-decision-preparation-lifecycle] strict preparation arguments, non-destructive preflight, forced stale revocation, unique staging, atomic publication, cleanup, fixtures, router registration, and docs are cross-guarded');
