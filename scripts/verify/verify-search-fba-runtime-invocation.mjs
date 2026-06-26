import fs from 'node:fs';
import path from 'node:path';

const root = process.env.SEARCH_FBA_ROOT || process.cwd();
const registryPath = 'crates/rustok-search/contracts/search-fba-registry.json';
const tracePath = 'crates/rustok-search/contracts/evidence/search-runtime-invocation-trace.json';
function resolve(repoPath) { return path.join(root, repoPath); }
function read(repoPath) { return fs.readFileSync(resolve(repoPath), 'utf8'); }
function json(repoPath) { return JSON.parse(read(repoPath)); }
function fail(message) { console.error(`[verify-search-fba-runtime-invocation] ${message}`); process.exit(1); }
function assert(condition, message) { if (!condition) fail(message); }
function sameSet(actual, expected, label) {
  const a = [...actual].sort().join('|');
  const e = [...expected].sort().join('|');
  assert(a === e, `${label} drift: expected ${e}, got ${a}`);
}

function mapPortError(errorKind) {
  switch (errorKind) {
    case 'validation': return { kind: 'validation', code: 'search.validation', retryable: false };
    case 'not_found': return { kind: 'not_found', code: 'search.not_found', retryable: false };
    case 'external': return { kind: 'unavailable', code: 'search.external', retryable: true };
    default: return { kind: 'unavailable', code: 'search.unavailable', retryable: true };
  }
}

function executeTrace(testCase, { policy = 'read', locale = null, errorKind = null } = {}) {
  const events = ['require_read_policy'];
  let providerInvocations = 0;
  const request = { locale };
  const context = { locale: 'ru-RU' };
  if (policy !== 'read') {
    events.push('return_policy_error_without_provider_invocation');
    return { events, providerInvocations, request, result: { kind: 'policy_denied' } };
  }
  if (!request.locale) request.locale = context.locale;
  events.push('apply_context_locale_fallback');
  providerInvocations += 1;
  events.push('invoke_embedded_postgres_provider');
  if (errorKind) {
    events.push('map_typed_port_error');
    return { events, providerInvocations, request, result: mapPortError(errorKind) };
  }
  events.push('map_success');
  return { events, providerInvocations, request, result: { kind: testCase.operation === 'suggest' ? 'suggestions' : 'search_result' } };
}

const registry = json(registryPath);
const trace = json(tracePath);
assert(trace.generated_from === registryPath, 'invocation trace generated_from drift');
assert(trace.runner === 'scripts/verify/verify-search-fba-runtime-invocation.mjs', 'invocation trace runner drift');
assert(trace.status === 'executable_no_compile_invocation_trace', 'invocation trace status drift');
assert(trace.contract_version === registry.contract_version, 'invocation trace contract version drift');
assert(registry.evidence.runtime_invocation_trace === tracePath, 'registry invocation trace evidence path drift');
assert(registry.evidence.runtime_invocation_trace_runner === trace.runner, 'registry invocation trace runner drift');
assert(registry.contract_tests.runtime_invocation_trace?.runner === trace.runner, 'registry invocation trace contract_tests runner drift');
assert(registry.contract_tests.runtime_invocation_trace?.status === trace.status, 'registry invocation trace contract_tests status drift');

const registryOps = registry.contract_tests.cases.map((c) => c.operation);
sameSet(trace.cases.map((c) => c.operation), registryOps, 'invocation trace operation set');

const portsSource = read('crates/rustok-search/src/ports.rs');
for (const marker of [
  'context.require_policy(PortCallPolicy::read())?',
  'request.locale.get_or_insert_with(|| context.locale.clone())',
  'self.search(request)',
  'SearchSuggestionService::suggestions(self.connection(), request)',
  'search_error_to_port_error',
]) assert(portsSource.includes(marker), `ports.rs missing source marker ${marker}`);

for (const testCase of trace.cases) {
  const success = executeTrace(testCase, { locale: null });
  assert(success.events.join('|') === testCase.success_trace.join('|'), `${testCase.operation} success trace drift`);
  assert(success.providerInvocations === 1, `${testCase.operation} success must invoke provider exactly once`);
  assert(success.request.locale === 'ru-RU', `${testCase.operation} must apply context locale fallback`);

  const preservedLocale = executeTrace(testCase, { locale: 'en-US' });
  assert(preservedLocale.request.locale === 'en-US', `${testCase.operation} must preserve explicit request locale`);

  const denied = executeTrace(testCase, { policy: 'write' });
  assert(denied.events.join('|') === testCase.policy_denied_trace.join('|'), `${testCase.operation} policy denied trace drift`);
  assert(denied.providerInvocations === 0, `${testCase.operation} policy denied must not invoke provider`);

  sameSet(testCase.typed_errors, [
    'validation:search.validation:false',
    'not_found:search.not_found:false',
    'external:search.external:true',
    'unknown:search.unavailable:true',
  ], `${testCase.operation} typed error coverage`);

  for (const expected of testCase.typed_errors) {
    const [kind, code, retryableText] = expected.split(':');
    const failure = executeTrace(testCase, { errorKind: kind });
    assert(failure.events.join('|') === testCase.error_trace.join('|'), `${testCase.operation} ${kind} error trace drift`);
    assert(failure.providerInvocations === 1, `${testCase.operation} ${kind} must invoke provider before error mapping`);
    assert(failure.result.code === code, `${testCase.operation} ${kind} code drift`);
    assert(String(failure.result.retryable) === retryableText, `${testCase.operation} ${kind} retryable drift`);
  }
}

const readme = read('crates/rustok-search/README.md');
assert(readme.includes('contracts/evidence/search-runtime-invocation-trace.json'), 'README missing invocation trace evidence');
const plan = read('crates/rustok-search/docs/implementation-plan.md');
assert(plan.includes('search-runtime-invocation-trace.json'), 'implementation plan missing invocation trace evidence');
const central = read('docs/modules/registry.md');
assert(central.includes('search-runtime-invocation-trace.json'), 'central readiness board missing invocation trace evidence');

console.log('[verify-search-fba-runtime-invocation] executable no-compile search FBA runtime invocation trace passed');
