#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/rustok-commerce/src/controllers/admin/orders.rs');
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
  'fn map_order_detail_fulfillment_error(',
  'admin order detail handler',
);
const mapper = between(
  source,
  'fn map_order_detail_fulfillment_error(',
  '/// Mark admin ecommerce order as paid',
  'admin order detail fulfillment mapper',
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
  ['use rustok_fulfillment::{FulfillmentError, FulfillmentService};', 'typed fulfillment error import'],
  ['use rustok_web::{HttpError, HttpResult};', 'typed HTTP error import'],
]) requireText(source, value, label);

for (const [value, label] of [
  [
    '.map_err(|error| map_order_detail_fulfillment_error(tenant.id, id, error))?',
    'order-detail mapper handoff',
  ],
  ['[Permission::ORDERS_READ]', 'order read permission'],
  ['Path(id): Path<Uuid>', 'typed order path'],
  ['HttpResult<Json<AdminOrderDetailResponse>>', 'order detail result contract'],
]) requireText(showOrder, value, label);

for (const [value, label] of [
  ['FulfillmentError::Validation(_)', 'validation variant'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping option not-found variant'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment not-found variant'],
  ['FulfillmentError::InvalidTransition { .. }', 'transition variant'],
  ['FulfillmentError::Database(_)', 'database variant'],
  ['error = ?error', 'internal typed cause'],
  ['owner = ADMIN_ORDER_DETAIL_FULFILLMENT_OWNER', 'owner log'],
  ['tenant_id = %tenant_id', 'tenant log'],
  ['order_id = %order_id', 'order identity log'],
  ['operation = ADMIN_ORDER_DETAIL_FULFILLMENT_OPERATION', 'operation log'],
  ['error_kind,', 'error kind log'],
  ['public_code = code', 'stable code log'],
  ['status = %status', 'status log'],
  ['boundary = "commerce_admin_order_detail_http"', 'HTTP boundary log'],
]) requireText(mapper, value, label);

for (const [value, label] of [
  ['"Fulfillment request is invalid"', 'static validation envelope'],
  ['"Commerce resource not found"', 'static not-found envelope'],
  [
    '"Fulfillment operation conflicts with the current state"',
    'static transition envelope',
  ],
  [
    '"Fulfillment storage is temporarily unavailable"',
    'static storage envelope',
  ],
  ['HttpError::new(status, code, message)', 'single public envelope constructor'],
]) requireText(mapper, value, label);

for (const value of [
  '.map_err(super::map_fulfillment_error)',
  'format!("Fulfillment request is invalid: {msg}")',
  'HttpError::bad_request("commerce_admin_fulfillment_invalid", msg)',
  'error.to_string()',
]) forbidText(showOrder + mapper, value, 'unsafe admin order detail fulfillment mapping');

if (failures.length > 0) {
  console.error('Commerce admin order-detail fulfillment error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin order detail keeps fulfillment causes internal and returns static public envelopes',
);
