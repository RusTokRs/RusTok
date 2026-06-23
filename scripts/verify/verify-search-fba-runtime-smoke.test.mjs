import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const repoRoot = process.cwd();
const script = path.join(repoRoot, 'scripts/verify/verify-search-fba-runtime-smoke.mjs');

function run(root = repoRoot) {
  return spawnSync(process.execPath, [script], {
    cwd: repoRoot,
    env: { ...process.env, SEARCH_FBA_ROOT: root },
    encoding: 'utf8',
  });
}

function copyFixture() {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'search-fba-runtime-smoke-'));
  for (const file of [
    'crates/rustok-search/contracts/search-fba-registry.json',
    'crates/rustok-search/contracts/evidence/search-runtime-fallback-smoke.json',
    'crates/rustok-search/src/ports.rs',
    'crates/rustok-search/src/pg_engine.rs',
    'crates/rustok-search/src/suggestions.rs',
  ]) {
    const target = path.join(tmp, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(repoRoot, file), target);
  }
  return tmp;
}

function assert(condition, message) {
  if (!condition) {
    console.error(`[verify-search-fba-runtime-smoke.test] ${message}`);
    process.exit(1);
  }
}

const success = run();
assert(success.status === 0, `expected repository fixture to pass\nSTDOUT:\n${success.stdout}\nSTDERR:\n${success.stderr}`);

const missingLocale = copyFixture();
const portsPath = path.join(missingLocale, 'crates/rustok-search/src/ports.rs');
fs.writeFileSync(
  portsPath,
  fs.readFileSync(portsPath, 'utf8').replaceAll('request.locale.get_or_insert_with(|| context.locale.clone())', '/* locale fallback removed */'),
);
const localeResult = run(missingLocale);
assert(localeResult.status !== 0, 'expected missing locale fallback marker to fail');
assert(localeResult.stderr.includes('locale') || localeResult.stderr.includes('source marker'), `expected locale/source marker failure, got ${localeResult.stderr}`);

const missingMode = copyFixture();
const smokePath = path.join(missingMode, 'crates/rustok-search/contracts/evidence/search-runtime-fallback-smoke.json');
const smoke = JSON.parse(fs.readFileSync(smokePath, 'utf8'));
smoke.cases = smoke.cases.map((testCase) => ({
  ...testCase,
  degraded_modes: testCase.degraded_modes.filter((mode) => mode !== 'hide_suggestions'),
}));
fs.writeFileSync(smokePath, `${JSON.stringify(smoke, null, 2)}\n`);
const modeResult = run(missingMode);
assert(modeResult.status !== 0, 'expected missing degraded mode to fail');
assert(modeResult.stderr.includes('hide_suggestions'), `expected degraded mode failure, got ${modeResult.stderr}`);

console.log('[verify-search-fba-runtime-smoke.test] fixture coverage passed');
