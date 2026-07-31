#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const wrapper = read('crates/rustok-order/src/checkout_compensation_local_context.rs');
const shared = read('crates/rustok-order/src/checkout_owner_context.rs');
const owner = read('crates/rustok-order/src/checkout_compensation.rs');
const lib = read('crates/rustok-order/src/lib.rs');
const doc = read('crates/rustok-order/docs/checkout-compensation-local-context.md');
const ownerDoc = read('crates/rustok-order/docs/checkout-owner-context.md');
const commercePlan = read('crates/rustok-commerce/docs/implementation-plan.md');
const evidence = JSON.parse(
  read('crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source.json'),
);
const review = JSON.parse(
  read('crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source-review.json'),
);

const failures = [];
const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};

for (const marker of [
  'mod checkout_compensation_local_context;',
  '#[path = "checkout_owner_context.rs"]',
  'mod checkout_owner_context_impl;',
  'pub mod checkout_owner_context {',
  'pub use checkout_compensation_local_context::{',
]) requireText(lib, marker, 'public compensation facade');

const wrapperOperation = between(
  wrapper,
  'async fn compensate_checkout_order(',
  'pub fn in_process_checkout_order_compensation_port(',
  'compensation wrapper operation',
);
for (const marker of [
  'let diagnostic_context = context.clone();',
  'let diagnostic_facts = order_compensation_request_facts(&request);',
  'self.inner.compensate_checkout_order(context, request).await',
  'map_checkout_order_compensation_local_port_error(',
]) requireText(wrapperOperation, marker, 'wrapper delegation');
const wrapperOrder = [
  wrapperOperation.indexOf('let diagnostic_context = context.clone();'),
  wrapperOperation.indexOf('order_compensation_request_facts(&request)'),
  wrapperOperation.indexOf('self.inner.compensate_checkout_order(context, request).await'),
  wrapperOperation.indexOf('map_checkout_order_compensation_local_port_error('),
];
if (!wrapperOrder.every((value, index) => value >= 0 && (index === 0 || wrapperOrder[index - 1] < value))) {
  failures.push('wrapper must retain context/request shape before unchanged delegation and map only returned errors');
}

for (const marker of [
  'struct OrderCompensationContextFacts',
  'struct OrderCompensationRequestFacts',
  'tenant_id_length: context.tenant_id.chars().count()',
  'actor_id_length: context.actor.id.chars().count()',
  'claim_count: context.claims.len()',
  'role_count: context.roles.len()',
  'channel_present: context.channel.is_some()',
  'locale_length: context.locale.chars().count()',
  'causation_id_present: context.causation_id.is_some()',
  'traceparent_present: context.traceparent.is_some()',
  'idempotency_key_present: context.idempotency_key.is_some()',
  'checkout_operation_id_non_nil: !request.checkout_operation_id.is_nil()',
  'cart_id_non_nil: !request.cart_id.is_nil()',
  'expected_order_id_present: request.expected_order_id.is_some()',
  'reason_length: request.reason.as_ref().map(|value| value.chars().count())',
]) requireText(wrapper, marker, 'safe wrapper shape');

for (const [code, operation] of [
  ['order.checkout_compensation_identity_invalid', 'validate_request'],
  ['order.checkout_compensation_identity_conflict', 'validate_durable_checkout_identity'],
  ['order.checkout_compensation_state_conflict', 'apply_compensation_state'],
  ['order.checkout_compensation_manual_reconciliation', 'require_manual_reconciliation'],
]) {
  requireText(wrapper, `"${code}"`, `wrapper code ${code}`);
  requireText(wrapper, `"${operation}"`, `wrapper operation ${operation}`);
}
for (const marker of [
  'fn order_compensation_local_operation(code: &str)',
  'match code {',
  'order_compensation_local_operation(error.code.as_str())',
  '_ => None,',
  '\n    error\n}',
]) requireText(wrapper, marker, 'stable-code wrapper mapper');
for (const forbidden of [
  'error.message.as_str()',
  'match (error.code.as_str(), error.message.as_str())',
  'adopt_cancelled_after_transition_race',
]) forbidText(wrapper, forbidden, 'message-independent wrapper mapper');

for (const marker of [
  'struct OrderCheckoutContextFacts',
  'fn order_checkout_context_facts(',
  'checkout_order_payment_settlement_local_operation(error.code.as_str())',
  'expected_checkout_operation_id_present',
  'expected_checkout_operation_id_non_nil',
  'causation_matches = false',
]) requireText(shared, marker, 'shared safe owner context');
forbidText(shared, 'match (error.code.as_str(), error.message.as_str())', 'shared message-pair classifier');

