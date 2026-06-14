import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, sep } from 'node:path';
import { pathToFileURL } from 'node:url';
import test from 'node:test';

import {
  EcommerceFbaRegistryVerificationError,
  verifyEcommerceFbaRegistries,
} from './verify-ecommerce-fba-registries.mjs';

const moduleSlug = 'pricing';

const createFixtureRoot = ({ mutateRegistry } = {}) => {
  const rootPath = mkdtempSync(join(tmpdir(), 'rustok-ecommerce-fba-'));
  const write = (relativePath, content) => {
    const fullPath = join(rootPath, ...relativePath.split('/'));
    mkdirSync(fullPath.slice(0, fullPath.lastIndexOf(sep)), { recursive: true });
    writeFileSync(fullPath, content);
  };

  const registry = {
    schema_version: 1,
    module: moduleSlug,
    role: 'provider',
    status: 'in_progress',
    contract_version: 'pricing.read_projection.v1',
    ports: [
      {
        name: 'PricingReadPort',
        owner: moduleSlug,
        operations: ['resolve_product_price'],
        context: 'rustok_api::ports::PortContext',
        error: 'rustok_api::ports::PortError',
        idempotency_required: false,
        deadline_required: true,
      },
    ],
    consumers: [
      {
        module: 'commerce',
        profile: 'checkout_pricing_projection',
        degraded_modes: ['use_cart_price_snapshot'],
        fallback_profiles: ['embedded_native', 'graphql_checkout_compat'],
      },
    ],
    evidence: {
      local_plan: 'crates/rustok-pricing/docs/implementation-plan.md',
      central_board: 'docs/modules/registry.md',
      verifier: 'scripts/verify/verify-ecommerce-fba-registries.mjs',
    },
    in_process_provider_impl: {
      service: 'PricingService',
      source: 'crates/rustok-pricing/src/ports.rs',
      status: 'implemented',
    },
    contract_tests: {
      status: 'planned_cases_locked',
      source: 'crates/rustok-pricing/contracts/pricing-fba-registry.json',
      runner: 'scripts/verify/verify-ecommerce-fba-registries.mjs',
      profiles: ['in_process', 'remote_adapter_placeholder'],
      cases: [
        {
          operation: 'resolve_product_price',
          profiles: ['in_process', 'remote_adapter_placeholder'],
          assertions: ['typed_port_error_mapping', 'context_deadline_preserved'],
        },
      ],
      fallback_smoke: {
        status: 'planned',
        profiles: ['embedded_native', 'graphql_checkout_compat'],
        degraded_modes: ['use_cart_price_snapshot'],
      },
    },
  };

  mutateRegistry?.(registry);

  write('docs/modules/registry.md', '| `pricing` | admin + storefront | `in_progress` | `in_progress` | `core_transport_ui` | `crates/rustok-pricing/docs/implementation-plan.md` (`crates/rustok-pricing/contracts/pricing-fba-registry.json`) |\n');
  write('crates/rustok-pricing/contracts/pricing-fba-registry.json', `${JSON.stringify(registry, null, 2)}\n`);
  write('crates/rustok-pricing/docs/implementation-plan.md', '# Plan\n- FBA status: `in_progress`\n`pricing-fba-registry.json`\n');
  write('crates/rustok-pricing/rustok-module.toml', '[fba.provider]\nregistry = "contracts/pricing-fba-registry.json"\ncontract_version = "pricing.read_projection.v1"\ncontext = "rustok_api::ports::PortContext"\nerror = "rustok_api::ports::PortError"\n');
  write('crates/rustok-pricing/Cargo.toml', '[dependencies]\nrustok-api.workspace = true\n');
  write('crates/rustok-pricing/src/lib.rs', 'pub mod ports;\npub use ports::*;\n');
  write('crates/rustok-pricing/src/ports.rs', 'use rustok_api::{PortContext, PortError};\ntrait PricingReadPort {\n  fn resolve_product_price(&self, context: PortContext) -> Result<(), PortError>;\n}\nimpl PricingReadPort for crate::PricingService {}\n');

  return pathToFileURL(`${rootPath}/`);
};

test('verifyEcommerceFbaRegistries accepts locked contract-test metadata', () => {
  assert.doesNotThrow(() => {
    verifyEcommerceFbaRegistries({
      root: createFixtureRoot(),
      modules: [moduleSlug],
    });
  });
});

test('verifyEcommerceFbaRegistries rejects fallback-smoke drift', () => {
  const root = createFixtureRoot({
    mutateRegistry(registry) {
      registry.contract_tests.fallback_smoke.profiles = ['embedded_native'];
    },
  });

  assert.throws(
    () => verifyEcommerceFbaRegistries({ root, modules: [moduleSlug] }),
    {
      name: EcommerceFbaRegistryVerificationError.name,
      message: 'pricing fallback smoke missing graphql_checkout_compat',
    },
  );
});

test('verifyEcommerceFbaRegistries rejects evidence drift', () => {
  const root = createFixtureRoot({
    mutateRegistry(registry) {
      registry.evidence.local_plan = 'crates/rustok-pricing/docs/old-plan.md';
    },
  });

  assert.throws(
    () => verifyEcommerceFbaRegistries({ root, modules: [moduleSlug] }),
    {
      name: EcommerceFbaRegistryVerificationError.name,
      message: 'pricing registry local_plan evidence drift',
    },
  );
});
