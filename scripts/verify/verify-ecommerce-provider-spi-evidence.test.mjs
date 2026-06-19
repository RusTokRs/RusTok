import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, sep } from 'node:path';
import { pathToFileURL } from 'node:url';
import test from 'node:test';

import {
  EcommerceProviderSpiEvidenceError,
  verifyEcommerceProviderSpiEvidence,
} from './verify-ecommerce-provider-spi-evidence.mjs';

const moduleSlug = 'payment';

const createFixtureRoot = ({ mutateEvidence, mutateRegistry } = {}) => {
  const rootPath = mkdtempSync(join(tmpdir(), 'rustok-provider-spi-evidence-'));
  const write = (relativePath, content) => {
    const fullPath = join(rootPath, ...relativePath.split('/'));
    mkdirSync(fullPath.slice(0, fullPath.lastIndexOf(sep)), { recursive: true });
    writeFileSync(fullPath, content);
  };
  const registry = {
    contract_version: 'payment.checkout.v1+provider_spi.v1',
    provider_spi: {
      status: 'manual_baseline_locked',
      default_provider_id: 'manual',
      operations: ['authorize', 'capture'],
      webhook_ingress: {
        idempotency_required: true,
        replay_required: true,
        adapter_operation: 'handle_webhook',
      },
      external_adapter_registration: {
        status: 'planned_contract_locked',
        requires_descriptor_capability_match: true,
        requires_health_status_mapping: true,
        requires_degraded_mode_mapping: true,
        disallows_persisted_lifecycle_state_in_adapter: true,
      },
    },
  };
  mutateRegistry?.(registry);
  const evidence = {
    schema_version: 1,
    module: moduleSlug,
    packet: 'provider-spi-contract-static-matrix',
    status: 'static_matrix_locked',
    generated_from: 'crates/rustok-payment/contracts/payment-fba-registry.json',
    runner: 'scripts/verify/verify-ecommerce-provider-spi-evidence.mjs',
    contract_version: registry.contract_version,
    provider_spi_status: registry.provider_spi.status,
    default_provider_id: registry.provider_spi.default_provider_id,
    operation_cases: registry.provider_spi.operations.map((operation) => ({
      operation,
      profiles: ['manual_provider', 'remote_adapter_placeholder'],
      assertions: [
        'typed_provider_error_mapping',
        'idempotency_key_preserved',
        'provider_side_effects_not_persisted_by_adapter',
      ],
      execution_status: 'static_locked_runtime_pending',
    })),
    webhook_replay_contract: {
      name: 'payment_provider_webhook',
      status: 'static_locked_runtime_pending',
      assertions: [
        'idempotency_key_required',
        'duplicate_delivery_replayed_without_duplicate_lifecycle_transition',
        'raw_payload_retained_for_audit',
        'lifecycle_transition_delegated_to_owner_service',
      ],
      adapter_operation: registry.provider_spi.webhook_ingress.adapter_operation,
      raw_payload_audit_required: true,
      owner_service_replay_guard_required: true,
    },
    external_adapter_registration: {
      status: registry.provider_spi.external_adapter_registration.status,
      assertions: [
        'descriptor_capability_match_required',
        'health_status_mapping_required',
        'degraded_mode_mapping_required',
        'adapter_does_not_persist_lifecycle_state',
      ],
      execution_status: 'static_locked_runtime_pending',
    },
    promotion_gate: 'does_not_raise_boundary_ready_without_runtime_execution',
  };
  mutateEvidence?.(evidence);
  write('crates/rustok-payment/contracts/payment-fba-registry.json', `${JSON.stringify(registry, null, 2)}\n`);
  write('crates/rustok-payment/contracts/evidence/payment-provider-spi-static-matrix.json', `${JSON.stringify(evidence, null, 2)}\n`);
  return pathToFileURL(`${rootPath}/`);
};

test('verifyEcommerceProviderSpiEvidence accepts matching provider SPI evidence', () => {
  assert.doesNotThrow(() => {
    verifyEcommerceProviderSpiEvidence({ root: createFixtureRoot(), modules: [moduleSlug] });
  });
});

test('verifyEcommerceProviderSpiEvidence rejects operation assertion drift', () => {
  const root = createFixtureRoot({
    mutateEvidence(evidence) {
      evidence.operation_cases[0].assertions = ['typed_provider_error_mapping'];
    },
  });

  assert.throws(
    () => verifyEcommerceProviderSpiEvidence({ root, modules: [moduleSlug] }),
    {
      name: EcommerceProviderSpiEvidenceError.name,
      message: 'payment.authorize provider SPI assertions drift',
    },
  );
});

test('verifyEcommerceProviderSpiEvidence rejects webhook replay assertion drift', () => {
  const root = createFixtureRoot({
    mutateEvidence(evidence) {
      evidence.webhook_replay_contract.assertions = ['idempotency_key_required'];
    },
  });

  assert.throws(
    () => verifyEcommerceProviderSpiEvidence({ root, modules: [moduleSlug] }),
    {
      name: EcommerceProviderSpiEvidenceError.name,
      message: 'payment webhook replay assertions drift',
    },
  );
});

test('verifyEcommerceProviderSpiEvidence rejects disabled registry replay requirement', () => {
  const root = createFixtureRoot({
    mutateRegistry(registry) {
      registry.provider_spi.webhook_ingress.replay_required = false;
    },
  });

  assert.throws(
    () => verifyEcommerceProviderSpiEvidence({ root, modules: [moduleSlug] }),
    {
      name: EcommerceProviderSpiEvidenceError.name,
      message: 'payment registry webhook ingress must keep idempotency and replay required',
    },
  );
});


test('verifyEcommerceProviderSpiEvidence rejects external adapter registration assertion drift', () => {
  const root = createFixtureRoot({
    mutateEvidence(evidence) {
      evidence.external_adapter_registration.assertions = ['descriptor_capability_match_required'];
    },
  });

  assert.throws(
    () => verifyEcommerceProviderSpiEvidence({ root, modules: [moduleSlug] }),
    {
      name: EcommerceProviderSpiEvidenceError.name,
      message: 'payment external adapter registration assertions drift',
    },
  );
});
