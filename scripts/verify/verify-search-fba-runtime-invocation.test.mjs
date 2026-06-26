import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const repoRoot = process.cwd();
const script = path.join(repoRoot, 'scripts/verify/verify-search-fba-runtime-invocation.mjs');
const fixturePaths = [
  'crates/rustok-search/contracts/search-fba-registry.json',
  'crates/rustok-search/contracts/evidence/search-runtime-invocation-trace.json',
  'crates/rustok-search/src/ports.rs',
  'crates/rustok-search/README.md',
  'crates/rustok-search/docs/implementation-plan.md',
  'docs/modules/registry.md',
];

function fail(message) {
  console.error(`[verify-search-fba-runtime-invocation.test] ${message}`);
  process.exit(1);
}

function copyFixture(name) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), `search-fba-runtime-invocation-${name}-`));
  for (const relative of fixturePaths) {
    const source = path.join(repoRoot, relative);
    const target = path.join(tmp, relative);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(source, target);
  }
  return tmp;
}

function run(root) {
  return spawnSync(process.execPath, [script], {
    cwd: repoRoot,
    env: { ...process.env, SEARCH_FBA_ROOT: root },
    encoding: 'utf8',
  });
}

const baseline = run(repoRoot);
if (baseline.status !== 0) fail(`baseline failed: ${baseline.stderr || baseline.stdout}`);

const missingShortCircuit = copyFixture('missing-short-circuit');
const tracePath = path.join(missingShortCircuit, 'crates/rustok-search/contracts/evidence/search-runtime-invocation-trace.json');
const trace = JSON.parse(fs.readFileSync(tracePath, 'utf8'));
trace.cases[0].policy_denied_trace = ['require_read_policy', 'invoke_embedded_postgres_provider'];
fs.writeFileSync(tracePath, JSON.stringify(trace, null, 2));
const shortCircuitResult = run(missingShortCircuit);
if (shortCircuitResult.status === 0) fail('expected policy-denied short-circuit regression to fail');
if (!`${shortCircuitResult.stderr}${shortCircuitResult.stdout}`.includes('policy denied trace drift')) fail('short-circuit fixture failed for the wrong reason');

const missingError = copyFixture('missing-error');
const errorTracePath = path.join(missingError, 'crates/rustok-search/contracts/evidence/search-runtime-invocation-trace.json');
const errorTrace = JSON.parse(fs.readFileSync(errorTracePath, 'utf8'));
errorTrace.cases[1].typed_errors = errorTrace.cases[1].typed_errors.filter((entry) => !entry.startsWith('external:'));
fs.writeFileSync(errorTracePath, JSON.stringify(errorTrace, null, 2));
const errorResult = run(missingError);
if (errorResult.status === 0) fail('expected typed error regression to fail');
if (!`${errorResult.stderr}${errorResult.stdout}`.includes('typed error coverage drift')) fail('typed error fixture failed for the wrong reason');

const missingDocs = copyFixture('missing-docs');
const readmePath = path.join(missingDocs, 'crates/rustok-search/README.md');
fs.writeFileSync(readmePath, fs.readFileSync(readmePath, 'utf8').replaceAll('contracts/evidence/search-runtime-invocation-trace.json', 'contracts/evidence/removed.json'));
const docsResult = run(missingDocs);
if (docsResult.status === 0) fail('expected docs regression to fail');
if (!`${docsResult.stderr}${docsResult.stdout}`.includes('README missing invocation trace evidence')) fail('docs fixture failed for the wrong reason');

console.log('[verify-search-fba-runtime-invocation.test] fixture coverage passed');
