#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/modules/rustok-commerce/src/controllers/admin/orders_owner_ports.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};

const showOrder = between(source, 'pub async fn show_order(', 'pub async fn mark_order_paid(', 'admin order detail handler');
const paymentMapper = between(source, 'fn map_payment_detail_port_error(', 'fn map_fulfillment_detail_port_error(', 'payment detail owner-port mapper');
const fulfillmentMapper = between(source, 'fn map_fulfillment_detail_port_error(', 'pub async fn list_orders(', 'fulfillment detail owner-port mapper');

for (const [value, label] of [
  ['.order_read_port()', 'order owner read handoff'],
  ['ReadOrderProjectionRequest {', 'order owner read request'],
  ['.payment_order_read_port()', 'payment owner read handoff'],
  ['LatestPaymentCollectionByOrderRequest { order_id: id }', 'payment owner request'],
  ['.fulfillment_read_port()', 'fulfillment owner read handoff'],
  ['FindLatestFulfillmentByOrderProjectionRequest { order_id: id }', 'fulfillment owner request'],
  ['map_payment_detail_port_error(tenant.id, id, &payment_context, error)', 'payment detail mapper handoff'],
  ['map_fulfillment_detail_port_error(tenant.id, id, &fulfillment_context, error)', 'fulfillment detail mapper handoff'],
  ['HttpResult<Json<AdminOrderDetailResponse>>', 'order detail result contract'],
]) requireText(showOrder, value, label);

for (const value of [
  'PaymentService::new(',
  'FulfillmentService::new(',
  'OrderService::new(',
  'find_latest_collection_by_order(tenant.id, id)',
  'find_by_order(tenant.id, id)',
  'map_order_detail_payment_error(',
  'map_order_detail_fulfillment_error(',
  'error = ?error',
  'internal_message = %error.message',
  'internal_code = %error.code',
  'order_id = %order_id',
]) forbidText(showOrder, value, 'mounted order-detail direct/unsafe boundary');

for (const [content, label, owner] of [
  [paymentMapper, 'payment detail mapper', 'payment'],
  [fulfillmentMapper, 'fulfillment detail mapper', 'fulfillment'],
]) {
  for (const value of [
    'error = ?error',
    'tenant_id = %tenant_id',
    'order_id = %order_id',
    'internal_code = %error.code',
    'internal_message = %error.message',
  ]) forbidText(content, value, `${label} raw diagnostic`);
  for (const [value, name] of [
    ['PortErrorKind::Validation', `${owner} validation mapping`],
    ['PortErrorKind::NotFound', `${owner} not-found mapping`],
    ['PortErrorKind::Conflict', `${owner} conflict mapping`],
    ['PortErrorKind::Forbidden', `${owner} forbidden mapping`],
    ['PortErrorKind::Unavailable | PortErrorKind::Timeout', `${owner} unavailable mapping`],
    ['PortErrorKind::InvariantViolation', `${owner} invariant mapping`],
    ['owner_code_length = error.code.chars().count()', `${owner} bounded code length`],
    ['retryable = error.retryable', `${owner} retryability`],
    ['owner_error_kind = port_error_kind(&error)', `${owner} bounded error kind`],
    ['HttpError::new(status, code, message)', `${owner} static HTTP envelope`],
  ]) requireText(content, value, name);
}

for (const [value, label] of [
  ['tenant_id_non_nil = !tenant_id.is_nil()', 'payment tenant shape'],
  ['order_id_non_nil = !order_id.is_nil()', 'payment/order shape'],
  ['tenant_id_non_nil = !tenant_id.is_nil()', 'fulfillment tenant shape'],
  ['commerce_admin_order_detail_http', 'order-detail HTTP boundary'],
]) requireText(paymentMapper + fulfillmentMapper, value, label);

if (failures.length > 0) {
  console.error('Commerce admin order-detail fulfillment error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Commerce admin order detail uses owner read ports and bounded static HTTP error envelopes');