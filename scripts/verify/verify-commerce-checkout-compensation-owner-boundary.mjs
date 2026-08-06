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
const facade = read('crates/rustok-commerce/src/services/checkout_compensation_payment_safe.rs');
const legacy = read('crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs');
const compensation = `${facade}\n${legacy}`;
const order = read('crates/rustok-order/src/checkout_compensation.rs');
const payment = read('crates/rustok-payment/src/checkout_compensation.rs');
const inventoryContract = read('crates/rustok-inventory/src/ports.rs');
const inventoryWrapper = read('crates/rustok-inventory/src/reservation_owner_context.rs');

requireText(
  services,
  '#[path = "checkout_compensation_payment_safe.rs"]\nmod checkout_compensation;',
  'commerce services mount',
);
forbidText(
  services.replace(
    '#[path = "checkout_compensation_payment_safe.rs"]\nmod checkout_compensation;',
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
  'wrap_checkout_payment_compensation_port(',
  'wrap_checkout_order_compensation_port(',
  'wrap_inventory_reservation_identity_port(',
]) requireText(facade, marker, 'mounted combined compensation facade');

for (const value of [
  'CheckoutOrderCompensationPort',
  'CheckoutPaymentCompensationPort',
  'InventoryReservationIdentityPort',
  'compensate_checkout_order(',
  'compensate_checkout_payment(',
  'release_inventory_by_identity(',
  'with_causation_id(',
  'with_idempotency_key(',
  'with_deadline(',
]) requireText(compensation, value, 'mounted commerce compensation');

for (const value of [
  'OrderService',
  'PaymentService',
  'PaymentProviderOperationJournal',
  'PaymentOrchestrationService',
  'InventoryService',
  'CancelPaymentInput',
  '.cancel_order(',
  '.cancel_collection(',
]) forbidText(compensation, value, 'mounted commerce owner bypass');

for (const [source, label, traitName, operation] of [
  [order, 'order compensation owner', 'CheckoutOrderCompensationPort', 'compensate_checkout_order'],
  [payment, 'payment compensation owner', 'CheckoutPaymentCompensationPort', 'compensate_checkout_payment'],
]) {
  requireText(source, `trait ${traitName}`, label);
  requireText(source, `async fn ${operation}(`, label);
}

for (const marker of [
  'pub trait InventoryReservationIdentityPort',
  'async fn release_inventory_by_identity(',
]) requireText(inventoryContract, marker, 'inventory compensation owner contract');

requireText(order, 'require_policy(PortCallPolicy::write())?', 'order owner write policy');
requireText(order, 'require_write_semantics()?', 'order owner write semantics');
requireText(payment, 'require_policy(PortCallPolicy::write())?', 'payment owner write policy');
requireText(payment, 'require_write_semantics()?', 'payment owner write semantics');
requireText(
  inventoryWrapper,
  'require_inventory_reservation_write_admission(&context, RELEASE_OPERATION)?;',
  'inventory owner write admission',
);

requireText(order, 'OrderService::new(', 'order compensation owner');
requireText(payment, 'PaymentService::new(', 'payment compensation owner');
requireText(payment, 'PaymentProviderOperationJournal::new(', 'payment compensation owner');
requireText(payment, 'execute_cancel(', 'payment compensation owner');
requireText(
  payment,
  'PROVIDER_OPERATION_RECONCILIATION_REQUIRED',
  'payment compensation owner',
);
requireText(
  inventoryWrapper,
  'map_inventory_reservation_identity_local_port_error(',
  'inventory bounded owner wrapper',
);

if (failures.length > 0) {
  console.error('Checkout compensation owner-boundary verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout compensation is mounted through typed payment, order, and inventory owner ports with bounded consumer adapters; cart remains separate',
);
