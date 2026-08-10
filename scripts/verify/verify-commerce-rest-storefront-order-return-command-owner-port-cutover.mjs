#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const controller = read('crates/rustok-commerce/src/controllers/store/orders.rs');
const httpRuntime = read('crates/rustok-commerce/src/controllers/mod.rs');
const owner = read('crates/rustok-order/src/post_order_command.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-commerce/docs/rest-storefront-order-return-command-owner-port-cutover-2026-08-10.md',
);
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

for (const [value, label] of [
  ['CreateOrderReturnRequest', 'owner create-return request import'],
  ['fn storefront_order_return_command_context(', 'storefront write context builder'],
  ['PortActor::user(auth.user_id.to_string())', 'authenticated actor'],
  ['format!("commerce-storefront-order:create-return:{order_id}")', 'operation correlation'],
  ['.with_deadline(std::time::Duration::from_secs(2))', 'bounded deadline'],
  ['.with_idempotency_key(Uuid::new_v4().to_string())', 'write-admission identity'],
  ['let customer_id = ensure_customer_owns_order(', 'ownership admission retained'],
  ['"create_order_return_access"', 'ownership operation label'],
  ['.order_post_order_command_port()', 'host-selected Order command port'],
  ['.create_return(', 'owner create-return call'],
  ['CreateOrderReturnRequest {', 'typed owner request construction'],
  ['order_id: id', 'order id forwarding'],
  ['input,', 'input forwarding'],
  ['StatusCode::CREATED', 'created response status'],
  ['fn map_storefront_order_command_port_error(', 'bounded command mapper'],
]) requireText(controller, value, label);

for (const value of [
  'OrderService::new(',
  'error::OrderError',
  'map_storefront_order_error(',
  '.create_return(tenant.id, id, input)',
]) forbidText(controller, value, 'mounted storefront Order return command must not construct concrete Order service');

const accessIndex = controller.indexOf('let customer_id = ensure_customer_owns_order(');
const commandIndex = controller.indexOf('.order_post_order_command_port()');
if (accessIndex < 0 || commandIndex < 0 || accessIndex > commandIndex) {
  failures.push('storefront return ownership admission must happen before the owner command call');
}

const commandMapper = between(
  controller,
  'fn map_storefront_order_command_port_error(',
  'fn storefront_order_payment_error_policy(',
  'storefront Order command mapper',
);
for (const [value, label] of [
  ['PortErrorKind::Validation', 'validation mapping'],
  ['PortErrorKind::NotFound', 'not-found mapping'],
  ['PortErrorKind::Conflict', 'conflict mapping'],
  ['PortErrorKind::Forbidden', 'forbidden mapping'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'unavailable mapping'],
  ['PortErrorKind::InvariantViolation', 'invariant mapping'],
  ['"commerce_store_order_invalid"', 'validation public code'],
  ['"commerce_store_order_not_found"', 'not-found public code'],
  ['"commerce_store_order_state_conflict"', 'conflict public code'],
  ['"commerce_store_order_access_denied"', 'forbidden public code'],
  ['"commerce_store_order_unavailable"', 'unavailable public code'],
  ['"commerce_store_order_failed"', 'fail-closed public code'],
  ['owner_error_kind = ?error.kind', 'bounded owner kind diagnostic'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner code diagnostic'],
  ['retryable = error.retryable', 'retryability diagnostic'],
]) requireText(commandMapper, value, label);
for (const value of [
  'error = ?error',
  'error.message',
  'error.to_string()',
  'internal_message',
]) forbidText(commandMapper, value, 'storefront Order command diagnostics must stay bounded');

for (const [value, label] of [
  ['fn order_post_order_command_port(', 'HTTP runtime accessor'],
  ['std::sync::Arc<dyn rustok_order::OrderPostOrderCommandPort>', 'HTTP command trait object'],
  ['self.order_post_order_command_runtime.command_port()', 'HTTP selected owner runtime'],
  ['shared_get::<rustok_order::OrderPostOrderCommandRuntime>()', 'HTTP host runtime requirement'],
]) requireText(httpRuntime, value, label);

for (const [value, label] of [
  ['pub trait OrderPostOrderCommandPort: Send + Sync', 'Order owner command trait'],
  ['async fn create_return(', 'Order owner return capability'],
  ['request: CreateOrderReturnRequest', 'Order owner request type'],
  ['context.require_policy(PortCallPolicy::write())', 'owner write policy'],
  ['inner: OrderService', 'concrete Order service retained inside owner'],
  ['.create_return(tenant_id, request.order_id, request.input)', 'owner-local execution'],
]) requireText(owner, value, label);

requireText(
  controller,
  'let payment_service = PaymentService::new(runtime.db_clone());',
  'separate mounted Payment refund-list gap remains explicit',
);
requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'broad ecommerce topology P0 remains open',
);

for (const [value, label] of [
  ['# Commerce REST storefront order-return command owner-port cutover', 'record title'],
  ['Status: `source_complete_unvalidated`', 'record status'],
  ['OrderPostOrderCommandPort::create_return', 'record owner operation'],
  ['write-admission metadata only', 'record replay limitation'],
  ['`GET /store/orders/{id}/refunds` still constructs `PaymentService` directly', 'record remaining Payment gap'],
  ['no tests, Cargo commands, Node verifiers, formatter', 'record no validation execution'],
]) requireText(record, value, label);

if (failures.length > 0) {
  console.error('Commerce REST storefront order-return owner command cutover verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ mounted storefront order-return creation uses the host-selected Order owner command port');
