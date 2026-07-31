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
const wrapperEvidence = JSON.parse(
  read(
    'crates/rustok-payment/contracts/evidence/checkout-compensation-wrapper-diagnostic-safety-source.json',
  ),
);
const wrapperReview = JSON.parse(
  read(
    'crates/rustok-payment/contracts/evidence/checkout-compensation-wrapper-diagnostic-safety-source-review.json',
  ),
);
const ownerEvidence = JSON.parse(
  read(
    'crates/rustok-payment/contracts/evidence/checkout-compensation-owner-diagnostic-safety-source.json',
  ),
);
const ownerReview = JSON.parse(
  read(
    'crates/rustok-payment/contracts/evidence/checkout-compensation-owner-diagnostic-safety-source-review.json',
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

for (const [value, label] of [
  ['#[path = "checkout_compensation.rs"]\nmod checkout_compensation_persistent;', 'private owner module'],
  ['#[path = "checkout_compensation_api.rs"]\npub mod checkout_compensation;', 'public facade module'],
  ['mod checkout_compensation_context;', 'private wrapper module'],
  ['pub use checkout_compensation::{', 'root facade export'],
  ['CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,', 'root contracts'],
  ['InProcessCheckoutPaymentCompensationPort, in_process_checkout_payment_compensation_port,', 'root wrapper construction'],
]) requireText(lib, value, label);
for (const value of [
  'pub mod checkout_compensation_persistent',
  'pub use checkout_compensation_persistent::',
  'pub use checkout_compensation_context::',
]) forbidText(lib, value, 'public owner bypass');

for (const [value, label] of [
  ['pub use crate::checkout_compensation_context::{', 'module wrapper export'],
  ['InProcessCheckoutPaymentCompensationPort, in_process_checkout_payment_compensation_port,', 'module wrapper type/factory'],
  ['pub use crate::checkout_compensation_persistent::{', 'module owner contract export'],
  ['CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,', 'module contracts'],
]) requireText(api, value, label);
for (const value of [
  'PersistentCheckoutPaymentCompensationPort',
  'checkout_compensation_persistent::InProcessCheckoutPaymentCompensationPort',
  'checkout_compensation_persistent::in_process_checkout_payment_compensation_port',
]) forbidText(api, value, 'public persistent implementation exposure');

for (const [value, label] of [
  ['inner: PersistentCheckoutPaymentCompensationPort', 'wrapper inner owner'],
  ['PersistentCheckoutPaymentCompensationPort::new(db)', 'wrapper default constructor'],
  ['PersistentCheckoutPaymentCompensationPort::with_provider_registry(', 'wrapper registry constructor'],
  ['let diagnostic_context = context.clone();', 'wrapper context capture'],
  ['let diagnostic_facts = checkout_payment_compensation_diagnostic_facts(&request);', 'wrapper request facts'],
  ['.compensate_checkout_payment(context, request)', 'unchanged wrapper delegation'],
  ['checkout_payment_compensation_local_operation(error.code.as_str())', 'stable-code wrapper attribution'],
  ['tenant_id_length = context_facts.tenant_id_length', 'wrapper tenant shape'],
  ['checkout_operation_id_non_nil = facts.checkout_operation_id_non_nil', 'wrapper operation shape'],
  ['collection_id_present = facts.collection_id_present', 'wrapper collection shape'],
  ['boundary = PAYMENT_COMPENSATION_BOUNDARY', 'wrapper boundary'],
  ['\n    error\n}', 'same wrapper PortError return'],
]) requireText(wrapper, value, label);
for (const forbidden of [
  'match (error.code.as_str(), error.message.as_str())',
  'error.message.as_str()',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'checkout_operation_id = %facts.checkout_operation_id',
  'collection_id = ?facts.collection_id',
]) forbidText(wrapper, forbidden, 'wrapper diagnostic safety');

for (const marker of [
  'const PAYMENT_OWNER: &str = "rustok_payment";',
  'const PAYMENT_COMPENSATION_BOUNDARY: &str = "checkout_payment_compensation_port";',
  'struct CheckoutPaymentCompensationOwnerContextFacts',
  'fn checkout_payment_compensation_owner_context_facts(',
  'tenant_id_length: context.tenant_id.chars().count()',
  'actor_id_length: context.actor.id.chars().count()',
  'claim_count: context.claims.len()',
  'role_count: context.roles.len()',
  'channel_present: context.channel.is_some()',
  'locale_length: context.locale.chars().count()',
  'causation_id_present: context.causation_id.is_some()',
  'traceparent_present: context.traceparent.is_some()',
  'idempotency_key_present: context.idempotency_key.is_some()',
  'fn log_checkout_payment_compensation_owner_error<',
  'operation_id_present = operation_id.is_some()',
  'operation_id_non_nil = operation_id.map(|value| !value.is_nil())',
  'fn log_checkout_payment_compensation_context_warning(',
  'checkout_operation_id_non_nil = ?checkout_operation_id_non_nil',
  'causation_matches = ?causation_matches',
  'owner = PAYMENT_OWNER',
  'correlation_id = %context.correlation_id',
  'boundary = PAYMENT_COMPENSATION_BOUNDARY',
]) requireText(owner, marker, 'owner safe-context source');

for (const [operation, code] of [
  ['commit_recovered_cancel_checkpoint', 'payment.checkout_compensation_commit_checkpoint_failed'],
  ['encode_provider_cancel_request', 'payment.checkout_compensation_encoding_failed'],
  ['checkpoint_provider_cancel_failure', 'payment.checkout_compensation_provider_failure_checkpoint_failed'],
  ['encode_provider_cancel_result', 'payment.checkout_compensation_provider_result_encoding_failed'],
  ['checkpoint_provider_cancel_success', 'payment.checkout_compensation_provider_checkpoint_failed'],
  ['commit_provider_cancel_checkpoint', 'payment.checkout_compensation_commit_checkpoint_failed'],
  ['decode_provider_cancel_checkpoint', 'payment.provider_invalid_response'],
  ['validate_causation_context', 'payment.checkout_compensation_causation_invalid'],
  ['parse_tenant_context', 'payment.tenant_id_invalid'],
  ['map_payment_owner_error', 'payment checkout compensation owner operation failed'],
]) {
  requireText(owner, `"${operation}"`, `owner local operation ${operation}`);
  requireText(owner, `"${code}"`, `owner stable code/event ${code}`);
}

for (const forbidden of [
  'tenant_id = %context.tenant_id',
  'internal_tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'checkout_operation_id = %checkout_operation_id',
  'collection_id = %',
  'collection_id = ?',
  'operation_id = %operation.id',
  'operation_id = %outcome.operation_id',
  'operation_id = %',
  'provider_id = %',
  'reason = ?',
  'reason = %',
  'metadata = ?',
  'metadata = %',
  'request_amount =',
]) forbidText(owner, forbidden, 'owner raw diagnostic values');

requireCount(
  owner,
  'log_checkout_payment_compensation_owner_error(',
  9,
  'nine owner error calls',
);

for (const [value, label] of [
  ['pub trait CheckoutPaymentCompensationPort', 'owner trait'],
  ['pub struct CheckoutPaymentCompensationRequest', 'owner request DTO'],
  ['context.require_policy(PortCallPolicy::write())?;', 'write policy'],
  ['context.require_write_semantics()?;', 'write semantics'],
  ['parse_tenant_id(&context, owner_operation)?;', 'tenant admission order'],
  ['require_operation_context(&context, owner_operation, request.checkout_operation_id)?;', 'causation admission order'],
  ['let Some(collection_id) = request.collection_id else {\n            return Ok(None);', 'optional no-op'],
  ['PaymentCollectionStatusKind::Cancelled', 'cancelled lifecycle'],
  ['PaymentCollectionStatusKind::Captured', 'captured reconciliation'],
  ['PaymentCollectionStatusKind::Pending | PaymentCollectionStatusKind::Authorized', 'effectable lifecycle'],
  ['PaymentCollectionStatusKind::Unknown', 'unknown lifecycle'],
  ['format!("payment_collection:{}:cancel", collection.id)', 'canonical cancel key'],
  ['"operation": "cancel_payment_collection"', 'canonical cancel metadata'],
  ['.execute_cancel(provider_id.as_str(), provider_request)', 'provider cancel'],
  ['.begin(BeginProviderOperation {', 'journal begin'],
  ['.claim_execution(operation.id)', 'journal claim'],
  ['persisted_cancel_result(context, owner_operation, &operation)', 'journal replay adoption'],
  ['.mark_provider_succeeded(', 'provider success checkpoint'],
  ['.mark_reconciliation_required(operation.id, code)', 'reconciliation checkpoint'],
  ['.mark_provider_error(operation.id, code)', 'provider error checkpoint'],
  ['.cancel_local_collection(', 'local cancellation'],
  ['.mark_committed(outcome.operation_id)', 'final commit checkpoint'],
  ['"payment.checkout_compensation_manual_reconciliation"', 'manual reconciliation code'],
  ['"payment checkout compensation requires manual reconciliation"', 'manual reconciliation message'],
  ['PaymentError::Database(_) => PortError::unavailable(', 'database public mapping'],
  ['PaymentError::Validation(_) => PortError::validation(', 'validation public mapping'],
  ['PaymentError::PaymentCollectionNotFound(_) => PortError::not_found(', 'collection public mapping'],
  ['PaymentError::InvalidTransition { .. } => PortError::conflict(', 'lifecycle public mapping'],
  ['PaymentError::ProviderUnavailable { .. } => PortError::unavailable(', 'provider unavailable mapping'],
  ['PaymentError::ProviderRejected { .. } => PortError::conflict(', 'provider rejection mapping'],
  ['PaymentError::ProviderInvalidResponse { .. } => PortError::invariant_violation(', 'provider invalid response mapping'],
  ['PaymentError::ProviderConfiguration { .. } => PortError::invariant_violation(', 'provider configuration mapping'],
]) requireText(owner, value, label);

for (const [value, label] of [
  ['use rustok_payment::{', 'commerce root imports'],
  ['CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,', 'commerce contracts'],
  ['InProcessCheckoutPaymentCompensationPort, PaymentCollectionStatusKind, PaymentProviderRegistry,', 'commerce wrapper type'],
  ['in_process_checkout_payment_compensation_port,', 'commerce wrapper factory'],
]) requireText(commerce, value, label);
forbidText(
  commerce,
  'rustok_payment::checkout_compensation_persistent::',
  'commerce persistent bypass',
);

if (
  wrapperEvidence.status !==
  'payment_checkout_compensation_wrapper_diagnostic_safety_source_unvalidated'
) failures.push(`unexpected wrapper evidence status: ${wrapperEvidence.status}`);
if (
  wrapperReview.status !==
  'payment_checkout_compensation_wrapper_diagnostic_safety_source_reviewed_unvalidated'
) failures.push(`unexpected wrapper review status: ${wrapperReview.status}`);
if (
  ownerEvidence.status !==
  'payment_checkout_compensation_owner_diagnostic_safety_source_unvalidated'
) failures.push(`unexpected owner evidence status: ${ownerEvidence.status}`);
if (
  ownerReview.status !==
  'payment_checkout_compensation_owner_diagnostic_safety_source_reviewed_unvalidated'
) failures.push(`unexpected owner review status: ${ownerReview.status}`);

for (const [key, expected] of Object.entries({
  owner_context_shape_logged: true,
  correlation_id_logged: true,
  original_internal_error_private: true,
  stable_diagnostic_codes_preserved: true,
  raw_tenant_id_logged_by_owner: false,
  raw_actor_id_logged_by_owner: false,
  raw_channel_logged_by_owner: false,
  raw_locale_logged_by_owner: false,
  raw_causation_id_logged_by_owner: false,
  raw_traceparent_logged_by_owner: false,
  raw_idempotency_key_logged_by_owner: false,
  raw_checkout_operation_id_logged_by_owner: false,
  raw_collection_id_logged_by_owner: false,
  raw_provider_operation_id_logged_by_owner: false,
  raw_reason_logged_by_owner: false,
  raw_metadata_logged_by_owner: false,
  operation_id_shape_logged: true,
  causation_match_fact_logged: true,
  public_wrapper_changed: false,
  request_response_dto_changed: false,
  public_code_changed: false,
  public_message_changed: false,
  public_kind_changed: false,
  public_retryability_changed: false,
  provider_cancel_policy_changed: false,
  provider_request_payload_changed: false,
  provider_journal_policy_changed: false,
  provider_replay_policy_changed: false,
  local_cancellation_policy_changed: false,
  manual_reconciliation_envelope_changed: false,
  payment_error_mapping_changed: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (ownerEvidence.source_contract?.[key] !== expected) {
    failures.push(`owner evidence source_contract.${key} must be ${expected}`);
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
  'production_behavior_proven',
]) {
  if (ownerEvidence.validation?.[key] !== false) {
    failures.push(`owner evidence validation.${key} must remain false`);
  }
}

for (const marker of [
  'Status: **source-ready / unvalidated**',
  'Human-readable `PortError.message` is not used as control flow.',
  'The private owner now uses one shared safe-context model',
  'The owner no longer writes raw tenant',
  'Compile, provider replay, process-exit, restart',
  'No FBA',
]) requireText(doc, marker, 'compensation documentation');
requireText(
  paymentPlan,
  'Payment checkout compensation diagnostic safety: `source_ready_unvalidated`',
  'payment owner status',
);
requireText(
  paymentPlan,
  'The private persistent owner now applies the same safe-context policy',
  'owner diagnostic status detail',
);
requireText(
  commercePlan,
  'Finish correlation-safe mapper cleanup for order, payment execution/compensation,',
  'broad ecommerce cleanup remains open',
);

if (failures.length > 0) {
  console.error('Payment checkout compensation diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Payment checkout compensation wrapper and persistent owner diagnostics use stable-code/safe-shape attribution while preserving public PortError and provider state-machine behavior; runtime evidence remains open',
);
