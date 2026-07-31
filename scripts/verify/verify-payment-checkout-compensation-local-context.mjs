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
const api = read('crates/rustok-payment/src/checkout_compensation_api.rs');
const wrapper = read('crates/rustok-payment/src/checkout_compensation_context.rs');
const owner = read('crates/rustok-payment/src/checkout_compensation.rs');
const commerce = read('crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs');
const doc = read('crates/rustok-payment/docs/checkout-compensation-local-context.md');
const paymentPlan = read('crates/rustok-payment/docs/implementation-plan.md');
const commercePlan = read('crates/rustok-commerce/docs/implementation-plan.md');
const evidence = JSON.parse(
  read(
    'crates/rustok-payment/contracts/evidence/checkout-compensation-wrapper-diagnostic-safety-source.json',
  ),
);
const review = JSON.parse(
  read(
    'crates/rustok-payment/contracts/evidence/checkout-compensation-wrapper-diagnostic-safety-source-review.json',
  ),
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

for (const [value, label] of [
  ['#[path = "checkout_compensation.rs"]\nmod checkout_compensation_persistent;', 'private persistent owner module'],
  ['#[path = "checkout_compensation_api.rs"]\npub mod checkout_compensation;', 'public compensation facade'],
  ['mod checkout_compensation_context;', 'private wrapper module'],
  ['pub use checkout_compensation::{', 'root facade export'],
  ['CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,', 'root contracts'],
  ['InProcessCheckoutPaymentCompensationPort, in_process_checkout_payment_compensation_port,', 'root wrapper type/factory'],
]) requireText(lib, value, label);
for (const value of [
  'pub mod checkout_compensation_persistent',
  'pub use checkout_compensation_persistent::',
  'pub use checkout_compensation_context::',
]) forbidText(lib, value, 'persistent owner public bypass');

for (const [value, label] of [
  ['pub use crate::checkout_compensation_context::{', 'module wrapper export'],
  ['InProcessCheckoutPaymentCompensationPort, in_process_checkout_payment_compensation_port,', 'module wrapper type/factory'],
  ['pub use crate::checkout_compensation_persistent::{', 'module contract export'],
  ['CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,', 'module contracts'],
]) requireText(api, value, label);
for (const value of [
  'PersistentCheckoutPaymentCompensationPort',
  'checkout_compensation_persistent::InProcessCheckoutPaymentCompensationPort',
  'checkout_compensation_persistent::in_process_checkout_payment_compensation_port',
]) forbidText(api, value, 'persistent implementation exposure');

for (const [value, label] of [
  ['use crate::checkout_compensation_persistent::{', 'private persistent import'],
  ['InProcessCheckoutPaymentCompensationPort as PersistentCheckoutPaymentCompensationPort', 'persistent alias'],
  ['inner: PersistentCheckoutPaymentCompensationPort', 'wrapper inner owner'],
  ['PersistentCheckoutPaymentCompensationPort::new(db)', 'default constructor delegation'],
  ['PersistentCheckoutPaymentCompensationPort::with_provider_registry(', 'registry constructor delegation'],
  ['pub fn in_process_checkout_payment_compensation_port(', 'canonical wrapper factory'],
  ['Arc::new(InProcessCheckoutPaymentCompensationPort::new(db))', 'factory wrapper construction'],
]) requireText(wrapper, value, label);

const operation = between(
  wrapper,
  'async fn compensate_checkout_payment(',
  'fn checkout_payment_compensation_context_facts(',
  'wrapper operation',
);
for (const [value, label] of [
  ['let diagnostic_context = context.clone();', 'context retention'],
  ['let diagnostic_facts = checkout_payment_compensation_diagnostic_facts(&request);', 'request-shape retention'],
  ['.inner\n            .compensate_checkout_payment(context, request)', 'persistent delegation'],
  ['result.map_err(|error| {', 'post-delegation mapping'],
  ['map_checkout_payment_compensation_local_port_error(', 'wrapper mapper'],
]) requireText(operation, value, label);
const order = [
  operation.indexOf('let diagnostic_context = context.clone();'),
  operation.indexOf('checkout_payment_compensation_diagnostic_facts(&request)'),
  operation.indexOf('.compensate_checkout_payment(context, request)'),
  operation.indexOf('result.map_err(|error| {'),
  operation.indexOf('map_checkout_payment_compensation_local_port_error('),
];
if (!order.every((value, index) => value >= 0 && (index === 0 || order[index - 1] < value))) {
  failures.push('wrapper must retain safe context/facts before unchanged delegation and map only returned errors');
}

for (const marker of [
  'struct CheckoutPaymentCompensationContextFacts',
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
  'collection_id_present: request.collection_id.is_some()',
  'collection_id_non_nil: request.collection_id.map(|value| !value.is_nil())',
  'reason_present: request.reason.is_some()',
  'reason_length: request.reason.as_ref().map(|value| value.chars().count())',
  'metadata_kind: payment_metadata_kind(&request.metadata)',
  'Value::Object(entries) => Some(entries.len())',
  'Value::Array(entries) => Some(entries.len())',
]) requireText(wrapper, marker, 'safe wrapper facts');

for (const marker of [
  'fn checkout_payment_compensation_local_operation(code: &str)',
  'match code {',
  'checkout_payment_compensation_local_operation(error.code.as_str())',
  '"port.idempotency_key_required" => Some("admit_write_idempotency")',
  '"port.deadline_required" => Some("admit_deadline")',
  '"payment.checkout_compensation_identity_invalid"',
  'Some("validate_compensation_identity")',
  '"payment.checkout_compensation_state_conflict" => Some("apply_compensation_state")',
  '"payment.checkout_compensation_provider_state_conflict"',
  'Some("validate_provider_journal_state")',
  '"payment.provider_unavailable" | "payment.provider_rejected"',
  'Some("execute_provider_cancel")',
  '"payment.provider_invalid_response" => Some("normalize_provider_result")',
  '_ => None,',
]) requireText(wrapper, marker, 'stable-code wrapper attribution');
for (const forbidden of [
  'error.message.as_str()',
  'match (error.code.as_str(), error.message.as_str())',
  'write port calls require a non-empty idempotency key',
  'payment storage is temporarily unavailable',
  'payment provider is temporarily unavailable',
]) forbidText(wrapper, forbidden, 'message-independent wrapper classifier');

for (const marker of [
  'let context_facts = checkout_payment_compensation_context_facts(context);',
  'correlation_id = %context.correlation_id',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'actor_id_length = context_facts.actor_id_length',
  'claim_count = context_facts.claim_count',
  'role_count = context_facts.role_count',
  'channel_present = context_facts.channel_present',
  'locale_length = context_facts.locale_length',
  'causation_id_present = context_facts.causation_id_present',
  'traceparent_present = context_facts.traceparent_present',
  'idempotency_key_present = context_facts.idempotency_key_present',
  'checkout_operation_id_non_nil = facts.checkout_operation_id_non_nil',
  'collection_id_present = facts.collection_id_present',
  'collection_id_non_nil = ?facts.collection_id_non_nil',
  'reason_present = facts.reason_present',
  'reason_length = ?facts.reason_length',
  'metadata_kind = facts.metadata_kind',
  'metadata_entry_count = ?facts.metadata_entry_count',
  'internal_code = %error.code',
  'internal_message = %error.message',
  'error_kind = ?error.kind',
  'retryable = error.retryable',
  'boundary = PAYMENT_COMPENSATION_BOUNDARY',
  '\n    error\n}',
]) requireText(wrapper, marker, 'safe wrapper mapper');

for (const forbidden of [
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'checkout_operation_id = %',
  'collection_id = ?',
  'reason =',
  'metadata =',
]) forbidText(wrapper, forbidden, 'raw wrapper diagnostics');

for (const [value, label] of [
  ['pub trait CheckoutPaymentCompensationPort', 'persistent trait'],
  ['pub struct CheckoutPaymentCompensationRequest', 'persistent request'],
  ['context.require_policy(PortCallPolicy::write())?;', 'persistent write policy'],
  ['context.require_write_semantics()?;', 'persistent write semantics'],
  ['parse_tenant_id(&context, owner_operation)?;', 'persistent tenant validation'],
  ['require_operation_context(&context, owner_operation, request.checkout_operation_id)?;', 'persistent causation validation'],
  ['format!("payment_collection:{}:cancel", collection.id)', 'canonical cancel key'],
  ['"operation": "cancel_payment_collection"', 'canonical cancel metadata'],
  ['.execute_cancel(provider_id.as_str(), provider_request)', 'provider cancel'],
  ['.mark_provider_succeeded(', 'provider success checkpoint'],
  ['.cancel_local_collection(', 'local cancellation'],
  ['.mark_committed(outcome.operation_id)', 'final commit checkpoint'],
]) requireText(owner, value, label);
for (const marker of [
  'tenant_id = %context.tenant_id',
  'operation_id = %operation.id',
  'checkout_operation_id = %checkout_operation_id',
]) requireText(owner, marker, 'persistent owner diagnostic cleanup remains open');

for (const [value, label] of [
  ['use rustok_payment::{', 'commerce root imports'],
  ['CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,', 'commerce contracts'],
  ['InProcessCheckoutPaymentCompensationPort, PaymentCollectionStatusKind, PaymentProviderRegistry,', 'commerce wrapper type'],
  ['in_process_checkout_payment_compensation_port,', 'commerce wrapper factory'],
]) requireText(commerce, value, label);
forbidText(commerce, 'rustok_payment::checkout_compensation_persistent::', 'commerce persistent bypass');

if (evidence.status !== 'payment_checkout_compensation_wrapper_diagnostic_safety_source_unvalidated') {
  failures.push(`unexpected evidence status: ${evidence.status}`);
}
if (
  review.status !==
  'payment_checkout_compensation_wrapper_diagnostic_safety_source_reviewed_unvalidated'
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
  persistent_owner_source_changed: false,
  provider_cancel_policy_changed: false,
  provider_journal_policy_changed: false,
  raw_tenant_id_logged_by_wrapper: false,
  raw_actor_id_logged_by_wrapper: false,
  raw_channel_logged_by_wrapper: false,
  raw_locale_logged_by_wrapper: false,
  raw_causation_id_logged_by_wrapper: false,
  raw_traceparent_logged_by_wrapper: false,
  raw_idempotency_key_logged_by_wrapper: false,
  raw_checkout_operation_id_logged_by_wrapper: false,
  raw_collection_id_logged_by_wrapper: false,
  raw_reason_logged_by_wrapper: false,
  raw_metadata_logged_by_wrapper: false,
  safe_context_shape_logged: true,
  safe_request_shape_logged: true,
  correlation_id_logged: true,
  original_port_error_private: true,
  persistent_owner_diagnostic_cleanup_complete: false,
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
  'The persistent owner in `checkout_compensation.rs` still contains raw tenant',
  'No FBA or FFA status is promoted from source',
]) requireText(doc, marker, 'compensation wrapper documentation');
requireText(
  paymentPlan,
  'Payment checkout compensation wrapper diagnostic safety:',
  'payment owner wrapper status',
);
requireText(
  paymentPlan,
  'its owner-local raw identifier diagnostics remain the next',
  'persistent owner nonclaim',
);
requireText(
  commercePlan,
  'Finish correlation-safe mapper cleanup for order, payment execution/compensation,',
  'broad ecommerce cleanup remains open',
);

if (failures.length > 0) {
  console.error('Payment checkout compensation wrapper diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Payment checkout compensation wrapper uses stable-code attribution and safe context/request shape while preserving persistent owner behavior and public PortError results; runtime evidence remains open',
);
