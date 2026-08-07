#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (file) => readFileSync(new URL(file, root), 'utf8');
const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const facade = read('crates/rustok-commerce/src/services/checkout_payment_stages.rs');
const legacy = read('crates/rustok-commerce/src/services/checkout_payment_stages_legacy.rs');
const staged = read('crates/rustok-commerce/src/services/staged_checkout.rs');
const evidence = JSON.parse(read(
  'crates/rustok-commerce/contracts/evidence/checkout-payment-stage-error-safety-source-review.json',
));

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
  ['message: public_message.to_string()', 'static stage message'],
  ['code: error.code', 'owner code preservation'],
  ['retryable: error.retryable', 'owner retryability preservation'],
  ['wrap_checkout_payment_execution_port(payment_port)', 'custom port adapter'],
  ['advance_to_payment_captured(', 'advance delegation'],
  ['load_payment_captured_state(', 'recovery delegation'],
]) requireText(facade, value, label);

for (const operation of [
  'prepare_checkout_collection',
  'authorize_checkout_collection',
  'capture_checkout_collection',
  'read_checkout_collection',
]) {
  requireText(facade, `"${operation}"`, `${operation} bounded operation`);
  requireText(legacy, `${operation}(`, `${operation} retained execution path`);
}

for (const [value, label] of [
  ['message: error.message', 'raw owner message propagation'],
  ['error = ?boundary_error', 'raw legacy PortError diagnostic'],
  ['tenant_id = %context.tenant_id', 'raw tenant diagnostic'],
  ['actor = ?context.actor', 'raw actor diagnostic'],
  ['correlation_id = %context.correlation_id', 'raw correlation diagnostic'],
  ['idempotency_key = ?context.idempotency_key', 'raw idempotency diagnostic'],
]) forbidText(facade, value, label);

for (const [value, label] of [
  ['CheckoutPaymentStageError', 'typed payment stage disposition import'],
  [
    'CheckoutStagePipelineError::PaymentStage(CheckoutPaymentStageError::Boundary {\n            retryable: true,',
    'retryable payment boundary disposition',
  ],
  ['fn retryable_payment_stage_boundary_does_not_force_compensation()', 'retryable disposition source test'],
  ['fn non_retryable_payment_stage_boundary_requires_compensation()', 'non-retryable disposition source test'],
]) requireText(staged, value, label);

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
  failure_disposition_changed: true,
  retryable_payment_boundary_disposition_retryable: true,
  non_retryable_payment_boundary_disposition_compensation_required: true,
  retryable_payment_boundary_synchronous_compensation: false,
  operation_journal_implementation_changed: false,
  recovery_service_implementation_changed: false,
  payment_stage_execution_changed: false,
  runtime_evidence_claimed: false,
};

if (evidence.status !== requiredEvidence.status) {
  failures.push(`evidence status: expected ${requiredEvidence.status}, got ${evidence.status}`);
}
for (const [key, expected] of Object.entries(requiredEvidence).slice(1)) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence ${key}: expected ${expected}, got ${evidence.source_contract?.[key]}`);
  }
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation ${key}: expected false`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push('evidence execution must remain empty');
}

if (failures.length > 0) {
  console.error('Checkout payment stage error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Mounted payment errors stay bounded and retryable boundaries remain resumable');
