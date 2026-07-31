#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const checkoutExecution = read('crates/rustok-payment/src/checkout_execution.rs');
const diagnosticSafety = read(
  'crates/rustok-payment/src/checkout_execution/diagnostic_safety.rs',
);
const portImpl = read('crates/rustok-payment/src/checkout_execution/port_impl.rs');
const validationErrors = read(
  'crates/rustok-payment/src/checkout_execution/validation_errors.rs',
);
const types = read('crates/rustok-payment/src/checkout_execution/types.rs');
const prepareAuthorize = read(
  'crates/rustok-payment/src/checkout_execution/prepare_authorize.rs',
);
const captureProvider = read(
  'crates/rustok-payment/src/checkout_execution/capture_provider.rs',
);
const compensation = read('crates/rustok-payment/src/checkout_compensation_context.rs');
const doc = read('crates/rustok-payment/docs/checkout-execution-local-context.md');
const paymentPlan = read('crates/rustok-payment/docs/implementation-plan.md');
const commercePlan = read('crates/rustok-commerce/docs/implementation-plan.md');
const evidence = JSON.parse(
  read(
    'crates/rustok-payment/contracts/evidence/checkout-execution-diagnostic-safety-source.json',
  ),
);
const review = JSON.parse(
  read(
    'crates/rustok-payment/contracts/evidence/checkout-execution-diagnostic-safety-source-review.json',
  ),
);

const failures = [];
const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (content, value, expected, label) => {
  const count = content.split(value).length - 1;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
};

for (const marker of [
  'include!("checkout_execution/types.rs");',
  'include!("checkout_execution/diagnostic_safety.rs");',
  'include!("checkout_execution/port_impl.rs");',
  'include!("checkout_execution/validation_errors.rs");',
]) requireText(checkoutExecution, marker, 'checkout execution include topology');

for (const marker of [
  'async fn prepare_checkout_collection(',
  'async fn authorize_checkout_collection(',
  'async fn capture_checkout_collection(',
  'async fn read_checkout_collection(',
]) requireText(types, marker, 'published checkout execution port');

requireCount(
  portImpl,
  'map_checkout_payment_execution_local_port_error(',
  5,
  'four mapper calls plus mapper definition',
);
requireCount(
  portImpl,
  'checkout_payment_execution_diagnostic_facts(',
  4,
  'four request-shape captures',
);
requireCount(
  portImpl,
  'let diagnostic_context = context.clone();',
  4,
  'four accepted context captures',
);

for (const marker of [
  'self.prepare(&context, owner_operation, tenant_id, request).await',
  '.authorize(&context, owner_operation, tenant_id, request)',
  'self.capture(&context, owner_operation, tenant_id, request).await',
  'self.read(&context, owner_operation, tenant_id, request).await',
  'validate_identity(&request.identity)?;',
  '.get_collection(tenant_id, request.collection_id)',
  'validate_collection(&collection, tenant_id, &request.identity)?;',
]) requireText(portImpl, marker, 'preserved execution delegation');

for (const marker of [
  'const PAYMENT_EXECUTION_BOUNDARY: &str = "checkout_payment_execution_port";',
  'struct CheckoutPaymentExecutionContextFacts',
  'tenant_id_length: context.tenant_id.chars().count()',
  'actor_id_length: context.actor.id.chars().count()',
  'claim_count: context.claims.len()',
  'role_count: context.roles.len()',
  'channel_present: context.channel.is_some()',
  'locale_length: context.locale.chars().count()',
  'causation_id_present: context.causation_id.is_some()',
  'traceparent_present: context.traceparent.is_some()',
  'idempotency_key_present: context.idempotency_key.is_some()',
  'checkout_operation_id_non_nil: !identity.checkout_operation_id.is_nil()',
  'cart_id_non_nil: !identity.cart_id.is_nil()',
  'order_id_non_nil: !identity.order_id.is_nil()',
  'customer_id_present: identity.customer_id.is_some()',
  'collection_id_present: collection_id.is_some()',
  'amount_text_length: identity.amount.to_string().chars().count()',
  'requested_provider_id_present: requested_provider_id.is_some()',
  'provider_payment_id_present: provider_payment_id.is_some()',
  'fn checkout_payment_execution_local_operation(',
  'match code {',
]) requireText(diagnosticSafety, marker, 'safe diagnostic source');

