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
const requireMarkers = (source, markers, message) => {
  for (const marker of markers) {
    if (!source.includes(marker)) fail(message(marker));
  }
};

const requiredOperationAssertions = [
  'typed_provider_error_mapping',
  'idempotency_key_preserved',
  'provider_side_effects_not_persisted_by_adapter',
];
const requiredExternalAdapterAssertions = [
  'descriptor_capability_match_required',
  'health_status_mapping_required',
  'degraded_mode_mapping_required',
  'adapter_does_not_persist_lifecycle_state',
];
const requiredRuntimeModeCases = [
  {
    case: 'missing_provider',
    assertions: [
      'registry_lookup_before_adapter_invocation',
      'typed_owner_error_mapping',
      'no_provider_side_effect',
    ],
  },
  {
    case: 'unsupported_operation',
    assertions: [
      'capability_check_before_adapter_invocation',
      'typed_owner_error_mapping',
      'no_provider_side_effect',
    ],
  },
  {
    case: 'unknown_operation',
    assertions: [
      'operation_allowlist_before_adapter_invocation',
      'typed_owner_error_mapping',
      'no_provider_side_effect',
    ],
  },
  {
    case: 'degraded_provider',
    assertions: [
      'degraded_mode_propagated',
      'fallback_profile_required',
      'adapter_invocation_remains_owner_controlled',
    ],
  },
  {
    case: 'unavailable_provider',
    assertions: [
      'unavailable_maps_to_non_executable',
      'fallback_profile_required',
      'adapter_invocation_blocked_by_owner',
    ],
  },
];
const requiredRegistrationCases = [
  'descriptor_id_mismatch',
  'adapter_descriptor_mismatch',
  'duplicate_provider',
  'non_ready_without_degraded_mode',
  'unavailable_default_provider',
];
const requiredLiveExecutionCases = [
  'successful_operation_invokes_adapter_once_after_owner_runtime_guard',
  'provider_error_maps_to_typed_owner_error_without_lifecycle_persistence',
  'degraded_mode_propagates_fallback_profile_with_adapter_invocation_allowed',
  'unavailable_mode_blocks_adapter_invocation',
  'webhook_replay_is_idempotent_and_delegates_lifecycle_to_owner_service',
];

const webhookAuditPolicy = (module) => {
  if (module === 'payment') {
    return {
      evidenceAssertions: [
        'idempotency_key_required',
        'duplicate_delivery_replayed_without_duplicate_lifecycle_transition',
        'raw_payload_hash_retained_for_audit',
        'lifecycle_transition_delegated_to_owner_service',
      ],
      runtimeAssertions: [
        'idempotency_key_required_by_contract',
        'payload_hash_audit_required',
        'owner_service_replay_guard_required',
      ],
      liveAssertions: [
        'idempotency_key_required_by_contract',
        'duplicate_delivery_replayed_without_duplicate_lifecycle_transition',
        'raw_payload_hash_retained_for_audit',
        'lifecycle_transition_delegated_to_owner_service',
      ],
      validate({ providerSpi, webhook }) {
        if (
          providerSpi.webhook_ingress?.raw_payload_persisted !== false ||
          providerSpi.webhook_ingress?.payload_hash_audit_required !== true ||
          webhook.raw_payload_persisted !== false ||
          webhook.payload_hash_audit_required !== true ||
          webhook.audit_artifact !== 'sha256_payload_hash'
        ) {
          fail('payment webhook replay must require SHA-256 hash-only audit and forbid raw payload persistence');
        }
      },
    };
  }

  return {
    evidenceAssertions: [
      'idempotency_key_required',
      'duplicate_delivery_replayed_without_duplicate_lifecycle_transition',
      'raw_payload_retained_for_audit',
      'lifecycle_transition_delegated_to_owner_service',
    ],
    runtimeAssertions: [
      'idempotency_key_required_by_contract',
      'raw_payload_audit_required',
      'owner_service_replay_guard_required',
    ],
    liveAssertions: [
      'idempotency_key_required_by_contract',
      'duplicate_delivery_replayed_without_duplicate_lifecycle_transition',
      'raw_payload_retained_for_audit',
      'lifecycle_transition_delegated_to_owner_service',
    ],
    validate({ providerSpi, webhook }) {
      if (
        providerSpi.webhook_ingress?.raw_payload_audit_required !== true ||
        webhook.raw_payload_audit_required !== true
      ) {
        fail(`${module} webhook replay must lock raw payload audit`);
      }
    },
  };
};

