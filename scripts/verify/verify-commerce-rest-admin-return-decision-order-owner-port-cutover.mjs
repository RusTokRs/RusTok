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
const returns = read('crates/rustok-commerce/src/controllers/admin/returns.rs');
const ownerDecision = read(
  'crates/rustok-commerce/src/services/return_decision_owner_orchestration.rs',
);
const servicesMod = read('crates/rustok-commerce/src/services/mod.rs');
const commerceLib = read('crates/rustok-commerce/src/lib.rs');
const ownerCommand = read('crates/rustok-order/src/post_order_command.rs');
const ownerLib = read('crates/rustok-order/src/lib.rs');
const graphql = read('crates/rustok-commerce/src/graphql/mutations/fulfillment.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-commerce/docs/rest-admin-return-decision-order-owner-port-cutover-2026-08-10.md',
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

const decisionRoute = between(
  returns,
  'pub async fn create_order_return_decision(',
  '#[utoipa::path(\n    get,\n    path = "/admin/returns"',
  'mounted REST return-decision route',
);
const ownerPortMapper = between(
  returns,
  'fn map_admin_return_decision_order_port_error(',
  'fn map_admin_order_return_orchestration_error(',
  'REST return-decision Order owner mapper',
);
const ownerDecisionMethod = between(
  ownerDecision,
  'pub async fn create_return_decision(',
  '    async fn complete_return_decision(',
  'owner-backed return-decision method',
);

for (const [value, label] of [
  ['pub mod returns;', 'admin returns module mount'],
  ['"/orders/{id}/returns/decision"', 'admin decision route'],
  ['axum::routing::post(returns::create_order_return_decision)', 'mounted decision handler'],
  ['axum::routing::post(post_order_commands::create_order_return)', 'mounted create replacement'],
  ['axum::routing::get(post_order_reads::list_order_returns)', 'mounted list replacement'],
  ['axum::routing::get(post_order_reads::show_order_return)', 'mounted show replacement'],
  ['axum::routing::post(post_order_commands::cancel_order_return)', 'mounted cancel replacement'],
]) requireText(adminRouter, value, label);

for (const [value, label] of [
  ['request_context: RequestContext,', 'request context extractor'],
  ['[Permission::ORDERS_UPDATE]', 'orders:update admission'],
  ['super::decision_requires_payments_update(', 'conditional payment admission'],
  ['[Permission::PAYMENTS_UPDATE]', 'payments:update admission'],
  ['admin_return_decision_order_context(&tenant, &auth, &request_context, id)', 'owner base context'],
  ['PortActor::user(auth.user_id.to_string())', 'authenticated Order actor'],
  ['format!("commerce-admin-return-decision:{order_id}")', 'root correlation identity'],
  ['.with_idempotency_key(Uuid::new_v4().to_string())', 'root write admission identity'],
  ['.with_deadline(std::time::Duration::from_secs(2))', 'bounded deadline'],
  ['request_context.channel_slug.as_deref()', 'request channel forwarding'],
  ['ReturnDecisionOwnerOrchestrationService::new(', 'owner-backed orchestration'],
  ['runtime.order_post_order_command_port()', 'host-selected Order command port'],
  ['.create_return_decision(context.clone(), tenant.id, id, input)', 'owner-backed decision call'],
  ['ReturnDecisionOwnerOrchestrationError::OrderCommand(error)', 'Order owner error branch'],
  ['map_admin_return_decision_order_port_error(', 'bounded owner mapper'],
  ['ReturnDecisionOwnerOrchestrationError::PostOrder(error)', 'preserved Payment/validation branch'],
  ['map_admin_order_return_orchestration_error(', 'preserved post-order mapper'],
]) requireText(decisionRoute, value, label);
for (const value of [
  'PostOrderOrchestrationService::new(',
  'OrderService::new(',
  '.create_return(tenant.id, id,',
  '.create_order_change(',
  '.complete_return(tenant.id,',
]) forbidText(decisionRoute, value, 'mounted REST decision concrete Order dependency');

for (const [value, label] of [
  ['owner = "rustok_order.post_order_command"', 'owner diagnostic label'],
  ['consumer_operation = "create_return_decision"', 'consumer operation'],
  ['correlation_id = %context.correlation_id', 'correlation diagnostic'],
  ['tenant_id_non_nil = !tenant_id.is_nil()', 'tenant diagnostic'],
  ['actor_id_non_nil = !actor_id.is_nil()', 'actor diagnostic'],
  ['order_id_non_nil = !order_id.is_nil()', 'order diagnostic'],
  ['owner_error_kind = ?error.kind', 'bounded owner kind'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner code'],
  ['retryable = error.retryable', 'owner retryability'],
  ['public_code = code', 'public code'],
  ['status = %status', 'public status'],
]) requireText(ownerPortMapper, value, label);
for (const value of ['error = ?error', 'error.message', 'error.to_string()', 'internal_message']) {
  forbidText(ownerPortMapper, value, 'REST Order owner raw diagnostics');
}

for (const [value, label] of [
  ['Arc<dyn OrderPostOrderCommandPort>', 'Order owner dependency'],
  ['base_context.tenant_id != tenant_id.to_string()', 'tenant context admission'],
  ['input.validate()', 'legacy input validation'],
  ['normalize_decision_action(&input.decision.action)', 'legacy action normalization'],
  ['validate_decision_shape(&action, &input.decision)', 'legacy action shape validation'],
  ['.create_return(', 'typed Order return creation'],
  ['CreateOrderReturnRequest {', 'typed create-return request'],
  ['.create_change(', 'typed Order change creation'],
  ['CreateOrderChangeRequest {', 'typed create-change request'],
  ['.complete_return_decision(', 'typed completion helper'],
  ['"return_only"', 'return-only branch'],
  ['"refund"', 'refund branch'],
  ['"exchange"', 'exchange branch'],
  ['"claim"', 'claim branch'],
]) requireText(ownerDecisionMethod, value, label);
for (const value of ['OrderService::new(', '.create_order_change(', '.complete_return(tenant_id,']) {
  forbidText(ownerDecisionMethod, value, 'owner-backed return-decision concrete Order dependency');
}

for (const [value, label] of [
  ['async fn complete_return(', 'Order complete-return capability'],
  ['_request: CompleteOrderReturnRequest', 'default external-adapter request'],
  ['context.require_policy(PortCallPolicy::write())?', 'default write admission'],
  ['"order.post_order_complete_return_unavailable"', 'default fail-closed capability'],
  ['request: CompleteOrderReturnRequest', 'in-process completion request'],
  ['.complete_return(tenant_id, request.return_id, request.input)', 'owner-local completion execution'],
]) requireText(ownerCommand, value, label);
requireText(ownerLib, 'CompleteOrderReturnRequest', 'Order completion request export');

for (const [value, label] of [
  ['mod return_decision_owner_orchestration;', 'Commerce service module'],
  ['ReturnDecisionOwnerOrchestrationService', 'Commerce service export'],
]) requireText(servicesMod, value, label);
for (const value of [
  'ReturnDecisionOwnerOrchestrationError',
  'ReturnDecisionOwnerOrchestrationResult',
  'ReturnDecisionOwnerOrchestrationService',
]) requireText(commerceLib, value, 'Commerce root export');

// These gaps are deliberately outside this REST Order-only slice.
requireText(
  graphql,
  '.create_return_decision(',
  'mounted GraphQL return-decision compatibility call remains',
);
requireText(
  ownerDecision,
  'PaymentService::new(self.db.clone())',
  'mounted Payment collection lookup remains explicit',
);

requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'broad ecommerce topology P0 remains open',
);

for (const [value, label] of [
  ['# Commerce REST admin return-decision Order owner-port cutover', 'record title'],
  ['Status: `source_complete_unvalidated`', 'record status'],
  ['OrderPostOrderCommandPort', 'record owner port'],
  ['CompleteOrderReturnRequest', 'record completion capability'],
  ['write-admission metadata only', 'record replay limitation'],
  ['PaymentService', 'record deferred Payment gap'],
  ['GraphQL `createOrderReturnDecision`', 'record deferred GraphQL gap'],
  ['broad topology P0', 'record broad invariant'],
  ['No tests, Cargo commands, Node verifiers, formatter', 'record validation state'],
]) requireText(record, value, label);

if (failures.length > 0) {
  console.error('Commerce REST admin return-decision Order owner-port verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ mounted REST admin return decision uses the host-selected Order command owner port');
