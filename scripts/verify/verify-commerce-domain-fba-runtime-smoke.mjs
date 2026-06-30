import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const defaultRoot = process.env.COMMERCE_DOMAIN_FBA_ROOT || process.cwd();
export const commerceDomainModules = ['product', 'pricing', 'inventory', 'customer', 'cart', 'tax'];
const invocationTracePath = 'crates/rustok-commerce/contracts/evidence/commerce-domain-provider-invocation-trace.json';

export class CommerceDomainFbaRuntimeSmokeError extends Error {
  constructor(message) {
    super(message);
    this.name = 'CommerceDomainFbaRuntimeSmokeError';
  }
}

const fail = (message) => { throw new CommerceDomainFbaRuntimeSmokeError(message); };
const sameSet = (actual, expected) =>
  Array.isArray(actual) && Array.isArray(expected) &&
  actual.length === expected.length && expected.every((item) => actual.includes(item));

function functionBody(source, name) {
  const signature = new RegExp(`async\\s+fn\\s+${name}\\s*\\(`, 'g');
  let match;
  while ((match = signature.exec(source)) !== null) {
    const semicolon = source.indexOf(';', match.index);
    const open = source.indexOf('{', match.index);
    if (open < 0 || (semicolon >= 0 && semicolon < open)) continue;
    let depth = 0;
    for (let index = open; index < source.length; index += 1) {
      if (source[index] === '{') depth += 1;
      if (source[index] === '}' && --depth === 0) return source.slice(open + 1, index);
    }
  }
  return null;
}

function simulatePolicy({ deadlineMs, write, idempotencyKey }) {
  if (!deadlineMs || deadlineMs <= 0) return { ok: false, code: 'port.deadline_required' };
  if (write && !idempotencyKey) return { ok: false, code: 'port.idempotency_key_required' };
  return { ok: true };
}

