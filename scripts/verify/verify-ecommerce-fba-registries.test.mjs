import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
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

  const commerceRegistry = {
    schema_version: 1,
    module: 'commerce',
    role: 'orchestrator_consumer',
    status: 'in_progress',
    contract_version: 'commerce.checkout_orchestration.fba.v1',
    providers: [
      {
        module: moduleSlug,
        contract_version: registry.contract_version,
        registry: 'crates/rustok-pricing/contracts/pricing-fba-registry.json',
        ports: ['PricingReadPort'],
        profiles: ['checkout_pricing_projection'],
        fallback_profiles: ['embedded_native', 'graphql_checkout_compat'],
        degraded_modes: ['use_cart_price_snapshot'],
      },
    ],
    evidence: {
      local_plan: 'crates/rustok-commerce/docs/implementation-plan.md',
      central_board: 'docs/modules/registry.md',
      verifier: 'scripts/verify/verify-ecommerce-fba-registries.mjs',
    },
  };

  write('docs/modules/registry.md', '| `pricing` | admin + storefront | `in_progress` | `in_progress` | `core_transport_ui` | `crates/rustok-pricing/docs/implementation-plan.md` (`crates/rustok-pricing/contracts/pricing-fba-registry.json`) |\n| `commerce` | admin + storefront | `in_progress` | `in_progress` | `core_transport_ui` | `crates/rustok-commerce/docs/implementation-plan.md` (`crates/rustok-commerce/contracts/commerce-fba-registry.json`) |\n');
  write('crates/rustok-pricing/contracts/pricing-fba-registry.json', `${JSON.stringify(registry, null, 2)}\n`);
  write('crates/rustok-pricing/docs/implementation-plan.md', '# Plan\n- FBA status: `in_progress`\n`pricing-fba-registry.json`\n');
  write('crates/rustok-pricing/rustok-module.toml', '[fba.provider]\nregistry = "contracts/pricing-fba-registry.json"\ncontract_version = "pricing.read_projection.v1"\ncontext = "rustok_api::ports::PortContext"\nerror = "rustok_api::ports::PortError"\n');
  write('crates/rustok-pricing/Cargo.toml', '[dependencies]\nrustok-api.workspace = true\n');
  write('crates/rustok-pricing/src/lib.rs', 'pub mod ports;\npub use ports::*;\n');
  write('crates/rustok-pricing/src/ports.rs', 'use rustok_api::{PortContext, PortError};\ntrait PricingReadPort {\n  fn resolve_product_price(&self, context: PortContext) -> Result<(), PortError>;\n}\nimpl PricingReadPort for crate::PricingService { fn resolve_product_price(&self, context: PortContext) -> Result<(), PortError> { context.require_deadline_semantics()?; Ok(()) } }\n');
  write('crates/rustok-commerce/contracts/commerce-fba-registry.json', `${JSON.stringify(commerceRegistry, null, 2)}\n`);
  write('crates/rustok-commerce/rustok-module.toml', '[fba.consumer]\nregistry = "contracts/commerce-fba-registry.json"\n');
  write('crates/rustok-commerce/docs/implementation-plan.md', '# Plan\ncommerce-fba-registry.json\n');
  write('crates/rustok-commerce/src/lib.rs', 'pub mod fba;\n');
  write('crates/rustok-commerce/src/fba.rs', 'pub const COMMERCE_FBA_REGISTRY_JSON: &str = include_str!("../contracts/commerce-fba-registry.json");\n');

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

test('verifyEcommerceFbaRegistries rejects write-idempotency assertions on read-only operations', () => {
  const root = createFixtureRoot({
    mutateRegistry(registry) {
      registry.contract_tests.cases[0].assertions.push('write_idempotency_required');
    },
  });

  assert.throws(
    () => verifyEcommerceFbaRegistries({ root, modules: [moduleSlug] }),
    {
      name: EcommerceFbaRegistryVerificationError.name,
      message: 'pricing.resolve_product_price read-only contract test case must not require write idempotency',
    },
  );
});

