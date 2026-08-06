#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const paths = {
  services: 'crates/rustok-commerce/src/services/mod.rs',
  facade:
    'crates/rustok-commerce/src/services/checkout_compensation_payment_safe.rs',
  legacy:
    'crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs',
  evidence:
    'crates/rustok-commerce/contracts/evidence/checkout-payment-compensation-error-safety-source-review.json',
  doc: 'crates/rustok-commerce/docs/checkout-payment-compensation-error-safety.md',
};

const services = read(paths.services);
const facade = read(paths.facade);
const legacy = read(paths.legacy);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (source, value, expected, label) => {
  const count = source.split(value).length - 1;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
};

for (const [source, value, label] of [
  [
    services,
    '#[path = "checkout_compensation_payment_safe.rs"]\nmod checkout_compensation;',
    'mounted payment-safe compensation facade',
  ],
  [
    facade,
    'include!("checkout_compensation_owner_ports.rs");',
    'private retained compensation source',
  ],
  [
    facade,
    'use super::rustok_payment_shim as rustok_payment;',
    'legacy payment shim alias',
  ],
  [facade, 'use super::tracing_shim as tracing;', 'legacy tracing shim alias'],
  [
    facade,
    'CheckoutPaymentCompensationPort as CanonicalCheckoutPaymentCompensationPort',
    'canonical custom-port API',
  ],
]) requireText(source, value, label);

for (const marker of [
  'struct SanitizingCheckoutPaymentCompensationPort',
  'impl CheckoutPaymentCompensationPort for SanitizingCheckoutPaymentCompensationPort',
  'let error_context = context.clone();',
  'let request_facts = PaymentCompensationRequestFacts::from(&request);',
  '.compensate_checkout_payment(context, request)',
  'sanitize_payment_compensation_error(&error_context, request_facts, error)',
  '::rustok_payment::in_process_checkout_payment_compensation_port(db)',
  '::rustok_payment::InProcessCheckoutPaymentCompensationPort::with_provider_registry(',
  'rustok_payment_shim::wrap_checkout_payment_compensation_port(',
]) requireText(facade, marker, `${paths.facade}: complete payment composition`);
requireCount(
  facade,
  'wrap_checkout_payment_compensation_port(',
  4,
  'wrapper definition plus default, provider-registry, and custom composition',
);

for (const [kind, message] of [
  ['PortErrorKind::Validation', 'Checkout payment compensation request is invalid'],
  ['PortErrorKind::NotFound', 'Checkout payment compensation resource was not found'],
  [
    'PortErrorKind::Conflict',
    'Checkout payment compensation conflicts with the current payment state',
  ],
  ['PortErrorKind::Forbidden', 'Checkout payment compensation is not permitted'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout',
    'Checkout payment compensation service is temporarily unavailable',
  ],
  [
    'PortErrorKind::InvariantViolation',
    'Checkout payment compensation could not be completed safely',
  ],
]) {
  requireText(facade, kind, `${kind} mapping`);
  requireText(facade, message, `${kind} static message`);
}
for (const marker of [
  'error.code == PAYMENT_MANUAL_RECONCILIATION_CODE',
  'Checkout payment compensation requires manual reconciliation',
  'kind: error.kind',
  'code: error.code',
  'retryable: error.retryable',
]) requireText(facade, marker, `${paths.facade}: stable payment error envelope`);
for (const forbidden of [
  'message: error.message',
  'PortError::new(',
  'error.to_string()',
]) forbidText(facade, forbidden, `${paths.facade}: raw or owner-rewritten message`);

for (const marker of [
  'struct PaymentCompensationContextFacts',
  'struct PaymentCompensationRequestFacts',
  'struct PaymentCompensationDiagnosticError',
  'formatter.write_str("redacted")',
  'tenant_id_shape = context_facts.tenant_id_shape',
  'actor_kind = context_facts.actor_kind',
  'actor_id_shape = context_facts.actor_id_shape',
  'claim_count = context_facts.claim_count',
  'role_count = context_facts.role_count',
  'channel_shape = context_facts.channel_shape',
  'locale_shape = context_facts.locale_shape',
  'correlation_id_shape = context_facts.correlation_id_shape',
  'causation_id_shape = context_facts.causation_id_shape',
  'traceparent_shape = context_facts.traceparent_shape',
  'idempotency_key_shape = context_facts.idempotency_key_shape',
  'checkout_operation_id_non_nil = request_facts.checkout_operation_id_non_nil',
  'collection_id_shape = request_facts.collection_id_shape',
  'reason_shape = request_facts.reason_shape',
  'reason_len = ?request_facts.reason_len',
  'metadata_kind = request_facts.metadata_kind',
  'metadata_entry_count = ?request_facts.metadata_entry_count',
  'owner_code = %error.code',
  'owner_message_present',
  'owner_message_len',
  'owner_kind = ?error.kind',
  'owner_retryable = error.retryable',
  'boundary = PAYMENT_COMPENSATION_ADAPTER_BOUNDARY',
  'tracing::error!(',
  'tracing::warn!(',
]) requireText(facade, marker, `${paths.facade}: bounded diagnostics`);
for (const forbidden of [
  'error = ?error',
  'error = %error',
  'internal_message',
  '%error.message',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'correlation_id = %context.correlation_id',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'reason = ?',
  'metadata = ?',
]) forbidText(facade, forbidden, `${paths.facade}: raw diagnostic payload`);

