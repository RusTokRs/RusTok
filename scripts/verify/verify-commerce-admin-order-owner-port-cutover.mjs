#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const adminModule = read('crates/rustok-commerce/src/controllers/admin/mod.rs');
const mountedOrders = read(
  'crates/rustok-commerce/src/controllers/admin/orders_owner_ports.rs',
);
const commerceRuntime = read('crates/rustok-commerce/src/controllers/mod.rs');
const orderRoot = read('crates/rustok-order/src/lib.rs');
const orderCommand = read('crates/rustok-order/src/admin_command.rs');
const paymentRoot = read('crates/rustok-payment/src/lib.rs');
const paymentOrderRead = read('crates/rustok-payment/src/order_read.rs');
const fulfillmentRead = read('crates/rustok-fulfillment/src/fulfillment_read.rs');
const serverComposition = read(
  'apps/server/src/services/commerce_provider_runtime.rs',
);
const note = read(
  'crates/rustok-commerce/docs/admin-order-owner-port-cutover-2026-08-08.md',
);

const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [source, value, label] of [
  [adminModule, '#[path = "orders_owner_ports.rs"]', 'mounted admin Order path'],
  [adminModule, 'pub mod orders;', 'mounted admin Order module'],
  [mountedOrders, '.order_read_port()', 'mounted Order read port'],
  [mountedOrders, '.order_admin_command_port()', 'mounted Order command port'],
  [mountedOrders, '.payment_order_read_port()', 'mounted Payment Order read port'],
  [mountedOrders, '.fulfillment_read_port()', 'mounted Fulfillment read port'],
  [mountedOrders, 'FindLatestFulfillmentByOrderProjectionRequest', 'Fulfillment Order projection request'],
  [mountedOrders, 'LatestPaymentCollectionByOrderRequest', 'Payment Order projection request'],
  [mountedOrders, 'OwnerMarkOrderPaidRequest', 'Order mark-paid owner request'],
  [mountedOrders, 'OwnerShipOrderRequest', 'Order ship owner request'],
  [mountedOrders, 'OwnerDeliverOrderRequest', 'Order deliver owner request'],
  [mountedOrders, 'OwnerCancelOrderRequest', 'Order cancel owner request'],
  [mountedOrders, '.with_deadline(', 'mounted deadline context'],
  [mountedOrders, '.with_idempotency_key(', 'mounted write attempt identity'],

  [commerceRuntime, 'order_admin_command_runtime: rustok_order::OrderAdminCommandRuntime', 'Commerce Order command runtime field'],
  [commerceRuntime, 'payment_order_read_runtime: rustok_payment::PaymentOrderReadRuntime', 'Commerce Payment Order read runtime field'],
  [commerceRuntime, 'fn order_admin_command_port(', 'Commerce Order command runtime accessor'],
  [commerceRuntime, 'fn payment_order_read_port(', 'Commerce Payment Order read runtime accessor'],
  [commerceRuntime, '.shared_get::<rustok_order::OrderAdminCommandRuntime>()', 'Commerce host Order command requirement'],
  [commerceRuntime, '.shared_get::<rustok_payment::PaymentOrderReadRuntime>()', 'Commerce host Payment Order read requirement'],

  [orderRoot, 'pub use admin_command::{', 'Order owner command exports'],
  [orderCommand, 'pub trait OrderAdminCommandPort', 'Order admin command capability'],
  [orderCommand, 'pub struct OrderAdminCommandRuntime', 'Order admin command runtime'],
  [orderCommand, 'PortCallPolicy::write()', 'Order write admission policy'],
  [orderCommand, 'OrderService::new(db, event_bus)', 'Order owner in-process delegation'],

  [paymentRoot, 'pub use order_read::{', 'Payment Order read exports'],
  [paymentOrderRead, 'pub trait PaymentOrderReadPort', 'Payment Order read capability'],
  [paymentOrderRead, 'pub struct PaymentOrderReadRuntime', 'Payment Order read runtime'],
  [paymentOrderRead, 'PortCallPolicy::read()', 'Payment read admission policy'],
  [paymentOrderRead, 'PaymentService::new(db)', 'Payment owner in-process delegation'],

  [fulfillmentRead, 'find_latest_fulfillment_by_order_projection', 'existing Fulfillment Order read capability'],

  [serverComposition, 'host\n            .shared_get::<rustok_order::OrderAdminCommandRuntime>()', 'host-selected Order command preference'],
  [serverComposition, '.or_else(|| server.shared_get::<rustok_order::OrderAdminCommandRuntime>())', 'server-shared Order command fallback'],
  [serverComposition, 'rustok_order::OrderAdminCommandRuntime::in_process(', 'built-in Order command fallback'],
  [serverComposition, 'host\n            .shared_get::<rustok_payment::PaymentOrderReadRuntime>()', 'host-selected Payment read preference'],
  [serverComposition, '.or_else(|| server.shared_get::<rustok_payment::PaymentOrderReadRuntime>())', 'server-shared Payment read fallback'],
  [serverComposition, 'rustok_payment::PaymentOrderReadRuntime::in_process(server.db_clone())', 'built-in Payment read fallback'],

  [note, 'source-complete for the mounted admin Order route', 'cutover note status'],
  [note, 'does **not** claim durable Order command replay', 'cutover durable replay non-claim'],
]) requireText(source, value, label);

for (const [source, value, label] of [
  [mountedOrders, 'OrderService::new', 'mounted direct Order service construction'],
  [mountedOrders, 'PaymentService::new', 'mounted direct Payment service construction'],
  [mountedOrders, 'FulfillmentService::new', 'mounted direct Fulfillment service construction'],
  [mountedOrders, 'use rustok_order::OrderService', 'mounted concrete Order import'],
  [mountedOrders, 'use rustok_payment::PaymentService', 'mounted concrete Payment import'],
  [mountedOrders, 'use rustok_fulfillment::FulfillmentService', 'mounted concrete Fulfillment import'],
  [mountedOrders, 'runtime.db_clone()', 'mounted raw DB capability escape'],
  [mountedOrders, 'runtime.event_bus()', 'mounted raw event bus capability escape'],
]) forbidText(source, value, label);

if (failures.length > 0) {
  console.error('Commerce admin Order owner-port cutover verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Mounted Commerce admin Order routes use host-composed Order, Payment, and Fulfillment owner ports; execution evidence remains open',
);
