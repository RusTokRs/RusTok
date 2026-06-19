import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const defaultModules = ['payment', 'fulfillment'];
const defaultRoot = new URL('../../', import.meta.url);

export class EcommerceProviderSpiEvidenceError extends Error {
  constructor(message) {
    super(message);
    this.name = 'EcommerceProviderSpiEvidenceError';
  }
}

const readJson = (root, path) => JSON.parse(readFileSync(new URL(path, root), 'utf8'));
const readText = (root, path) => readFileSync(new URL(path, root), 'utf8');
const fail = (message) => {
  throw new EcommerceProviderSpiEvidenceError(message);
};
const sameSet = (actual, expected) =>
  Array.isArray(actual) &&
  Array.isArray(expected) &&
  actual.length === expected.length &&
  expected.every((item) => actual.includes(item));

const requiredOperationAssertions = [
  'typed_provider_error_mapping',
  'idempotency_key_preserved',
  'provider_side_effects_not_persisted_by_adapter',
];

const requiredWebhookAssertions = [
  'idempotency_key_required',
  'duplicate_delivery_replayed_without_duplicate_lifecycle_transition',
  'raw_payload_retained_for_audit',
  'lifecycle_transition_delegated_to_owner_service',
];

const requiredExternalAdapterAssertions = [
  'descriptor_capability_match_required',
  'health_status_mapping_required',
  'degraded_mode_mapping_required',
  'adapter_does_not_persist_lifecycle_state',
];

