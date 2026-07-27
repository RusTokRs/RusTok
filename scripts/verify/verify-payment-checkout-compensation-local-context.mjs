#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const lib = read('crates/rustok-payment/src/lib.rs');
const wrapper = read('crates/rustok-payment/src/checkout_compensation_context.rs');
const owner = read('crates/rustok-payment/src/checkout_compensation.rs');
const commerce = read('crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs');
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

for (const [value, label] of [
  ['pub mod checkout_compensation;', 'legacy compensation module path'],
  ['mod checkout_compensation_context;', 'private context wrapper module'],
  ['pub use checkout_compensation::{', 'selective legacy contract export'],
  ['CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,', 'root trait/request compatibility'],
  ['pub use checkout_compensation_context::{', 'canonical context wrapper export'],
  ['InProcessCheckoutPaymentCompensationPort, in_process_checkout_payment_compensation_port,', 'root type/factory cutover'],
]) requireText(lib, value, label);
forbidText(lib, 'pub use checkout_compensation::*;', 'legacy root glob bypass');

for (const [value, label] of [
  ['InProcessCheckoutPaymentCompensationPort as PersistentCheckoutPaymentCompensationPort', 'persistent owner alias'],
  ['inner: PersistentCheckoutPaymentCompensationPort', 'wrapper inner owner'],
  ['PersistentCheckoutPaymentCompensationPort::new(db)', 'default constructor delegation'],
  ['PersistentCheckoutPaymentCompensationPort::with_provider_registry(', 'provider registry constructor delegation'],
  ['pub fn in_process_checkout_payment_compensation_port(', 'canonical factory'],
  ['Arc::new(InProcessCheckoutPaymentCompensationPort::new(db))', 'factory wrapper construction'],
]) requireText(wrapper, value, label);

const operation = between(
  wrapper,
  'async fn compensate_checkout_payment(',
  'fn checkout_payment_compensation_diagnostic_facts(',
  'compensation wrapper operation',
);
for (const [value, label] of [
  ['let diagnostic_context = context.clone();', 'accepted context retention'],
  ['let diagnostic_facts = checkout_payment_compensation_diagnostic_facts(&request);', 'safe request-fact retention'],
  ['.inner\n            .compensate_checkout_payment(context, request)', 'unchanged owner delegation'],
  ['result.map_err(|error| {', 'post-delegation mapping'],
  ['map_checkout_payment_compensation_local_port_error(', 'local mapper call'],
  ['&diagnostic_context,', 'retained context mapper argument'],
  ['&diagnostic_facts,', 'retained facts mapper argument'],
]) requireText(operation, value, label);
const operationIndexes = [
  operation.indexOf('let diagnostic_context = context.clone();'),
  operation.indexOf('checkout_payment_compensation_diagnostic_facts(&request)'),
  operation.indexOf('.compensate_checkout_payment(context, request)'),
  operation.indexOf('result.map_err(|error| {'),
  operation.indexOf('map_checkout_payment_compensation_local_port_error('),
];
if (!operationIndexes.every((value, index) => value >= 0 && (index === 0 || operationIndexes[index - 1] < value))) {
  failures.push('compensation wrapper must retain context/facts before unchanged delegation and map only returned errors');
}

const facts = between(
  wrapper,
  'fn checkout_payment_compensation_diagnostic_facts(',
  'fn payment_metadata_kind(',
  'safe compensation facts',
);
for (const [value, label] of [
  ['checkout_operation_id: request.checkout_operation_id', 'checkout operation id fact'],
  ['collection_id: request.collection_id', 'collection id fact'],
  ['reason_length: request.reason.as_ref().map(|value| value.chars().count())', 'reason length fact'],
  ['metadata_kind: payment_metadata_kind(&request.metadata)', 'metadata kind fact'],
  ['Value::Object(entries) => Some(entries.len())', 'metadata object entry count'],
  ['Value::Array(entries) => Some(entries.len())', 'metadata array entry count'],
]) requireText(facts, value, label);
for (const value of [
  'reason: request.reason.clone()',
  'metadata: request.metadata.clone()',
  'reason: request.reason',
  'metadata: request.metadata',
]) forbidText(facts, value, 'raw compensation payload retention');

