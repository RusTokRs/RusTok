#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const adminRouter = read('crates/rustok-commerce/src/controllers/admin/mod.rs');
const controller = read('crates/rustok-commerce/src/controllers/admin/changes.rs');
const httpRuntime = read('crates/rustok-commerce/src/controllers/mod.rs');
const orchestration = read('crates/rustok-commerce/src/services/order_change_orchestration.rs');
const ownerCommand = read('crates/rustok-order/src/post_order_command.rs');
const ownerLib = read('crates/rustok-order/src/lib.rs');
const graphql = read('crates/rustok-commerce/src/graphql/mutations/fulfillment.rs');
const graphqlRuntime = read('crates/rustok-commerce/src/graphql_runtime.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-commerce/docs/rest-admin-order-change-apply-owner-port-cutover-2026-08-10.md',
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

const applyRoute = between(
  controller,
  'pub async fn apply_order_change(',
  '/// Cancel admin order change',
  'mounted REST apply route',
);
const portMapper = between(
  controller,
  'fn map_admin_order_change_port_error(',
  'fn map_admin_order_change_orchestration_error(',
  'REST owner-port mapper',
);
const refundContext = between(
  orchestration,
  'fn with_exchange_refund_context(',
  '/// Explicit compatibility seam for the still-open Payment half of exchange application.',
  'exchange refund metadata helper',
);
const compatRefund = between(
  orchestration,
  'async fn create_exchange_difference_refund_compat(',
  '/// Routes order-change application through the correct post-order workflow.',
  'exchange Payment compatibility helper',
);
const ownerMethod = between(
  orchestration,
  'pub async fn apply_order_change_with_owner_ports(',
  '\n    }\n}',
  'mounted owner-port orchestration method',
);

for (const [value, label] of [
  ['pub mod changes;', 'admin changes module mount'],
  ['"/order-changes/{id}/apply"', 'admin apply route'],
  ['axum::routing::post(changes::apply_order_change)', 'mounted apply handler'],
  ['axum::routing::post(post_order_commands::create_order_change)', 'create route mounted replacement'],
  ['axum::routing::get(post_order_reads::list_order_changes)', 'list route mounted replacement'],
  ['axum::routing::get(post_order_reads::show_order_change)', 'show route mounted replacement'],
  ['axum::routing::post(post_order_commands::cancel_order_change)', 'cancel route mounted replacement'],
]) requireText(adminRouter, value, label);

for (const [value, label] of [
  ['fn admin_order_change_read_context(', 'read context builder'],
  ['fn admin_order_change_apply_context(', 'write context builder'],
  ['PortActor::user(auth.user_id.to_string())', 'authenticated owner actor'],
  ['format!("commerce-admin-order-change:read:{change_id}")', 'read correlation identity'],
  ['format!("commerce-admin-order-change:apply:{change_id}")', 'write correlation identity'],
  ['.with_idempotency_key(Uuid::new_v4().to_string())', 'write admission identity'],
]) requireText(controller, value, label);

for (const [value, label] of [
  ['request_context: RequestContext,', 'request context extractor'],
  ['OrderChangeOrchestrationService::from_order_ports(', 'host-composed orchestration'],
  ['runtime.order_read_port()', 'host-selected Order read'],
  ['runtime.order_post_order_command_port()', 'host-selected Order command'],
  ['.apply_order_change_with_owner_ports(', 'REST owner-port entrypoint'],
  ['read_context.clone()', 'read context forwarding'],
  ['command_context.clone()', 'command context forwarding'],
  ['map_admin_order_change_apply_error(', 'typed error boundary'],
]) requireText(applyRoute, value, label);

for (const value of [
  'OrderService::new(',
  '.get_order_change(',
  '.apply_order_change(tenant.id, id,',
]) forbidText(applyRoute, value, 'mounted REST apply must not call concrete Order service');

for (const [value, label] of [
  ['fn order_read_port(', 'HTTP Order read accessor'],
  ['std::sync::Arc<dyn rustok_order::OrderReadPort>', 'HTTP Order read trait object'],
  ['fn order_post_order_command_port(', 'HTTP post-order command accessor'],
  ['std::sync::Arc<dyn rustok_order::OrderPostOrderCommandPort>', 'HTTP post-order trait object'],
]) requireText(httpRuntime, value, label);

for (const [value, label] of [
  ['fn with_order_change_apply_action(', 'mounted apply-action metadata helper'],
  ['pub async fn apply_order_change_with_owner_ports(', 'owner-port orchestration method'],
  ['.read_order_change_projection(', 'typed Order read'],
  ['ReadOrderChangeProjectionRequest { change_id }', 'typed read request'],
  ['.apply_change(', 'typed Order apply command'],
  ['ApplyOrderChangeRequest {', 'typed apply request'],
  ['OrderChangeOrchestrationError::OrderRead', 'read error preservation'],
  ['OrderChangeOrchestrationError::OrderCommand', 'command error preservation'],
  ['with_order_change_apply_action(metadata, "exchange")', 'exchange apply-action preservation'],
  ['create_exchange_difference_refund_compat(', 'explicit Payment compatibility seam'],
  ['with_order_change_apply_action(metadata, "claim")', 'claim apply-action preservation'],
]) requireText(orchestration, value, label);
for (const value of [
  'OrderService::new(',
  '.get_order_change(',
  'PostOrderOrchestrationService::new(',
  '.apply_exchange_order_change(',
  '.apply_claim_order_change(',
]) forbidText(ownerMethod, value, 'mounted owner-port orchestration concrete/re-entry dependency');
const mountedApplyCommands = ownerMethod.match(/\.apply_change\(/g) ?? [];
if (mountedApplyCommands.length < 3) {
  failures.push(
    `mounted owner-port orchestration: expected exchange, claim, and default Order apply commands, found ${mountedApplyCommands.length}`,
  );
}

for (const [value, label] of [
  ['PaymentService::new(db.clone())', 'remaining Payment collection compatibility'],
  ['status: Some("captured".to_string())', 'captured collection semantics'],
  ['order_id: Some(order_id)', 'order collection filter'],
  ['PaymentOrchestrationService::new(db.clone())', 'remaining Payment provider compatibility'],
  ['.create_refund(', 'difference refund creation'],
  ['Some("exchange_difference".to_string())', 'default difference-refund reason'],
]) requireText(compatRefund, value, label);
for (const [value, label] of [
  ['"order_change_id".to_string()', 'durable refund workflow identity'],
  ['Value::String("exchange".to_string())', 'refund apply-action identity'],
]) requireText(refundContext, value, label);
for (const value of ['OrderService::new(', '.apply_order_change(']) {
  forbidText(compatRefund, value, 'Payment compatibility helper must not own Order transition');
}

for (const [value, label] of [
  ['async fn apply_change(', 'Order owner apply capability'],
  ['_request: ApplyOrderChangeRequest', 'default external-adapter request'],
  ['context.require_policy(PortCallPolicy::write())?', 'default write admission'],
  ['"order.post_order_apply_change_unavailable"', 'default fail-closed capability'],
  ['request: ApplyOrderChangeRequest', 'in-process apply request'],
  ['.apply_order_change(tenant_id, request.change_id, request.input)', 'owner-local concrete execution'],
]) requireText(ownerCommand, value, label);
requireText(ownerLib, 'ApplyOrderChangeRequest', 'Order owner request export');

for (const [value, label] of [
  ['owner_error_kind = ?error.kind', 'bounded owner kind'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner code'],
  ['retryable = error.retryable', 'bounded retryability'],
  ['public_code = code', 'public code'],
  ['status = %status', 'public status'],
]) requireText(portMapper, value, label);
for (const value of ['error = ?error', 'error.message', 'internal_message', 'error.to_string()']) {
  forbidText(portMapper, value, 'REST owner-port raw diagnostics');
}

requireText(
  graphql,
  '.apply_order_change_with_owner_ports(',
  'GraphQL reuses the mounted owner-port orchestration entrypoint',
);
requireText(
  graphqlRuntime,
  'OrderChangeOrchestrationService::from_order_ports(',
  'GraphQL runtime composes the same host-selected Order owner ports',
);
forbidText(
  graphql,
  '.apply_order_change(tenant_id, id, difference_refund, metadata)',
  'GraphQL must not regress to the concrete compatibility entrypoint',
);

requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'broad ecommerce topology P0 remains open for Payment compatibility',
);

for (const [value, label] of [
  ['# Commerce REST admin order-change apply owner-port cutover', 'record title'],
  ['Status: `source_complete_unvalidated`', 'record status'],
  ['OrderPostOrderCommandPort::apply_change', 'record owner command'],
  ['write-admission metadata only', 'record replay limitation'],
  ['mounted GraphQL `applyOrderChange`', 'historical deferred GraphQL scope'],
  ['no tests, Cargo commands, Node verifiers, formatter', 'record validation status'],
]) requireText(record, value, label);

if (failures.length > 0) {
  console.error('Commerce admin order-change Order-owner verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ mounted REST/GraphQL order-change apply uses host-selected Order owner ports for exchange, claim, and default transitions; Payment difference-refund compatibility remains explicit',
);