for (const marker of [
  '(error = ?$error:expr, owner = $owner:expr, $($rest:tt)*)',
  'if $owner != "rustok_payment"',
  '::tracing::error!(error = ?$error, owner = $owner, $($rest)*);',
  '::tracing::warn!(error = ?$error, owner = $owner, $($rest)*);',
]) requireText(facade, marker, `${paths.facade}: payment-only legacy log suppression`);
requireCount(
  facade,
  'if $owner != "rustok_payment"',
  2,
  'error and warning payment-only suppression',
);

for (const marker of [
  '.compensate_checkout_payment(',
  'owner_boundary_error(',
  'PAYMENT_COMPENSATION_OWNER',
  'PAYMENT_COMPENSATION_OPERATION',
  'CheckoutCompensationError::ManualReconciliation(error.message)',
  'message: error.message',
  'let message = compensation.to_string();',
  '.mark_compensation_retryable(',
  'self.compensate_payment(tenant_id, actor_id, operation)',
  'self.compensate_order(tenant_id, actor_id, operation)',
  'self.release_remaining_reservations(tenant_id, operation)',
  'self.release_cart(tenant_id, operation).await?;',
]) requireText(legacy, marker, `${paths.legacy}: retained behavior`);
for (const owner of [
  'ORDER_COMPENSATION_OWNER',
  'INVENTORY_COMPENSATION_OWNER',
  'CART_COMPENSATION_OWNER',
]) requireText(legacy, owner, `${paths.legacy}: ${owner} remains unchanged`);

if (
  evidence.status !==
  'commerce_checkout_payment_compensation_error_safety_source_reviewed_unvalidated'
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  mounted_payment_compensation_facade_active: true,
  retained_compensation_source_private: true,
  retained_compensation_business_logic_changed: false,
  canonical_payment_port_request_changed: false,
  canonical_payment_port_response_changed: false,
  payment_owner_delegation_changed: false,
  default_payment_port_wrapped: true,
  provider_registry_payment_port_wrapped: true,
  custom_payment_port_wrapped: true,
  payment_owner_kind_preserved: true,
  payment_owner_code_preserved: true,
  payment_owner_retryability_preserved: true,
  payment_owner_message_public: false,
  payment_owner_message_persisted: false,
  payment_manual_reconciliation_message_static: true,
  complete_payment_port_error_logged: false,
  payment_port_error_message_text_logged: false,
  raw_payment_context_values_logged: false,
  raw_payment_request_values_logged: false,
  bounded_payment_context_shapes_logged: true,
  bounded_payment_request_shapes_logged: true,
  payment_owner_message_presence_logged: true,
  payment_owner_message_length_logged: true,
  payment_error_severity_classification_preserved: true,
  legacy_payment_diagnostic_suppressed: true,
  non_payment_legacy_diagnostics_suppressed: false,
  compensation_ordering_changed: false,
  operation_journal_flow_changed: false,
  order_compensation_mapper_changed: false,
  inventory_compensation_mapper_changed: false,
  cart_compensation_mapper_changed: false,
  non_payment_compensation_cleanup_closed: false,
  broad_ecommerce_cleanup_closed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  'tests_run',
  'verifier_run',
  'cargo_run',
  'format_run',
  'payment_provider_calls_run',
  'database_scenarios_run',
  'restart_scenarios_run',
  'remote_port_scenarios_run',
  'workflow_checks_run',
  'ci_run',
  'compile_proven',
  'runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push(`${paths.evidence}: execution must remain empty`);
}

for (const marker of [
  'Status: **source-reviewed / unvalidated**',
  'The facade suppresses the retained compatibility diagnostic only when its truthful owner is `rustok_payment`',
  'the original payment-owner message can no longer reach `mark_compensation_retryable`',
  'Order, inventory, and cart compensation boundaries are intentionally outside this slice',
  'Tests, Node verifiers, Cargo commands, formatting commands',
]) requireText(doc, marker, `${paths.doc}: truthful source review`);

if (failures.length > 0) {
  console.error('Checkout payment compensation error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Mounted Commerce payment compensation adapts every payment port to static persisted messages and bounded diagnostics; runtime validation and non-payment mapper cleanup remain open',
);
