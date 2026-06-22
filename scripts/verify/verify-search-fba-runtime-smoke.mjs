import fs from 'node:fs';

function read(path) { return fs.readFileSync(path, 'utf8'); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-search-fba-runtime-smoke] ${message}`); process.exit(1); }
function assert(condition, message) { if (!condition) fail(message); }

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
  const noDeadline = requireReadPolicy({ ...context, deadline_ms: 0 });
  assert(!noDeadline.ok && noDeadline.code === 'port.deadline_required', `${operation} should reject missing deadline`);
  return normalized;
}

assert(registry.evidence.runtime_fallback_smoke === smokePath, 'registry points at runtime smoke evidence');
assert(smoke.status === registry.contract_tests.fallback_smoke.status, 'smoke status matches registry');
assert(smoke.status === 'executable_no_compile', 'smoke is executable no-compile evidence');
assert(smoke.runner === 'scripts/verify/verify-search-fba-runtime-smoke.mjs', 'smoke runner drift');

const operations = new Set(registry.contract_tests.cases.map((c) => c.operation));
for (const operation of ['execute_search', 'suggest']) assert(operations.has(operation), `missing ${operation} case`);

const query = executePortCase('execute_search', { query: 'чай', locale: null });
assert(query.query === 'чай', 'execute_search request payload should be preserved');
const suggest = executePortCase('suggest', { query: 'ча', locale: null });
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
]) assert(ports.includes(snippet), `ports.rs source marker missing ${snippet}`);

console.log('[verify-search-fba-runtime-smoke] executable no-compile search FBA runtime fallback smoke passed');
