#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/rustok-order/src/checkout_compensation.rs');
const evidence = JSON.parse(
  read('crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source.json'),
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
  'const ORDER_COMPENSATION_OWNER: &str = "rustok_order.checkout_compensation";',
  'const ORDER_COMPENSATION_BOUNDARY: &str = "checkout_order_compensation_port";',
  'const COMPENSATE_OPERATION: &str = "compensate_checkout_order";',
  'struct OrderCompensationContextFacts',
  'tenant_id_length: context.tenant_id.chars().count()',
  'actor_id_length: context.actor.id.chars().count()',
  'claim_count: context.claims.len()',
  'role_count: context.roles.len()',
  'channel_present: context.channel.is_some()',
  'locale_length: context.locale.chars().count()',
  'causation_id_present: context.causation_id.is_some()',
  'traceparent_present: context.traceparent.is_some()',
  'idempotency_key_present: context.idempotency_key.is_some()',
]) requireText(source, marker, 'owner safe context');

const compensation = between(
  source,
  'impl CheckoutOrderCompensationPort for InProcessCheckoutOrderCompensationPort {',
  'fn order_compensation_context_facts(',
  'compensation owner operation',
);
for (const marker of [
  'context.require_policy(PortCallPolicy::write())?;',
  'context.require_write_semantics()?;',
  'let tenant_id = parse_tenant_id(&context, COMPENSATE_OPERATION)?;',
  'let actor_id = parse_actor_id(&context, COMPENSATE_OPERATION)?;',
  'require_operation_context(',
  'log_invalid_compensation_request(&context, &request);',
  'self.resolve_identity(&context, &request).await?',
  'return if request.expected_order_id.is_none()',
  'validate_identity(&context, tenant_id, &request, &identity)?;',
  '.get_order(tenant_id, identity.order_id)',
  '.cancel_or_adopt_cancelled(&context, tenant_id, actor_id, order, request.reason)',
]) requireText(compensation, marker, 'preserved operation flow');

for (const marker of [
  'fn log_invalid_compensation_request(',
  'checkout_operation_id_non_nil = !request.checkout_operation_id.is_nil()',
  'cart_id_non_nil = !request.cart_id.is_nil()',
  'expected_order_id_present = request.expected_order_id.is_some()',
  'reason_present = request.reason.is_some()',
  'reason_length = ?request.reason.as_ref().map(|value| value.chars().count())',
  'fn log_compensation_transition_conflict(',
  'order_id_non_nil = !order_id.is_nil()',
  'current_state = ?current_state',
  'local_operation = "apply_compensation_state"',
]) requireText(source, marker, 'safe request and lifecycle evidence');

const identity = between(
  source,
  'fn validate_identity(',
  'fn require_operation_context(',
  'identity validation',
);
for (const marker of [
  'let tenant_matches = identity.tenant_id == tenant_id;',
  'let checkout_operation_matches =',
  'let source_cart_matches = identity',
  'let expected_order_matches = request',
  'tenant_matches,',
  'checkout_operation_matches,',
  'source_cart_matches,',
  'expected_order_matches,',
  'request_checkout_operation_id_non_nil',
  'request_cart_id_non_nil',
  'identity_checkout_operation_id_non_nil',
  'identity_order_id_non_nil',
  'identity_source_cart_id_present',
]) requireText(identity, marker, 'safe identity comparison evidence');

for (const marker of [
  'checkout_operation_id_non_nil = !checkout_operation_id.is_nil()',
  'causation_matches = false',
  'fn log_context_parse_rejection',
  'field,',
  'value_length,',
  'fn manual_reconciliation(',
  'order_id_present = order_id.is_some()',
  'order_id_non_nil = ?order_id.map(|value| !value.is_nil())',
  'order_state = ?order_state',
  'internal_reason = reason',
  'fn log_order_owner_warning(',
  'resource_id_present = resource_id.is_some()',
  'resource_id_non_nil = ?resource_id.map(|value| !value.is_nil())',
  'fn log_order_owner_error',
]) requireText(source, marker, 'safe owner outcome evidence');

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
  'resource_id = %',
  'request_checkout_operation_id = %',
  'request_cart_id = %',
  'request_expected_order_id = ?',
  'identity_tenant_id = %',
  'identity_checkout_operation_id = %',
  'identity_order_id = %',
  'identity_source_cart_id = ?',
  'reason = %reason',
]) forbidText(source, forbidden, 'raw compensation owner diagnostic');

for (const marker of [
  'Err(OrderError::InvalidTransition { from, to })',
  'if current.status_kind() == OrderStatusKind::Cancelled',
  'state @ (OrderStatusKind::Paid',
  'OrderStatusKind::Unknown => Err(manual_reconciliation(',
  '.read_by_operation(',
  '.adopt_legacy(',
  '.cancel_order(tenant_id, actor_id, order.id, reason)',
]) requireText(source, marker, 'preserved compensation state machine');

for (const [code, message] of [
  ['order.checkout_compensation_identity_invalid', 'checkout compensation request is invalid'],
  ['order.checkout_compensation_identity_conflict', 'checkout order identity conflicts with the compensation request'],
  ['order.checkout_compensation_state_conflict', 'checkout order changed while compensation was being applied'],
  ['order.checkout_compensation_manual_reconciliation', 'checkout requires manual reconciliation'],
  ['order.database_unavailable', 'order storage is temporarily unavailable'],
  ['order.order_not_found', 'order was not found'],
  ['order.checkout_compensation_validation', 'checkout order compensation request is invalid'],
  ['order.related_resource_not_found', 'related order resource was not found'],
  ['order.invariant_violation', 'order compensation failed an internal invariant'],
]) {
  requireText(source, `"${code}"`, `public code ${code}`);
  requireText(source, `"${message}"`, `public message ${message}`);
}

for (const [variant, localOperation] of [
  ['OrderError::Database(error)', 'owner_storage'],
  ['OrderError::OrderNotFound(order_id)', 'load_order'],
  ['OrderError::Validation(cause)', 'validate_owner_request'],
  ['OrderError::InvalidTransition { from, to }', 'apply_compensation_state'],
  ['OrderError::OrderReturnNotFound(return_id)', 'load_related_order_resource'],
  ['OrderError::OrderChangeNotFound(change_id)', 'load_related_order_resource'],
  ['OrderError::Core(error)', 'owner_invariant'],
]) {
  requireText(source, variant, `owner mapper variant ${variant}`);
  requireText(source, `"${localOperation}"`, `owner mapper operation ${localOperation}`);
}

if (evidence.source_contract?.owner_safe_context_shape !== true) {
  failures.push('evidence owner_safe_context_shape must be true');
}
if (evidence.source_contract?.owner_safe_identity_shape !== true) {
  failures.push('evidence owner_safe_identity_shape must be true');
}
if (evidence.source_contract?.public_message_changed !== false) {
  failures.push('evidence public_message_changed must be false');
}
if (evidence.validation?.compile_proven !== false) {
  failures.push('evidence compile_proven must remain false');
}

if (failures.length > 0) {
  console.error('Order checkout compensation owner diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Order checkout compensation owner diagnostics retain safe context/identity shape and static public envelopes while preserving cancellation and reconciliation behavior',
);