const mapper = wrapper.slice(
  wrapper.indexOf('fn map_checkout_payment_compensation_local_port_error('),
);
requireText(
  mapper,
  'match (error.code.as_str(), error.message.as_str())',
  'exact code-and-message matching',
);
for (const [code, message, operationLabel] of [
  ['payment.checkout_compensation_identity_invalid', 'checkout operation and payment collection identity must be non-nil UUIDs', 'validate_compensation_identity'],
  ['payment.collection_not_found', 'payment collection was not found', 'load_collection'],
  ['payment.checkout_compensation_manual_reconciliation', 'payment checkout compensation requires manual reconciliation', 'require_manual_reconciliation'],
  ['payment.checkout_compensation_state_conflict', 'payment collection changed while compensation was being applied', 'apply_compensation_state'],
  ['payment.checkout_compensation_state_conflict', 'payment lifecycle conflicts with checkout compensation', 'apply_payment_lifecycle'],
  ['payment.checkout_compensation_provider_state_conflict', 'payment provider cancellation is in an unsupported state', 'validate_provider_journal_state'],
  ['payment.checkout_compensation_metadata_invalid', 'payment compensation metadata must be a JSON object', 'validate_provider_metadata'],
  ['payment.checkout_compensation_provider_identity_conflict', 'payment provider identity conflicts with the durable authorization', 'validate_provider_identity'],
  ['payment.checkout_compensation_encoding_failed', 'payment compensation request could not be encoded', 'encode_provider_cancel_request'],
  ['payment.database_unavailable', 'payment storage is temporarily unavailable', 'owner_storage'],
  ['payment.checkout_compensation_validation', 'payment compensation request is invalid', 'validate_owner_request'],
  ['payment.payment_not_found', 'payment was not found', 'load_payment'],
  ['payment.refund_not_found', 'refund was not found', 'load_refund'],
  ['payment.provider_unavailable', 'payment provider is temporarily unavailable', 'execute_provider_cancel'],
  ['payment.provider_rejected', 'payment provider rejected the requested operation', 'execute_provider_cancel'],
  ['payment.provider_invalid_response', 'payment provider response could not be applied safely', 'normalize_provider_result'],
  ['payment.provider_not_configured', 'payment provider is not configured', 'resolve_provider'],
]) {
  requireText(mapper, `"${code}"`, `${operationLabel} code`);
  requireText(mapper, `"${message}"`, `${operationLabel} message`);
  requireText(mapper, `"${operationLabel}"`, `${operationLabel} local operation`);
}
for (const [value, label] of [
  ['_ => return error,', 'unknown error pass-through'],
  ['"require_manual_reconciliation" | "validate_provider_journal_state"', 'integrity severity classification'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation', 'technical severity classification'],
  ['tracing::error!(', 'technical local event'],
  ['tracing::warn!(', 'ordinary local event'],
  ['error = ?error', 'complete delegated error'],
  ['owner = PAYMENT_OWNER', 'truthful owner'],
  ['operation = COMPENSATE_CHECKOUT_PAYMENT_OPERATION', 'exact public operation'],
  ['local_operation,', 'local operation field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['checkout_operation_id = %facts.checkout_operation_id', 'checkout operation context'],
  ['collection_id = ?facts.collection_id', 'collection context'],
  ['reason_length = ?facts.reason_length', 'reason length context'],
  ['metadata_kind = facts.metadata_kind', 'metadata kind context'],
  ['metadata_entry_count = ?facts.metadata_entry_count', 'metadata count context'],
  ['internal_code = %error.code', 'stable error code'],
  ['internal_message = %error.message', 'stable error message'],
  ['error_kind = ?error.kind', 'typed error kind'],
  ['retryable = error.retryable', 'retryability'],
  ['boundary = PAYMENT_COMPENSATION_BOUNDARY', 'owner boundary'],
  ['\n    error\n}', 'same delegated error return'],
]) requireText(mapper, value, label);
for (const value of [
  'payment.tenant_id_invalid',
  'payment.checkout_compensation_causation_invalid',
]) forbidText(mapper, value, 'pre-delegation context errors must pass through');
for (const value of [
  'reason =',
  'metadata =',
]) forbidText(mapper, value, 'raw compensation payload diagnostics');

for (const [value, label] of [
  ['pub trait CheckoutPaymentCompensationPort', 'legacy owner trait'],
  ['pub struct CheckoutPaymentCompensationRequest', 'legacy request DTO'],
  ['pub struct InProcessCheckoutPaymentCompensationPort', 'legacy direct owner path'],
  ['pub fn in_process_checkout_payment_compensation_port(', 'legacy module factory'],
  ['context.require_policy(PortCallPolicy::write())?;', 'legacy write policy'],
  ['context.require_write_semantics()?;', 'legacy write semantics'],
  ['parse_tenant_id(&context, owner_operation)?;', 'legacy tenant validation'],
  ['require_operation_context(&context, owner_operation, request.checkout_operation_id)?;', 'legacy causation validation'],
  ['format!("payment_collection:{}:cancel", collection.id)', 'canonical cancel idempotency key'],
  ['"operation": "cancel_payment_collection"', 'canonical provider metadata operation'],
  ['.execute_cancel(provider_id.as_str(), provider_request)', 'provider cancel execution'],
  ['.mark_provider_succeeded(', 'provider success checkpoint'],
  ['.cancel_local_collection(', 'local cancellation'],
  ['.mark_committed(outcome.operation_id)', 'provider commit checkpoint'],
  ['"payment.checkout_compensation_identity_invalid"', 'identity envelope source'],
  ['"payment.checkout_compensation_provider_state_conflict"', 'provider state envelope source'],
  ['"payment.checkout_compensation_metadata_invalid"', 'metadata envelope source'],
  ['"payment.checkout_compensation_provider_identity_conflict"', 'provider identity envelope source'],
  ['"payment.checkout_compensation_encoding_failed"', 'encoding envelope source'],
  ['"payment.checkout_compensation_manual_reconciliation"', 'manual reconciliation envelope source'],
]) requireText(owner, value, label);

for (const [value, label] of [
  ['use rustok_payment::{', 'commerce root payment imports'],
  ['CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,', 'commerce root compensation contracts'],
  ['InProcessCheckoutPaymentCompensationPort, PaymentCollectionStatusKind, PaymentProviderRegistry,', 'commerce root wrapper type'],
  ['in_process_checkout_payment_compensation_port,', 'commerce root wrapper factory'],
]) requireText(commerce, value, label);
forbidText(commerce, 'rustok_payment::checkout_compensation::', 'commerce direct legacy owner construction');

if (failures.length > 0) {
  console.error('Payment checkout compensation local-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Canonical payment compensation root construction retains delegated context and safe request facts for exact stable local outcomes without changing persistent owner semantics',
);