export function verifyCommerceDomainFbaRuntimeSmoke({ root = defaultRoot, modules = commerceDomainModules } = {}) {
  const read = (repoPath) => fs.readFileSync(path.join(root, repoPath), 'utf8');
  const json = (repoPath) => JSON.parse(read(repoPath));
  const trace = json(invocationTracePath);
  const commerceRegistry = json('crates/rustok-commerce/contracts/commerce-fba-registry.json');

  if (trace.schema_version !== 1) fail('commerce-domain invocation trace schema_version drift');
  if (trace.status !== 'executable_no_compile') fail('commerce-domain invocation trace status drift');
  if (trace.runner !== 'scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs') {
    fail('commerce-domain invocation trace runner drift');
  }
  if (trace.generated_from !== 'crates/rustok-commerce/contracts/commerce-fba-registry.json') {
    fail('commerce-domain invocation trace source drift');
  }
  if (commerceRegistry.evidence?.runtime_invocation_trace !== invocationTracePath) {
    fail('commerce registry runtime invocation trace evidence drift');
  }
  if (!sameSet(trace.modules.map((entry) => entry.provider_module), modules)) {
    fail('commerce-domain invocation trace module set drift');
  }

  for (const module of modules) {
    const registryPath = `crates/rustok-${module}/contracts/${module}-fba-registry.json`;
    const smokePath = `crates/rustok-${module}/contracts/evidence/${module}-runtime-contract-smoke.json`;
    const registry = json(registryPath);
    const smoke = json(smokePath);
    const ports = read(`crates/rustok-${module}/src/ports.rs`);
    const traceEntry = trace.modules.find((entry) => entry.provider_module === module);

    if (!traceEntry) fail(`${module} invocation trace entry missing`);
    if (traceEntry.provider_registry !== registryPath) fail(`${module} invocation trace provider registry drift`);
    if (traceEntry.runtime_contract_smoke !== smokePath) fail(`${module} invocation trace smoke path drift`);
    if (traceEntry.contract_version !== registry.contract_version) fail(`${module} invocation trace contract version drift`);
    if (!sameSet(traceEntry.ports, registry.ports.map((entry) => entry.name))) fail(`${module} invocation trace port drift`);
    if (!sameSet(traceEntry.operations, registry.ports.flatMap((entry) => entry.operations))) {
      fail(`${module} invocation trace operation drift`);
    }
    if (!sameSet(traceEntry.fallback_profiles, smoke.fallback_profiles)) fail(`${module} invocation trace fallback profile drift`);
    if (!sameSet(traceEntry.degraded_modes, smoke.degraded_modes)) fail(`${module} invocation trace degraded mode drift`);

    if (registry.status !== 'in_progress') fail(`${module} registry must remain in_progress before live runtime execution`);
    if (smoke.status !== 'executable_no_compile') fail(`${module} runtime smoke status drift`);
    if (smoke.generated_from !== registryPath) fail(`${module} runtime smoke source drift`);
    if (smoke.runner !== 'scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs') fail(`${module} runtime smoke runner drift`);
    if (smoke.contract_version !== registry.contract_version) fail(`${module} runtime smoke contract version drift`);
    if (registry.evidence?.runtime_contract_smoke !== smokePath) fail(`${module} registry runtime evidence path drift`);
    if (registry.evidence?.runtime_contract_smoke_runner !== smoke.runner) fail(`${module} registry runtime runner drift`);
    if (registry.contract_tests.status !== 'planned_cases_locked') fail(`${module} contract test status drift`);
    if (registry.contract_tests.fallback_smoke.status !== 'planned') fail(`${module} fallback smoke must remain planned before live runtime execution`);
    if (!sameSet(smoke.fallback_profiles, registry.contract_tests.fallback_smoke.profiles)) fail(`${module} fallback profile drift`);
    if (!sameSet(smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes)) fail(`${module} degraded mode drift`);
    if (!sameSet(traceEntry.fallback_profiles, registry.contract_tests.fallback_smoke.profiles)) {
      fail(`${module} invocation trace fallback profile does not mirror planned fallback smoke`);
    }
    if (!sameSet(traceEntry.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes)) {
      fail(`${module} invocation trace degraded mode does not mirror planned fallback smoke`);
    }

    const registryConsumer = registry.consumers.find((consumer) => consumer.module === traceEntry.consumer_module);
    if (!registryConsumer) fail(`${module} invocation trace consumer ${traceEntry.consumer_module} missing from provider registry`);
    if (!registryConsumer.fallback_profiles || !sameSet(traceEntry.consumer_fallback_profiles, registryConsumer.fallback_profiles)) {
      fail(`${module} invocation trace consumer fallback profile drift`);
    }
    if (!registryConsumer.degraded_modes || !sameSet(traceEntry.consumer_degraded_modes, registryConsumer.degraded_modes)) {
      fail(`${module} invocation trace consumer degraded mode drift`);
    }

    if (traceEntry.consumer_module === 'commerce') {
      const commerceProvider = commerceRegistry.providers.find((provider) => provider.module === module);
      if (!commerceProvider) fail(`${module} invocation trace missing from commerce consumer registry`);
      if (commerceProvider.registry !== registryPath) fail(`${module} commerce registry provider path drift`);
      if (commerceProvider.contract_version !== registry.contract_version) fail(`${module} commerce registry contract version drift`);
      if (!sameSet(commerceProvider.ports, traceEntry.ports)) fail(`${module} commerce registry port drift`);
      if (!sameSet(commerceProvider.fallback_profiles, traceEntry.consumer_fallback_profiles)) {
        fail(`${module} commerce registry fallback profile drift`);
      }
      if (!sameSet(commerceProvider.degraded_modes, traceEntry.consumer_degraded_modes)) {
        fail(`${module} commerce registry degraded mode drift`);
      }
    }

    const registryCases = registry.contract_tests.cases;
    if (!sameSet(smoke.cases.map((entry) => entry.operation), registryCases.map((entry) => entry.operation))) {
      fail(`${module} runtime operation set drift`);
    }

    for (const testCase of smoke.cases) {
      const registryCase = registryCases.find((entry) => entry.operation === testCase.operation);
      if (!registryCase) fail(`${module}.${testCase.operation} missing registry case`);
      const body = functionBody(ports, testCase.operation);
      if (!body) fail(`${module}.${testCase.operation} implementation body missing`);
      let previous = -1;
      for (const marker of testCase.source_order) {
        const index = body.indexOf(marker);
        if (index < 0) fail(`${module}.${testCase.operation} source marker missing: ${marker}`);
        if (index <= previous) fail(`${module}.${testCase.operation} runtime order drift at: ${marker}`);
        previous = index;
      }

      const write = registryCase.assertions.includes('write_idempotency_required');
      const accepted = simulatePolicy({ deadlineMs: 250, write, idempotencyKey: write ? 'smoke-key' : null });
      if (!accepted.ok) fail(`${module}.${testCase.operation} valid context rejected`);
      const noDeadline = simulatePolicy({ deadlineMs: 0, write, idempotencyKey: 'smoke-key' });
      if (noDeadline.code !== 'port.deadline_required') fail(`${module}.${testCase.operation} deadline rejection drift`);
      if (write) {
        const noIdempotency = simulatePolicy({ deadlineMs: 250, write, idempotencyKey: null });
        if (noIdempotency.code !== 'port.idempotency_key_required') fail(`${module}.${testCase.operation} idempotency rejection drift`);
      }
    }
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    verifyCommerceDomainFbaRuntimeSmoke();
    console.log(`commerce-domain FBA executable runtime smoke verified: ${commerceDomainModules.join(', ')}`);
  } catch (error) {
    if (error instanceof CommerceDomainFbaRuntimeSmokeError) {
      console.error(`commerce-domain FBA runtime smoke failed: ${error.message}`);
      process.exit(1);
    }
    throw error;
  }
}
