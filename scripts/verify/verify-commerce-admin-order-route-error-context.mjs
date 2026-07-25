#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const orders = read('crates/rustok-commerce/src/controllers/admin/orders.rs');
const orderErrors = read('crates/rustok-order/src/error.rs');
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

const mapper = between(
  orders,
  'fn admin_order_error_policy(',
  '/// Show admin ecommerce order',
  'admin order route mapper',
);
const listRoute = between(
  orders,
  'pub async fn list_orders(',
  'pub async fn show_order(',
  'list orders route',
);
const showRoute = between(
  orders,
  'pub async fn show_order(',
  'fn map_order_detail_payment_error(',
  'show order route',
);
const markPaidRoute = between(
  orders,
  'pub async fn mark_order_paid(',
  '/// Ship admin ecommerce order',
  'mark-paid route',
);
const shipRoute = between(
  orders,
  'pub async fn ship_order(',
  '/// Deliver admin ecommerce order',
  'ship order route',
);
const deliverRoute = between(
  orders,
  'pub async fn deliver_order(',
  '/// Cancel admin ecommerce order',
  'deliver order route',
);
const cancelStart = orders.indexOf('pub async fn cancel_order(');
const cancelRoute = cancelStart < 0 ? '' : orders.slice(cancelStart);
if (cancelStart < 0) failures.push('cancel order route: unable to isolate source block');

for (const [value, label] of [
  ['use rustok_order::error::OrderError;', 'typed order error import'],
  ['use rustok_web::{HttpError, HttpResult};', 'typed HTTP error import'],
  ['const ADMIN_ORDER_OWNER: &str = "rustok_order.admin_orders";', 'owner constant'],
  ['const ADMIN_ORDER_BOUNDARY: &str = "commerce_admin_order_http";', 'HTTP boundary constant'],
  ['type AdminOrderHttpPolicy = (', 'static HTTP policy type'],
  ['struct AdminOrderErrorContext {', 'order error context'],
  ['tenant_id: Uuid,', 'tenant field'],
  ['actor_id: Uuid,', 'actor field'],
  ['order_id: Option<Uuid>,', 'order identity field'],
  ['customer_id: Option<Uuid>,', 'customer identity field'],
  ["operation: &'static str,", 'operation field'],
]) requireText(orders, value, label);

for (const [value, label] of [
  ['OrderError::Validation(_)', 'validation variant'],
  ['OrderError::OrderNotFound(_)', 'order not-found variant'],
  ['OrderError::OrderReturnNotFound(_)', 'return not-found variant'],
  ['OrderError::OrderChangeNotFound(_)', 'change not-found variant'],
  ['OrderError::InvalidTransition { .. }', 'transition variant'],
  ['OrderError::Database(_)', 'database variant'],
  ['OrderError::Core(_)', 'core variant'],
  ['StatusCode::BAD_REQUEST', 'bad-request status'],
  ['StatusCode::NOT_FOUND', 'not-found status'],
  ['StatusCode::CONFLICT', 'conflict status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'storage status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'fail-closed status'],
  ['"commerce_admin_order_invalid"', 'validation code'],
  ['"commerce_admin_not_found"', 'not-found code'],
  ['"commerce_admin_order_state_conflict"', 'conflict code'],
  ['"commerce_admin_order_storage_unavailable"', 'storage code'],
  ['"commerce_admin_order_failed"', 'fail-closed code'],
  ['"Order request is invalid"', 'static validation message'],
  ['"Commerce resource not found"', 'static not-found message'],
  ['"Order operation conflicts with the current state"', 'static conflict message'],
  ['"Order storage is temporarily unavailable"', 'static storage message'],
  ['"Order operation could not be completed safely"', 'static fail-closed message'],
  ['if let OrderError::OrderNotFound(id) = &error', 'typed order identity adoption'],
  ['context.order_id = Some(*id);', 'adopted order identity'],
  ['error = ?error', 'typed cause log'],
  ['owner = ADMIN_ORDER_OWNER', 'owner log'],
  ['tenant_id = %context.tenant_id', 'tenant log'],
  ['actor_id = %context.actor_id', 'actor log'],
  ['order_id = ?context.order_id', 'order identity log'],
  ['customer_id = ?context.customer_id', 'customer identity log'],
  ['operation = %context.operation', 'operation log'],
  ['error_kind,', 'error-kind log'],
  ['public_code = code', 'public-code log'],
  ['status = %status', 'status log'],
  ['boundary = ADMIN_ORDER_BOUNDARY', 'boundary log'],
  ['HttpError::new(status, code, message)', 'static envelope constructor'],
]) requireText(mapper, value, label);