test('verifyEcommerceFbaRegistries rejects missing read deadline enforcement', () => {
  const root = createFixtureRoot();
  const rootPath = fileURLToPath(root);
  writeFileSync(
    join(rootPath, 'crates/rustok-pricing/src/ports.rs'),
    'use rustok_api::{PortContext, PortError};\ntrait PricingReadPort {\n  fn resolve_product_price(&self, context: PortContext) -> Result<(), PortError>;\n}\nimpl PricingReadPort for crate::PricingService { fn resolve_product_price(&self, context: PortContext) -> Result<(), PortError> { Ok(()) } }\n',
  );

  assert.throws(
    () => verifyEcommerceFbaRegistries({ root, modules: [moduleSlug] }),
    {
      name: EcommerceFbaRegistryVerificationError.name,
      message: 'pricing in-process provider impl must enforce read deadline semantics',
    },
  );
});

test('verifyEcommerceFbaRegistries verifies provider SPI source markers when registry declares provider_spi', () => {
  const root = createFixtureRoot({
    mutateRegistry(registry) {
      registry.contract_version = 'pricing.read_projection.v1+provider_spi.v1';
      registry.provider_spi = {
        status: 'manual_baseline_locked',
        source: 'crates/rustok-pricing/src/providers.rs',
        default_provider_id: 'manual',
        operations: ['authorize'],
        capabilities: ['authorize'],
        side_effect_boundary: 'provider adapters execute external effects; PricingService owns persisted lifecycle transitions',
        webhook_ingress: {
          status: 'planned',
          idempotency_required: true,
          replay_required: true,
        },
      };
    },
  });
  const rootPath = fileURLToPath(root);
  writeFileSync(
    join(rootPath, 'crates/rustok-pricing/src/lib.rs'),
    'pub mod ports;\npub use ports::*;\npub mod providers;\npub use providers::*;\n',
  );
  writeFileSync(
    join(rootPath, 'crates/rustok-pricing/rustok-module.toml'),
    '[fba.provider]\nregistry = "contracts/pricing-fba-registry.json"\ncontract_version = "pricing.read_projection.v1+provider_spi.v1"\ncontext = "rustok_api::ports::PortContext"\nerror = "rustok_api::ports::PortError"\n',
  );
  writeFileSync(
    join(rootPath, 'crates/rustok-pricing/src/providers.rs'),
    'pub struct PricingProviderCapabilities { pub authorize: bool }\npub struct PricingProviderOperationRequest { pub idempotency_key: Option<String> }\npub trait PricingProvider: Send + Sync { fn descriptor(&self); async fn authorize(&self, request: PricingProviderOperationRequest); }\n',
  );

  assert.doesNotThrow(() => verifyEcommerceFbaRegistries({ root, modules: [moduleSlug] }));
});

test('verifyEcommerceFbaRegistries rejects provider SPI operations missing from source', () => {
  const root = createFixtureRoot({
    mutateRegistry(registry) {
      registry.contract_version = 'pricing.read_projection.v1+provider_spi.v1';
      registry.provider_spi = {
        status: 'manual_baseline_locked',
        source: 'crates/rustok-pricing/src/providers.rs',
        default_provider_id: 'manual',
        operations: ['authorize'],
        capabilities: ['authorize'],
        side_effect_boundary: 'provider adapters execute external effects; PricingService owns persisted lifecycle transitions',
        webhook_ingress: {
          status: 'planned',
          idempotency_required: true,
          replay_required: true,
        },
      };
    },
  });
  const rootPath = fileURLToPath(root);
  writeFileSync(
    join(rootPath, 'crates/rustok-pricing/src/lib.rs'),
    'pub mod ports;\npub use ports::*;\npub mod providers;\npub use providers::*;\n',
  );
  writeFileSync(
    join(rootPath, 'crates/rustok-pricing/rustok-module.toml'),
    '[fba.provider]\nregistry = "contracts/pricing-fba-registry.json"\ncontract_version = "pricing.read_projection.v1+provider_spi.v1"\ncontext = "rustok_api::ports::PortContext"\nerror = "rustok_api::ports::PortError"\n',
  );
  writeFileSync(
    join(rootPath, 'crates/rustok-pricing/src/providers.rs'),
    'pub struct PricingProviderCapabilities { pub authorize: bool }\npub struct PricingProviderOperationRequest { pub idempotency_key: Option<String> }\npub trait PricingProvider: Send + Sync { fn descriptor(&self); }\n',
  );

  assert.throws(
    () => verifyEcommerceFbaRegistries({ root, modules: [moduleSlug] }),
    {
      name: EcommerceFbaRegistryVerificationError.name,
      message: 'pricing provider SPI source lacks operation authorize',
    },
  );
});
