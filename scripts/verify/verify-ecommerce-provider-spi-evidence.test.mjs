import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, sep } from 'node:path';
import { pathToFileURL } from 'node:url';
import test from 'node:test';

import {
  EcommerceProviderSpiEvidenceError,
  verifyEcommerceProviderSpiEvidence,
} from './verify-ecommerce-provider-spi-evidence.mjs';

const moduleSlug = 'payment';

const createFixtureRoot = ({ mutateEvidence, mutateRegistry, mutateLiveAdapterContract, mutateLiveAdapterEvidence, providerSource } = {}) => {
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
  const runtimeSmoke = {
    schema_version: 1,
    module: moduleSlug,
    packet: 'provider-spi-runtime-mode-smoke',
    status: 'runtime_mode_smoke_locked',
    generated_from: 'crates/rustok-payment/contracts/payment-fba-registry.json',
    runner: 'scripts/verify/verify-ecommerce-provider-spi-evidence.mjs',
    source_contract: 'crates/rustok-payment/src/providers.rs',
    execution_scope: 'no_compile_static_runtime_contract_evidence',
    promotion_gate: 'does_not_raise_boundary_ready_without_live_adapter_execution',
    runtime_mode_cases: [
      {
        case: 'missing_provider',
        expected_error: 'not registered',
        assertions: [
          'registry_lookup_before_adapter_invocation',
          'typed_owner_error_mapping',
          'no_provider_side_effect',
        ],
      },
      {
        case: 'unsupported_operation',
        expected_error: 'does not support',
        assertions: [
          'capability_check_before_adapter_invocation',
          'typed_owner_error_mapping',
          'no_provider_side_effect',
        ],
      },
      {
        case: 'unknown_operation',
        expected_error: 'unknown payment provider operation',
        assertions: [
          'operation_allowlist_before_adapter_invocation',
          'typed_owner_error_mapping',
          'no_provider_side_effect',
        ],
      },
      {
        case: 'degraded_provider',
        expected_can_execute: true,
        assertions: [
          'degraded_mode_propagated',
          'fallback_profile_required',
          'adapter_invocation_remains_owner_controlled',
        ],
      },
      {
        case: 'unavailable_provider',
        expected_can_execute: false,
        assertions: [
          'unavailable_maps_to_non_executable',
          'fallback_profile_required',
          'adapter_invocation_blocked_by_owner',
        ],
      },
    ],
    registration_cases: [
      { case: 'descriptor_id_mismatch', expected_error: 'does not match registration id' },
      { case: 'adapter_descriptor_mismatch', expected_error: 'does not match descriptor id' },
      { case: 'duplicate_provider', expected_error: 'already registered' },
      { case: 'non_ready_without_degraded_mode', expected_error: 'must declare degraded mode' },
      { case: 'unavailable_default_provider', expected_error: 'cannot be default' },
    ],
    webhook_runtime_case: {
      adapter_operation: registry.provider_spi.webhook_ingress.adapter_operation,
      assertions: [
        'idempotency_key_required_by_contract',
        'raw_payload_audit_required',
        'owner_service_replay_guard_required',
      ],
    },
    live_execution_plan: {
      status: 'planned_contract_locked',
      promotion_gate: 'requires_concrete_external_adapter_execution',
      adapter_profile: 'external_gateway_adapter',
      required_cases: [
        'successful_operation_invokes_adapter_once_after_owner_runtime_guard',
        'provider_error_maps_to_typed_owner_error_without_lifecycle_persistence',
        'degraded_mode_propagates_fallback_profile_with_adapter_invocation_allowed',
        'unavailable_mode_blocks_adapter_invocation',
        'webhook_replay_is_idempotent_and_delegates_lifecycle_to_owner_service',
      ],
      adapter_operation: registry.provider_spi.webhook_ingress.adapter_operation,
      evidence_status: 'runtime_execution_pending',
    },
  };
  const liveAdapterContract = {
    schema_version: 1,
    module: moduleSlug,
    packet: 'provider-spi-live-adapter-execution-contract',
    status: 'live_adapter_contract_locked',
    generated_from: 'crates/rustok-payment/contracts/evidence/payment-provider-spi-runtime-smoke.json',
    runner: 'scripts/verify/verify-ecommerce-provider-spi-evidence.mjs',
    execution_scope: 'contract_locked_runtime_execution_pending',
    adapter_profile: 'external_gateway_adapter',
    promotion_gate: 'requires_concrete_external_adapter_execution_before_boundary_ready',
    required_cases: [
      { case: 'successful_operation_invokes_adapter_once_after_owner_runtime_guard', expected_adapter_invocations: 1, assertions: ['owner_runtime_guard_passes_before_invocation', 'adapter_called_exactly_once', 'lifecycle_persistence_delegated_to_owner_service'] },
      { case: 'provider_error_maps_to_typed_owner_error_without_lifecycle_persistence', expected_adapter_invocations: 1, assertions: ['provider_error_normalized_to_owner_error', 'adapter_result_not_persisted_directly', 'owner_service_controls_lifecycle_state'] },
      { case: 'degraded_mode_propagates_fallback_profile_with_adapter_invocation_allowed', expected_can_execute: true, assertions: ['degraded_mode_returned_by_runtime_mode', 'fallback_profile_propagated_to_orchestrator', 'adapter_invocation_allowed_after_owner_guard'] },
      { case: 'unavailable_mode_blocks_adapter_invocation', expected_can_execute: false, expected_adapter_invocations: 0, assertions: ['unavailable_runtime_mode_is_non_executable', 'owner_guard_blocks_adapter_invocation', 'typed_owner_error_mapping'] },
      { case: 'webhook_replay_is_idempotent_and_delegates_lifecycle_to_owner_service', adapter_operation: registry.provider_spi.webhook_ingress.adapter_operation, assertions: ['idempotency_key_required_by_contract', 'duplicate_delivery_replayed_without_duplicate_lifecycle_transition', 'raw_payload_retained_for_audit', 'lifecycle_transition_delegated_to_owner_service'] },
    ],
    evidence_status: 'runtime_execution_pending',
  };
  mutateEvidence?.(evidence);
  const liveAdapterEvidence = {
    schema_version: 1,
    module: moduleSlug,
    packet: 'provider-spi-live-adapter-runtime-evidence',
    status: 'concrete_external_adapter_contract_executed',
    generated_from: 'crates/rustok-payment/contracts/evidence/payment-provider-spi-live-adapter-contract.json',
    runner: 'scripts/verify/verify-ecommerce-provider-spi-evidence.mjs',
    adapter_profile: 'external_gateway_adapter',
    evidence_status: 'runtime_contract_executed',
    executed_cases: [
      { case: 'successful_operation_invokes_adapter_once_after_owner_runtime_guard', result: 'pass', observed_adapter_invocations: 1, assertions: ['owner_runtime_guard_passes_before_invocation', 'adapter_called_exactly_once', 'lifecycle_persistence_delegated_to_owner_service'] },
      { case: 'provider_error_maps_to_typed_owner_error_without_lifecycle_persistence', result: 'pass', observed_adapter_invocations: 1, assertions: ['provider_error_normalized_to_owner_error', 'adapter_result_not_persisted_directly', 'owner_service_controls_lifecycle_state'] },
      { case: 'degraded_mode_propagates_fallback_profile_with_adapter_invocation_allowed', result: 'pass', observed_can_execute: true, fallback_profile: 'manual_review', assertions: ['degraded_mode_returned_by_runtime_mode', 'fallback_profile_propagated_to_orchestrator', 'adapter_invocation_allowed_after_owner_guard'] },
      { case: 'unavailable_mode_blocks_adapter_invocation', result: 'pass', observed_can_execute: false, observed_adapter_invocations: 0, assertions: ['unavailable_runtime_mode_is_non_executable', 'owner_guard_blocks_adapter_invocation', 'typed_owner_error_mapping'] },
      { case: 'webhook_replay_is_idempotent_and_delegates_lifecycle_to_owner_service', result: 'pass', adapter_operation: registry.provider_spi.webhook_ingress.adapter_operation, assertions: ['idempotency_key_required_by_contract', 'duplicate_delivery_replayed_without_duplicate_lifecycle_transition', 'raw_payload_retained_for_audit', 'lifecycle_transition_delegated_to_owner_service'] },
    ],
  };
  mutateLiveAdapterEvidence?.(liveAdapterEvidence);
  mutateLiveAdapterContract?.(liveAdapterContract);
  write('crates/rustok-payment/contracts/payment-fba-registry.json', `${JSON.stringify(registry, null, 2)}\n`);
  write('crates/rustok-payment/contracts/evidence/payment-provider-spi-static-matrix.json', `${JSON.stringify(evidence, null, 2)}\n`);
  write('crates/rustok-payment/contracts/evidence/payment-provider-spi-runtime-smoke.json', `${JSON.stringify(runtimeSmoke, null, 2)}\n`);
  write('crates/rustok-payment/contracts/evidence/payment-provider-spi-live-adapter-contract.json', `${JSON.stringify(liveAdapterContract, null, 2)}\n`);
  write('crates/rustok-payment/contracts/evidence/payment-provider-spi-live-adapter-evidence.json', `${JSON.stringify(liveAdapterEvidence, null, 2)}\n`);
  write(
    'crates/rustok-payment/src/providers.rs',
    providerSource ?? readFileSync(join(process.cwd(), 'crates/rustok-payment/src/providers.rs'), 'utf8'),
  );
  for (const sourcePath of [
    'crates/rustok-commerce/src/services/checkout.rs',
    'crates/rustok-commerce/src/services/payment_orchestration.rs',
  ]) {
    write(sourcePath, readFileSync(join(process.cwd(), ...sourcePath.split('/')), 'utf8'));
  }
  return pathToFileURL(`${rootPath}/`);
};

