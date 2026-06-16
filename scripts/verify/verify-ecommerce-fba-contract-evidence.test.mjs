import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, sep } from 'node:path';
import { pathToFileURL } from 'node:url';
import test from 'node:test';

import {
  EcommerceFbaContractEvidenceError,
  verifyEcommerceFbaContractEvidence,
} from './verify-ecommerce-fba-contract-evidence.mjs';

const moduleSlug = 'pricing';

const createFixtureRoot = ({ mutateEvidence } = {}) => {
  const rootPath = mkdtempSync(join(tmpdir(), 'rustok-ecommerce-fba-evidence-'));
  const write = (relativePath, content) => {
    const fullPath = join(rootPath, ...relativePath.split('/'));
    mkdirSync(fullPath.slice(0, fullPath.lastIndexOf(sep)), { recursive: true });
    writeFileSync(fullPath, content);
  };
  const registry = {
    schema_version: 1,
    module: moduleSlug,
    contract_version: 'pricing.read_projection.v1',
    contract_tests: {
      profiles: ['in_process', 'remote_adapter_placeholder'],
      cases: [
        {
          operation: 'resolve_product_price',
          profiles: ['in_process', 'remote_adapter_placeholder'],
          assertions: ['typed_port_error_mapping', 'context_deadline_preserved'],
        },
      ],
      fallback_smoke: {
        profiles: ['embedded_native', 'graphql_checkout_compat'],
        degraded_modes: ['use_cart_price_snapshot'],
      },
    },
  };
  const evidence = {
    schema_version: 1,
    module: moduleSlug,
    packet: 'contract-test-placeholder-matrix',
    status: 'static_matrix_locked',
    generated_from: 'crates/rustok-pricing/contracts/pricing-fba-registry.json',
    runner: 'scripts/verify/verify-ecommerce-fba-contract-evidence.mjs',
    contract_version: registry.contract_version,
    profiles: registry.contract_tests.profiles,
    cases: registry.contract_tests.cases.map((entry) => ({
      ...entry,
      execution_status: 'static_locked_runtime_pending',
    })),
    fallback_smoke: {
      status: 'static_locked_runtime_pending',
      profiles: registry.contract_tests.fallback_smoke.profiles,
      degraded_modes: registry.contract_tests.fallback_smoke.degraded_modes,
    },
    promotion_gate: 'does_not_raise_boundary_ready_without_runtime_execution',
  };
  mutateEvidence?.(evidence);
  write('crates/rustok-pricing/contracts/pricing-fba-registry.json', `${JSON.stringify(registry, null, 2)}\n`);
  write('crates/rustok-pricing/contracts/evidence/pricing-contract-test-static-matrix.json', `${JSON.stringify(evidence, null, 2)}\n`);
  return pathToFileURL(`${rootPath}/`);
};

test('verifyEcommerceFbaContractEvidence accepts matching static evidence', () => {
  assert.doesNotThrow(() => {
    verifyEcommerceFbaContractEvidence({ root: createFixtureRoot(), modules: [moduleSlug] });
  });
});

test('verifyEcommerceFbaContractEvidence rejects assertion drift', () => {
  const root = createFixtureRoot({
    mutateEvidence(evidence) {
      evidence.cases[0].assertions = ['typed_port_error_mapping'];
    },
  });

  assert.throws(
    () => verifyEcommerceFbaContractEvidence({ root, modules: [moduleSlug] }),
    {
      name: EcommerceFbaContractEvidenceError.name,
      message: 'pricing.resolve_product_price evidence assertion drift',
    },
  );
});


test('verifyEcommerceFbaContractEvidence rejects unknown evidence cases', () => {
  const root = createFixtureRoot({
    mutateEvidence(evidence) {
      evidence.cases.push({
        operation: 'legacy_checkout_shadow_case',
        profiles: ['in_process'],
        assertions: ['typed_port_error_mapping'],
        execution_status: 'static_locked_runtime_pending',
      });
    },
  });

  assert.throws(
    () => verifyEcommerceFbaContractEvidence({ root, modules: [moduleSlug] }),
    {
      name: EcommerceFbaContractEvidenceError.name,
      message: 'pricing evidence case count drift',
    },
  );
});

test('verifyEcommerceFbaContractEvidence rejects missing fallback smoke evidence', () => {
  const root = createFixtureRoot({
    mutateEvidence(evidence) {
      delete evidence.fallback_smoke;
    },
  });

  assert.throws(
    () => verifyEcommerceFbaContractEvidence({ root, modules: [moduleSlug] }),
    {
      name: EcommerceFbaContractEvidenceError.name,
      message: 'pricing evidence lacks fallback_smoke',
    },
  );
});