for (const [block, operation, identity, serviceCall, label] of [
  [
    listRoute,
    '"list_orders"',
    'tenant.id,\n                    auth.user_id,\n                    None,\n                    customer_id,',
    '.list_orders_with_locale_fallback(',
    'list route',
  ],
  [
    showRoute,
    '"get_order"',
    'tenant.id,\n                    auth.user_id,\n                    Some(id),\n                    None,',
    '.get_order_with_locale_fallback(',
    'show route',
  ],
  [
    markPaidRoute,
    '"mark_order_paid"',
    'tenant.id,\n                    auth.user_id,\n                    Some(id),\n                    None,',
    '.mark_paid(',
    'mark-paid route',
  ],
  [
    shipRoute,
    '"ship_order"',
    'tenant.id,\n                    auth.user_id,\n                    Some(id),\n                    None,',
    '.ship_order(',
    'ship route',
  ],
  [
    deliverRoute,
    '"deliver_order"',
    'tenant.id,\n                    auth.user_id,\n                    Some(id),\n                    None,',
    '.deliver_order(tenant.id, auth.user_id, id, input.delivered_signature)',
    'deliver route',
  ],
  [
    cancelRoute,
    '"cancel_order"',
    'tenant.id,\n                    auth.user_id,\n                    Some(id),\n                    None,',
    '.cancel_order(tenant.id, auth.user_id, id, input.reason)',
    'cancel route',
  ],
]) {
  requireText(block, '.map_err(|error| {', `${label} typed mapping closure`);
  requireText(block, 'map_admin_order_error(', `${label} mapper handoff`);
  requireText(block, 'AdminOrderErrorContext::new(', `${label} context construction`);
  requireText(block, operation, `${label} operation`);
  requireText(block, identity, `${label} truthful route identity`);
  requireText(block, serviceCall, `${label} service contract`);
}

for (const [value, label] of [
  ['[Permission::ORDERS_LIST]', 'list permission'],
  ['[Permission::ORDERS_READ]', 'read permission'],
  ['[Permission::ORDERS_UPDATE]', 'update permission'],
  ['let customer_id = params.customer_id;', 'customer filter capture'],
  ['status: params.status', 'status filter forwarding'],
  ['customer_id,', 'customer filter forwarding'],
  ['page: pagination.page', 'pagination page forwarding'],
  ['per_page: pagination.limit()', 'pagination size forwarding'],
  ['request_context.locale.as_str()', 'requested locale forwarding'],
  ['Some(tenant.default_locale.as_str())', 'tenant fallback locale forwarding'],
  ['input.payment_id', 'payment identity forwarding'],
  ['input.payment_method', 'payment method forwarding'],
  ['input.tracking_number', 'tracking forwarding'],
  ['input.carrier', 'carrier forwarding'],
]) requireText(orders, value, label);

for (const [value, label] of [
  ['fn map_order_detail_payment_error(', 'payment detail mapper'],
  ['fn map_order_detail_fulfillment_error(', 'fulfillment detail mapper'],
  ['find_latest_collection_by_order(tenant.id, id)', 'payment detail lookup'],
  ['find_by_order(tenant.id, id)', 'fulfillment detail lookup'],
  ['map_order_detail_payment_error(tenant.id, id, error)', 'payment detail mapper call'],
  ['map_order_detail_fulfillment_error(tenant.id, id, error)', 'fulfillment detail mapper call'],
]) requireText(showRoute + orders, value, label);

const mapperUses =
  orders.match(/map_admin_order_error\(\s+AdminOrderErrorContext::new\(/g) ?? [];
if (mapperUses.length !== 6) {
  failures.push(`expected six context-aware admin order mapper callsites, found ${mapperUses.length}`);
}

for (const [value, label] of [
  ['Validation(String)', 'owner validation variant'],
  ['OrderNotFound(Uuid)', 'owner order-not-found variant'],
  ['OrderReturnNotFound(Uuid)', 'owner return-not-found variant'],
  ['OrderChangeNotFound(Uuid)', 'owner change-not-found variant'],
  ['InvalidTransition { from: String, to: String }', 'owner transition variant'],
  ['Database(#[from] DbErr)', 'owner database variant'],
  ['Core(#[from] rustok_core::Error)', 'owner core variant'],
]) requireText(orderErrors, value, label);

for (const value of [
  '.map_err(super::map_order_error)?;',
  'format!("Order request is invalid:',
  'error.to_string()',
  'err.to_string()',
  'other.to_string()',
  'HttpError::bad_request("commerce_operation_failed"',
]) forbidText(orders, value, 'unsafe admin order route public conversion');

if (failures.length > 0) {
  console.error('Commerce admin order route error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin order routes retain typed causes, route identities, and static public envelopes',
);
