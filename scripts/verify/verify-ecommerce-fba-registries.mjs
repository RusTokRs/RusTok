import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

export const ecommerceFbaModules = ['payment', 'fulfillment', 'order', 'pricing', 'inventory'];

export class EcommerceFbaRegistryVerificationError extends Error {
  constructor(message) {
    super(message);
    this.name = 'EcommerceFbaRegistryVerificationError';
  }
}

const defaultRoot = new URL('../../', import.meta.url);

const createReader = (root) => (path) => readFileSync(new URL(path, root), 'utf8');

const fail = (message) => {
  throw new EcommerceFbaRegistryVerificationError(message);
};

const readOnlyOperationPrefixes = ['read_', 'list_', 'check_', 'resolve_', 'get_'];
const isReadOnlyOperation = (operation) =>
  readOnlyOperationPrefixes.some((prefix) => operation.startsWith(prefix));

const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

const containsBoolField = (source, fieldName) =>
  new RegExp(`(?:pub\\s+)?${escapeRegExp(fieldName)}\\s*:\\s*bool\\b`).test(source);

const containsStringLiteral = (source, value) =>
  source.includes(`"${value}"`) || source.includes(`'${value}'`);

const containsAsyncFunction = (source, functionName) =>
  new RegExp(`async\\s+fn\\s+${escapeRegExp(functionName)}\\s*\\(`).test(source);

const assertProviderSpiSource = ({ module, providerSpi, providerSource, libSource, ownerService }) => {
  if (providerSpi.status !== 'manual_baseline_locked') fail(`${module} provider SPI status drift`);
  if (!providerSpi.source || !providerSpi.source.startsWith(`crates/rustok-${module}/src/`)) {
    fail(`${module} provider SPI source must stay module-owned`);
  }
  if (!providerSpi.default_provider_id) fail(`${module} provider SPI lacks default_provider_id`);
  if (!Array.isArray(providerSpi.operations) || providerSpi.operations.length === 0) {
    fail(`${module} provider SPI lacks operations`);
  }
  if (!Array.isArray(providerSpi.capabilities) || providerSpi.capabilities.length === 0) {
    fail(`${module} provider SPI lacks capabilities`);
  }
  if (providerSpi.lifecycle_owner_service !== ownerService) {
    fail(`${module} provider SPI lifecycle_owner_service must be ${ownerService}`);
  }
  if (!providerSpi.side_effect_boundary || !providerSpi.side_effect_boundary.includes(`${ownerService} owns persisted lifecycle transitions`)) {
    fail(`${module} provider SPI side_effect_boundary must reference ${ownerService}`);
  }
  if (!providerSpi.webhook_ingress || providerSpi.webhook_ingress.status !== 'planned') {
    fail(`${module} provider SPI webhook ingress must remain planned until evidence lands`);
  }
  if (providerSpi.webhook_ingress.idempotency_required !== true || providerSpi.webhook_ingress.replay_required !== true) {
    fail(`${module} provider SPI webhook ingress must declare idempotency and replay requirements`);
  }
  for (const operation of providerSpi.operations) {
    if (!containsAsyncFunction(providerSource, operation)) {
      fail(`${module} provider SPI source lacks operation ${operation}`);
    }
  }
  if (!containsStringLiteral(providerSource, providerSpi.default_provider_id)) {
    fail(`${module} provider SPI source lacks default provider id ${providerSpi.default_provider_id}`);
  }
  for (const capability of providerSpi.capabilities) {
    if (!containsBoolField(providerSource, capability)) {
      fail(`${module} provider SPI source lacks bool capability field ${capability}`);
    }
  }
  if (!providerSource.includes('trait ') || !providerSource.includes('Provider: Send + Sync')) {
    fail(`${module} provider SPI source must expose a Send + Sync provider trait`);
  }
  if (!providerSource.includes('descriptor(&self)')) fail(`${module} provider SPI source lacks descriptor contract`);
  if (!providerSource.includes('idempotency_key: Option<String>')) {
    fail(`${module} provider SPI operation request must carry idempotency_key`);
  }
  if (!libSource.includes('pub mod providers;') || !libSource.includes('pub use providers::*;')) {
    fail(`${module} lib.rs must export provider SPI`);
  }
};