for (const [code, operation] of [
  ['payment.checkout_identity_invalid', 'validate_checkout_identity'],
  ['payment.checkout_currency_invalid', 'validate_checkout_currency'],
  ['payment.checkout_plan_hash_invalid', 'validate_checkout_plan_hash'],
  ['payment.checkout_collection_identity_missing', 'require_collection_identity'],
  ['payment.checkout_authorize_state_conflict', 'validate_authorize_lifecycle'],
  ['payment.checkout_capture_state_conflict', 'validate_capture_lifecycle'],
  ['payment.provider_metadata_invalid', 'validate_provider_metadata'],
  ['payment.provider_identity_conflict', 'validate_provider_identity'],
  ['payment.provider_request_encoding_failed', 'encode_provider_request'],
  ['payment.database_unavailable', 'owner_storage'],
  ['payment.checkout_execution_state_conflict', 'apply_payment_lifecycle'],
  ['payment.provider_unavailable', 'execute_provider_operation'],
  ['payment.checkout_execution_manual_reconciliation', 'require_manual_reconciliation'],
  ['payment.provider_not_configured', 'resolve_provider'],
]) {
  requireText(diagnosticSafety, `"${code}"`, `stable code ${code}`);
  requireText(diagnosticSafety, `"${operation}"`, `local operation ${operation}`);
}

for (const forbidden of [
  'error.message.as_str()',
  'match (error.code.as_str(), error.message.as_str())',
  'checkout payment identity contains invalid UUID or amount fields',
  'payment storage is temporarily unavailable',
  'payment provider is temporarily unavailable',
]) forbidText(diagnosticSafety, forbidden, 'message-independent classifier');

for (const marker of [
  'checkout_payment_execution_local_operation(operation, error.code.as_str())',
  'let context_facts = checkout_payment_execution_context_facts(context);',
  'correlation_id = %context.correlation_id',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'channel_present = context_facts.channel_present',
  'locale_length = context_facts.locale_length',
  'idempotency_key_present = context_facts.idempotency_key_present',
  'checkout_operation_id_non_nil = facts.checkout_operation_id_non_nil',
  'cart_id_non_nil = facts.cart_id_non_nil',
  'collection_id_present = facts.collection_id_present',
  'amount_text_length = facts.amount_text_length',
  'internal_code = %error.code',
  'internal_message = %error.message',
  'boundary = PAYMENT_EXECUTION_BOUNDARY',
  '\n    error\n}',
]) requireText(portImpl, marker, 'safe delegated outcome mapping');

for (const marker of [
  'fn log_checkout_payment_execution_admission_rejection(',
  'checkout_payment_execution_context_facts(context)',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_id_length = context_facts.actor_id_length',
  'claim_count = context_facts.claim_count',
  'role_count = context_facts.role_count',
  'causation_id_present = context_facts.causation_id_present',
  'traceparent_present = context_facts.traceparent_present',
  'idempotency_key_present = context_facts.idempotency_key_present',
  'checkout_operation_id_non_nil = !checkout_operation_id.is_nil()',
  'causation_matches = false',
  'operation_id_non_nil = !operation.id.is_nil()',
  'boundary = PAYMENT_EXECUTION_BOUNDARY',
  'PortError::validation(',
  'PortError::unavailable(',
  'PortError::not_found(',
  'PortError::conflict(',
  'PortError::invariant_violation(',
]) requireText(validationErrors, marker, 'safe admission and owner mapping');

