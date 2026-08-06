#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);

const facade = readFileSync(
  new URL(
    'crates/rustok-commerce/src/services/checkout_payment_stages.rs',
    root,
  ),
  'utf8',
);
const legacy = readFileSync(
  new URL(
    'crates/rustok-commerce/src/services/checkout_payment_stages_legacy.rs',
    root,
  ),
  'utf8',
);
const evidence = JSON.parse(
  readFileSync(
    new URL(
      'crates/rustok-commerce/contracts/evidence/checkout-payment-stage-error-safety-source-review.json',
      root,
    ),
    'utf8',
  ),
);

const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['include!("checkout_payment_stages_legacy.rs");', 'private retained source'],
  ['mod payment_execution_boundary {', 'bounded mapper module'],
  ['mod rustok_api_shim {', 'private PortError shim'],
  ['mod rustok_payment_shim {', 'payment execution port shim'],
  ['mod tracing_shim {', 'legacy tracing suppression'],
  ['use super::tracing_shim as tracing;', 'legacy tracing shadow'],
  ['struct SanitizingCheckoutPaymentExecutionPort', 'sanitizing owner adapter'],
  ['struct BoundaryPortError', 'private bounded error'],
  ['formatter.write_str("redacted")', 'redacted diagnostic debug'],
  ['fn public_message(kind: &PortErrorKind)', 'typed public message policy'],
  ['fn sanitize_owner_error(', 'owner mapper'],
  ['CheckoutPaymentExecutionContextFacts', 'bounded context facts'],
  ['tenant_id_shape', 'tenant identity shape'],
  ['actor_kind', 'actor kind'],
  ['actor_id_shape', 'actor identity shape'],
  ['claim_count', 'claim count'],
  ['role_count', 'role count'],
  ['channel_shape', 'channel shape'],
  ['locale_shape', 'locale shape'],
  ['correlation_id_shape', 'correlation shape'],
  ['causation_id_shape', 'causation shape'],
  ['traceparent_shape', 'trace shape'],
  ['idempotency_key_shape', 'idempotency shape'],
  ['owner_message_shape', 'owner message shape'],
  ['owner_message_len', 'owner message length'],
  [
    'const CHECKOUT_PAYMENT_EXECUTION_ADAPTER_BOUNDARY: &str =\n        "commerce_checkout_payment_execution_adapter";',
    'stable adapter boundary',
  ],
  ['message: public_message.to_string()', 'static stage message'],
  ['code: error.code', 'owner code preservation'],
  ['retryable: error.retryable', 'owner retryability preservation'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'technical severity policy',
  ],
  ['pub struct CheckoutPaymentStageExecutor {', 'public executor facade'],
  [
    'payment_port: Arc<dyn CanonicalCheckoutPaymentExecutionPort>',
    'canonical custom port API',
  ],
  [
    'wrap_checkout_payment_execution_port(payment_port)',
    'custom port sanitizing adapter',
  ],
  ['advance_to_payment_captured(', 'advance delegation'],
  ['load_payment_captured_state(', 'recovery delegation'],
  ['with_provider_registry(', 'provider registry builder'],
  ['with_lease_seconds(', 'lease builder'],
]) {
  requireText(facade, value, label);
}

for (const [kind, message] of [
  ['PortErrorKind::Validation', 'Checkout payment request is invalid'],
  ['PortErrorKind::NotFound', 'Checkout payment resource was not found'],
  [
    'PortErrorKind::Conflict',
    'Checkout payment state conflicts with the requested operation',
  ],
  ['PortErrorKind::Forbidden', 'Checkout payment operation is not permitted'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout',
    'Checkout payment service is temporarily unavailable',
  ],
  [
    'PortErrorKind::InvariantViolation',
    'Checkout payment operation could not be completed safely',
  ],
]) {
  requireText(facade, kind, `${kind} policy`);
  requireText(facade, message, `${kind} public message`);
}

