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
const mounted = `${facade}\n${retained}`;
const order = read('crates/rustok-order/src/checkout_compensation.rs');
const payment = read('crates/rustok-payment/src/checkout_compensation.rs');
const inventory = read('crates/rustok-inventory/src/ports.rs');
const cart = read('crates/rustok-cart/src/ports.rs');

requireText(
  services,
  '#[path = "checkout_compensation_error_safe.rs"]\nmod checkout_compensation;',
  'commerce services mount',
);
forbidText(
  services.replace(
    '#[path = "checkout_compensation_error_safe.rs"]\nmod checkout_compensation;',
    '',
  ),
  'mod checkout_compensation;',
  'duplicate commerce compensation mount',
);

for (const marker of [
  'include!("checkout_compensation_owner_ports.rs");',
  'use super::rustok_payment_shim as rustok_payment;',
  'use super::rustok_order_shim as rustok_order;',
  'use super::rustok_inventory_shim as rustok_inventory;',
  'use super::rustok_cart_shim as rustok_cart;',
  'wrap_checkout_payment_compensation_port(',
  'wrap_checkout_order_compensation_port(',
  'wrap_inventory_reservation_identity_port(',
  'wrap_cart_checkout_port(',
]) requireText(facade, marker, 'mounted combined compensation facade');

for (const value of [
  'CheckoutOrderCompensationPort',
  'CheckoutPaymentCompensationPort',
  'InventoryReservationIdentityPort',
  'CartCheckoutPort',
  'compensate_checkout_order(',
  'compensate_checkout_payment(',
  'release_inventory_by_identity(',
  'read_cart_checkout_snapshot(',
  'release_cart_checkout(',
  'with_causation_id(',
  'with_idempotency_key(',
  'with_deadline(',
]) requireText(mounted, value, 'mounted commerce compensation');

for (const value of [
  'OrderService',
  'PaymentService',
  'PaymentProviderOperationJournal',
  'PaymentOrchestrationService',
  'InventoryService',
  'CartService',
  'CancelPaymentInput',
  '.cancel_order(',
  '.cancel_collection(',
]) forbidText(facade, value, 'mounted commerce owner bypass');

for (const [source, label, traitName, operations] of [
  [order, 'order owner contract', 'CheckoutOrderCompensationPort', ['compensate_checkout_order']],
  [payment, 'payment owner contract', 'CheckoutPaymentCompensationPort', ['compensate_checkout_payment']],
  [inventory, 'inventory owner contract', 'InventoryReservationIdentityPort', ['release_inventory_by_identity']],
  [cart, 'cart owner contract', 'CartCheckoutPort', ['read_cart_checkout_snapshot', 'release_cart_checkout']],
]) {
  requireText(source, `trait ${traitName}`, label);
  for (const operation of operations) requireText(source, `async fn ${operation}(`, label);
}

for (const owner of ['rustok_payment', 'rustok_order', 'rustok_inventory', 'rustok_cart']) {
  const suppressions = facade.match(new RegExp(`\\$owner != "${owner}"`, 'g')) ?? [];
  if (suppressions.length !== 2) {
    failures.push(`expected two compatibility-log suppressions for ${owner}, found ${suppressions.length}`);
  }
}

for (const marker of [
  'kind: error.kind',
  'code: error.code',
  'retryable: error.retryable',
  'formatter.write_str("redacted")',
  'tenant_id_shape',
  'correlation_id_shape',
  'owner_message_present',
  'owner_message_len',
]) requireText(facade, marker, 'bounded consumer error policy');

for (const marker of [
  'error = ?error',
  'internal_message = %error.message',
  'correlation_id = %context.correlation_id',
  'tenant_id = %context.tenant_id',
]) forbidText(facade, marker, 'raw mounted diagnostics');

for (const marker of [
  'self.compensate_payment(tenant_id, actor_id, operation)',
  'self.compensate_order(tenant_id, actor_id, operation)',
  'self.release_remaining_reservations(tenant_id, operation)',
  'self.release_cart(tenant_id, operation)',
  'let message = compensation.to_string();',
  'mark_compensation_retryable(',
]) requireText(retained, marker, 'retained compensation flow');

if (failures.length > 0) {
  console.error('Checkout compensation owner-boundary verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Checkout compensation is mounted through bounded payment, order, inventory, and cart owner adapters');