const verifyProviderSpiEvidence = ({
  module,
  registryPath,
  evidencePath,
  runtimeSmokePath,
  liveAdapterContractPath,
  registry,
  evidence,
  runtimeSmoke,
  liveAdapterContract,
  liveAdapterEvidence,
  providerSource,
  commerceCheckoutSource,
  commercePaymentOrchestrationSource,
  root,
}) => {
  const providerSpi = registry.provider_spi;
  const auditPolicy = webhookAuditPolicy(module);

  if (!providerSpi) fail(`${module} registry lacks provider_spi`);
  if (evidence.schema_version !== 1) fail(`${module} provider SPI evidence schema_version must be 1`);
  if (evidence.module !== module) fail(`${module} provider SPI evidence module drift`);
  if (evidence.status !== 'static_matrix_locked') fail(`${module} provider SPI evidence status drift`);
  if (evidence.generated_from !== registryPath) fail(`${module} provider SPI evidence source drift`);
  if (evidence.runner !== 'scripts/verify/verify-ecommerce-provider-spi-evidence.mjs') {
    fail(`${module} provider SPI evidence runner drift`);
  }
  if (evidence.contract_version !== registry.contract_version) {
    fail(`${module} provider SPI contract version drift`);
  }
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
  if (!sameSet(webhook.assertions, auditPolicy.evidenceAssertions)) {
    fail(`${module} webhook replay assertions drift`);
  }
  if (providerSpi.webhook_ingress?.idempotency_required !== true || providerSpi.webhook_ingress?.replay_required !== true) {
    fail(`${module} registry webhook ingress must keep idempotency and replay required`);
  }
  if (!providerSpi.webhook_ingress?.adapter_operation) {
    fail(`${module} registry webhook ingress must declare adapter operation`);
  }
  if (webhook.adapter_operation !== providerSpi.webhook_ingress.adapter_operation) {
    fail(`${module} webhook replay adapter operation drift`);
  }
  if (webhook.owner_service_replay_guard_required !== true) {
    fail(`${module} webhook replay must lock owner replay guard`);
  }
  auditPolicy.validate({ providerSpi, webhook });

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

  if (runtimeSmoke.schema_version !== 1) fail(`${module} runtime smoke schema_version must be 1`);
  if (runtimeSmoke.module !== module) fail(`${module} runtime smoke module drift`);
  if (runtimeSmoke.packet !== 'provider-spi-runtime-mode-smoke') fail(`${module} runtime smoke packet drift`);
  if (runtimeSmoke.status !== 'runtime_mode_smoke_locked') fail(`${module} runtime smoke status drift`);
  if (runtimeSmoke.generated_from !== registryPath) fail(`${module} runtime smoke source drift`);
  if (runtimeSmoke.runner !== 'scripts/verify/verify-ecommerce-provider-spi-evidence.mjs') {
    fail(`${module} runtime smoke runner drift`);
  }
  if (runtimeSmoke.source_contract !== `crates/rustok-${module}/src/providers.rs`) {
    fail(`${module} runtime smoke source contract drift`);
  }
  if (runtimeSmoke.execution_scope !== 'no_compile_static_runtime_contract_evidence') {
    fail(`${module} runtime smoke execution scope drift`);
  }
  if (runtimeSmoke.promotion_gate !== 'does_not_raise_boundary_ready_without_live_adapter_execution') {
    fail(`${module} runtime smoke must not promote boundary_ready without live adapter execution`);
  }
  if (!Array.isArray(runtimeSmoke.runtime_mode_cases)) fail(`${module} runtime smoke lacks runtime mode cases`);
  for (const requiredCase of requiredRuntimeModeCases) {
    const runtimeCase = runtimeSmoke.runtime_mode_cases.find((entry) => entry.case === requiredCase.case);
    if (!runtimeCase) fail(`${module} runtime smoke lacks case ${requiredCase.case}`);
    if (!sameSet(runtimeCase.assertions, requiredCase.assertions)) {
      fail(`${module} runtime smoke case ${requiredCase.case} assertions drift`);
    }
  }
  const degradedCase = runtimeSmoke.runtime_mode_cases.find((entry) => entry.case === 'degraded_provider');
  const unavailableCase = runtimeSmoke.runtime_mode_cases.find((entry) => entry.case === 'unavailable_provider');
  if (degradedCase?.expected_can_execute !== true) fail(`${module} degraded runtime smoke can_execute drift`);
  if (unavailableCase?.expected_can_execute !== false) fail(`${module} unavailable runtime smoke can_execute drift`);
  if (!Array.isArray(runtimeSmoke.registration_cases)) fail(`${module} runtime smoke lacks registration cases`);
  for (const requiredCase of requiredRegistrationCases) {
    const registrationCase = runtimeSmoke.registration_cases.find((entry) => entry.case === requiredCase);
    if (!registrationCase?.expected_error) fail(`${module} runtime smoke lacks registration case ${requiredCase}`);
  }
  if (runtimeSmoke.webhook_runtime_case?.adapter_operation !== providerSpi.webhook_ingress.adapter_operation) {
    fail(`${module} runtime smoke webhook adapter operation drift`);
  }
  if (!sameSet(runtimeSmoke.webhook_runtime_case?.assertions, auditPolicy.runtimeAssertions)) {
    fail(`${module} runtime smoke webhook assertions drift`);
  }

  const liveExecutionPlan = runtimeSmoke.live_execution_plan;
  if (!liveExecutionPlan) fail(`${module} runtime smoke lacks live execution plan`);
  if (liveExecutionPlan.status !== 'planned_contract_locked') fail(`${module} live execution plan status drift`);
  if (liveExecutionPlan.promotion_gate !== 'requires_concrete_external_adapter_execution') {
    fail(`${module} live execution plan must require concrete external adapter execution`);
  }
  const expectedAdapterProfile = module === 'payment' ? 'external_gateway_adapter' : 'external_carrier_adapter';
  if (liveExecutionPlan.adapter_profile !== expectedAdapterProfile) {
    fail(`${module} live execution plan adapter profile drift`);
  }
  if (!sameSet(liveExecutionPlan.required_cases, requiredLiveExecutionCases)) {
    fail(`${module} live execution plan required cases drift`);
  }
  if (liveExecutionPlan.adapter_operation !== providerSpi.webhook_ingress.adapter_operation) {
    fail(`${module} live execution plan adapter operation drift`);
  }
  if (liveExecutionPlan.evidence_status !== 'runtime_execution_pending') {
    fail(`${module} live execution plan evidence status drift`);
  }

  if (liveAdapterContract.schema_version !== 1) fail(`${module} live adapter contract schema_version must be 1`);
  if (liveAdapterContract.module !== module) fail(`${module} live adapter contract module drift`);
  if (liveAdapterContract.packet !== 'provider-spi-live-adapter-execution-contract') {
    fail(`${module} live adapter contract packet drift`);
  }
  if (liveAdapterContract.status !== 'live_adapter_contract_locked') {
    fail(`${module} live adapter contract status drift`);
  }
  if (liveAdapterContract.generated_from !== runtimeSmokePath) fail(`${module} live adapter contract source drift`);
  if (liveAdapterContract.runner !== 'scripts/verify/verify-ecommerce-provider-spi-evidence.mjs') {
    fail(`${module} live adapter contract runner drift`);
  }
  if (liveAdapterContract.execution_scope !== 'contract_locked_runtime_execution_pending') {
    fail(`${module} live adapter contract execution scope drift`);
  }
  if (liveAdapterContract.adapter_profile !== expectedAdapterProfile) {
    fail(`${module} live adapter contract profile drift`);
  }
  if (liveAdapterContract.promotion_gate !== 'requires_concrete_external_adapter_execution_before_boundary_ready') {
    fail(`${module} live adapter contract promotion gate drift`);
  }
  if (liveAdapterContract.evidence_status !== 'runtime_execution_pending') {
    fail(`${module} live adapter contract evidence status drift`);
  }
  if (!Array.isArray(liveAdapterContract.required_cases)) {
    fail(`${module} live adapter contract lacks required cases`);
  }
  for (const requiredCase of requiredLiveExecutionCases) {
    const contractCase = liveAdapterContract.required_cases.find((entry) => entry.case === requiredCase);
    if (!contractCase) fail(`${module} live adapter contract lacks case ${requiredCase}`);
    if (!Array.isArray(contractCase.assertions) || contractCase.assertions.length < 3) {
      fail(`${module} live adapter contract case ${requiredCase} assertions drift`);
    }
  }
  const successCase = liveAdapterContract.required_cases.find(
    (entry) => entry.case === 'successful_operation_invokes_adapter_once_after_owner_runtime_guard',
  );
  const unavailableLiveCase = liveAdapterContract.required_cases.find(
    (entry) => entry.case === 'unavailable_mode_blocks_adapter_invocation',
  );
  const degradedLiveCase = liveAdapterContract.required_cases.find(
    (entry) => entry.case === 'degraded_mode_propagates_fallback_profile_with_adapter_invocation_allowed',
  );
  const webhookLiveCase = liveAdapterContract.required_cases.find(
    (entry) => entry.case === 'webhook_replay_is_idempotent_and_delegates_lifecycle_to_owner_service',
  );
  if (successCase?.expected_adapter_invocations !== 1) {
    fail(`${module} live adapter success invocation count drift`);
  }
  if (unavailableLiveCase?.expected_adapter_invocations !== 0 || unavailableLiveCase?.expected_can_execute !== false) {
    fail(`${module} live adapter unavailable blocking drift`);
  }
  if (degradedLiveCase?.expected_can_execute !== true) fail(`${module} live adapter degraded mode drift`);
  if (webhookLiveCase?.adapter_operation !== providerSpi.webhook_ingress.adapter_operation) {
    fail(`${module} live adapter webhook operation drift`);
  }
  if (!sameSet(webhookLiveCase?.assertions, auditPolicy.liveAssertions)) {
    fail(`${module} live adapter webhook assertions drift`);
  }

  if (liveAdapterEvidence.schema_version !== 1) fail(`${module} live adapter evidence schema_version must be 1`);
  if (liveAdapterEvidence.module !== module) fail(`${module} live adapter evidence module drift`);
  if (liveAdapterEvidence.packet !== 'provider-spi-live-adapter-runtime-evidence') {
    fail(`${module} live adapter evidence packet drift`);
  }
  if (liveAdapterEvidence.status !== 'concrete_external_adapter_contract_executed') {
    fail(`${module} live adapter evidence status drift`);
  }
  if (liveAdapterEvidence.generated_from !== liveAdapterContractPath) {
    fail(`${module} live adapter evidence source drift`);
  }
  if (liveAdapterEvidence.runner !== 'scripts/verify/verify-ecommerce-provider-spi-evidence.mjs') {
    fail(`${module} live adapter evidence runner drift`);
  }
  if (liveAdapterEvidence.adapter_profile !== expectedAdapterProfile) {
    fail(`${module} live adapter evidence profile drift`);
  }
  if (liveAdapterEvidence.evidence_status !== 'runtime_contract_executed') {
    fail(`${module} live adapter evidence execution status drift`);
  }
  if (!Array.isArray(liveAdapterEvidence.executed_cases)) {
    fail(`${module} live adapter evidence lacks executed cases`);
  }
  for (const requiredCase of requiredLiveExecutionCases) {
    const executedCase = liveAdapterEvidence.executed_cases.find((entry) => entry.case === requiredCase);
    if (!executedCase) fail(`${module} live adapter evidence lacks case ${requiredCase}`);
    if (executedCase.result !== 'pass') fail(`${module} live adapter evidence case ${requiredCase} did not pass`);
    if (!Array.isArray(executedCase.assertions) || executedCase.assertions.length < 3) {
      fail(`${module} live adapter evidence case ${requiredCase} assertions drift`);
    }
  }
  const executedSuccessCase = liveAdapterEvidence.executed_cases.find(
    (entry) => entry.case === 'successful_operation_invokes_adapter_once_after_owner_runtime_guard',
  );
  const executedUnavailableCase = liveAdapterEvidence.executed_cases.find(
    (entry) => entry.case === 'unavailable_mode_blocks_adapter_invocation',
  );
  const executedDegradedCase = liveAdapterEvidence.executed_cases.find(
    (entry) => entry.case === 'degraded_mode_propagates_fallback_profile_with_adapter_invocation_allowed',
  );
  const executedWebhookCase = liveAdapterEvidence.executed_cases.find(
    (entry) => entry.case === 'webhook_replay_is_idempotent_and_delegates_lifecycle_to_owner_service',
  );
  if (executedSuccessCase?.observed_adapter_invocations !== 1) {
    fail(`${module} live adapter evidence success invocation count drift`);
  }
  if (
    executedUnavailableCase?.observed_adapter_invocations !== 0 ||
    executedUnavailableCase?.observed_can_execute !== false
  ) {
    fail(`${module} live adapter evidence unavailable blocking drift`);
  }
  if (executedDegradedCase?.observed_can_execute !== true || !executedDegradedCase?.fallback_profile) {
    fail(`${module} live adapter evidence degraded mode drift`);
  }
  if (executedWebhookCase?.adapter_operation !== providerSpi.webhook_ingress.adapter_operation) {
    fail(`${module} live adapter evidence webhook operation drift`);
  }
  if (!sameSet(executedWebhookCase?.assertions, auditPolicy.liveAssertions)) {
    fail(`${module} live adapter evidence webhook assertions drift`);
  }

  const registrationType =
    module === 'payment' ? 'ExternalPaymentProviderRegistration' : 'ExternalFulfillmentProviderRegistration';
  const healthType = module === 'payment' ? 'PaymentProviderHealth' : 'FulfillmentProviderHealth';
  const degradedType = module === 'payment' ? 'PaymentProviderDegradedMode' : 'FulfillmentProviderDegradedMode';
  const registryType = module === 'payment' ? 'PaymentProviderRegistry' : 'FulfillmentProviderRegistry';
  const runtimeModeType = module === 'payment' ? 'PaymentProviderRuntimeMode' : 'FulfillmentProviderRuntimeMode';
  requireMarkers(
    providerSource,
    [
      registrationType,
      healthType,
      degradedType,
      registryType,
      runtimeModeType,
      'pub fn validate(&self, expected_provider_id: &str)',
      'pub fn register_external(',
      'pub fn runtime_mode(',
      'fn executable_provider(',
    ],
    (marker) => `${module} provider SPI source lacks external registration marker ${marker}`,
  );

  const executionMarkers =
    module === 'payment'
      ? [
          'pub async fn execute_authorize(',
          'pub async fn execute_capture(',
          'pub async fn execute_cancel(',
          'pub async fn execute_refund(',
          'pub async fn execute_webhook(',
          '.authorize(request)',
          '.capture(request)',
          '.cancel(request)',
          '.refund(request)',
          '.handle_webhook(request)',
        ]
      : [
          'pub async fn execute_quote_rates(',
          'pub async fn execute_create_label(',
          'pub async fn execute_cancel(',
          'pub async fn execute_tracking_webhook(',
          '.quote_rates(request)',
          '.create_label(request)',
          '.cancel(request)',
          '.handle_tracking_webhook(request)',
        ];
  requireMarkers(
    providerSource,
    executionMarkers,
    (marker) => `${module} provider SPI source lacks guarded execution marker ${marker}`,
  );
  if (!providerSource.includes('if !mode.can_execute')) {
    fail(`${module} provider SPI source lacks unavailable execution guard`);
  }

  if (module === 'payment') {
    requireMarkers(
      commerceCheckoutSource,
      [
        'payment_provider_registry: PaymentProviderRegistry',
        'pub fn with_provider_registries(',
        '.execute_authorize(',
        '.execute_capture(',
        'execute_authorize_payment_provider',
        'execute_capture_payment_provider',
        'PaymentProviderOperationRequest {',
        'idempotency_key: Some(format!',
      ],
      (marker) => `commerce checkout orchestration lacks payment guarded execution marker ${marker}`,
    );
    requireMarkers(
      commercePaymentOrchestrationSource,
      [
        'PaymentOrchestrationService',
        'payment_provider_registry: PaymentProviderRegistry',
        '.execute_cancel(',
        '.execute_refund(',
        'cancel_payment_collection',
        'create_refund',
        'PaymentProviderOperationRequest {',
        'idempotency_key: Some(format!',
      ],
      (marker) => `commerce payment orchestration lacks guarded post-order execution marker ${marker}`,
    );
  } else {
    if (
      commerceCheckoutSource.includes('.execute_create_label(') ||
      commerceCheckoutSource.includes('fulfillment_provider_registry: FulfillmentProviderRegistry')
    ) {
      fail('commerce checkout must not execute fulfillment labels before payment');
    }
    const durableFulfillmentSource = [
      readText(root, 'crates/rustok-commerce/src/services/paid_order_create_label.rs'),
      readText(root, 'crates/rustok-commerce/src/services/fulfillment_create_label_recovery.rs'),
      readText(root, 'crates/rustok-commerce/src/services/paid_order_create_label_sweep.rs'),
    ].join('\n');
    requireMarkers(
      durableFulfillmentSource,
      [
        'PaidOrderCreateLabelHandler',
        'DomainEvent::OrderStatusChanged',
        'new_status == "paid"',
        'PaidOrderCreateLabelSweepService',
        'FulfillmentCreateLabelRecoveryService',
        '.execute_create_label(',
        'FulfillmentProviderOperationRequest',
        'claim_execution(',
        'mark_provider_succeeded(',
        'commit_provider_result',
      ],
      (marker) => `commerce fulfillment orchestration lacks durable post-payment marker ${marker}`,
    );
  }

  for (const marker of [
    'descriptor.provider_id',
    'descriptor.provider_id != registration.descriptor.provider_id',
    'providers.contains_key(expected_provider_id)',
    'degraded_mode.is_none()',
    'can_execute: registration.health !=',
    'PaymentProviderHealth::Unavailable',
    'FulfillmentProviderHealth::Unavailable',
  ]) {
    if (marker.includes(module === 'payment' ? 'Fulfillment' : 'Payment')) continue;
    if (!providerSource.includes(marker)) {
      fail(`${module} provider SPI source lacks registration guard ${marker}`);
    }
  }
};

