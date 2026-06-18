import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const defaultModules = ['payment', 'fulfillment', 'order', 'pricing', 'inventory', 'product'];
const defaultRoot = new URL('../../', import.meta.url);

export class EcommerceFbaContractEvidenceError extends Error {
  constructor(message) {
    super(message);
    this.name = 'EcommerceFbaContractEvidenceError';
  }
}

const createJsonReader = (root) => (path) => JSON.parse(readFileSync(new URL(path, root), 'utf8'));
const fail = (message) => {
  throw new EcommerceFbaContractEvidenceError(message);
};
const sameSet = (actual, expected) =>
  Array.isArray(actual) &&
  Array.isArray(expected) &&
  actual.length === expected.length &&
  expected.every((item) => actual.includes(item));

export function verifyEcommerceFbaContractEvidence({
  root = defaultRoot,
  modules = defaultModules,
} = {}) {
  const readJson = createJsonReader(root);
  for (const module of modules) {
    const registryPath = `crates/rustok-${module}/contracts/${module}-fba-registry.json`;
    const evidencePath = `crates/rustok-${module}/contracts/evidence/${module}-contract-test-static-matrix.json`;
    const registry = readJson(registryPath);
    const evidence = readJson(evidencePath);

    if (evidence.schema_version !== 1) fail(`${module} evidence schema_version must be 1`);
    if (evidence.module !== module) fail(`${module} evidence module drift`);
    if (evidence.status !== 'static_matrix_locked') fail(`${module} evidence status drift`);
    if (evidence.generated_from !== registryPath) fail(`${module} evidence source drift`);
    if (evidence.runner !== 'scripts/verify/verify-ecommerce-fba-contract-evidence.mjs') {
      fail(`${module} evidence runner drift`);
    }
    if (evidence.contract_version !== registry.contract_version) {
      fail(`${module} evidence contract_version drift`);
    }
    if (!sameSet(evidence.profiles, registry.contract_tests.profiles)) {
      fail(`${module} evidence profile drift`);
    }
    if (evidence.promotion_gate !== 'does_not_raise_boundary_ready_without_runtime_execution') {
      fail(`${module} evidence must keep boundary_ready gated on runtime execution`);
    }

    if (!Array.isArray(evidence.cases) || evidence.cases.length !== registry.contract_tests.cases.length) {
      fail(`${module} evidence case count drift`);
    }

    for (const evidenceCase of evidence.cases) {
      if (!registry.contract_tests.cases.some((entry) => entry.operation === evidenceCase.operation)) {
        fail(`${module} evidence has unknown case ${evidenceCase.operation}`);
      }
    }

    for (const registryCase of registry.contract_tests.cases) {
      const evidenceCase = evidence.cases.find((entry) => entry.operation === registryCase.operation);
      if (!evidenceCase) fail(`${module} evidence lacks case ${registryCase.operation}`);
      if (evidenceCase.execution_status !== 'static_locked_runtime_pending') {
        fail(`${module}.${registryCase.operation} evidence execution status drift`);
      }
      if (!sameSet(evidenceCase.profiles, registryCase.profiles)) {
        fail(`${module}.${registryCase.operation} evidence profile drift`);
      }
      if (!sameSet(evidenceCase.assertions, registryCase.assertions)) {
        fail(`${module}.${registryCase.operation} evidence assertion drift`);
      }
    }

    if (!evidence.fallback_smoke) {
      fail(`${module} evidence lacks fallback_smoke`);
    }
    const fallback = registry.contract_tests.fallback_smoke;
    if (evidence.fallback_smoke?.status !== 'static_locked_runtime_pending') {
      fail(`${module} fallback evidence status drift`);
    }
    if (!sameSet(evidence.fallback_smoke.profiles, fallback.profiles)) {
      fail(`${module} fallback evidence profile drift`);
    }
    if (!sameSet(evidence.fallback_smoke.degraded_modes, fallback.degraded_modes)) {
      fail(`${module} fallback evidence degraded mode drift`);
    }
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    verifyEcommerceFbaContractEvidence();
    console.log('ecommerce FBA static contract evidence verified: payment, fulfillment, order, pricing, inventory, product');
  } catch (error) {
    if (error instanceof EcommerceFbaContractEvidenceError) {
      console.error(`ecommerce FBA contract evidence verification failed: ${error.message}`);
      process.exit(1);
    }
    throw error;
  }
}