export function verifyEcommerceFbaRegistries({
  root = defaultRoot,
  modules = ecommerceFbaModules,
} = {}) {
  const read = createReader(root);
  const central = read('docs/modules/registry.md');
  const providerRegistries = new Map();

  for (const module of modules) {
    const registryPath = `crates/rustok-${module}/contracts/${module}-fba-registry.json`;
    const registry = JSON.parse(read(registryPath));
    const plan = read(`crates/rustok-${module}/docs/implementation-plan.md`);
    const manifest = read(`crates/rustok-${module}/rustok-module.toml`);
    const cargo = read(`crates/rustok-${module}/Cargo.toml`);
    const portSource = read(`crates/rustok-${module}/src/ports.rs`);
    const libSource = read(`crates/rustok-${module}/src/lib.rs`);

    if (registry.schema_version !== 1) fail(`${registryPath} schema_version must be 1`);
    if (registry.module !== module) fail(`${registryPath} has module=${registry.module}`);
    if (registry.role !== 'provider') fail(`${module} registry role must be provider`);
    if (registry.status !== 'in_progress') fail(`${module} registry status must be in_progress`);
    if (!Array.isArray(registry.ports) || registry.ports.length === 0) fail(`${module} has no ports`);
    if (!Array.isArray(registry.consumers) || registry.consumers.length === 0) fail(`${module} has no consumers`);
    if (!registry.contract_tests || registry.contract_tests.status !== 'planned_cases_locked') {
      fail(`${module} must lock planned contract test cases before boundary_ready`);
    }
    if (registry.contract_tests.runner !== 'scripts/verify/verify-ecommerce-fba-registries.mjs') {
      fail(`${module} contract test runner drift`);
    }
    if (registry.contract_tests.source !== registryPath) {
      fail(`${module} contract test source drift`);
    }
    if (!Array.isArray(registry.contract_tests.profiles) || !registry.contract_tests.profiles.includes('in_process') || !registry.contract_tests.profiles.includes('remote_adapter_placeholder')) {
      fail(`${module} contract tests must cover in_process and remote_adapter_placeholder profiles`);
    }

    for (const port of registry.ports) {
      if (port.owner !== module) fail(`${module}.${port.name} owner must be ${module}`);
      if (port.context !== 'rustok_api::ports::PortContext') fail(`${module}.${port.name} must use PortContext`);
      if (port.error !== 'rustok_api::ports::PortError') fail(`${module}.${port.name} must use PortError`);
      if (!Array.isArray(port.operations) || port.operations.length === 0) fail(`${module}.${port.name} has no operations`);
      if (port.deadline_required !== true) fail(`${module}.${port.name} must declare deadline_required=true`);
      if (!portSource.includes(`trait ${port.name}`)) fail(`${module} src/ports.rs lacks trait ${port.name}`);
      for (const operation of port.operations) {
        if (!portSource.includes(`${operation}(`)) fail(`${module}.${port.name} lacks operation ${operation}`);
        const testCase = registry.contract_tests.cases.find((entry) => entry.operation === operation);
        if (!testCase) fail(`${module}.${port.name} lacks contract test case for ${operation}`);
        if (!testCase.profiles.includes('in_process') || !testCase.profiles.includes('remote_adapter_placeholder')) {
          fail(`${module}.${operation} contract test case lacks both execution profiles`);
        }
        if (!testCase.assertions.includes('typed_port_error_mapping') || !testCase.assertions.includes('context_deadline_preserved')) {
          fail(`${module}.${operation} contract test case lacks baseline assertions`);
        }
        if (isReadOnlyOperation(operation) && testCase.assertions.includes('write_idempotency_required')) {
          fail(`${module}.${operation} read-only contract test case must not require write idempotency`);
        }
        if (!isReadOnlyOperation(operation) && port.idempotency_required === true && !testCase.assertions.includes('write_idempotency_required')) {
          fail(`${module}.${operation} write contract test case lacks write idempotency assertion`);
        }
      }
    }

    const fallbackSmoke = registry.contract_tests.fallback_smoke;
    if (!fallbackSmoke || fallbackSmoke.status !== 'planned') fail(`${module} fallback smoke status must remain planned until evidence lands`);
    const consumerFallbacks = registry.consumers.flatMap((consumer) => consumer.fallback_profiles || []);
    for (const profile of consumerFallbacks) {
      if (!fallbackSmoke.profiles.includes(profile)) fail(`${module} fallback smoke missing ${profile}`);
    }

    if (!manifest.includes('[fba.provider]')) fail(`${module} manifest lacks [fba.provider]`);
    if (!manifest.includes(`registry = "contracts/${module}-fba-registry.json"`)) fail(`${module} manifest registry path drift`);
    if (!manifest.includes(`contract_version = "${registry.contract_version}"`)) fail(`${module} manifest contract version drift`);
    if (!manifest.includes('context = "rustok_api::ports::PortContext"')) fail(`${module} manifest context drift`);
    if (!manifest.includes('error = "rustok_api::ports::PortError"')) fail(`${module} manifest error drift`);
    if (!cargo.includes('rustok-api.workspace = true')) fail(`${module} Cargo.toml lacks rustok-api dependency`);
    if (!libSource.includes('pub mod ports;') || !libSource.includes('pub use ports::*;')) fail(`${module} lib.rs must export ports`);
    if (!portSource.includes('rustok_api::{PortContext, PortError}')) fail(`${module} src/ports.rs must import neutral port primitives`);

    if (registry.provider_spi) {
      if (!registry.in_process_provider_impl?.service) {
        fail(`${module} provider SPI must declare in_process_provider_impl.service as lifecycle owner`);
      }
      if (!registry.provider_spi.source || !registry.provider_spi.source.startsWith(`crates/rustok-${module}/src/`)) {
        fail(`${module} provider SPI source must stay module-owned`);
      }
      const providerSource = read(registry.provider_spi.source);
      assertProviderSpiSource({
        module,
        providerSpi: registry.provider_spi,
        providerSource,
        libSource,
        ownerService: registry.in_process_provider_impl.service,
      });
    }

    if (registry.in_process_provider_impl) {
      const implDeclaration = `impl ${registry.ports[0].name} for crate::${registry.in_process_provider_impl.service}`;
      if (!portSource.includes(implDeclaration)) fail(`${module} lacks in-process provider impl ${implDeclaration}`);
      if (registry.ports.some((port) => port.idempotency_required === true) && !portSource.includes('require_write_semantics()?')) {
        fail(`${module} in-process provider impl must enforce write semantics`);
      }
      if (registry.ports.some((port) => port.deadline_required === true) && !portSource.includes('require_deadline_semantics()?')) {
        fail(`${module} in-process provider impl must enforce read deadline semantics`);
      }
    }

    if (!plan.includes('- FBA status: `in_progress`')) fail(`${module} local plan FBA status drift`);
    if (!plan.includes(`${module}-fba-registry.json`)) fail(`${module} local plan lacks registry evidence`);
    if (registry.evidence?.local_plan !== `crates/rustok-${module}/docs/implementation-plan.md`) {
      fail(`${module} registry local_plan evidence drift`);
    }
    if (registry.evidence?.central_board !== 'docs/modules/registry.md') {
      fail(`${module} registry central_board evidence drift`);
    }
    if (registry.evidence?.verifier !== 'scripts/verify/verify-ecommerce-fba-registries.mjs') {
      fail(`${module} registry verifier evidence drift`);
    }
    if (!central.includes(`| \`${module}\` |`) || !central.includes(`crates/rustok-${module}/contracts/${module}-fba-registry.json`)) {
      fail(`${module} central readiness board lacks registry evidence`);
    }

    providerRegistries.set(module, registry);
  }

  const commerceRegistryPath = 'crates/rustok-commerce/contracts/commerce-fba-registry.json';
  const commerceRegistry = JSON.parse(read(commerceRegistryPath));
  const commerceManifest = read('crates/rustok-commerce/rustok-module.toml');
  const commercePlan = read('crates/rustok-commerce/docs/implementation-plan.md');
  const commerceLib = read('crates/rustok-commerce/src/lib.rs');
  const commerceFbaSource = read('crates/rustok-commerce/src/fba.rs');

  if (commerceRegistry.schema_version !== 1) fail(`${commerceRegistryPath} schema_version must be 1`);
  if (commerceRegistry.module !== 'commerce') fail('commerce FBA registry module must be commerce');
  if (commerceRegistry.role !== 'orchestrator_consumer') fail('commerce FBA registry role must be orchestrator_consumer');
  if (commerceRegistry.status !== 'in_progress') fail('commerce FBA registry status must be in_progress');
  if (!Array.isArray(commerceRegistry.providers) || commerceRegistry.providers.length !== modules.length) {
    fail('commerce FBA registry must list every ecommerce provider');
  }
  if (!commerceManifest.includes('[fba.consumer]')) fail('commerce manifest lacks [fba.consumer]');
  if (!commerceManifest.includes('registry = "contracts/commerce-fba-registry.json"')) {
    fail('commerce manifest consumer registry path drift');
  }
  if (!commercePlan.includes('commerce-fba-registry.json')) fail('commerce local plan lacks consumer registry evidence');
  if (!central.includes('crates/rustok-commerce/contracts/commerce-fba-registry.json')) {
    fail('commerce central readiness board lacks consumer registry evidence');
  }
  if (!commerceLib.includes('pub mod fba;')) fail('commerce lib.rs must export fba registry module');
  if (!commerceFbaSource.includes('include_str!("../contracts/commerce-fba-registry.json")')) {
    fail('commerce fba.rs must embed commerce FBA registry');
  }

  for (const module of modules) {
    const provider = providerRegistries.get(module);
    const consumer = commerceRegistry.providers.find((entry) => entry.module === module);
    if (!consumer) fail(`commerce FBA registry lacks provider ${module}`);
    if (consumer.contract_version !== provider.contract_version) {
      fail(`commerce provider ${module} contract version drift`);
    }
    if (consumer.registry !== `crates/rustok-${module}/contracts/${module}-fba-registry.json`) {
      fail(`commerce provider ${module} registry path drift`);
    }
    for (const port of provider.ports) {
      if (!consumer.ports.includes(port.name)) {
        fail(`commerce provider ${module} port drift`);
      }
    }
    const commerceConsumer = provider.consumers.find((entry) => entry.module === 'commerce');
    if (!commerceConsumer) fail(`${module} provider registry lacks commerce consumer`);
    if (!consumer.profiles.includes(commerceConsumer.profile)) {
      fail(`commerce provider ${module} consumer profile drift`);
    }
    for (const profile of commerceConsumer.fallback_profiles || []) {
      if (!consumer.fallback_profiles.includes(profile)) {
        fail(`commerce provider ${module} fallback profile drift`);
      }
    }
    for (const mode of commerceConsumer.degraded_modes || []) {
      if (!consumer.degraded_modes.includes(mode)) {
        fail(`commerce provider ${module} degraded mode drift`);
      }
    }
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    verifyEcommerceFbaRegistries();
    console.log('ecommerce FBA registries verified: payment, fulfillment, order, pricing, inventory');
  } catch (error) {
    if (error instanceof EcommerceFbaRegistryVerificationError) {
      console.error(`ecommerce FBA registry verification failed: ${error.message}`);
      process.exit(1);
    }
    throw error;
  }
}
