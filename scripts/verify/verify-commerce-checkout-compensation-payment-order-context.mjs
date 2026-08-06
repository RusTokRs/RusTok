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

const services = read('crates/rustok-commerce/src/services/mod.rs');
const facade = read('crates/rustok-commerce/src/services/checkout_compensation_payment_safe.rs');
const legacy = read('crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs');

requireText(
  services,
  '#[path = "checkout_compensation_payment_safe.rs"]\nmod checkout_compensation;',
  'mounted compensation facade',
);

for (const marker of [
  'use super::rustok_payment_shim as rustok_payment;',
  'use super::rustok_order_shim as rustok_order;',
  'use super::rustok_inventory_shim as rustok_inventory;',
  'include!("checkout_compensation_owner_ports.rs");',
  'BoundaryFacts::payment(&request)',
  'BoundaryFacts::order(&request)',
  'BoundaryFacts::inventory(&request)',
  'operation_id_shape = facts.operation_id_shape',
  'primary_id_shape = facts.primary_id_shape',
  'secondary_id_shape = facts.secondary_id_shape',
  'opaque_text_shape = facts.opaque_text_shape',
  'opaque_text_len = ?facts.opaque_text_len',
  'owner_message_present',
  'owner_message_len',
  'formatter.write_str("redacted")',
]) requireText(facade, marker, 'bounded combined compensation facade');

for (const forbidden of [
  'message: error.message',
  'error = ?error',
  'internal_message',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'correlation_id = %context.correlation_id',
  'idempotency_key = ?context.idempotency_key',
]) forbidText(facade, forbidden, 'mounted facade raw payload');

requireCount(
  facade,
  '&& $owner != "rustok_inventory"',
  2,
  'payment/order/inventory retained diagnostic suppression',
);

for (const marker of [
  'let payment_context = payment_context(',
  'payment_context.clone()',
  '&payment_context',
  'PAYMENT_COMPENSATION_OWNER',
  'PAYMENT_COMPENSATION_OPERATION',
  'let order_context = order_context(',
  'order_context.clone()',
  '&order_context',
  'ORDER_COMPENSATION_OWNER',
  'ORDER_COMPENSATION_OPERATION',
  'let inventory_context =',
  'inventory_context.clone()',
  '&inventory_context',
  'INVENTORY_COMPENSATION_OWNER',
  'INVENTORY_COMPENSATION_OPERATION',
  'CheckoutCompensationError::ManualReconciliation(error.message)',
  'message: error.message',
  'let message = compensation.to_string();',
  '.mark_compensation_retryable(',
]) requireText(legacy, marker, 'retained compensation context and flow');

for (const marker of [
  'self.compensate_payment(tenant_id, actor_id, operation)',
  'self.compensate_order(tenant_id, actor_id, operation)',
  'self.release_remaining_reservations(tenant_id, operation)',
  'self.release_cart(tenant_id, operation)',
  'PaymentCollectionStatusKind::Cancelled',
  'OrderStatusKind::Cancelled',
  'release_inventory_by_identity(',
  'read_cart_checkout_snapshot(',
  'release_cart_checkout(',
]) requireText(legacy, marker, 'unchanged compensation orchestration');

for (const marker of [
  'error = ?error',
  'tenant_id = %context.tenant_id',
  'correlation_id = %context.correlation_id',
  'internal_message = %error.message',
]) requireText(legacy, marker, 'private retained compatibility diagnostic');

if (failures.length > 0) {
  console.error('Checkout compensation payment/order/inventory context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout payment, order, and inventory compensation retain delegated owner context behind bounded mounted adapters while cart remains unchanged',
);