export function verifyEcommerceProviderSpiEvidence({ root = defaultRoot, modules = defaultModules } = {}) {
  for (const module of modules) {
    const registryPath = `crates/rustok-${module}/contracts/${module}-fba-registry.json`;
    const evidencePath = `crates/rustok-${module}/contracts/evidence/${module}-provider-spi-static-matrix.json`;
    const registry = readJson(root, registryPath);
    const evidence = readJson(root, evidencePath);
    const providerSpi = registry.provider_spi;
    const providerSource = readText(root, `crates/rustok-${module}/src/providers.rs`);

    if (!providerSpi) fail(`${module} registry lacks provider_spi`);
    if (evidence.schema_version !== 1) fail(`${module} provider SPI evidence schema_version must be 1`);
    if (evidence.module !== module) fail(`${module} provider SPI evidence module drift`);
    if (evidence.status !== 'static_matrix_locked') fail(`${module} provider SPI evidence status drift`);
    if (evidence.generated_from !== registryPath) fail(`${module} provider SPI evidence source drift`);
    if (evidence.runner !== 'scripts/verify/verify-ecommerce-provider-spi-evidence.mjs') {
      fail(`${module} provider SPI evidence runner drift`);
    }
    if (evidence.contract_version !== registry.contract_version) fail(`${module} provider SPI contract version drift`);
    if (evidence.provider_spi_status !== providerSpi.status) fail(`${module} provider SPI status drift`);
    if (evidence.default_provider_id !== providerSpi.default_provider_id) fail(`${module} default provider drift`);
    if (evidence.promotion_gate !== 'does_not_raise_boundary_ready_without_runtime_execution') {
      fail(`${module} provider SPI evidence must not promote boundary_ready without runtime execution`);
    }

    if (!Array.isArray(evidence.operation_cases) || evidence.operation_cases.length !== providerSpi.operations.length) {
      fail(`${module} provider SPI operation case count drift`);
    }
    for (const operation of providerSpi.operations) {
      const evidenceCase = evidence.operation_cases.find((entry) => entry.operation === operation);
      if (!evidenceCase) fail(`${module} provider SPI evidence lacks operation ${operation}`);
      if (!sameSet(evidenceCase.profiles, ['manual_provider', 'remote_adapter_placeholder'])) {
        fail(`${module}.${operation} provider SPI profiles drift`);
      }
      if (!sameSet(evidenceCase.assertions, requiredOperationAssertions)) {
        fail(`${module}.${operation} provider SPI assertions drift`);
      }
      if (evidenceCase.execution_status !== 'static_locked_runtime_pending') {
        fail(`${module}.${operation} provider SPI execution status drift`);
      }
    }

    const webhook = evidence.webhook_replay_contract;
    if (!webhook) fail(`${module} provider SPI evidence lacks webhook replay contract`);
    if (webhook.status !== 'static_locked_runtime_pending') fail(`${module} webhook replay status drift`);
    if (!sameSet(webhook.assertions, requiredWebhookAssertions)) fail(`${module} webhook replay assertions drift`);
    if (providerSpi.webhook_ingress?.idempotency_required !== true || providerSpi.webhook_ingress?.replay_required !== true) {
      fail(`${module} registry webhook ingress must keep idempotency and replay required`);
    }
    if (!providerSpi.webhook_ingress?.adapter_operation) {
      fail(`${module} registry webhook ingress must declare adapter operation`);
    }
    if (webhook.adapter_operation !== providerSpi.webhook_ingress.adapter_operation) {
      fail(`${module} webhook replay adapter operation drift`);
    }
    if (webhook.raw_payload_audit_required !== true || webhook.owner_service_replay_guard_required !== true) {
      fail(`${module} webhook replay must lock raw payload audit and owner replay guard`);
    }

    const externalRegistration = evidence.external_adapter_registration;
    if (!externalRegistration) fail(`${module} provider SPI evidence lacks external adapter registration contract`);
    if (providerSpi.external_adapter_registration?.status !== 'planned_contract_locked') {
      fail(`${module} registry external adapter registration status drift`);
    }
    if (externalRegistration.status !== providerSpi.external_adapter_registration.status) {
      fail(`${module} external adapter registration status drift`);
    }
    if (!sameSet(externalRegistration.assertions, requiredExternalAdapterAssertions)) {
      fail(`${module} external adapter registration assertions drift`);
    }
    if (externalRegistration.execution_status !== 'static_locked_runtime_pending') {
      fail(`${module} external adapter registration execution status drift`);
    }
    for (const flag of [
      'requires_descriptor_capability_match',
      'requires_health_status_mapping',
      'requires_degraded_mode_mapping',
      'disallows_persisted_lifecycle_state_in_adapter',
    ]) {
      if (providerSpi.external_adapter_registration?.[flag] !== true) {
        fail(`${module} registry external adapter registration must keep ${flag}`);
      }
    }

    const registrationType =
      module === 'payment' ? 'ExternalPaymentProviderRegistration' : 'ExternalFulfillmentProviderRegistration';
    const healthType = module === 'payment' ? 'PaymentProviderHealth' : 'FulfillmentProviderHealth';
    const degradedType =
      module === 'payment' ? 'PaymentProviderDegradedMode' : 'FulfillmentProviderDegradedMode';
    for (const marker of [registrationType, healthType, degradedType, 'pub fn validate(&self, expected_provider_id: &str)']) {
      if (!providerSource.includes(marker)) {
        fail(`${module} provider SPI source lacks external registration marker ${marker}`);
      }
    }
    for (const marker of ['descriptor.provider_id', 'degraded_mode.is_none()', 'PaymentProviderHealth::Unavailable', 'FulfillmentProviderHealth::Unavailable']) {
      if (marker.includes(module === 'payment' ? 'Fulfillment' : 'Payment')) continue;
      if (!providerSource.includes(marker)) {
        fail(`${module} provider SPI source lacks registration guard ${marker}`);
      }
    }
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    verifyEcommerceProviderSpiEvidence();
    console.log('ecommerce provider SPI static evidence verified: payment, fulfillment');
  } catch (error) {
    if (error instanceof EcommerceProviderSpiEvidenceError) {
      console.error(`ecommerce provider SPI evidence verification failed: ${error.message}`);
      process.exit(1);
    }
    throw error;
  }
}