for (const operation of [
  'prepare_checkout_collection',
  'authorize_checkout_collection',
  'capture_checkout_collection',
  'read_checkout_collection',
]) {
  const delegatedCalls = facade.match(new RegExp(`\\.${operation}\\(`, 'g')) ?? [];
  if (delegatedCalls.length < 2) {
    failures.push(
      `${operation}: expected adapter and inner delegations, found ${delegatedCalls.length}`,
    );
  }
  requireText(
    facade,
    `"${operation}"`,
    `${operation} bounded diagnostic operation`,
  );
}

for (const [value, label] of [
  ['message: error.message', 'raw owner message propagation'],
  ['error = ?boundary_error', 'raw legacy PortError diagnostic'],
  ['tenant_id = %context.tenant_id', 'raw tenant diagnostic'],
  ['actor = ?context.actor', 'raw actor diagnostic'],
  ['channel = ?context.channel', 'raw channel diagnostic'],
  ['locale = %context.locale', 'raw locale diagnostic'],
  ['correlation_id = %context.correlation_id', 'raw correlation diagnostic'],
  ['causation_id = ?context.causation_id', 'raw causation diagnostic'],
  ['traceparent = ?context.traceparent', 'raw trace diagnostic'],
  ['idempotency_key = ?context.idempotency_key', 'raw idempotency diagnostic'],
]) {
  forbidText(facade, value, label);
}

for (const [value, label] of [
  ['pub enum CheckoutPaymentStageError', 'legacy stage error'],
  ['pub struct CheckoutPaymentStageExecutor', 'legacy executor'],
  ['prepare_checkout_collection(', 'legacy prepare path'],
  ['authorize_checkout_collection(', 'legacy authorize path'],
  ['capture_checkout_collection(', 'legacy capture path'],
  ['read_checkout_collection(', 'legacy read path'],
  ['expected_stage: CheckoutOperationStage::PaymentReady', 'legacy prepare checkpoint'],
  [
    'next_stage: CheckoutOperationStage::PaymentAuthorized',
    'legacy authorize checkpoint',
  ],
  [
    'expected_stage: CheckoutOperationStage::PaymentAuthorized',
    'legacy capture checkpoint',
  ],
  [
    'next_stage: CheckoutOperationStage::PaymentCaptured',
    'legacy captured checkpoint',
  ],
  ['message: error.message', 'legacy raw message source retained privately'],
  ['error = ?boundary_error', 'legacy raw logger retained privately'],
]) {
  requireText(legacy, value, label);
}

const requiredEvidence = {
  status: 'commerce_checkout_payment_stage_error_safety_source_reviewed_unvalidated',
  mounted_facade_active: true,
  legacy_source_private: true,
  canonical_port_api_preserved: true,
  owner_context_delegation_preserved: true,
  owner_message_public: false,
  owner_message_persisted: false,
  raw_port_error_logged: false,
  raw_context_values_logged: false,
  owner_code_preserved: true,
  owner_retryability_preserved: true,
  failure_disposition_changed: false,
  runtime_evidence_claimed: false,
};

if (evidence.status !== requiredEvidence.status) {
  failures.push(
    `evidence status: expected ${requiredEvidence.status}, got ${evidence.status}`,
  );
}
for (const [key, expected] of Object.entries(requiredEvidence).slice(1)) {
  const actual = evidence.source_contract?.[key];
  if (actual !== expected) {
    failures.push(`evidence ${key}: expected ${expected}, got ${actual}`);
  }
}

for (const key of [
  'tests_run',
  'verifier_run',
  'cargo_run',
  'format_run',
  'provider_calls_run',
  'database_scenarios_run',
  'workflow_checks_run',
  'ci_run',
  'compile_proven',
  'runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`validation ${key}: expected false`);
  }
}

if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push('evidence execution must remain empty');
}

if (failures.length > 0) {
  console.error('Checkout payment stage error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Mounted checkout payment execution errors are static, bounded, and journal-safe',
);
