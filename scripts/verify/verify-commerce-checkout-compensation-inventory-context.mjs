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
  ownerContract: 'crates/rustok-inventory/src/ports.rs',
  ownerWrapper: 'crates/rustok-inventory/src/reservation_owner_context.rs',
  evidence:
    'crates/rustok-commerce/contracts/evidence/checkout-inventory-compensation-error-safety-source-review.json',
  doc: 'crates/rustok-commerce/docs/checkout-compensation-inventory-context.md',
};

const services = read(paths.services);
const facade = read(paths.facade);
const legacy = read(paths.legacy);
const ownerContract = read(paths.ownerContract);
const ownerWrapper = read(paths.ownerWrapper);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);

for (const [source, value, label] of [
  [services, '#[path = "checkout_compensation_payment_safe.rs"]\nmod checkout_compensation;', 'mounted facade'],
  [facade, 'include!("checkout_compensation_owner_ports.rs");', 'retained source'],
  [facade, 'use super::rustok_inventory_shim as rustok_inventory;', 'inventory shim alias'],
  [facade, 'InventoryReservationIdentityPort as CanonicalInventoryReservationIdentityPort', 'canonical inventory API'],
]) requireText(source, value, label);

for (const marker of [
  'impl InventoryReservationIdentityPort for SanitizingPort',
  'let facts = BoundaryFacts::inventory(&request);',
  '.release_inventory_by_identity(context, request)',
  'sanitize(&error_context, facts, error)',
  'rustok_inventory_shim::wrap_inventory_reservation_identity_port(',
  'reservation_port: Arc<dyn CanonicalInventoryReservationIdentityPort>',
]) requireText(facade, marker, `${paths.facade}: inventory composition`);
requireCount(
  facade,
  'wrap_inventory_reservation_identity_port(',
  2,
  'definition plus constructor inventory wrapping',
);

for (const message of [
  'Checkout inventory compensation request is invalid',
  'Checkout inventory compensation resource was not found',
  'Checkout inventory compensation conflicts with the current inventory state',
  'Checkout inventory compensation is not permitted',
  'Checkout inventory compensation service is temporarily unavailable',
  'Checkout inventory compensation could not be completed safely',
]) requireText(facade, message, `${paths.facade}: static inventory message`);

for (const marker of [
  'family: MessageFamily::Inventory',
  'owner: "rustok_inventory"',
  'operation: "release_inventory_by_identity"',
  'stage: "release_inventory"',
  'boundary: "commerce_checkout_inventory_compensation_adapter"',
  'primary_id_shape: uuid_shape(request.reservation_id)',
  'opaque_text_shape: text_shape(request.external_id.as_str())',
  'opaque_text_len: Some(request.external_id.chars().count())',
  'tenant_id_shape = context_facts.tenant_id_shape',
  'correlation_id_shape = context_facts.correlation_id_shape',
  'primary_id_shape = facts.primary_id_shape',
  'opaque_text_shape = facts.opaque_text_shape',
  'opaque_text_len = ?facts.opaque_text_len',
  'owner_code = %error.code',
  'owner_message_present',
  'owner_message_len',
  'owner_kind = ?error.kind',
  'owner_retryable = error.retryable',
  'formatter.write_str("redacted")',
  'kind: error.kind',
  'code: error.code',
  'message: message.to_string()',
  'retryable: error.retryable',
]) requireText(facade, marker, `${paths.facade}: bounded inventory contract`);

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
  'external_id = %',
  'external_id = ?',
]) forbidText(facade, forbidden, `${paths.facade}: raw inventory payload`);

requireCount(
  facade,
  '&& $owner != "rustok_inventory"',
  2,
  'technical and warning inventory compatibility suppression',
);

for (const marker of [
  'release_inventory_by_identity(',
  'InventoryIdentityReservationReleaseRequest {',
  'reservation_id: reservation.reservation_id',
  'external_id: reservation.external_id.clone()',
  'released.reservation_id != reservation.reservation_id',
  'released.external_id != reservation.external_id',
  'released.variant_id != reservation.variant_id',
  '.mark_released(tenant_id, reservation.reservation_id)',
  'let message = compensation.to_string();',
  '.mark_compensation_retryable(',
]) requireText(legacy, marker, `${paths.legacy}: retained inventory flow`);

for (const marker of [
  'pub trait InventoryReservationIdentityPort',
  'async fn release_inventory_by_identity(',
  'InventoryIdentityReservationReleaseRequest',
  'InventoryIdentityReservationReleaseSnapshot',
]) requireText(ownerContract, marker, `${paths.ownerContract}: canonical owner contract`);

for (const marker of [
  'impl InventoryReservationIdentityPort for PersistentInventoryReservationIdentityPort',
  'require_inventory_reservation_write_admission(&context, RELEASE_OPERATION)?;',
  'map_inventory_reservation_identity_local_port_error(',
  'error_message_present = !error.message.is_empty()',
  'error_message_length = error.message.chars().count()',
]) requireText(ownerWrapper, marker, `${paths.ownerWrapper}: bounded owner wrapper`);

if (
  evidence.status !==
  'commerce_checkout_inventory_compensation_error_safety_source_reviewed_unvalidated'
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);

for (const [key, expected] of Object.entries({
  mounted_combined_payment_order_inventory_facade_active: true,
  retained_compensation_business_logic_changed: false,
  constructor_inventory_port_wrapped: true,
  inventory_owner_kind_preserved: true,
  inventory_owner_code_preserved: true,
  inventory_owner_retryability_preserved: true,
  inventory_owner_message_public: false,
  inventory_owner_message_persisted: false,
  complete_inventory_port_error_logged: false,
  raw_inventory_context_values_logged: false,
  raw_inventory_request_values_logged: false,
  bounded_inventory_context_shapes_logged: true,
  bounded_inventory_request_shapes_logged: true,
  legacy_payment_diagnostic_suppressed: true,
  legacy_order_diagnostic_suppressed: true,
  legacy_inventory_diagnostic_suppressed: true,
  legacy_cart_diagnostic_suppressed: false,
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
  'inventory_scenarios_run', 'database_scenarios_run',
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
  'prevents the owner message from reaching',
  'leaves the retained cart diagnostic active',
  'Cart snapshot and cart release consumer mapping remain open',
  'No tests, Node verifiers, Cargo commands, formatting',
]) requireText(doc, marker, `${paths.doc}: truthful inventory review`);

if (failures.length > 0) {
  console.error('Checkout inventory compensation error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Mounted Commerce inventory compensation preserves typed owner classification while using static persisted messages and bounded diagnostics; cart and runtime validation remain open',
);
