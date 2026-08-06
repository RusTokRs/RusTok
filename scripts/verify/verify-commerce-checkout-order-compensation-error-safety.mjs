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
  owner: 'crates/rustok-order/src/checkout_compensation.rs',
  evidence: 'crates/rustok-commerce/contracts/evidence/checkout-order-compensation-error-safety-source-review.json',
  doc: 'crates/rustok-commerce/docs/checkout-order-compensation-error-safety.md',
};

const services = read(paths.services);
const facade = read(paths.facade);
const legacy = read(paths.legacy);
const owner = read(paths.owner);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);

for (const [source, value, label] of [
  [services, '#[path = "checkout_compensation_payment_safe.rs"]\nmod checkout_compensation;', 'mounted facade'],
  [facade, 'include!("checkout_compensation_owner_ports.rs");', 'retained source'],
  [facade, 'use super::rustok_order_shim as rustok_order;', 'order shim alias'],
  [facade, 'use super::rustok_inventory_shim as rustok_inventory;', 'inventory shim alias'],
  [facade, 'CheckoutOrderCompensationPort as CanonicalCheckoutOrderCompensationPort', 'canonical order API'],
]) requireText(source, value, label);

for (const marker of [
  'impl CheckoutOrderCompensationPort for SanitizingPort',
  'let facts = BoundaryFacts::order(&request);',
  '.compensate_checkout_order(context, request)',
  'sanitize(&error_context, facts, error)',
  '::rustok_order::in_process_checkout_order_compensation_port(db, event_bus)',
  '::rustok_order::InProcessCheckoutOrderCompensationPort::with_identity_port(',
  'rustok_order_shim::wrap_checkout_order_compensation_port(order_compensation_port)',
]) requireText(facade, marker, `${paths.facade}: order composition`);
requireCount(
  facade,
  'wrap_checkout_order_compensation_port(',
  4,
  'definition plus default, identity-aware, and custom order wrapping',
);

for (const message of [
  'Checkout order compensation request is invalid',
  'Checkout order compensation resource was not found',
  'Checkout order compensation conflicts with the current order state',
  'Checkout order compensation is not permitted',
  'Checkout order compensation service is temporarily unavailable',
  'Checkout order compensation could not be completed safely',
  'Checkout order compensation requires manual reconciliation',
]) requireText(facade, message, `${paths.facade}: static order message`);

for (const marker of [
  'family: MessageFamily::Order',
  'operation_id_shape: uuid_shape(request.checkout_operation_id)',
  'primary_id_shape: uuid_shape(request.cart_id)',
  'secondary_id_shape: optional_uuid_shape(request.expected_order_id)',
  'opaque_text_shape: optional_text_shape(request.reason.as_deref())',
  'tenant_id_shape = context_facts.tenant_id_shape',
  'operation_id_shape = facts.operation_id_shape',
  'secondary_id_shape = facts.secondary_id_shape',
  'opaque_text_shape = facts.opaque_text_shape',
  'owner_code = %error.code',
  'owner_message_present',
  'owner_message_len',
  'owner_kind = ?error.kind',
  'owner_retryable = error.retryable',
  'formatter.write_str("redacted")',
  'message: message.to_string()',
]) requireText(facade, marker, `${paths.facade}: bounded order contract`);

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
  'expected_order_id = ?',
  'reason = ?',
]) forbidText(facade, forbidden, `${paths.facade}: raw order payload`);

requireCount(
  facade,
  '&& $owner != "rustok_inventory"',
  2,
  'payment/order/inventory compatibility suppression',
);

for (const marker of [
  'CheckoutOrderCompensationPort',
  'compensate_checkout_order(',
  'require_policy(PortCallPolicy::write())?',
  'require_write_semantics()?',
  'OrderService::new(',
]) requireText(owner, marker, `${paths.owner}: canonical owner behavior`);

for (const marker of [
  'owner_boundary_error(',
  'ORDER_COMPENSATION_OWNER',
  'ORDER_COMPENSATION_OPERATION',
  'CheckoutCompensationError::ManualReconciliation(error.message)',
  'message: error.message',
  'let message = compensation.to_string();',
  '.mark_compensation_retryable(',
  'self.compensate_order(tenant_id, actor_id, operation)',
  'self.release_remaining_reservations(tenant_id, operation)',
]) requireText(legacy, marker, `${paths.legacy}: retained order flow`);

if (
  evidence.status !==
  'commerce_checkout_order_compensation_error_safety_source_reviewed_unvalidated'
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);

for (const [key, expected] of Object.entries({
  mounted_combined_payment_order_inventory_facade_active: true,
  retained_compensation_business_logic_changed: false,
  default_order_port_wrapped: true,
  identity_order_port_wrapped: true,
  custom_order_port_wrapped: true,
  order_owner_kind_preserved: true,
  order_owner_code_preserved: true,
  order_owner_retryability_preserved: true,
  order_owner_message_public: false,
  order_owner_message_persisted: false,
  complete_order_port_error_logged: false,
  raw_order_context_values_logged: false,
  bounded_order_context_shapes_logged: true,
  legacy_order_diagnostic_suppressed: true,
  legacy_payment_diagnostic_suppressed: true,
  legacy_inventory_diagnostic_suppressed: true,
  legacy_cart_diagnostic_suppressed: false,
  inventory_compensation_mapper_changed: true,
  cart_compensation_mapper_changed: false,
  remaining_compensation_cleanup_closed: false,
  broad_ecommerce_cleanup_closed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}

for (const key of [
  'tests_run', 'verifier_run', 'cargo_run', 'format_run',
  'order_scenarios_run', 'database_scenarios_run',
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
  'combined payment/order/inventory facade',
  'Owner text cannot',
  'Inventory compensation is now also adapted',
  'Cart compensation consumer mapping',
  'No tests, Node verifiers, Cargo commands, formatting',
]) requireText(doc, marker, `${paths.doc}: truthful order review`);

if (failures.length > 0) {
  console.error('Checkout order compensation error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Mounted Commerce order compensation preserves typed owner classification while using static persisted messages and bounded diagnostics; validation and cart cleanup remain open',
);
