#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/rustok-order/src/checkout_payment_settlement.rs');
const portContract = read('crates/rustok-api/src/ports.rs');
const doc = read('crates/rustok-order/docs/checkout-payment-settlement-local-context.md');
const evidence = JSON.parse(
  read('crates/rustok-order/contracts/evidence/checkout-payment-settlement-diagnostic-safety-source.json'),
);
const review = JSON.parse(
  read('crates/rustok-order/contracts/evidence/checkout-payment-settlement-diagnostic-safety-source-review.json'),
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

const settlement = between(
  source,
  'impl CheckoutOrderPaymentSettlementPort for InProcessCheckoutOrderPaymentSettlementPort {',
  'fn order_payment_settlement_context_facts(',
  'checkout payment settlement implementation',
);
const identityValidation = between(
  source,
  'fn validate_identity(',
  'fn log_payment_settlement_lifecycle_conflict(',
  'durable identity validation',
);
const paymentIdentity = between(
  source,
  'fn log_payment_identity_conflict(',
  'fn validate_request(',
  'settled payment identity diagnostic',
);
const requestValidation = between(
  source,
  'fn validate_request(',
  'fn require_operation_context(',
  'request validation',
);
const contextValidation = between(
  source,
  'fn require_operation_context(',
  'fn log_order_payment_owner_warning(',
  'owner context validation',
);
const ownerMapper = source.slice(source.indexOf('fn log_order_payment_owner_warning('));

for (const [value, label] of [
  [
    'const ORDER_PAYMENT_SETTLEMENT_OWNER: &str = "rustok_order.checkout_payment_settlement";',
    'truthful owner',
  ],
  [
    'const ORDER_PAYMENT_SETTLEMENT_BOUNDARY: &str = "checkout_order_payment_settlement_port";',
    'stable owner boundary',
  ],
  ['const SETTLE_PAYMENT_OPERATION: &str = "settle_checkout_payment";', 'public operation'],
  ['struct OrderPaymentSettlementContextFacts {', 'safe context facts'],
  ['struct OrderPaymentSettlementRequestFacts {', 'safe request facts'],
  ['fn order_payment_settlement_context_facts(', 'context facts helper'],
  ['fn order_payment_settlement_request_facts(', 'request facts helper'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['context.require_policy(PortCallPolicy::write())?;', 'write policy admission'],
  ['context.require_write_semantics()?;', 'write semantics admission'],
  ['parse_tenant_id(&context, SETTLE_PAYMENT_OPERATION)?;', 'tenant parsing'],
  ['parse_actor_id(&context, SETTLE_PAYMENT_OPERATION)?;', 'actor parsing'],
  ['require_operation_context(', 'causation validation'],
  ['validate_request(&context, &request)?;', 'request validation call'],
  ['.read_by_operation(', 'identity read'],
  ['.adopt_legacy(', 'legacy identity adoption'],
  ['log_missing_checkout_identity(&context, &request);', 'missing identity diagnostic'],
  ['validate_identity(&context, tenant_id, &request, &identity)?;', 'identity validation call'],
  ['let current = self.load_order(&context, tenant_id, &request).await?;', 'order load'],
  ['OrderStatusKind::Confirmed => self', 'confirmed settlement transition'],
  ['.mark_paid(', 'mark paid owner call'],
  ['request.payment_reference.clone()', 'payment reference delegation'],
  ['request.payment_method.clone()', 'payment method delegation'],
  ['OrderStatusKind::Paid | OrderStatusKind::Shipped | OrderStatusKind::Delivered', 'replay states'],
  ['state @ (OrderStatusKind::Pending', 'rejected lifecycle states'],
  ['settled.payment_id.as_deref() == Some(request.payment_reference.as_str())', 'reference equality policy'],
  ['settled.payment_method.as_deref() == Some(request.payment_method.as_str())', 'method equality policy'],
]) requireText(settlement, value, label);

for (const [value, label] of [
  ['tenant_matches = identity.tenant_id == tenant_id', 'tenant comparison'],
  ['identity.checkout_operation_id == request.checkout_operation_id', 'operation comparison'],
  ['order_matches = identity.order_id == request.order_id', 'order comparison'],
  ['.is_none_or(|cart_id| cart_id == request.cart_id)', 'optional source-cart comparison'],
  ['.is_none_or(|collection_id| collection_id == request.payment_collection_id)', 'optional collection comparison'],
  ['tenant_matches,', 'tenant comparison evidence'],
  ['checkout_operation_matches,', 'operation comparison evidence'],
  ['order_matches,', 'order comparison evidence'],
  ['source_cart_matches,', 'cart comparison evidence'],
  ['payment_collection_matches,', 'collection comparison evidence'],
]) requireText(identityValidation, value, label);

for (const [content, value, label] of [
  [source, 'tenant_id_length = context_facts.tenant_id_length', 'tenant length fact'],
  [source, 'actor_kind = context_facts.actor_kind', 'actor kind fact'],
  [source, 'claim_count = context_facts.claim_count', 'claim count fact'],
  [source, 'channel_present = context_facts.channel_present', 'channel presence fact'],
  [source, 'causation_id_present = context_facts.causation_id_present', 'causation presence fact'],
  [source, 'idempotency_key_present = context_facts.idempotency_key_present', 'idempotency presence fact'],
  [source, 'boundary = ORDER_PAYMENT_SETTLEMENT_BOUNDARY', 'boundary diagnostic'],
  [source, 'correlation_id = %context.correlation_id', 'correlation diagnostic'],
  [requestValidation, 'checkout_operation_id_non_nil = request_facts.checkout_operation_id_non_nil', 'request operation shape'],
  [requestValidation, 'payment_reference_length = request_facts.payment_reference_length', 'reference length'],
  [requestValidation, 'payment_method_length = request_facts.payment_method_length', 'method length'],
  [paymentIdentity, 'payment_reference_matches,', 'reference equality fact'],
  [paymentIdentity, 'payment_method_matches,', 'method equality fact'],
  [paymentIdentity, 'settled_payment_reference_length = ?settled', 'settled reference length'],
  [paymentIdentity, 'settled_payment_method_length = ?settled', 'settled method length'],
  [source, 'order_state = ?order_state', 'typed lifecycle fact'],
  [ownerMapper, 'resource_id_present = resource_id.is_some()', 'resource presence fact'],
  [ownerMapper, 'resource_id_non_nil = ?resource_id.map(|value| !value.is_nil())', 'resource non-nil fact'],
  [ownerMapper, 'internal_cause_length = ?internal_cause.map(|value| value.chars().count())', 'owner cause length'],
  [ownerMapper, 'error = ?error', 'private technical cause'],
]) requireText(content, value, label);

for (const [value, label] of [
  ['"require_durable_checkout_identity"', 'missing identity operation'],
  ['"validate_durable_checkout_identity"', 'identity conflict operation'],
  ['"validate_payment_settlement_lifecycle"', 'lifecycle operation'],
  ['"validate_settled_payment_identity"', 'payment identity operation'],
  ['"validate_request"', 'request operation'],
  ['"validate_causation_context"', 'causation operation'],
  ['"validate_owner_context"', 'owner context operation'],
  ['"owner_storage"', 'storage operation'],
  ['"validate_owner_request"', 'owner validation operation'],
  ['"apply_payment_settlement_state"', 'owner transition operation'],
  ['"owner_invariant"', 'owner invariant operation'],
]) requireText(source, value, label);

for (const forbidden of [
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'checkout_operation_id = %request.checkout_operation_id',
  'cart_id = %request.cart_id',
  'request_cart_id = %request.cart_id',
  'request_order_id = %request.order_id',
  'request_payment_collection_id = %request.payment_collection_id',
  'identity_tenant_id = %identity.tenant_id',
  'identity_checkout_operation_id = %identity.checkout_operation_id',
  'identity_order_id = %identity.order_id',
  'identity_source_cart_id = ?identity.source_cart_id',
  'identity_payment_collection_id = ?identity.payment_collection_id',
  'order_id = %current.id',
  'order_id = %settled.id',
  'order_id = %order_id',
  'requested_payment_reference = %request.payment_reference',
  'requested_payment_method = %request.payment_method',
  'settled_payment_reference = ?settled.payment_id',
  'settled_payment_method = ?settled.payment_method',
  'expected_checkout_operation_id = %checkout_operation_id',
  'actual_causation_id = ?context.causation_id',
  'resource_id = %return_id',
  'resource_id = %change_id',
  'cause = %cause',
]) forbidText(source, forbidden, 'raw owner diagnostic');

for (const [value, label] of [
  ['"checkout requires manual reconciliation"', 'static identity-missing envelope'],
  ['"checkout order identity conflicts with the payment settlement request"', 'static identity-conflict envelope'],
  ['"checkout order lifecycle does not allow payment settlement"', 'static lifecycle envelope'],
  ['"checkout order is settled by another payment identity"', 'static payment-identity envelope'],
  ['"checkout payment settlement request is invalid"', 'static request envelope'],
  ['"checkout operation context is invalid"', 'static causation envelope'],
  ['"order request context is invalid"', 'static owner-context envelope'],
  ['"order storage is temporarily unavailable"', 'static storage envelope'],
  ['"order was not found"', 'static not-found envelope'],
  ['"checkout order payment settlement request is invalid"', 'static owner-validation envelope'],
  ['"order lifecycle conflicts with payment settlement"', 'static transition envelope'],
  ['"related order resource was not found"', 'static related-resource envelope'],
  ['"order payment settlement failed an internal invariant"', 'static invariant envelope'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['OrderError::Database(error)', 'database mapping'],
  ['OrderError::OrderNotFound(order_id)', 'order not-found mapping'],
  ['OrderError::Validation(cause)', 'owner validation mapping'],
  ['OrderError::InvalidTransition { from, to }', 'transition mapping'],
  ['OrderError::OrderReturnNotFound(return_id)', 'return mapping'],
  ['OrderError::OrderChangeNotFound(change_id)', 'change mapping'],
  ['OrderError::Core(error)', 'core mapping'],
]) requireText(ownerMapper, value, label);

for (const [value, label] of [
  ['pub struct PortContext {', 'shared port context'],
  ['pub correlation_id: String', 'shared correlation field'],
  ['pub struct PortError {', 'shared port error'],
  ['pub fn validation(', 'typed validation constructor'],
  ['pub fn conflict(', 'typed conflict constructor'],
  ['pub fn invariant_violation(', 'typed invariant constructor'],
]) requireText(portContract, value, label);

if (evidence.status !== 'order_checkout_payment_settlement_diagnostic_safety_source_unvalidated') {
  failures.push('unexpected settlement diagnostic source evidence status');
}
for (const key of [
  'canonical_owner_safe_context_shape',
  'canonical_owner_safe_request_shape',
  'canonical_owner_safe_identity_shape',
  'canonical_owner_safe_payment_identity_shape',
  'identity_comparison_facts_logged',
  'payment_reference_and_method_equality_facts_logged',
]) {
  if (evidence.source_contract?.[key] !== true) failures.push(`evidence ${key} must be true`);
}
for (const key of [
  'raw_tenant_id_logged',
  'raw_checkout_operation_id_logged',
  'raw_payment_reference_logged',
  'raw_payment_method_logged',
  'public_code_changed',
  'public_message_changed',
  'commerce_orchestration_changed',
  'broad_ecommerce_cleanup_closed',
]) {
  if (evidence.source_contract?.[key] !== false) failures.push(`evidence ${key} must be false`);
}
for (const key of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
  'compile_proven',
  'settlement_replay_proven',
  'restart_proven',
  'mounted_runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) failures.push(`validation ${key} must be false`);
}
if (review.status !== 'order_checkout_payment_settlement_diagnostic_safety_source_reviewed_unvalidated') {
  failures.push('unexpected settlement diagnostic source-review status');
}
if (review.review_findings?.all_public_port_errors_preserved !== true) {
  failures.push('source review must preserve all public PortError envelopes');
}
if (review.review_findings?.runtime_evidence_claimed !== false) {
  failures.push('source review must not claim runtime evidence');
}

for (const marker of [
  '# Order checkout payment settlement diagnostic safety',
  'Status: **source-ready / unvalidated**',
  'Human-readable `PortError.message` is not used as control flow.',
  'The modified boundary does not log raw tenant, actor, channel, locale, causation,',
  'The broad ecommerce correlation-safe mapper cleanup remains open.',
  'These commands were intentionally not run by the implementation agent:',
]) requireText(doc, marker, 'settlement diagnostic documentation');

if (failures.length > 0) {
  console.error('Order payment settlement diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Order payment settlement owner diagnostics use safe context/request/identity/payment shape while preserving settlement behavior and public PortError envelopes',
);
