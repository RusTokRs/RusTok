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

const facade = read('crates/rustok-commerce/src/services/checkout_fulfillment_stages.rs');
const legacy = read('crates/rustok-commerce/src/services/checkout_fulfillment_stages_legacy.rs');
const evidence = JSON.parse(read(
  'crates/rustok-commerce/contracts/evidence/checkout-fulfillment-stage-error-safety-source-review.json',
));

for (const [source, value, label] of [
  [facade, 'include!("checkout_fulfillment_stages_legacy.rs");', 'mounted legacy include'],
  [facade, 'struct SanitizingCheckoutFulfillmentExecutionPort', 'fulfillment sanitizer'],
  [facade, 'struct SanitizingCheckoutOrderPaymentSettlementPort', 'order settlement sanitizer'],
  [facade, 'fn sanitize_owner_error(', 'shared typed sanitizer'],
  [facade, '"ensure_checkout_fulfillments"', 'ensure operation'],
  [facade, '"read_checkout_fulfillments"', 'read operation'],
  [facade, '"settle_checkout_payment"', 'settlement operation'],
  [facade, '"ensure_fulfillments"', 'ensure stage'],
  [facade, '"read_fulfillments"', 'read stage'],
  [facade, '"settle_order_payment"', 'settlement stage'],
  [facade, 'message: public_message.to_string()', 'static public message projection'],
  [facade, 'code: error.code', 'owner code preservation'],
  [facade, 'retryable: error.retryable', 'owner retryability preservation'],
  [facade, 'owner_message_shape = owner_message_shape', 'owner message shape'],
  [facade, 'owner_message_len = owner_message_len', 'owner message length'],
  [facade, 'tenant_id_shape = diagnostic_context.tenant_id_shape', 'bounded tenant facts'],
  [facade, 'actor_id_shape = diagnostic_context.actor_id_shape', 'bounded actor facts'],
  [facade, 'correlation_id_shape = diagnostic_context.correlation_id_shape', 'bounded correlation facts'],
  [facade, 'idempotency_key_shape = diagnostic_context.idempotency_key_shape', 'bounded idempotency facts'],
  [facade, 'error = ?diagnostic_error', 'redacted diagnostic token'],
  [facade, '"commerce_checkout_fulfillment_execution_adapter"', 'stable safe boundary'],
  [facade, 'macro_rules! error', 'legacy error tracing suppression'],
  [facade, 'macro_rules! warn', 'legacy warn tracing suppression'],
  [legacy, 'fulfillment_context.clone()', 'legacy fulfillment context delegation'],
  [legacy, 'order_context.clone()', 'legacy order context delegation'],
  [legacy, 'message: error.message,', 'retained legacy mapper business logic'],
  [legacy, 'next_stage: CheckoutOperationStage::FulfillmentCreated', 'checkpoint preservation'],
  [legacy, 'state.payment_collection.status_kind() != PaymentCollectionStatusKind::Captured', 'typed captured admission'],
  [legacy, '.with_idempotency_key(format!("checkout:{operation_id}:fulfillment-set"))', 'fulfillment idempotency'],
  [legacy, '.with_idempotency_key(format!("checkout:{operation_id}:order:payment-settlement"))', 'settlement idempotency'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['error = ?boundary_error', 'raw owner error logging'],
  ['correlation_id = %context.correlation_id', 'raw correlation logging'],
  ['tenant_id = %context.tenant_id', 'raw tenant logging'],
  ['actor = ?context.actor', 'raw actor logging'],
  ['channel = ?context.channel', 'raw channel logging'],
  ['locale = %context.locale', 'raw locale logging'],
  ['causation_id = ?context.causation_id', 'raw causation logging'],
  ['traceparent = ?context.traceparent', 'raw trace logging'],
  ['idempotency_key = ?context.idempotency_key', 'raw idempotency logging'],
  ['message: error.message,', 'owner message publication from mounted facade'],
]) forbidText(facade, value, label);

for (const message of [
  'Checkout fulfillment request is invalid',
  'Checkout fulfillment resource was not found',
  'Checkout fulfillment state conflicts with the requested operation',
  'Checkout fulfillment operation is not permitted',
  'Checkout fulfillment service is temporarily unavailable',
  'Checkout fulfillment operation could not be completed safely',
  'Checkout order settlement request is invalid',
  'Checkout order settlement resource was not found',
  'Checkout order settlement state conflicts with the requested operation',
  'Checkout order settlement operation is not permitted',
  'Checkout order settlement service is temporarily unavailable',
  'Checkout order settlement could not be completed safely',
]) requireText(facade, message, 'static stage message');

for (const [key, expected] of Object.entries({
  mounted_facade_active: true,
  legacy_source_private: true,
  legacy_source_business_logic_changed: false,
  fulfillment_owner_calls_wrapped: true,
  order_settlement_owner_call_wrapped: true,
  owner_message_public: false,
  raw_port_error_logged: false,
  raw_context_values_logged: false,
  bounded_context_shapes_logged: true,
  owner_message_shape_logged: true,
  owner_message_length_logged: true,
  owner_code_preserved: true,
  owner_retryability_preserved: true,
  legacy_tracing_suppressed: true,
  retry_disposition_preserved: true,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}

for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push('evidence execution must remain empty');
}

if (failures.length > 0) {
  console.error('Checkout fulfillment stage error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout fulfillment and order-settlement owner failures are sanitized before the retained stage mapper',
);
