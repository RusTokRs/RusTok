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

const services = read('crates/rustok-commerce/src/services/mod.rs');
const facade = read('crates/rustok-commerce/src/services/checkout_compensation_error_safe.rs');
const retained = read('crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs');
const ownerContract = read('crates/rustok-inventory/src/ports.rs');
const ownerWrapper = read('crates/rustok-inventory/src/reservation_owner_context.rs');
const evidence = JSON.parse(read(
  'crates/rustok-commerce/contracts/evidence/checkout-inventory-compensation-error-safety-source-review.json',
));
const doc = read('crates/rustok-commerce/docs/checkout-compensation-inventory-context.md');

requireText(services, '#[path = "checkout_compensation_error_safe.rs"]\nmod checkout_compensation;', 'mounted facade');
for (const marker of [
  'use super::rustok_inventory_shim as rustok_inventory;',
  'InventoryReservationIdentityPort as CanonicalInventoryReservationIdentityPort',
  'BoundaryFacts::inventory(&request)',
  'wrap_inventory_reservation_identity_port(',
  'release_inventory_by_identity(context, request)',
  '"Checkout inventory compensation request is invalid"',
  '"Checkout inventory compensation resource was not found"',
  '"Checkout inventory compensation conflicts with the current inventory state"',
  '"Checkout inventory compensation is not permitted"',
  '"Checkout inventory compensation service is temporarily unavailable"',
  '"Checkout inventory compensation could not be completed safely"',
  'kind: error.kind',
  'code: error.code',
  'retryable: error.retryable',
]) requireText(facade, marker, 'inventory adapter');

for (const marker of [
  'error = ?error',
  'internal_message = %error.message',
  'correlation_id = %context.correlation_id',
]) forbidText(facade, marker, 'raw mounted inventory diagnostics');

for (const marker of [
  'release_inventory_by_identity(',
  'mark_released(',
  'let message = compensation.to_string();',
  'mark_compensation_retryable(',
]) requireText(retained, marker, 'retained inventory flow');
for (const marker of [
  'pub trait InventoryReservationIdentityPort',
  'async fn release_inventory_by_identity(',
]) requireText(ownerContract, marker, 'inventory owner contract');
requireText(
  ownerWrapper,
  'require_inventory_reservation_write_admission(&context, RELEASE_OPERATION)?;',
  'inventory owner write admission',
);

for (const [key, expected] of Object.entries({
  mounted_combined_payment_order_inventory_cart_facade_active: true,
  inventory_owner_message_public: false,
  inventory_owner_message_persisted: false,
  legacy_inventory_diagnostic_suppressed: true,
  legacy_cart_diagnostic_suppressed: true,
  cart_compensation_mapper_changed: true,
  remaining_compensation_cleanup_closed: true,
  broad_ecommerce_cleanup_closed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push('evidence execution must remain empty');
}
requireText(doc, 'Status: **source-reviewed / unvalidated**', 'truthful inventory documentation');

if (failures.length > 0) {
  console.error('Checkout inventory compensation error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Checkout inventory compensation remains guarded in the four-owner facade');
