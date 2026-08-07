#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (file) => readFileSync(new URL(file, root), 'utf8');
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
const cartContract = read('crates/rustok-cart/src/ports.rs');
const cartOwner = read('crates/rustok-cart/src/owner_ports.rs');
const doc = read('crates/rustok-commerce/docs/checkout-compensation-cart-context.md');
const evidenceText = read(
  'crates/rustok-commerce/contracts/evidence/checkout-cart-compensation-error-safety-source-review.json',
);
const evidence = JSON.parse(evidenceText);

requireText(
  services,
  '#[path = "checkout_compensation_error_safe.rs"]\nmod checkout_compensation;',
  'mounted cart-safe compensation facade',
);
forbidText(
  services,
  '#[path = "checkout_compensation_payment_safe.rs"]\nmod checkout_compensation;',
  'superseded compensation mount',
);

for (const marker of [
  'include!("checkout_compensation_owner_ports.rs");',
  'use super::rustok_cart_shim as rustok_cart;',
  'use super::rustok_inventory_shim as rustok_inventory;',
  'use super::rustok_order_shim as rustok_order;',
  'use super::rustok_payment_shim as rustok_payment;',
  'wrap_cart_checkout_port(cart_port)',
  'pub(crate) fn cart_snapshot(',
  'pub(crate) fn cart_release(',
  'BoundaryFacts::cart_snapshot(&request)',
  'BoundaryFacts::cart_release(&request)',
  '.read_cart_checkout_snapshot(context, request)',
  '.release_cart_checkout(context, request)',
]) requireText(facade, marker, 'cart-safe facade topology');

for (const marker of [
  'owner: "rustok_cart"',
  'operation: "read_cart_checkout_snapshot"',
  'operation: "release_cart_checkout"',
  'stage: "read_cart"',
  'stage: "release_cart"',
  'boundary: "commerce_checkout_cart_snapshot_compensation_adapter"',
  'boundary: "commerce_checkout_cart_release_compensation_adapter"',
  'primary_id_shape: uuid_shape(request.cart_id)',
  'opaque_text_shape: optional_text_shape(request.locale.as_deref())',
  'opaque_text_len: request.locale.as_ref().map(|value| value.chars().count())',
]) requireText(facade, marker, 'bounded cart request facts');

for (const marker of [
  '"Checkout cart compensation request is invalid"',
  '"Checkout cart compensation resource was not found"',
  '"Checkout cart compensation conflicts with the current cart state"',
  '"Checkout cart compensation is not permitted"',
  '"Checkout cart compensation service is temporarily unavailable"',
  '"Checkout cart compensation could not be completed safely"',
]) requireText(facade, marker, 'static cart public message');

for (const marker of [
  'kind: error.kind',
  'code: error.code',
  'retryable: error.retryable',
  'owner_message_present',
  'owner_message_len',
  'tenant_id_shape',
  'actor_id_shape',
  'correlation_id_shape',
  'causation_id_shape',
  'traceparent_shape',
  'idempotency_key_shape',
  'struct DiagnosticError;',
  'formatter.write_str("redacted")',
]) requireText(facade, marker, 'bounded cart error diagnostics');

for (const marker of [
  'error = ?error',
  'internal_message = %error.message',
  'correlation_id = %context.correlation_id',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'idempotency_key = ?context.idempotency_key',
]) forbidText(facade, marker, 'raw mounted cart diagnostics');

const cartSuppressions = facade.match(/&& \$owner != "rustok_cart"/g) ?? [];
if (cartSuppressions.length !== 2) {
  failures.push(`expected two cart compatibility-log suppressions, found ${cartSuppressions.length}`);
}

for (const marker of [
  'async fn read_cart_checkout_snapshot(',
  'async fn release_cart_checkout(',
]) requireText(cartContract, marker, 'canonical cart contract');
for (const marker of [
  'READ_CART_CHECKOUT_SNAPSHOT_OPERATION',
  'RELEASE_CART_CHECKOUT_OPERATION',
  'PortError::validation("cart.validation", "cart request is invalid")',
  'PortError::not_found("cart.cart_not_found", "cart was not found")',
]) requireText(cartOwner, marker, 'cart owner boundary');

for (const marker of [
  'self.release_remaining_reservations(tenant_id, operation)',
  'self.release_cart(tenant_id, operation)',
  'read_cart_checkout_snapshot(',
  'release_cart_checkout(',
  'CartStatus::CheckingOut',
  'CartStatus::Active => {}',
  'CartStatus::Completed',
  'CartStatus::Abandoned',
  'let message = compensation.to_string();',
  'mark_compensation_retryable(',
]) requireText(retained, marker, 'retained compensation behavior');

for (const marker of [
  'Status: **source-reviewed / unvalidated**',
  'No tests, Node verifiers, Cargo commands',
]) requireText(doc, marker, 'truthful cart documentation');

if (evidence.status !== 'commerce_checkout_cart_compensation_error_safety_source_reviewed_unvalidated') {
  failures.push(`unexpected evidence status: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  mounted_combined_payment_order_inventory_cart_facade_active: true,
  retained_compensation_business_logic_changed: false,
  constructor_cart_port_wrapped: true,
  cart_snapshot_path_wrapped: true,
  cart_release_path_wrapped: true,
  cart_owner_message_public: false,
  cart_owner_message_persisted: false,
  raw_cart_context_values_logged: false,
  raw_cart_request_values_logged: false,
  legacy_cart_diagnostic_suppressed: true,
  cart_lifecycle_behavior_changed: false,
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

if (failures.length > 0) {
  console.error('Checkout cart compensation error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Checkout cart compensation errors are source-guarded behind bounded Commerce adapters');
