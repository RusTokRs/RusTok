#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const admin = read('crates/modules/rustok-commerce/src/controllers/admin/mod.rs');
const orders = read('crates/modules/rustok-commerce/src/controllers/admin/orders_owner_ports.rs');
const changes = read('crates/modules/rustok-commerce/src/controllers/admin/changes.rs');
const returns = read('crates/modules/rustok-commerce/src/controllers/admin/returns.rs');
const fulfillments = read('crates/modules/rustok-commerce/src/controllers/admin/fulfillments_owner_commands.rs');
const orderCommands = read('crates/modules/rustok-order/src/admin_command.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

requireText(admin, '#[path = "orders_owner_ports.rs"]', 'active admin order module wiring');
requireText(admin, 'pub mod fulfillments;', 'active fulfillment owner command module wiring');

for (const [value, label] of [
  ['pub async fn list_orders(', 'admin list-orders handler'],
  ['pub async fn show_order(', 'admin show-order handler'],
  ['pub async fn mark_order_paid(', 'admin mark-paid handler'],
  ['pub async fn ship_order(', 'admin ship handler'],
  ['pub async fn deliver_order(', 'admin deliver handler'],
  ['pub async fn cancel_order(', 'admin cancel handler'],
  ['fn admin_order_port_context(', 'read port context'],
  ['fn admin_order_command_port_context(', 'command port context'],
  ['fn require_idempotency_key(headers: &HeaderMap)', 'caller-owned idempotency parser'],
  ['.order_read_port()', 'order read owner port'],
  ['.payment_order_read_port()', 'payment detail owner port'],
  ['.fulfillment_read_port()', 'fulfillment detail owner port'],
  ['.order_admin_command_port()', 'order command owner port'],
  ['OwnerMarkOrderPaidRequest {', 'typed mark-paid request'],
  ['OwnerShipOrderRequest {', 'typed ship request'],
  ['OwnerDeliverOrderRequest {', 'typed deliver request'],
  ['OwnerCancelOrderRequest {', 'typed cancel request'],
]) requireText(orders, value, label);

for (const value of [
  'OrderService::new(',
  'PaymentService::new(',
  'FulfillmentService::new(',
  'Uuid::new_v4().to_string()',
  'error = ?error',
  'tenant_id = %context.tenant_id',
  'internal_code = %error.code',
  'internal_message = %error.message',
  'fn admin_order_error_policy(',
  'fn map_admin_order_error(',
]) forbidText(orders, value, 'active admin order direct/unsafe boundary');

for (const [value, label] of [
  ['tenant_id_non_nil = !tenant_id.is_nil()', 'tenant presence fact'],
  ['actor_id_non_nil = !actor_id.is_nil()', 'actor presence fact'],
  ['order_id_present = order_id.is_some()', 'order identity presence fact'],
  ['order_id_non_nil = order_id.map(|value| !value.is_nil()).unwrap_or(false)', 'order identity shape fact'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner code fact'],
  ['retryable = error.retryable', 'retryability fact'],
  ['public_code = code', 'stable public code fact'],
  ['.with_idempotency_key(idempotency_key)', 'caller-owned idempotency propagation'],
  ['headers: HeaderMap,', 'write request idempotency header extraction'],
  ['let idempotency_key = require_idempotency_key(&headers)?;', 'write request idempotency enforcement'],
]) requireText(orders, value, label);

if ((orders.match(/let idempotency_key = require_idempotency_key\(&headers\)\?;/g) ?? []).length !== 4) {
  failures.push('expected four admin order write handlers to require caller-owned Idempotency-Key');
}
if ((orders.match(/\.with_idempotency_key\(idempotency_key\)/g) ?? []).length !== 1) {
  failures.push('expected exactly one command-context propagation of caller-owned Idempotency-Key');
}

for (const [content, label] of [
  [changes, 'admin order changes'],
  [returns, 'admin order returns'],
  [fulfillments, 'admin fulfillments'],
]) {
  for (const value of [
    'error = ?error',
    'internal_message = %error.message',
    'E: std::fmt::Debug',
    'err.to_string()',
  ]) forbidText(content, value, `${label} unsafe diagnostic/public conversion`);
}

for (const [content, values] of [
  [changes, [
    ['pub async fn create_order_change(', 'order-change create'],
    ['pub async fn list_order_changes(', 'order-change list'],
    ['pub async fn show_order_change(', 'order-change detail'],
    ['pub async fn apply_order_change(', 'order-change apply'],
    ['pub async fn cancel_order_change(', 'order-change cancel'],
  ]],
  [returns, [
    ['pub async fn list_order_returns(', 'return list'],
    ['pub async fn show_order_return(', 'return detail'],
    ['pub async fn create_order_return(', 'return create'],
    ['pub async fn create_order_return_decision(', 'return decision'],
    ['pub async fn complete_order_return(', 'return complete'],
    ['pub async fn cancel_order_return(', 'return cancel'],
  ]],
  [fulfillments, [
    ['pub async fn list_fulfillments(', 'fulfillment list'],
    ['pub async fn create_fulfillment(', 'fulfillment create'],
    ['pub async fn show_fulfillment(', 'fulfillment detail'],
    ['pub async fn ship_fulfillment(', 'fulfillment ship'],
    ['pub async fn deliver_fulfillment(', 'fulfillment deliver'],
    ['pub async fn reopen_fulfillment(', 'fulfillment reopen'],
    ['pub async fn reship_fulfillment(', 'fulfillment reship'],
    ['pub async fn cancel_fulfillment(', 'fulfillment cancel'],
  ]],
]) {
  for (const [value, label] of values) requireText(content, value, label);
}

for (const [content, label] of [
  [orders, 'active admin order controller'],
  [orderCommands, 'order admin command owner'],
]) {
  forbidText(content, 'error.to_string()', `${label} raw error conversion`);
  forbidText(content, 'error = ?error', `${label} raw error serialization`);
}

requireText(orderCommands, 'pub trait OrderAdminCommandPort', 'owner command port trait');
requireText(orderCommands, 'fn require_admin_command_context(', 'owner command context validation');

if (failures.length > 0) {
  console.error('Commerce admin order/fulfillment HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Active Commerce admin order/fulfillment boundaries use typed owner ports and bounded HTTP diagnostics');