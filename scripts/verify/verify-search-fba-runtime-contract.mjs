import fs from 'node:fs';
import path from 'node:path';

const root = process.env.SEARCH_FBA_ROOT || process.cwd();
const registryPath = 'crates/rustok-search/contracts/search-fba-registry.json';
const contractPath = 'crates/rustok-search/contracts/evidence/search-runtime-contract-smoke.json';
function resolve(repoPath) { return path.join(root, repoPath); }
function read(repoPath) { return fs.readFileSync(resolve(repoPath), 'utf8'); }
function json(repoPath) { return JSON.parse(read(repoPath)); }
function fail(message) { console.error(`[verify-search-fba-runtime-contract] ${message}`); process.exit(1); }
function assert(condition, message) { if (!condition) fail(message); }
function indexOfOrFail(text, marker, label) {
  const index = text.indexOf(marker);
  assert(index >= 0, `${label} missing source marker ${marker}`);
  return index;
}
function sameSet(actual, expected, label) {
  const a = [...actual].sort().join('|');
  const e = [...expected].sort().join('|');
  assert(a === e, `${label} drift: expected ${e}, got ${a}`);
}

const registry = json(registryPath);
const contract = json(contractPath);
assert(contract.generated_from === registryPath, 'runtime contract generated_from drift');
assert(contract.runner === 'scripts/verify/verify-search-fba-runtime-contract.mjs', 'runtime contract runner drift');
assert(contract.status === 'executable_no_compile', 'runtime contract status drift');
assert(contract.contract_version === registry.contract_version, 'runtime contract version drift');
assert(registry.evidence.runtime_contract_smoke === contractPath, 'registry runtime contract evidence path drift');
assert(registry.evidence.runtime_contract_smoke_runner === contract.runner, 'registry runtime contract runner drift');
assert(registry.contract_tests.runtime_contract_smoke?.runner === contract.runner, 'registry contract_tests runtime runner drift');
assert(registry.contract_tests.runtime_contract_smoke?.status === contract.status, 'registry contract_tests runtime status drift');

const registryOps = registry.contract_tests.cases.map((c) => c.operation);
sameSet(contract.cases.map((c) => c.operation), registryOps, 'runtime contract operation set');
const portNames = new Set(registry.ports.map((p) => p.name));
const consumerProfiles = new Set(registry.consumers.map((c) => `${c.module}.${c.profile}`));

const portsSource = read(contract.source_markers.provider_source);
for (const testCase of contract.cases) {
  assert(portNames.has(testCase.port), `${testCase.operation} references unknown port ${testCase.port}`);
  for (const consumerPath of testCase.consumer_paths) {
    assert(consumerProfiles.has(consumerPath), `${testCase.operation} references unknown consumer ${consumerPath}`);
  }
  for (const marker of testCase.source_markers) indexOfOrFail(portsSource, marker, testCase.operation);
  const implStart = indexOfOrFail(portsSource, `impl ${testCase.port} for PgSearchEngine`, testCase.operation);
  const policyIndex = portsSource.indexOf('context.require_policy(PortCallPolicy::read())?', implStart);
  const localeIndex = portsSource.indexOf('request.locale.get_or_insert_with(|| context.locale.clone())', implStart);
  const errorIndex = portsSource.indexOf('.map_err(search_error_to_port_error)', implStart);
  assert(policyIndex > implStart, `${testCase.operation} read policy marker is outside provider impl`);
  assert(localeIndex > policyIndex, `${testCase.operation} locale fallback must happen after read policy`);
  assert(errorIndex > localeIndex, `${testCase.operation} typed error mapping must happen after locale fallback`);
  const executionMarker = testCase.operation === 'execute_search'
    ? 'self.search(request)'
    : 'SearchSuggestionService::suggestions(self.connection(), request)';
  const executionIndex = portsSource.indexOf(executionMarker, implStart);
  assert(executionIndex > localeIndex && executionIndex < errorIndex, `${testCase.operation} embedded PostgreSQL execution order drift`);
}

const readme = read('crates/rustok-search/README.md');
assert(readme.includes('contracts/evidence/search-runtime-contract-smoke.json'), 'README missing runtime contract evidence');
const plan = read('crates/rustok-search/docs/implementation-plan.md');
assert(plan.includes('search-runtime-contract-smoke.json'), 'implementation plan missing runtime contract evidence');
const central = read('docs/modules/registry.md');
assert(central.includes('search-runtime-contract-smoke.json'), 'central readiness board missing runtime contract evidence');

console.log('[verify-search-fba-runtime-contract] executable no-compile search FBA runtime contract smoke passed');
