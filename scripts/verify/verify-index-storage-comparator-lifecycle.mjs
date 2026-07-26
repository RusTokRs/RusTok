#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-comparator-lifecycle] ${message}`);
  process.exit(1);
};

const wrapper = read('scripts/verify/compare-index-storage-evidence.mjs');
const core = read('scripts/verify/compare-index-storage-evidence-core.mjs');
const fixture = read('scripts/verify/compare-index-storage-evidence-lifecycle.test.mjs');
const router = read('scripts/verify/index-storage-tooling.mjs');
const guide = read('crates/rustok-index/docs/storage-decision.md');

const requireMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (!content.includes(marker)) fail(`${label} is missing contract marker: ${marker}`);
  }
};

requireMarkers(wrapper, 'comparator lifecycle wrapper', [
  "const corePath = fileURLToPath(new URL('./compare-index-storage-evidence-core.mjs', import.meta.url))",
  "if (args.length !== 1) throw new Error('help must be the only argument')",
  "if (outputProvided) throw new Error('--output was provided more than once')",
  'export const runComparatorCoreWithAtomicComparison = ({',
  'spawn = spawnSync',
  'finalizeComparison = finalizeDatabaseSettingsContract',
  'rename = renameSync',
  'rmSync(outputJson, { force: true })',
  "const stagingRoot = mkdtempSync(path.join(output, '.comparison-staging-'))",
  "args.push('--output', stagingRoot)",
  'const status = runCore({ args, spawn, stdout, stderr })',
  'finalizeComparison({ inputs, output: stagingRoot })',
  "throw new Error('comparator core exited successfully without complete comparison outputs')",
  'rename(stagedMarkdown, outputMarkdown)',
  'rename(stagedJson, outputJson)',
  'rmSync(stagingRoot, { recursive: true, force: true })',
  "rmSync(path.join(parsed.output, 'comparison.json'), { force: true })",
  'for (const input of parsed.inputs) validatePacketReadOrdering(input)',
]);

for (const forbidden of [
  "await import('./compare-index-storage-evidence-core.mjs')",
  'shell: true',
  'execSync(',
  'execFileSync(',
]) {
  if (wrapper.includes(forbidden)) fail(`comparator lifecycle wrapper contains forbidden behavior: ${forbidden}`);
}

const staleRevoke = wrapper.indexOf("rmSync(path.join(parsed.output, 'comparison.json'), { force: true })");
const ordering = wrapper.indexOf('for (const input of parsed.inputs) validatePacketReadOrdering(input)');
const lifecycleRun = wrapper.indexOf('const status = runComparatorCoreWithAtomicComparison(parsed)');
if ([staleRevoke, ordering, lifecycleRun].some((index) => index < 0)
    || !(staleRevoke < ordering && ordering < lifecycleRun)) {
  fail('real comparison ordering must be stale JSON revocation -> packet ordering preflight -> isolated comparator lifecycle');
}

const stagingCreate = wrapper.indexOf("const stagingRoot = mkdtempSync(path.join(output, '.comparison-staging-'))");
const coreRun = wrapper.indexOf('const status = runCore({ args, spawn, stdout, stderr })');
const methodology = wrapper.indexOf('finalizeComparison({ inputs, output: stagingRoot })');
const markdownPublish = wrapper.indexOf('rename(stagedMarkdown, outputMarkdown)');
const jsonPublish = wrapper.indexOf('rename(stagedJson, outputJson)');
const cleanup = wrapper.indexOf('rmSync(stagingRoot, { recursive: true, force: true })');
if ([stagingCreate, coreRun, methodology, markdownPublish, jsonPublish, cleanup].some((index) => index < 0)
    || !(stagingCreate < coreRun
      && coreRun < methodology
      && methodology < markdownPublish
      && markdownPublish < jsonPublish
      && jsonPublish < cleanup)) {
  fail('comparator lifecycle order must be staging -> core -> methodology -> Markdown publication -> JSON publication -> cleanup');
}

requireMarkers(core, 'byte-preserved comparator core', [
  'const die = (message) =>',
  "const resultDigestContract = 'ordered_length_prefixed_json_v1'",
  'const canonicalPrototypes = [',
  'decision_ready: decisionContract.required_scales_present',
  "writeFileSync(path.join(output, 'comparison.json')",
  "writeFileSync(path.join(output, 'comparison.md')",
]);

requireMarkers(fixture, 'comparator lifecycle fixture', [
  "test('direct comparator help is valid only as the sole argument'",
  "test('direct comparator rejects duplicate output before evidence access'",
  "test('comparator publishes finalized markdown before JSON and removes staging'",
  "test('comparator core failure cannot publish a partial decision input'",
  "test('post-processing failure leaves no decision input or staging residue'",
  "test('missing staged output after successful core leaves no decision input'",
  "assert.deepEqual(publication, ['comparison.md', 'comparison.json'])",
  "entry.startsWith('.comparison-staging-')",
  'assert.deepEqual(stagingEntries(output), [])',
]);

requireMarkers(router, 'storage tooling router', [
  "'verify-index-storage-comparator-lifecycle.mjs'",
  "scriptPath('compare-index-storage-evidence-lifecycle.test.mjs')",
  "runScript('compare-index-storage-evidence.mjs', args)",
]);

requireMarkers(guide, 'storage decision guide', [
  'A real comparison attempt revokes any previous `comparison.json` decision-input marker before packet ordering preflight.',
  'The comparator core and PostgreSQL methodology finalization run in a unique private staging directory.',
  'Finalized Markdown is published before `comparison.json`, so JSON is the last success marker exposed to decision preparation.',
]);

console.log('[verify-index-storage-comparator-lifecycle] strict comparator arguments, stale-marker revocation, child-process isolation, staged methodology, Markdown-first/JSON-last publication, cleanup, fixtures, router registration, and docs are cross-guarded');
