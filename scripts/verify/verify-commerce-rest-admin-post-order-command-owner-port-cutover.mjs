#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const router = read('crates/rustok-commerce/src/controllers/admin/mod.rs');
const commands = read('crates/rustok-commerce/src/controllers/admin/post_order_commands.rs');
const httpRuntime = read('crates/rustok-commerce/src/controllers/mod.rs');
const owner = read('crates/rustok-order/src/post_order_command.rs');
const server = read('apps/server/src/services/commerce_provider_runtime.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-commerce/docs/rest-admin-post-order-command-owner-port-cutover-2026-08-09.md',
);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['pub mod post_order_commands;', 'mounted post-order command module'],
  ['axum::routing::post(post_order_commands::create_order_change)', 'create change route'],
  ['axum::routing::post(post_order_commands::cancel_order_change)', 'cancel change route'],
  ['axum::routing::post(post_order_commands::create_order_return)', 'create return route'],
  ['axum::routing::post(post_order_commands::cancel_order_return)', 'cancel return route'],
  ['axum::routing::post(changes::apply_order_change)', 'apply orchestration retained'],
  ['axum::routing::post(returns::create_order_return_decision)', 'return decision orchestration retained'],
  ['axum::routing::post(returns::complete_order_return)', 'return completion orchestration retained'],
  ['axum::routing::get(post_order_reads::list_order_changes)', 'change reads remain owner-read mounted'],
  ['axum::routing::get(post_order_reads::list_order_returns)', 'return reads remain owner-read mounted'],
]) requireText(router, value, label);

for (const value of [
  'axum::routing::post(changes::create_order_change)',
  'axum::routing::post(changes::cancel_order_change)',
  'axum::routing::post(returns::create_order_return)',
  'axum::routing::post(returns::cancel_order_return)',
]) forbidText(router, value, 'stale mounted direct Order handler');

for (const [value, label] of [
  ['AuthContext, Permission, PortActor, PortContext, PortError, PortErrorKind, RequestContext,', 'typed request/port imports'],
  ['OwnerCreateOrderChangeRequest', 'owner create-change request'],
  ['OwnerCancelOrderChangeRequest', 'owner cancel-change request'],
  ['OwnerCreateOrderReturnRequest', 'owner create-return request'],
  ['OwnerCancelOrderReturnRequest', 'owner cancel-return request'],
  ['admin_post_order_command_context(', 'shared owner context builder'],
  ['PortActor::user(auth.user_id.to_string())', 'authenticated actor'],
  ['request_context.locale.as_str()', 'request locale'],
  ['request_context.channel_slug.as_deref()', 'request channel'],
  ['with_idempotency_key(Uuid::new_v4().to_string())', 'write admission identity'],
  ['with_deadline(std::time::Duration::from_secs(2))', 'bounded deadline'],
  ['[Permission::ORDERS_UPDATE]', 'order update admission'],
  ['.order_post_order_command_port()', 'host-selected owner port accessor'],
  ['.create_change(', 'create-change owner call'],
  ['.cancel_change(', 'cancel-change owner call'],
  ['.create_return(', 'create-return owner call'],
  ['.cancel_return(', 'cancel-return owner call'],
  ['StatusCode::CREATED', 'create success status'],
]) requireText(commands, value, label);

for (const value of [
  'OrderService::new(',
  '.create_order_change(tenant.id,',
  '.cancel_order_change(tenant.id,',
  '.create_return(tenant.id,',
  '.cancel_return(tenant.id,',
]) forbidText(commands, value, 'concrete Order service construction');

for (const [value, label] of [
  ['PortErrorKind::Validation', 'validation mapping'],
  ['PortErrorKind::NotFound', 'not-found mapping'],
  ['PortErrorKind::Conflict', 'conflict mapping'],
  ['PortErrorKind::Forbidden', 'forbidden mapping'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'unavailable mapping'],
  ['PortErrorKind::InvariantViolation', 'invariant mapping'],
  ['"commerce_admin_order_invalid"', 'validation public code'],
  ['"commerce_admin_not_found"', 'not-found public code'],
  ['"commerce_admin_order_state_conflict"', 'conflict public code'],
  ['"commerce_permission_denied"', 'forbidden public code'],
  ['"commerce_admin_order_storage_unavailable"', 'storage public code'],
  ['"commerce_admin_order_failed"', 'fail-closed public code'],
  ['correlation_id = %context.correlation_id', 'correlation diagnostic'],
  ['owner_error_kind = ?error.kind', 'owner error-kind diagnostic'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner-code diagnostic'],
  ['retryable = error.retryable', 'retryability diagnostic'],
]) requireText(commands, value, label);

for (const value of [
  'error = ?error',
  'error.message',
  'error.to_string()',
  'err.to_string()',
]) forbidText(commands, value, 'raw owner diagnostic');

for (const [value, label] of [
  ['order_post_order_command_runtime: rustok_order::OrderPostOrderCommandRuntime', 'HTTP runtime field'],
  ['fn order_post_order_command_port(', 'HTTP owner accessor'],
  ['std::sync::Arc<dyn rustok_order::OrderPostOrderCommandPort>', 'HTTP trait object'],
  ['self.order_post_order_command_runtime.command_port()', 'HTTP selected command port'],
  ['shared_get::<rustok_order::OrderPostOrderCommandRuntime>()', 'HTTP host runtime requirement'],
  ['"Commerce HTTP routes require OrderPostOrderCommandRuntime in HostRuntimeContext"', 'HTTP fail-closed host requirement'],
]) requireText(httpRuntime, value, label);

for (const [value, label] of [
  ['shared_get::<rustok_order::OrderPostOrderCommandRuntime>()', 'server preserves host runtime'],
  ['server.shared_get::<rustok_order::OrderPostOrderCommandRuntime>()', 'server preserves server runtime'],
  ['rustok_order::OrderPostOrderCommandRuntime::in_process(', 'server deterministic baseline'],
  ['host.with_shared_value(runtime)', 'server attaches runtime to host'],
]) requireText(server, value, label);

for (const [value, label] of [
  ['pub trait OrderPostOrderCommandPort: Send + Sync', 'owner trait'],
  ['async fn create_change(', 'owner create change capability'],
  ['async fn cancel_change(', 'owner cancel change capability'],
  ['async fn create_return(', 'owner create return capability'],
  ['async fn cancel_return(', 'owner cancel return capability'],
  ['context.require_policy(PortCallPolicy::write())', 'owner write admission'],
  ['inner: OrderService', 'concrete service retained inside owner'],
]) requireText(owner, value, label);

requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'broad ecommerce topology item remains open',
);

for (const [value, label] of [
  ['# Commerce REST admin post-order command owner-port cutover', 'record title'],
  ['Status: `source_complete_unvalidated`', 'record status'],
  ['`POST /admin/orders/{id}/changes`', 'record create-change route'],
  ['`POST /admin/returns/{id}/cancel`', 'record cancel-return route'],
  ['write-admission metadata only', 'record replay limitation'],
  ['The canonical broad item', 'record broad topology status'],
  ['no tests, Cargo commands, Node verifiers, formatter', 'record validation status'],
]) requireText(record, value, label);

if (failures.length > 0) {
  console.error('Commerce REST admin post-order owner command cutover verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ mounted admin post-order create/cancel writes use the host-selected Order owner port');