for (const marker of [
  'const ORDER_COMPENSATION_BOUNDARY: &str = "checkout_order_compensation_port";',
  'struct OrderCompensationContextFacts',
  'fn log_invalid_compensation_request(',
  'fn log_compensation_transition_conflict(',
  'tenant_matches',
  'checkout_operation_matches',
  'source_cart_matches',
  'expected_order_matches',
  'request_checkout_operation_id_non_nil',
  'identity_order_id_non_nil',
  'order_id_present = order_id.is_some()',
  'resource_id_present = resource_id.is_some()',
  'fn log_order_owner_warning(',
  'fn log_order_owner_error',
]) requireText(owner, marker, 'safe compensation owner diagnostics');

for (const [content, label] of [
  [wrapper, 'compensation wrapper'],
  [shared, 'shared checkout wrapper'],
  [owner, 'compensation owner'],
]) {
  for (const forbidden of [
    'tenant_id = %context.tenant_id',
    'actor = ?context.actor',
    'channel = ?context.channel',
    'locale = %context.locale',
    'causation_id = ?context.causation_id',
    'traceparent = ?context.traceparent',
    'idempotency_key = ?context.idempotency_key',
    'checkout_operation_id = %',
    'cart_id = %',
    'order_id = %',
    'resource_id = %',
    'request_checkout_operation_id = %',
    'identity_tenant_id = %',
    'identity_order_id = %',
    'reason = %reason',
  ]) forbidText(content, forbidden, `${label} raw diagnostics`);
}

for (const marker of [
  'context.require_policy(PortCallPolicy::write())?;',
  'context.require_write_semantics()?;',
  'let tenant_id = parse_tenant_id(&context, COMPENSATE_OPERATION)?;',
  'let actor_id = parse_actor_id(&context, COMPENSATE_OPERATION)?;',
  'request.checkout_operation_id',
  '.read_by_operation(',
  '.adopt_legacy(',
  'return if request.expected_order_id.is_none()',
  'validate_identity(&context, tenant_id, &request, &identity)?;',
  '.get_order(tenant_id, identity.order_id)',
  '.cancel_order(tenant_id, actor_id, order.id, reason)',
  'if current.status_kind() == OrderStatusKind::Cancelled',
  'state @ (OrderStatusKind::Paid',
]) requireText(owner, marker, 'preserved compensation behavior');

for (const message of [
  'checkout compensation request is invalid',
  'checkout order identity conflicts with the compensation request',
  'checkout order changed while compensation was being applied',
  'checkout requires manual reconciliation',
  'order storage is temporarily unavailable',
  'order was not found',
  'checkout order compensation request is invalid',
  'checkout order lifecycle conflicts with compensation',
  'related order resource was not found',
  'order compensation failed an internal invariant',
]) requireText(owner, `"${message}"`, `preserved public message ${message}`);

if (evidence.status !== 'order_checkout_compensation_diagnostic_safety_source_unvalidated') {
  failures.push(`unexpected evidence status: ${evidence.status}`);
}
if (review.status !== 'order_checkout_compensation_diagnostic_safety_source_reviewed_unvalidated') {
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
  write_admission_order_changed: false,
  identity_read_or_legacy_adoption_changed: false,
  cancellation_or_replay_policy_changed: false,
  raw_tenant_id_logged: false,
  raw_actor_id_logged: false,
  raw_checkout_operation_id_logged: false,
  raw_cart_id_logged: false,
  raw_order_id_logged: false,
  safe_context_shape_logged: true,
  safe_request_shape_logged: true,
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
  'compensation_replay_proven',
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
  'Raw tenant, actor, channel, locale, causation, traceparent, idempotency, checkout,',
  'The broad ecommerce correlation-safe mapper item remains open',
]) requireText(doc, marker, 'compensation documentation');
for (const marker of [
  'Status: **source-ready / unvalidated**',
  'Raw tenant, actor, channel, locale, causation, traceparent, and idempotency values are',
  'Payment-settlement owner-local request/identity/lifecycle diagnostics remain a',
]) requireText(ownerDoc, marker, 'owner-context documentation');
requireText(
  commercePlan,
  'Finish correlation-safe mapper cleanup for order, payment execution/compensation,',
  'broad ecommerce cleanup remains open',
);

if (failures.length > 0) {
  console.error('Order compensation diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Order checkout compensation uses stable-code attribution and safe context/request/identity shape while preserving public PortError and owner behavior; runtime evidence remains open',
);