export function verifyEcommerceProviderSpiEvidence({ root = defaultRoot, modules = defaultModules } = {}) {
  const commerceCheckoutSource = readText(root, 'crates/rustok-commerce/src/services/checkout.rs');
  const commercePaymentOrchestrationSource = readText(
    root,
    'crates/rustok-commerce/src/services/payment_orchestration.rs',
  );

  for (const module of modules) {
    const registryPath = `crates/rustok-${module}/contracts/${module}-fba-registry.json`;
    const evidencePath = `crates/rustok-${module}/contracts/evidence/${module}-provider-spi-static-matrix.json`;
    const runtimeSmokePath = `crates/rustok-${module}/contracts/evidence/${module}-provider-spi-runtime-smoke.json`;
    const liveAdapterContractPath = `crates/rustok-${module}/contracts/evidence/${module}-provider-spi-live-adapter-contract.json`;
    const liveAdapterEvidencePath = `crates/rustok-${module}/contracts/evidence/${module}-provider-spi-live-adapter-evidence.json`;

    verifyProviderSpiEvidence({
      module,
      registryPath,
      evidencePath,
      runtimeSmokePath,
      liveAdapterContractPath,
      registry: readJson(root, registryPath),
      evidence: readJson(root, evidencePath),
      runtimeSmoke: readJson(root, runtimeSmokePath),
      liveAdapterContract: readJson(root, liveAdapterContractPath),
      liveAdapterEvidence: readJson(root, liveAdapterEvidencePath),
      providerSource: readText(root, `crates/rustok-${module}/src/providers.rs`),
      commerceCheckoutSource,
      commercePaymentOrchestrationSource,
      root,
    });
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    verifyEcommerceProviderSpiEvidence();
    console.log('ecommerce provider SPI static + runtime-smoke evidence verified: payment, fulfillment');
  } catch (error) {
    if (error instanceof EcommerceProviderSpiEvidenceError) {
      console.error(`ecommerce provider SPI evidence verification failed: ${error.message}`);
      process.exit(1);
    }
    throw error;
  }
}
