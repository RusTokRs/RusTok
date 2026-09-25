#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/modules/rustok-commerce/src/controllers/admin/orders.rs');
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

const showOrder = between(
  source,
  'pub async fn show_order(',
  '/// Mark admin ecommerce order as paid',
  'admin order detail handler',
);
const paymentPortMapper = between(
  source,
  'fn map_order_detail_payment_port_error(',
  'fn map_order_detail_fulfillment_port_error(',
  'admin order detail payment owner-port mapper',
);
const fulfillmentPortMapper = between(
  source,
  'fn map_order_detail_fulfillment_port_error(',
  '/// Mark admin ecommerce order as paid',
  'admin order detail fulfillment owner-port mapper',
);

for (const [value, label] of [
  [
    'const ADMIN_ORDER_DETAIL_FULFILLMENT_OWNER: &str = "rustok_fulfillment.admin_order_detail";',
    'fulfillment owner constant',
  ],
  [
    'const ADMIN_ORDER_DETAIL_FULFILLMENT_OPERATION: &str = "find_fulfillment_by_order";',
    'fulfillment operation constant',
  ],
  ['use rustok_fulfillment::FulfillmentError;', 'typed fulfillment compatibility error import'],
  ['use rustok_web::{HttpError, HttpResult};', 'typed HTTP error import'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['.payment_order_read_port()', 'payment owner read port handoff'],
  ['find_latest_collection_by_order(', 'payment owner latest-collection operation'],
  ['LatestPaymentCollectionByOrderRequest { order_id: id }', 'payment owner request'],
  ['.fulfillment_read_port()', 'fulfillment owner read port handoff'],
  ['find_latest_fulfillment_by_order_projection(', 'fulfillment owner latest-fulfillment operation'],
  ['FindLatestFulfillmentByOrderProjectionRequest { order_id: id }', 'fulfillment owner request'],
  ['map_order_detail_payment_port_error(id, error)', 'payment port error mapper handoff'],
  ['map_order_detail_fulfillment_port_error(id, error)', 'fulfillment port error mapper handoff'],
  ['[Permission::ORDERS_READ]', 'order read permission'],
  ['Path(id): Path<Uuid>', 'typed order path'],
  ['HttpResult<Json<AdminOrderDetailResponse>>', 'order detail result contract'],
]) requireText(showOrder, value, label);

for (const value of [
  'PaymentService::new(runtime.db_clone())',
  'FulfillmentService::new(runtime.db_clone())',
  'map_order_detail_payment_error(',
  'map_order_detail_fulfillment_error(',
]) forbidText(showOrder, value, 'mounted order-detail direct owner construction/obsolete mapper');

for (const [value, label] of [
  ['fn map_order_detail_payment_port_error(order_id: Uuid, error: PortError)', 'payment owner-port mapper'],
  ['fn map_order_detail_fulfillment_port_error(', 'fulfillment owner-port mapper'],
  ['PortErrorKind::Validation', 'validation kind mapping'],
  ['PortErrorKind::NotFound', 'not-found kind mapping'],
  ['PortErrorKind::Conflict', 'conflict kind mapping'],
  ['PortErrorKind::Forbidden', 'forbidden kind mapping'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'unavailable kind mapping'],
  ['PortErrorKind::InvariantViolation', 'invariant kind mapping'],
  ['order_id = uuid_shape(order_id)', 'order shape logging'],
  ['internal_code_length = error.code.chars().count()', 'bounded owner code logging'],
  ['retryable = error.retryable', 'retryable logging'],
  ['owner = ADMIN_ORDER_DETAIL_PAYMENT_OWNER', 'payment owner log'],
  ['owner = ADMIN_ORDER_DETAIL_FULFILLMENT_OWNER', 'fulfillment owner log'],
  ['operation = ADMIN_ORDER_DETAIL_PAYMENT_OPERATION', 'payment operation log'],
  ['operation = ADMIN_ORDER_DETAIL_FULFILLMENT_OPERATION', 'fulfillment operation log'],
  ['error_variant = facts.error_variant', 'domain error shape logging'],
  ['text_field_count = facts.text_field_count', 'text shape logging'],
  ['uuid_field_count = facts.uuid_field_count', 'uuid shape logging'],
  ['opaque_payload_present = facts.opaque_payload_present', 'opaque cause logging'],
  ['public_code = code', 'stable code log'],
  ['status = %status', 'status log'],
  ['boundary = "commerce_admin_order_detail_http"', 'HTTP boundary log'],
]) requireText(paymentPortMapper + fulfillmentPortMapper, value, label);

for (const [value, label] of [
  ['"Payment request is invalid"', 'payment static validation envelope'],
  ['"Payment storage is temporarily unavailable"', 'payment static unavailable envelope'],
  ['"Payment data could not be read safely"', 'payment static invariant envelope'],
  ['"Fulfillment request is invalid"', 'fulfillment static validation envelope'],
  ['"Fulfillment storage is temporarily unavailable"', 'fulfillment static unavailable envelope'],
  ['"Fulfillment data could not be read safely"', 'fulfillment static invariant envelope'],
  ['"Commerce resource not found"', 'static not-found envelope'],
  ['HttpError::new(status, code, message)', 'single public envelope constructor'],
]) requireText(paymentPortMapper + fulfillmentPortMapper, value, label);

for (const value of [
  'error = ?error',
  'tenant_id = %tenant_id',
  'order_id = %order_id',
  'error.to_string()',
  'map_order_detail_payment_error(',
  'map_order_detail_fulfillment_error(',
]) forbidText(showOrder + paymentPortMapper + fulfillmentPortMapper, value, 'unsafe admin order detail owner-port mapping');

if (failures.length > 0) {
  console.error('Commerce admin order-detail fulfillment error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin order detail keeps fulfillment causes internal and returns static public envelopes',
);
