#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const returns = read('crates/rustok-commerce/src/controllers/admin/returns.rs');
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

const orderPolicy = between(
  returns,
  'fn admin_order_error_policy(',
  'fn admin_payment_error_policy(',
  'admin order policy',
);
const mapper = between(
  returns,
  'fn map_admin_order_return_error(',
  'fn map_admin_order_return_orchestration_error(',
  'admin order-return owner mapper',
);
const createRoute = between(
  returns,
  'pub async fn create_order_return(',
  '#[utoipa::path(\n    post,\n    path = "/admin/orders/{id}/returns/decision"',
  'create return route',
);
const listRoute = between(
  returns,
  'pub async fn list_order_returns(',
  '#[utoipa::path(\n    get,\n    path = "/admin/returns/{id}"',
  'list returns route',
);
const showRoute = between(
  returns,
  'pub async fn show_order_return(',
  '#[utoipa::path(\n    post,\n    path = "/admin/returns/{id}/complete"',
  'show return route',
);
const cancelStart = returns.indexOf('pub async fn cancel_order_return(');
const cancelRoute = cancelStart < 0 ? '' : returns.slice(cancelStart);
if (cancelStart < 0) failures.push('cancel return route: unable to isolate source block');

for (const [value, label] of [
  ['use rustok_order::error::OrderError;', 'typed order error import'],
  ['use rustok_web::{HttpError, HttpResult};', 'typed HTTP error import'],
  ['const ADMIN_ORDER_RETURN_OWNER: &str = "rustok_order.admin_returns";', 'owner constant'],
  [
    'const ADMIN_ORDER_RETURN_BOUNDARY: &str = "commerce_admin_order_return_http";',
    'HTTP boundary constant',
  ],
  ['struct AdminOrderReturnErrorContext {', 'owner error context'],
  ['tenant_id: Uuid,', 'tenant context field'],
  ['order_id: Option<Uuid>,', 'truthful order identity field'],
  ['return_id: Option<Uuid>,', 'truthful return identity field'],
  ["operation: &'static str,", 'operation field'],
]) requireText(returns, value, label);

for (const [value, label] of [
  ['OrderError::Validation(_)', 'validation variant'],
  ['OrderError::OrderNotFound(_)', 'order-not-found variant'],
  ['OrderError::OrderReturnNotFound(_)', 'return-not-found variant'],
  ['OrderError::OrderChangeNotFound(_)', 'change-not-found variant'],
  ['OrderError::InvalidTransition { .. }', 'transition variant'],
  ['OrderError::Database(_)', 'database variant'],
  ['OrderError::Core(_)', 'core variant'],
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
  ['StatusCode::BAD_REQUEST', 'validation status'],
  ['StatusCode::NOT_FOUND', 'not-found status'],
  ['StatusCode::CONFLICT', 'conflict status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'storage status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'fail-closed status'],
]) requireText(orderPolicy, value, label);

for (const [value, label] of [
  ['error: OrderError,', 'owned typed cause'],
  [') -> HttpError {', 'typed HTTP mapper result'],
  ['admin_order_error_policy(&error)', 'shared typed order policy'],
  ['error = ?error', 'typed internal cause'],
  ['owner = ADMIN_ORDER_RETURN_OWNER', 'owner log'],
  ['tenant_id = %context.tenant_id', 'tenant log'],
  ['order_id = ?context.order_id', 'order identity log'],
  ['return_id = ?context.return_id', 'return identity log'],
  ['operation = %context.operation', 'operation log'],
  ['error_kind,', 'error-kind log'],
  ['public_code = code', 'public-code log'],
  ['status = %status', 'status log'],
  ['boundary = ADMIN_ORDER_RETURN_BOUNDARY', 'boundary log'],
  ['HttpError::new(status, code, message)', 'single static envelope constructor'],
]) requireText(mapper, value, label);

for (const [block, operation, identity, label] of [
  [createRoute, '"create_return"', 'Some(id), None', 'create return context'],
  [listRoute, '"list_returns"', 'tenant.id, order_id, None', 'list return context'],
  [showRoute, '"get_return"', 'None, Some(id)', 'show return context'],
  [cancelRoute, '"cancel_return"', 'None, Some(id)', 'cancel return context'],
]) {
  requireText(block, '.map_err(|error| {', `${label} typed mapping closure`);
  requireText(block, 'map_admin_order_return_error(', `${label} mapper handoff`);
  requireText(block, operation, `${label} operation`);
  requireText(block, identity, `${label} identity`);
}

for (const [value, label] of [
  ['pub async fn create_order_return(', 'create handler'],
  ['pub async fn create_order_return_decision(', 'decision handler'],
  ['pub async fn list_order_returns(', 'list handler'],
  ['pub async fn show_order_return(', 'detail handler'],
  ['pub async fn complete_order_return(', 'complete handler'],
  ['pub async fn cancel_order_return(', 'cancel handler'],
  ['[Permission::ORDERS_READ]', 'read permission'],
  ['[Permission::ORDERS_UPDATE]', 'update permission'],
  ['[Permission::PAYMENTS_UPDATE]', 'payment permission'],
  ['page: pagination.page', 'page forwarding'],
  ['per_page: pagination.limit()', 'page-size forwarding'],
  ['let order_id = params.order_id;', 'list order filter capture'],
  ['.create_return(tenant.id, id, input)', 'create service contract'],
  ['.list_returns(', 'list service contract'],
  ['.get_return(tenant.id, id)', 'detail service contract'],
  ['.cancel_return(tenant.id, id, input)', 'cancel service contract'],
  ['fn map_admin_order_return_orchestration_error(', 'local orchestration mapper'],
]) requireText(returns, value, label);

const mapperUses = returns.match(/map_admin_order_return_error\(/g) ?? [];
if (mapperUses.length !== 5) {
  failures.push(`expected mapper definition plus four uses, found ${mapperUses.length}`);
}
const orchestrationMapperUses =
  returns.match(
    /map_admin_order_return_orchestration_error\(\s+AdminOrderReturnOrchestrationErrorContext::new\(/g,
  ) ?? [];
if (orchestrationMapperUses.length !== 2) {
  failures.push(
    `expected two context-aware orchestration mapper callsites, found ${orchestrationMapperUses.length}`,
  );
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
  '.map_err(super::map_post_order_orchestration_error)?;',
  'error.to_string()',
  'err.to_string()',
  'other.to_string()',
  'HttpError::bad_request("commerce_operation_failed"',
]) forbidText(returns, value, 'unsafe admin return public conversion');

if (failures.length > 0) {
  console.error('Commerce admin order-return owner error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin order-return owner errors retain route context and static public envelopes',
);
