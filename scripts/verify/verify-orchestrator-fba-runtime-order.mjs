import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const defaultRoot = process.env.ORCHESTRATOR_FBA_ROOT || process.cwd();

export class OrchestratorFbaRuntimeOrderError extends Error {
  constructor(message) {
    super(message);
    this.name = 'OrchestratorFbaRuntimeOrderError';
  }
}

const fail = (message) => { throw new OrchestratorFbaRuntimeOrderError(message); };
const sameSet = (actual, expected) =>
  Array.isArray(actual) && Array.isArray(expected) &&
  actual.length === expected.length && expected.every((item) => actual.includes(item));

function functionBody(source, name, startAt = 0) {
  const signature = new RegExp(`(?:pub\\s+)?async\\s+fn\\s+${name}\\s*\\(`, 'g');
  signature.lastIndex = startAt;
  const match = signature.exec(source);
  if (!match) return null;
  const open = source.indexOf('{', match.index);
  let depth = 0;
  for (let index = open; index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    if (source[index] === '}' && --depth === 0) return source.slice(open + 1, index);
  }
  return null;
}

function assertOrdered(body, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const index = body.indexOf(marker);
    if (index < 0) fail(`${label} source marker missing: ${marker}`);
    if (index <= previous) fail(`${label} runtime order drift at: ${marker}`);
    previous = index;
  }
}

function verifyAi({ read, json }) {
  const registryPath = 'crates/rustok-ai/contracts/ai-fba-registry.json';
  const smokePath = 'crates/rustok-ai/contracts/evidence/ai-orchestrator-runtime-order-smoke.json';
  const registry = json(registryPath);
  const smoke = json(smokePath);
  if (registry.status !== 'in_progress' || registry.role !== 'capability_orchestrator') fail('ai registry identity/status drift');
  if (registry.evidence.runtime_order_smoke !== smokePath || registry.evidence.runtime_order_smoke_runner !== smoke.runner) fail('ai registry runtime-order evidence drift');
  if (smoke.generated_from !== registryPath || smoke.status !== 'executable_no_compile') fail('ai smoke identity drift');
  if (!sameSet(smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes)) fail('ai degraded mode drift');

  if (!sameSet(smoke.support_adapters.map((entry) => entry.module), registry.support_adapters.map((entry) => entry.module))) {
    fail('ai support adapter set drift');
  }
  for (const adapter of registry.support_adapters) {
    const evidence = smoke.support_adapters.find((entry) => entry.module === adapter.module);
    if (!evidence || evidence.binding !== adapter.runtime_binding || evidence.registration_api !== adapter.registration_api) {
      fail(`ai support adapter evidence drift for ${adapter.module}`);
    }
    const ownerRegistry = json(adapter.registry);
    if (!['in_progress', 'boundary_ready'].includes(ownerRegistry.status)) fail(`ai support adapter ${adapter.module} status drift`);
    if (!read(adapter.runtime_binding).includes(adapter.registration_api)) fail(`ai runtime binding lacks ${adapter.registration_api}`);
  }
  for (const assertion of smoke.source_assertions) {
    const source = read(assertion.path);
    for (const marker of assertion.markers) if (!source.includes(marker)) fail(`ai ${assertion.name} missing ${marker}`);
  }
}

function verifyPageBuilder({ read, json }) {
  const registryPath = 'crates/rustok-page-builder/contracts/page-builder-fba-registry.json';
  const smokePath = 'crates/rustok-page-builder/contracts/evidence/page-builder-orchestrator-runtime-order-smoke.json';
  const registry = json(registryPath);
  const smoke = json(smokePath);
  if (registry.status !== 'in_progress' || registry.provider.module_slug !== 'page_builder') fail('page-builder registry identity/status drift');
  if (registry.evidence.runtime_order_smoke !== smokePath || registry.evidence.runtime_order_smoke_runner !== smoke.runner) fail('page-builder registry runtime-order evidence drift');
  if (smoke.generated_from !== registryPath || smoke.status !== 'executable_no_compile') fail('page-builder smoke identity drift');
  if (!sameSet(smoke.fallback_profiles, registry.fallback_profiles)) fail('page-builder fallback profile drift');
  if (!sameSet(smoke.capabilities.map((entry) => entry.capability), registry.provider.capabilities)) fail('page-builder capability set drift');

  const service = read('crates/rustok-page-builder/src/service.rs');
  const guardedStart = service.indexOf('impl<S> PageBuilderCapabilityService for CapabilityGuardedService<S>');
  const authorizedStart = service.indexOf('impl<S> AuthorizedPageBuilderHandlers<S>');
  for (const entry of smoke.capabilities) {
    const expectedPolicy = registry.provider.port_call_policies[entry.capability];
    if (entry.policy !== expectedPolicy) fail(`page-builder ${entry.capability} policy metadata drift`);
    const guardedBody = functionBody(service, entry.capability, guardedStart);
    if (!guardedBody) fail(`page-builder guarded ${entry.capability} body missing`);
    assertOrdered(guardedBody, entry.guarded_order, `page-builder guarded ${entry.capability}`);
    const authorizedBody = functionBody(service, entry.capability, authorizedStart);
    if (!authorizedBody) fail(`page-builder authorized ${entry.capability} body missing`);
    assertOrdered(authorizedBody, entry.authorized_order, `page-builder authorized ${entry.capability}`);
  }
  for (const assertion of smoke.source_assertions) {
    const source = read(assertion.path);
    for (const marker of assertion.markers) if (!source.includes(marker)) fail(`page-builder ${assertion.name} missing ${marker}`);
  }
}

export function verifyOrchestratorFbaRuntimeOrder({ root = defaultRoot } = {}) {
  const read = (repoPath) => fs.readFileSync(path.join(root, repoPath), 'utf8');
  const json = (repoPath) => JSON.parse(read(repoPath));
  verifyAi({ read, json });
  verifyPageBuilder({ read, json });
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    verifyOrchestratorFbaRuntimeOrder();
    console.log('orchestrator FBA runtime order verified: ai, page-builder');
  } catch (error) {
    if (error instanceof OrchestratorFbaRuntimeOrderError) {
      console.error(`orchestrator FBA runtime order failed: ${error.message}`);
      process.exit(1);
    }
    throw error;
  }
}
