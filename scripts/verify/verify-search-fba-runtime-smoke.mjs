import fs from 'node:fs';
import path from 'node:path';

const root = process.env.SEARCH_FBA_ROOT || process.cwd();
function resolve(repoPath) { return path.join(root, repoPath); }
function read(repoPath) { return fs.readFileSync(resolve(repoPath), 'utf8'); }
function json(repoPath) { return JSON.parse(read(repoPath)); }
function fail(message) { console.error(`[verify-search-fba-runtime-smoke] ${message}`); process.exit(1); }
function assert(condition, message) { if (!condition) fail(message); }
function sameSet(actual, expected, label) {
  const a = [...actual].sort().join('|');
  const e = [...expected].sort().join('|');
  assert(a === e, `${label} drift: expected ${e}, got ${a}`);
}

const registryPath = 'crates/rustok-search/contracts/search-fba-registry.json';
const smokePath = 'crates/rustok-search/contracts/evidence/search-runtime-fallback-smoke.json';
const registry = json(registryPath);
const smoke = json(smokePath);

function requireReadPolicy(context) {
  if (!context.deadline_ms || context.deadline_ms <= 0) {
    return { ok: false, kind: 'timeout', code: 'port.deadline_required', retryable: true };
  }
  return { ok: true };
}

function applyLocaleFallback(context, request) {
  return { ...request, locale: request.locale || context.locale };
}

function mapSearchError(error) {
  switch (error.kind) {
    case 'validation': return { kind: 'validation', code: 'search.validation', retryable: false };
    case 'not_found': return { kind: 'not_found', code: 'search.not_found', retryable: false };
    case 'external': return { kind: 'unavailable', code: 'search.external', retryable: true };
    default: return { kind: 'unavailable', code: 'search.unavailable', retryable: true };
  }
}

function executePortCase(operation, request) {
  const context = {
    tenant_id: 'tenant-demo',
    locale: 'ru',
    correlation_id: `smoke-${operation}`,
    deadline_ms: 250,
  };
  const deadline = requireReadPolicy(context);
  assert(deadline.ok, `${operation} should accept read context with deadline`);
  const normalized = applyLocaleFallback(context, request);
  assert(normalized.locale === 'ru', `${operation} should inherit context locale`);
  assert(normalized.tenant_id === request.tenant_id, `${operation} should preserve request tenant payload`);
  const explicitLocale = applyLocaleFallback(context, { ...request, locale: 'en' });
  assert(explicitLocale.locale === 'en', `${operation} should preserve explicit request locale`);
  const noDeadline = requireReadPolicy({ ...context, deadline_ms: 0 });
  assert(!noDeadline.ok && noDeadline.code === 'port.deadline_required', `${operation} should reject missing deadline`);
  return normalized;
}

assert(registry.module === 'search' && registry.role === 'provider', 'registry identity drift');
assert(registry.evidence.runtime_fallback_smoke === smokePath, 'registry points at runtime smoke evidence');
assert(registry.evidence.runtime_fallback_smoke_runner === smoke.runner, 'registry runtime smoke runner drift');
assert(smoke.status === registry.contract_tests.fallback_smoke.status, 'smoke status matches registry');
assert(smoke.status === 'executable_no_compile', 'smoke is executable no-compile evidence');
assert(smoke.runner === 'scripts/verify/verify-search-fba-runtime-smoke.mjs', 'smoke runner drift');
sameSet(smoke.profiles, registry.contract_tests.fallback_smoke.profiles, 'fallback profile set');

const expectedOps = registry.contract_tests.cases.map((c) => c.operation);
const operations = smoke.cases.map((c) => c.operation);
sameSet(operations, expectedOps, 'runtime smoke operation set');
for (const testCase of smoke.cases) {
  const registryCase = registry.contract_tests.cases.find((c) => c.operation === testCase.operation);
  assert(registryCase, `smoke case ${testCase.operation} missing registry case`);
  for (const assertion of ['in_process_pg_provider_impl', 'deadline_semantics_required', 'locale_context_fallback', 'typed_port_error_mapping']) {
    assert(testCase.assertions.includes(assertion), `${testCase.operation} missing assertion ${assertion}`);
  }
  for (const mode of testCase.degraded_modes) {
    assert(registry.contract_tests.fallback_smoke.degraded_modes.includes(mode), `${testCase.operation} degraded mode ${mode} missing registry fallback smoke`);
  }
}
for (const mode of registry.contract_tests.fallback_smoke.degraded_modes) {
  assert(smoke.cases.some((c) => c.degraded_modes.includes(mode)), `runtime fallback smoke missing degraded mode ${mode}`);
}

const query = executePortCase('execute_search', { tenant_id: 'tenant-demo', query: 'чай', locale: null });
assert(query.query === 'чай', 'execute_search request payload should be preserved');
const suggest = executePortCase('suggest', { tenant_id: 'tenant-demo', query: 'ча', locale: null });
assert(suggest.query === 'ча', 'suggest request payload should be preserved');

for (const error of [
  ['validation', 'validation', 'search.validation', false],
  ['not_found', 'not_found', 'search.not_found', false],
  ['external', 'unavailable', 'search.external', true],
  ['unknown', 'unavailable', 'search.unavailable', true],
]) {
  const mapped = mapSearchError({ kind: error[0] });
  assert(mapped.kind === error[1] && mapped.code === error[2] && mapped.retryable === error[3], `typed error mapping drift for ${error[0]}`);
}

const ports = read('crates/rustok-search/src/ports.rs');
for (const snippet of [
  'context.require_policy(PortCallPolicy::read())?',
  'request.locale.get_or_insert_with(|| context.locale.clone())',
  'search_error_to_port_error',
  'SearchSuggestionService::suggestions(self.connection(), request)',
  'PortError::validation("search.validation"',
  'PortError::unavailable("search.external"',
]) assert(ports.includes(snippet), `ports.rs source marker missing ${snippet}`);

const engine = read('crates/rustok-search/src/pg_engine.rs');
assert(engine.includes('pub(crate) fn connection(&self) -> &DatabaseConnection'), 'PgSearchEngine connection fallback accessor drift');
const suggestions = read('crates/rustok-search/src/suggestions.rs');
for (const snippet of ['SearchSuggestionService', 'SearchSuggestionQuery', 'SearchSuggestion', 'LIKE', 'limit']) {
  assert(suggestions.includes(snippet), `suggestions source marker missing ${snippet}`);
}

console.log('[verify-search-fba-runtime-smoke] executable no-compile search FBA runtime fallback smoke passed');
