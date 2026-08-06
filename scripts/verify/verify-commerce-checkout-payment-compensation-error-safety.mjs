#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const failures = [];
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

const paths = {
  services: 'crates/rustok-commerce/src/services/mod.rs',
  facade: 'crates/rustok-commerce/src/services/checkout_compensation_payment_safe.rs',
  legacy: 'crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs',
  evidence: 'crates/rustok-commerce/contracts/evidence/checkout-payment-compensation-error-safety-source-review.json',
  doc: 'crates/rustok-commerce/docs/checkout-payment-compensation-error-safety.md',
};

const services = read(paths.services);
const facade = read(paths.facade);
const legacy = read(paths.legacy);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);

for (const [source, value, label] of [
  [services, '#[path = "checkout_compensation_payment_safe.rs"]\nmod checkout_compensation;', 'mounted facade'],
  [facade, 'include!("checkout_compensation_owner_ports.rs");', 'retained source'],
  [facade, 'use super::rustok_payment_shim as rustok_payment;', 'payment shim alias'],
  [facade, 'use super::rustok_order_shim as rustok_order;', 'order shim alias'],
  [facade, 'CheckoutPaymentCompensationPort as CanonicalCheckoutPaymentCompensationPort', 'canonical payment API'],
]) requireText(source, value, label);

for (const marker of [
  'impl CheckoutPaymentCompensationPort for SanitizingPort',
  'let error_context = context.clone();',
  'let facts = BoundaryFacts::payment(&request);',
  '.compensate_checkout_payment(context, request)',
  'sanitize_payment(&error_context, facts, error)',
  '::rustok_payment::in_process_checkout_payment_compensation_port(db)',
  '::rustok_payment::InProcessCheckoutPaymentCompensationPort::with_provider_registry(',
  'rustok_payment_shim::wrap_checkout_payment_compensation_port(',
]) requireText(facade, marker, `${paths.facade}: payment composition`);
requireCount(
  facade,
  'wrap_checkout_payment_compensation_port(',
  4,
  'definition plus default, provider-registry, and custom payment wrapping',
);

for (const [kind, message] of [
  ['PortErrorKind::Validation', 'Checkout payment compensation request is invalid'],
  ['PortErrorKind::NotFound', 'Checkout payment compensation resource was not found'],
  ['PortErrorKind::Conflict', 'Checkout payment compensation conflicts with the current payment state'],
  ['PortErrorKind::Forbidden', 'Checkout payment compensation is not permitted'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'Checkout payment compensation service is temporarily unavailable'],
  ['PortErrorKind::InvariantViolation', 'Checkout payment compensation could not be completed safely'],
]) {
  requireText(facade, kind, `${kind} payment mapping`);
  requireText(facade, message, `${kind} payment message`);
}

for (const marker of [
  'error.code == PAYMENT_MANUAL_CODE',
  'Checkout payment compensation requires manual reconciliation',
  'kind: error.kind',
  'code: error.code',
  'retryable: error.retryable',
  'BoundaryFacts::payment(&request)',
  'tenant_id_shape = context_facts.tenant_id_shape',
  'correlation_id_shape = context_facts.correlation_id_shape',
  'subject_id_shape = facts.subject_id_shape',
  'payload_kind = facts.payload_kind',
  'owner_message_present',
  'owner_message_len',
  'boundary = facts.boundary',
  'formatter.write_str("redacted")',
]) requireText(facade, marker, `${paths.facade}: bounded payment contract`);

for (const forbidden of [
  'message: error.message',
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
]) forbidText(facade, forbidden, `${paths.facade}: raw payment payload`);

requireCount(
  facade,
  'if $owner != "rustok_payment" && $owner != "rustok_order"',
  2,
  'payment/order compatibility suppression',
);

for (const marker of [
  'CheckoutCompensationError::ManualReconciliation(error.message)',
  'message: error.message',
  'let message = compensation.to_string();',
  '.mark_compensation_retryable(',
  'self.compensate_payment(tenant_id, actor_id, operation)',
  'self.compensate_order(tenant_id, actor_id, operation)',
]) requireText(legacy, marker, `${paths.legacy}: unchanged retained flow`);

if (
  evidence.status !==
  'commerce_checkout_payment_compensation_error_safety_source_reviewed_unvalidated'
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);

for (const [key, expected] of Object.entries({
  mounted_payment_compensation_facade_active: true,
  combined_payment_order_facade_active: true,
  retained_compensation_business_logic_changed: false,
  default_payment_port_wrapped: true,
  provider_registry_payment_port_wrapped: true,
  custom_payment_port_wrapped: true,
  payment_owner_message_public: false,
  payment_owner_message_persisted: false,
  complete_payment_port_error_logged: false,
  raw_payment_context_values_logged: false,
  bounded_payment_context_shapes_logged: true,
  legacy_payment_diagnostic_suppressed: true,
  legacy_order_diagnostic_suppressed: true,
  inventory_cart_legacy_diagnostics_suppressed: false,
  order_compensation_mapper_changed: true,
  inventory_compensation_mapper_changed: false,
  cart_compensation_mapper_changed: false,
  broad_ecommerce_cleanup_closed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}

for (const key of [
  'tests_run', 'verifier_run', 'cargo_run', 'format_run',
  'payment_provider_calls_run', 'database_scenarios_run',
  'restart_scenarios_run', 'remote_port_scenarios_run',
  'workflow_checks_run', 'ci_run', 'compile_proven', 'runtime_proven',
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
  'combined facade',
  'owner message can no longer reach',
  'Order compensation is now also adapted',
  'Inventory and cart compensation mapper cleanup remains open',
  'No tests, Node verifiers, Cargo commands, formatting',
]) requireText(doc, marker, `${paths.doc}: truthful payment review`);

if (failures.length > 0) {
  console.error('Checkout payment compensation error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Mounted Commerce payment compensation preserves typed owner classification while using static persisted messages and bounded diagnostics; validation remains open',
);
