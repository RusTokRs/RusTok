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
const ownerCommands = read('crates/rustok-order/src/post_order_command.rs');
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

const legacyMapper = between(
  changes,
  'fn map_admin_order_change_error(',
  'fn map_admin_order_change_port_error(',
  'compatibility owner mapper',
);
const portMapper = between(
  changes,
  'fn map_admin_order_change_port_error(',
  'fn map_admin_order_change_orchestration_error(',
  'mounted owner-port mapper',
);
const applyRoute = between(
  changes,
  'pub async fn apply_order_change(',
  '/// Cancel admin order change',
  'mounted apply route',
);

for (const [value, label] of [
  ['use rustok_order::error::OrderError;', 'compatibility Order error import'],
  ['PortError, PortErrorKind, RequestContext', 'mounted owner-port imports'],
  ['fn admin_order_change_order_error_policy(', 'compatibility Order policy'],
  ['fn admin_order_change_port_error_policy(', 'owner-port policy'],
  ['const ADMIN_ORDER_CHANGE_BOUNDARY: &str = "commerce_admin_order_change_http";', 'HTTP boundary'],
]) requireText(changes, value, label);

for (const [value, label] of [
  ['error: OrderError,', 'typed compatibility cause'],
  ['OrderError::OrderNotFound(id)', 'compatibility order identity adoption'],
  ['OrderError::OrderChangeNotFound(id)', 'compatibility change identity adoption'],
  ['error = ?error', 'compatibility typed internal cause'],
  ['owner = ADMIN_ORDER_CHANGE_OWNER', 'compatibility owner diagnostic'],
  ['HttpError::new(status, code, message)', 'compatibility stable envelope'],
]) requireText(legacyMapper, value, label);

for (const [value, label] of [
  ['error: PortError,', 'typed owner-port cause'],
  ['owner = "rustok_order"', 'owner-port owner diagnostic'],
  ['owner_operation,', 'owner operation diagnostic'],
  ['correlation_id = %context.correlation_id', 'correlation diagnostic'],
  ['owner_error_kind = ?error.kind', 'bounded owner kind'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner code'],
  ['retryable = error.retryable', 'retryability diagnostic'],
  ['HttpError::new(status, code, message)', 'owner-port stable envelope'],
]) requireText(portMapper, value, label);
for (const value of ['error = ?error', 'error.message', 'internal_message', 'error.to_string()']) {
  forbidText(portMapper, value, 'owner-port raw diagnostic');
}

for (const [value, label] of [
  ['request_context: RequestContext,', 'request context extractor'],
  ['[Permission::ORDERS_UPDATE]', 'update permission'],
  ['admin_order_change_read_context(&tenant, &auth, &request_context, id)', 'read context'],
  ['admin_order_change_apply_context(&tenant, &auth, &request_context, id)', 'command context'],
  ['OrderChangeOrchestrationService::from_order_ports(', 'host-composed orchestration'],
  ['runtime.order_read_port()', 'host-selected read'],
  ['runtime.order_post_order_command_port()', 'host-selected command'],
  ['.apply_order_change_with_owner_ports(', 'mounted owner-port apply'],
  ['map_admin_order_change_apply_error(', 'typed apply error handoff'],
]) requireText(applyRoute, value, label);

for (const [value, label] of [
  ['pub struct ApplyOrderChangeRequest', 'owner apply request'],
  ['async fn apply_change(', 'owner apply capability'],
  ['"order.post_order_apply_change_unavailable"', 'external adapter fail-closed default'],
  ['context.require_policy(PortCallPolicy::write())?', 'write admission'],
  ['.apply_order_change(tenant_id, request.change_id, request.input)', 'in-process owner execution'],
]) requireText(ownerCommands, value, label);

for (const [value, label] of [
  ['Validation(String)', 'owner validation variant'],
  ['OrderNotFound(Uuid)', 'owner order-not-found variant'],
  ['OrderReturnNotFound(Uuid)', 'owner return-not-found variant'],
  ['OrderChangeNotFound(Uuid)', 'owner change-not-found variant'],
  ['InvalidTransition { from: String, to: String }', 'owner transition variant'],
  ['Database(#[from] DbErr)', 'owner database variant'],
  ['Core(#[from] rustok_core::Error)', 'owner core variant'],
]) requireText(orderErrors, value, label);

// Compatibility create/list/show/cancel implementations remain compiled in this file,
// but admin/mod.rs mounts their post_order_commands/post_order_reads replacements instead.
for (const value of [
  '.create_order_change(tenant.id, actor_id, id, input)',
  '.list_order_changes(',
  '.get_order_change(tenant.id, id)',
  '.cancel_order_change(tenant.id, id, input)',
]) requireText(changes, value, 'compiled compatibility OrderService path');

if (failures.length > 0) {
  console.error('Commerce admin order-change owner error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ mounted order-change apply uses bounded owner-port errors while compatibility handlers remain explicit',
);