test('verifyEcommerceProviderSpiEvidence accepts matching provider SPI evidence', () => {
  assert.doesNotThrow(() => {
    verifyEcommerceProviderSpiEvidence({ root: createFixtureRoot(), modules: [moduleSlug] });
  });
});

test('verifyEcommerceProviderSpiEvidence rejects live adapter evidence invocation drift', () => {
  const root = createFixtureRoot({
    mutateLiveAdapterEvidence(evidence) {
      evidence.executed_cases.find((entry) => entry.case === 'successful_operation_invokes_adapter_once_after_owner_runtime_guard').observed_adapter_invocations = 2;
    },
  });

  assert.throws(
    () => verifyEcommerceProviderSpiEvidence({ root, modules: [moduleSlug] }),
    {
      name: EcommerceProviderSpiEvidenceError.name,
      message: 'payment live adapter evidence success invocation count drift',
    },
  );
});

test('verifyEcommerceProviderSpiEvidence rejects runtime smoke assertion drift', () => {
  const root = createFixtureRoot();
  const runtimeSmokePath = new URL(
    'crates/rustok-payment/contracts/evidence/payment-provider-spi-runtime-smoke.json',
    root,
  );
  const runtimeSmoke = JSON.parse(readFileSync(runtimeSmokePath, 'utf8'));
  runtimeSmoke.runtime_mode_cases.find((entry) => entry.case === 'unavailable_provider').assertions = [
    'fallback_profile_required',
  ];
  writeFileSync(runtimeSmokePath, `${JSON.stringify(runtimeSmoke, null, 2)}\n`);

  assert.throws(
    () => verifyEcommerceProviderSpiEvidence({ root, modules: [moduleSlug] }),
    {
      name: EcommerceProviderSpiEvidenceError.name,
      message: 'payment runtime smoke case unavailable_provider assertions drift',
    },
  );
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


test('verifyEcommerceProviderSpiEvidence rejects missing external registration source marker', () => {
  const root = createFixtureRoot({
    providerSource:
      'pub enum PaymentProviderHealth { Ready, Degraded, Unavailable }\n' +
      'PaymentProviderHealth::Unavailable\n' +
      'pub struct PaymentProviderDegradedMode { reason: String }\n',
  });

  assert.throws(
    () => verifyEcommerceProviderSpiEvidence({ root, modules: [moduleSlug] }),
    {
      name: EcommerceProviderSpiEvidenceError.name,
      message:
        'payment provider SPI source lacks external registration marker ExternalPaymentProviderRegistration',
    },
  );
});

test('verifyEcommerceProviderSpiEvidence rejects live execution plan drift', () => {
  const root = createFixtureRoot();
  const runtimeSmokePath = new URL(
    'crates/rustok-payment/contracts/evidence/payment-provider-spi-runtime-smoke.json',
    root,
  );
  const runtimeSmoke = JSON.parse(readFileSync(runtimeSmokePath, 'utf8'));
  runtimeSmoke.live_execution_plan.required_cases = ['unavailable_mode_blocks_adapter_invocation'];
  writeFileSync(runtimeSmokePath, `${JSON.stringify(runtimeSmoke, null, 2)}
`);

  assert.throws(
    () => verifyEcommerceProviderSpiEvidence({ root, modules: [moduleSlug] }),
    {
      name: EcommerceProviderSpiEvidenceError.name,
      message: 'payment live execution plan required cases drift',
    },
  );
});


test('verifyEcommerceProviderSpiEvidence rejects live adapter contract drift', () => {
  const root = createFixtureRoot({
    mutateLiveAdapterContract(liveAdapterContract) {
      liveAdapterContract.required_cases.find(
        (entry) => entry.case === 'successful_operation_invokes_adapter_once_after_owner_runtime_guard',
      ).expected_adapter_invocations = 2;
    },
  });

  assert.throws(
    () => verifyEcommerceProviderSpiEvidence({ root, modules: [moduleSlug] }),
    {
      name: EcommerceProviderSpiEvidenceError.name,
      message: 'payment live adapter success invocation count drift',
    },
  );
});
