import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const ownerFbaModules = ['comments', 'rbac', 'workflow', 'region', 'media', 'outbox'];
const defaultRoot = process.env.OWNER_FBA_ROOT || process.cwd();

export class OwnerFbaRuntimeOrderError extends Error {
  constructor(message) {
    super(message);
    this.name = 'OwnerFbaRuntimeOrderError';
  }
}

const fail = (message) => { throw new OwnerFbaRuntimeOrderError(message); };
const sameSet = (actual, expected) =>
  Array.isArray(actual) && Array.isArray(expected) &&
  actual.length === expected.length && expected.every((item) => actual.includes(item));

function operationBody(source, operation) {
  const signature = new RegExp(`async\\s+fn\\s+${operation}\\s*\\(`, 'g');
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

function providerImplementationSource(source, registry, module) {
  const marker = registry.in_process_provider_impl?.marker;
  if (!marker) return source;

  const index = source.indexOf(marker);
  if (index < 0) fail(`${module} provider implementation marker missing: ${marker}`);
  return source.slice(index);
}

function registryFallback(registry) {
  if (registry.contract_tests.fallback_smoke.profiles && registry.contract_tests.fallback_smoke.degraded_modes) {
    return {
      profiles: registry.contract_tests.fallback_smoke.profiles,
      modes: registry.contract_tests.fallback_smoke.degraded_modes,
    };
  }
  return {
    profiles: registry.fallback_profiles,
    modes: [...new Set(registry.consumers.flatMap((consumer) => consumer.degraded_modes ?? []))],
  };
}

export function verifyOwnerFbaRuntimeOrder({ root = defaultRoot, modules = ownerFbaModules } = {}) {
  const read = (repoPath) => fs.readFileSync(path.join(root, repoPath), 'utf8');
  const json = (repoPath) => JSON.parse(read(repoPath));

  for (const module of modules) {
    const registryPath = `crates/rustok-${module}/contracts/${module}-fba-registry.json`;
    const smokePath = `crates/rustok-${module}/contracts/evidence/${module}-provider-runtime-order-smoke.json`;
    const registry = json(registryPath);
    const smoke = json(smokePath);
    const source = read(`crates/rustok-${module}/src/ports.rs`);
    const implementationSource = providerImplementationSource(source, registry, module);
    const fallback = registryFallback(registry);

    if (!['in_progress', 'boundary_ready'].includes(registry.status)) fail(`${module} must remain boundary_ready before live execution`);
    if (smoke.status !== 'executable_no_compile') fail(`${module} smoke status drift`);
    if (smoke.generated_from !== registryPath || smoke.contract_version !== registry.contract_version) fail(`${module} smoke identity drift`);
    if (smoke.runner !== 'scripts/verify/verify-owner-fba-runtime-order.mjs') fail(`${module} smoke runner drift`);
    if (registry.evidence?.runtime_order_smoke !== smokePath) fail(`${module} registry smoke path drift`);
    if (registry.evidence?.runtime_order_smoke_runner !== smoke.runner) fail(`${module} registry smoke runner drift`);
    if (!sameSet(smoke.fallback_profiles, fallback.profiles)) fail(`${module} fallback profile drift`);
    if (!sameSet(smoke.degraded_modes, fallback.modes)) fail(`${module} degraded mode drift`);
    for (const marker of smoke.source_markers ?? []) {
      if (!source.includes(marker)) fail(`${module} support source marker missing: ${marker}`);
    }

    const operations = registry.ports[0].operations;
    if (!sameSet(smoke.cases.map((entry) => entry.operation), operations)) fail(`${module} operation set drift`);
    for (const testCase of smoke.cases) {
      const body = operationBody(implementationSource, testCase.operation);
      if (!body) fail(`${module}.${testCase.operation} implementation body missing`);
      let previous = -1;
      for (const marker of testCase.source_order) {
        const index = body.indexOf(marker);
        if (index < 0) fail(`${module}.${testCase.operation} source marker missing: ${marker}`);
        if (index <= previous) fail(`${module}.${testCase.operation} runtime order drift at: ${marker}`);
        previous = index;
      }
      if (!testCase.write && body.includes('context.require_write_semantics()?')) {
        fail(`${module}.${testCase.operation} read path requires write semantics`);
      }
    }
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    verifyOwnerFbaRuntimeOrder();
    console.log(`owner FBA runtime order verified: ${ownerFbaModules.join(', ')}`);
  } catch (error) {
    if (error instanceof OwnerFbaRuntimeOrderError) {
      console.error(`owner FBA runtime order failed: ${error.message}`);
      process.exit(1);
    }
    throw error;
  }
}