for (const [content, label] of [
  [portImpl, 'delegated outcome diagnostics'],
  [validationErrors, 'admission and owner diagnostics'],
]) {
  for (const forbidden of [
    'tenant_id = %context.tenant_id',
    'internal_tenant_id = %context.tenant_id',
    'actor = ?context.actor',
    'channel = ?context.channel',
    'locale = %context.locale',
    'causation_id = ?context.causation_id',
    'traceparent = ?context.traceparent',
    'idempotency_key = ?context.idempotency_key',
    'checkout_operation_id = %',
    'cart_id = %',
    'order_id = %',
    'customer_id = ?',
    'collection_id = ?',
    'request_amount = %',
    'operation_id = %operation.id',
    'currency_code =',
    'order_plan_hash =',
    'requested_provider_id =',
    'provider_payment_id =',
    'metadata =',
  ]) forbidText(content, forbidden, label);
}

for (const marker of [
  'validate_identity(&request.identity)?;',
  'PaymentCollectionStatusKind',
  'payment_collection:{collection_id}:authorize',
]) requireText(
  `${prepareAuthorize}\n${captureProvider}\n${paymentPlan}`,
  marker,
  'preserved provider execution policy',
);

requireText(
  compensation,
  'match (error.code.as_str(), error.message.as_str())',
  'compensation remains separate message-pair cleanup',
);
requireText(
  compensation,
  'tenant_id = %context.tenant_id',
  'compensation raw context remains explicitly outside this slice',
);

if (evidence.status !== 'payment_checkout_execution_diagnostic_safety_source_unvalidated') {
  failures.push(`unexpected evidence status: ${evidence.status}`);
}
if (
  review.status !==
  'payment_checkout_execution_diagnostic_safety_source_reviewed_unvalidated'
) {
  failures.push(`unexpected review status: ${review.status}`);
}

for (const [key, expected] of Object.entries({
  stable_code_only_local_classification: true,
  human_message_control_flow: false,
  delegated_port_error_changed: false,
  public_code_changed: false,
  public_message_changed: false,
  public_kind_changed: false,
  public_retryability_changed: false,
  owner_delegation_changed: false,
  request_response_dto_changed: false,
  provider_journal_policy_changed: false,
  raw_tenant_id_logged: false,
  raw_actor_id_logged: false,
  raw_channel_logged: false,
  raw_locale_logged: false,
  raw_causation_id_logged: false,
  raw_traceparent_logged: false,
  raw_idempotency_key_logged: false,
  raw_checkout_operation_id_logged: false,
  raw_cart_id_logged: false,
  raw_order_id_logged: false,
  raw_customer_id_logged: false,
  raw_collection_id_logged: false,
  raw_amount_logged: false,
  raw_provider_identity_logged: false,
  safe_context_shape_logged: true,
  safe_request_shape_logged: true,
  correlation_id_logged: true,
  original_error_private: true,
  compensation_boundary_changed: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}

for (const key of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
  'compile_proven',
  'provider_replay_proven',
  'restart_proven',
  'remote_port_proven',
  'mounted_runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

for (const marker of [
  'Status: **source-ready / unvalidated**',
  'Human-readable `PortError.message` is not used as control flow.',
  'Payment checkout compensation still uses message-pair classification',
  'No FBA or FFA status is promoted from source inspection.',
]) requireText(doc, marker, 'diagnostic safety documentation');

requireText(
  paymentPlan,
  'Payment checkout execution diagnostic safety: `source_ready_unvalidated`',
  'payment owner source status',
);
requireText(
  paymentPlan,
  'Checkout compensation diagnostic cleanup remains open',
  'compensation nonclaim',
);
requireText(
  commercePlan,
  'Finish correlation-safe mapper cleanup for order, payment execution/compensation,',
  'broad ecommerce cleanup remains open',
);

if (failures.length > 0) {
  console.error('Payment checkout execution diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Payment checkout execution diagnostics use stable-code attribution and safe context/request shape while preserving public PortError behavior; execution evidence remains open',
);
