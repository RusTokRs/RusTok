#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const changes = read('crates/rustok-commerce/src/controllers/admin/changes.rs');
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
  changes,
  'fn map_admin_order_change_error(',
  '/// Create admin order change preview',
  'admin order-change owner mapper',
);
const createRoute = between(
  changes,
  'pub async fn create_order_change(',
  '/// List admin order changes',
  'create order change route',
);
const listRoute = between(
  changes,
  'pub async fn list_order_changes(',
  '/// Show admin order change',
  'list order changes route',
);
const showRoute = between(
  changes,
  'pub async fn show_order_change(',
  '#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]',
  'show order change route',
);
const applyRoute = between(
  changes,
  'pub async fn apply_order_change(',
  '/// Cancel admin order change',
  'apply order change route',
);
const cancelStart = changes.indexOf('pub async fn cancel_order_change(');
const cancelRoute = cancelStart < 0 ? '' : changes.slice(cancelStart);
if (cancelStart < 0) failures.push('cancel order change route: unable to isolate source block');

for (const [value, label] of [
  ['use rustok_order::error::OrderError;', 'typed order error import'],
  ['use rustok_web::{HttpError, HttpResult};', 'typed HTTP error import'],
  ['const ADMIN_ORDER_CHANGE_OWNER: &str = "rustok_order.admin_changes";', 'owner constant'],
  [
    'const ADMIN_ORDER_CHANGE_BOUNDARY: &str = "commerce_admin_order_change_http";',
    'HTTP boundary constant',
  ],
  ['struct AdminOrderChangeErrorContext {', 'owner error context'],
  ['tenant_id: Uuid,', 'tenant field'],
  ['order_id: Option<Uuid>,', 'order identity field'],
  ['order_change_id: Option<Uuid>,', 'change identity field'],
  ["operation: &'static str,", 'operation field'],
]) requireText(changes, value, label);

for (const [value, label] of [
  ['mut context: AdminOrderChangeErrorContext,', 'mutable typed context'],
  ['error: OrderError,', 'owned typed cause'],
  ['OrderError::Validation(_)', 'validation variant'],
  ['OrderError::OrderNotFound(id)', 'order not-found variant'],
  ['OrderError::OrderReturnNotFound(_)', 'return not-found variant'],
  ['OrderError::OrderChangeNotFound(id)', 'change not-found variant'],
  ['OrderError::InvalidTransition { .. }', 'transition variant'],
  ['OrderError::Database(_)', 'database variant'],
  ['OrderError::Core(_)', 'core variant'],
  ['context.order_id = Some(*id);', 'typed order identity adoption'],
  ['context.order_change_id = Some(*id);', 'typed change identity adoption'],
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
  ['error = ?error', 'typed internal cause'],
  ['owner = ADMIN_ORDER_CHANGE_OWNER', 'owner log'],
  ['tenant_id = %context.tenant_id', 'tenant log'],
  ['order_id = ?context.order_id', 'order identity log'],
  ['order_change_id = ?context.order_change_id', 'change identity log'],
  ['operation = %context.operation', 'operation log'],
  ['error_kind,', 'error-kind log'],
  ['public_code = code', 'public-code log'],
  ['status = %status', 'status log'],
  ['boundary = ADMIN_ORDER_CHANGE_BOUNDARY', 'boundary log'],
  ['HttpError::new(status, code, message)', 'single static envelope constructor'],
]) requireText(mapper, value, label);

for (const [block, operation, identity, serviceCall, label] of [
  [
    createRoute,
    '"create_order_change"',
    'tenant.id,\n                    Some(id),\n                    None,',
    '.create_order_change(tenant.id, actor_id, id, input)',
    'create route',
  ],
  [
    listRoute,
    '"list_order_changes"',
    'tenant.id,\n                    order_id,\n                    None,',
    '.list_order_changes(',
    'list route',
  ],
  [
    showRoute,
    '"get_order_change"',
    'tenant.id,\n                    None,\n                    Some(id),',
    '.get_order_change(tenant.id, id)',
    'show route',
  ],
  [
    cancelRoute,
    '"cancel_order_change"',
    'tenant.id,\n                    None,\n                    Some(id),',
    '.cancel_order_change(tenant.id, id, input)',
    'cancel route',
  ],
]) {
  requireText(block, '.map_err(|error| {', `${label} typed mapping closure`);
  requireText(block, 'map_admin_order_change_error(', `${label} mapper handoff`);
  requireText(block, 'AdminOrderChangeErrorContext::new(', `${label} context construction`);
  requireText(block, operation, `${label} operation`);
  requireText(block, identity, `${label} truthful route identity`);
  requireText(block, serviceCall, `${label} service contract`);
}

for (const [value, label] of [
  ['[Permission::ORDERS_READ]', 'read permission'],
  ['[Permission::ORDERS_UPDATE]', 'update permission'],
  ['let actor_id = auth.user_id;', 'create actor capture'],
  ['let order_id = params.order_id;', 'list order filter capture'],
  ['page: pagination.page', 'pagination page forwarding'],
  ['per_page: pagination.limit()', 'pagination size forwarding'],
  ['order_id,', 'list order filter forwarding'],
  ['status: params.status', 'list status forwarding'],
  ['change_type: params.change_type', 'list type forwarding'],
]) requireText(changes, value, label);

requireText(
  applyRoute,
  '.map_err(super::map_post_order_orchestration_error)?;',
  'unchanged apply orchestration mapping',
);
requireText(
  applyRoute,
  '.apply_order_change(tenant.id, id, input.difference_refund, input.metadata)',
  'apply service contract',
);

const ownerMapperUses =
  changes.match(
    /map_admin_order_change_error\(\s+AdminOrderChangeErrorContext::new\(/g,
  ) ?? [];
if (ownerMapperUses.length !== 4) {
  failures.push(`expected four context-aware order-change owner mapper callsites, found ${ownerMapperUses.length}`);
}
const orchestrationMapperUses =
  changes.match(/\.map_err\(super::map_post_order_orchestration_error\)\?;/g) ?? [];
if (orchestrationMapperUses.length !== 1) {
  failures.push(`expected one unchanged order-change orchestration mapper callsite, found ${orchestrationMapperUses.length}`);
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
]) forbidText(changes, value, 'unsafe admin order-change owner public conversion');

if (failures.length > 0) {
  console.error('Commerce admin order-change owner error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin order-change owner errors retain route context and static public envelopes',
);
