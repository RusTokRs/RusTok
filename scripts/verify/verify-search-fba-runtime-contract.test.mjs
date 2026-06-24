import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const repoRoot = process.cwd();
const script = path.join(repoRoot, 'scripts/verify/verify-search-fba-runtime-contract.mjs');
const fixtureFiles = [
  'crates/rustok-search/contracts/search-fba-registry.json',
  'crates/rustok-search/contracts/evidence/search-runtime-contract-smoke.json',
  'crates/rustok-search/src/ports.rs',
  'crates/rustok-search/src/pg_engine.rs',
  'crates/rustok-search/src/suggestions.rs',
  'crates/rustok-search/README.md',
  'crates/rustok-search/docs/implementation-plan.md',
  'docs/modules/registry.md',
];
function assert(condition, message) {
  if (!condition) {
    console.error(`[verify-search-fba-runtime-contract.test] ${message}`);
    process.exit(1);
  }
}
function run(root = repoRoot) {
  return spawnSync(process.execPath, [script], {
    cwd: repoRoot,
    env: { ...process.env, SEARCH_FBA_ROOT: root },
    encoding: 'utf8',
  });
}
function copyFixture() {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'search-fba-runtime-contract-'));
  for (const file of fixtureFiles) {
    const target = path.join(tmp, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(repoRoot, file), target);
  }
  return tmp;
}

const success = run();
assert(success.status === 0, `expected repository fixture to pass\nSTDOUT:\n${success.stdout}\nSTDERR:\n${success.stderr}`);

const reordered = copyFixture();
const portsPath = path.join(reordered, 'crates/rustok-search/src/ports.rs');
const ports = fs.readFileSync(portsPath, 'utf8');
fs.writeFileSync(
  portsPath,
  ports.replace(
    '        context.require_policy(PortCallPolicy::read())?;\n        request.locale.get_or_insert_with(|| context.locale.clone());',
    '        request.locale.get_or_insert_with(|| context.locale.clone());\n        context.require_policy(PortCallPolicy::read())?;',
  ),
);
const reorderedResult = run(reordered);
assert(reorderedResult.status !== 0, 'expected reordered policy/locale markers to fail');
assert(reorderedResult.stderr.includes('locale fallback must happen after read policy'), `expected order failure, got ${reorderedResult.stderr}`);

const missingDoc = copyFixture();
const readmePath = path.join(missingDoc, 'crates/rustok-search/README.md');
fs.writeFileSync(readmePath, fs.readFileSync(readmePath, 'utf8').replaceAll('contracts/evidence/search-runtime-contract-smoke.json', 'contracts/evidence/removed.json'));
const docResult = run(missingDoc);
assert(docResult.status !== 0, 'expected missing README evidence reference to fail');
assert(docResult.stderr.includes('README'), `expected README failure, got ${docResult.stderr}`);

console.log('[verify-search-fba-runtime-contract.test] fixture coverage passed');
