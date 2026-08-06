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

const facade = read('crates/rustok-commerce/src/services/checkout_order_stages.rs');
const legacy = read('crates/rustok-commerce/src/services/checkout_order_stages_legacy.rs');
const doc = read('crates/rustok-commerce/docs/checkout-order-stage-context.md');
const evidence = JSON.parse(read(
  'crates/rustok-commerce/contracts/evidence/checkout-order-stage-error-safety-source-review.json',
));
const combined = `${facade}\n${legacy}`;

for (const [value, label] of [
  ['include!("checkout_order_stages_legacy.rs");', 'mounted private legacy include'],
  ['struct SanitizingCheckoutCompletionPort', 'completion owner adapter'],
  ['pub struct CheckoutOrderRecoveryAdapter', 'recovery/read owner adapter'],
  ['wrap_checkout_completion_port', 'custom completion port wrapping'],
  ['sanitize_owner_error(&error_context, "complete_checkout", error)', 'completion sanitization'],
  ['sanitize_owner_error(&error_context, "recover_existing_checkout", error)', 'recovery sanitization'],
  ['sanitize_owner_error(&error_context, "read_checkout_order", error)', 'projection read sanitization'],
  ['struct CheckoutOrderStageDiagnosticError', 'redacted diagnostic token'],
  ['formatter.write_str("redacted")', 'redacted debug output'],
  ['struct CheckoutOrderStageContextFacts', 'bounded context projection'],
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
  ['owner_code = %error.code', 'owner code'],
  ['owner_kind = ?error.kind', 'owner kind'],
  ['owner_retryable = error.retryable', 'owner retryability'],
  ['boundary = CHECKOUT_ORDER_STAGE_ADAPTER_BOUNDARY', 'adapter boundary'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation', 'technical severity'],
  ['Checkout order request is invalid', 'validation public message'],
  ['Checkout order resource was not found', 'not-found public message'],
  ['Checkout order state conflicts with the requested operation', 'conflict public message'],
  ['Checkout order operation is not permitted', 'forbidden public message'],
  ['Checkout order service is temporarily unavailable', 'availability public message'],
  ['Checkout order operation could not be completed safely', 'invariant public message'],
  ['message: public_message.to_string()', 'sanitized owner message projection'],
]) requireText(facade, value, label);

for (const [value, label] of [
  ['error = ?boundary_error', 'raw legacy PortError diagnostic'],
  ['correlation_id = %context.correlation_id', 'raw legacy correlation diagnostic'],
  ['tenant_id = %context.tenant_id', 'raw legacy tenant diagnostic'],
  ['actor = ?context.actor', 'raw legacy actor diagnostic'],
  ['channel = ?context.channel', 'raw legacy channel diagnostic'],
  ['locale = %context.locale', 'raw legacy locale diagnostic'],
  ['internal_message = %boundary_error.message', 'raw legacy owner message diagnostic'],
]) requireText(legacy, value, label);

for (const [value, label] of [
  ['mod tracing_shim', 'legacy tracing suppression shim'],
  ['use super::tracing_shim as tracing;', 'legacy tracing redirect'],
]) requireText(facade, value, label);

for (const [value, label] of [
  ['error = ?error', 'raw canonical PortError logging'],
  ['error = ?boundary_error', 'raw boundary error logging'],
  ['correlation_id = %context.correlation_id', 'raw correlation logging'],
  ['tenant_id = %context.tenant_id', 'raw tenant logging'],
  ['actor = ?context.actor', 'raw actor logging'],
  ['channel = ?context.channel', 'raw channel logging'],
  ['locale = %context.locale', 'raw locale logging'],
  ['internal_message = %error.message', 'raw owner message logging'],
  ['message: error.message', 'raw owner message propagation'],
]) forbidText(facade, value, label);

for (const [value, label] of [
  ['CheckoutCompletionPort', 'retained completion port'],
  ['CheckoutOrderRecoveryAdapter', 'retained recovery adapter'],
  ['recover_existing_checkout(', 'retained recovery call'],
  ['complete_checkout(write_context.clone(), request)', 'retained completion call'],
  ['read_checkout_order(', 'retained projection read'],
  ['RecoverExistingCheckoutOrderRequest {', 'retained recovery request'],
  ['completion: request.clone()', 'retained completion request reuse'],
  ['legacy_snapshot_hash', 'retained legacy snapshot hash'],
  ['legacy_request_hash', 'retained legacy request hash'],
  ['validate_order_projection(&operation, &order, &[OrderStatusKind::Confirmed])', 'retained typed confirmed validation'],
  ['expected_stage: CheckoutOperationStage::InventoryReserved', 'retained inventory-reserved checkpoint'],
  ['next_stage: CheckoutOperationStage::OrderCreated', 'retained order-created checkpoint'],
  ['expected_stage: CheckoutOperationStage::OrderCreated', 'retained order-created admission'],
  ['next_stage: CheckoutOperationStage::PaymentReady', 'retained payment-ready checkpoint'],
  ['checkout:{operation_id}:order:complete', 'retained completion idempotency key'],
  ['.with_causation_id(operation_id.to_string())', 'retained causation'],
  ['.with_deadline(deadline)', 'retained deadline'],
  ['allowed_statuses: &[OrderStatusKind]', 'retained typed lifecycle policy'],
]) requireText(legacy, value, label);

for (const [value, label] of [
  ['pub struct CheckoutOrderStageExecutor', 'public executor facade'],
  ['pub fn with_completion_port(', 'public custom completion injection'],
  ['pub async fn advance_to_payment_ready(', 'public advance API'],
  ['pub async fn load_payment_ready_state(', 'public recovery API'],
  ['pub fn plan_journal(&self)', 'public plan journal API'],
]) requireText(facade, value, label);

for (const [key, expected] of Object.entries({
  mounted_facade_active: true,
  legacy_source_private: true,
  legacy_source_business_logic_changed: false,
  canonical_completion_port_preserved: true,
  canonical_recovery_adapter_preserved: true,
  recover_wrapped: true,
  complete_wrapped: true,
  read_wrapped: true,
  owner_message_public: false,
  owner_message_persisted: false,
  raw_port_error_logged: false,
  raw_context_values_logged: false,
  bounded_context_shapes_logged: true,
  owner_message_shape_logged: true,
  owner_message_length_logged: true,
  owner_code_preserved: true,
  owner_kind_preserved: true,
  owner_retryability_preserved: true,
  legacy_tracing_suppressed: true,
  completion_cutover_guard_updated: true,
  owner_stage_guard_updated: true,
  typed_lifecycle_guard_updated: true,
  successful_dtos_changed: false,
  checkpoint_order_changed: false,
  idempotency_identity_changed: false,
  commerce_ffa_status_changed: false,
  commerce_fba_status_changed: false,
  broad_mapper_cleanup_closed: false,
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

for (const [value, label] of [
  ['Status: **source-reviewed / unvalidated**', 'truthful document status'],
  ['Checkout order service is temporarily unavailable', 'documented static availability message'],
  ['No tests, Node verifiers, Cargo commands', 'validation disclosure'],
  ['The broader ecommerce correlation-safe mapper cleanup remains open', 'open broad mapper disclosure'],
]) requireText(doc, value, label);

if (!combined.includes('CheckoutOrderStageError::Boundary {')) {
  failures.push('combined stage source must retain the public Boundary variant');
}

if (failures.length > 0) {
  console.error('Checkout order stage error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout order recovery, completion, and projection-read owner failures are sanitized before the retained stage mapper',
);
