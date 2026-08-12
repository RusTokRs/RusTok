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
const commerceRuntime = read('crates/rustok-commerce/src/controllers/mod.rs');
const ownerCommand = read('crates/rustok-order/src/post_order_command.rs');
const paymentAdminRead = read('crates/rustok-payment/src/admin_read.rs');
const graphql = read('crates/rustok-commerce/src/graphql/mutations/fulfillment.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-commerce/docs/rest-admin-return-decision-payment-owner-read-cutover-2026-08-12.md',
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
const orderPortMapper = between(
  returns,
  'fn map_admin_return_decision_order_port_error(',
  'fn map_admin_return_decision_payment_port_error(',
  'REST return-decision Order owner mapper',
);
const paymentPortMapper = between(
  returns,
  'fn map_admin_return_decision_payment_port_error(',
  'fn map_admin_order_return_orchestration_error(',
  'REST return-decision Payment owner mapper',
);
const ownerDecisionMethod = between(
  ownerDecision,
  'pub async fn create_return_decision(',
  '    async fn complete_return_decision(',
  'owner-backed return-decision method',
);
const refundMethod = between(
  ownerDecision,
  '    async fn create_refund_for_return(',
  '\n}\n\nfn payment_read_context_for(',
  'owner-backed refund method',
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
  ['runtime.order_post_order_command_port()', 'host-selected Order command port'],
  ['runtime.payment_admin_read_port()', 'host-selected Payment read port'],
  ['.create_return_decision(context.clone(), tenant.id, id, input)', 'owner-backed decision call'],
  ['ReturnDecisionOwnerOrchestrationError::OrderCommand(error)', 'Order owner error branch'],
  ['map_admin_return_decision_order_port_error(', 'bounded Order owner mapper'],
  ['ReturnDecisionOwnerOrchestrationError::PaymentRead(error)', 'Payment owner error branch'],
  ['map_admin_return_decision_payment_port_error(', 'bounded Payment owner mapper'],
  ['ReturnDecisionOwnerOrchestrationError::PostOrder(error)', 'preserved Payment execution/validation branch'],
]) requireText(decisionRoute, value, label);
for (const value of [
  'PostOrderOrchestrationService::new(',
  'OrderService::new(',
  'PaymentService::new(',
]) forbidText(decisionRoute, value, 'mounted REST decision direct owner dependency');

for (const [source, values, label] of [
  [orderPortMapper, [
    'owner = "rustok_order.post_order_command"',
    'consumer_operation = "create_return_decision"',
    'owner_code_length = error.code.chars().count()',
    'retryable = error.retryable',
  ], 'Order owner bounded diagnostics'],
  [paymentPortMapper, [
    'owner = "rustok_payment.admin_read"',
    'owner_operation = "list_payment_collection_projections"',
    'consumer_operation = "create_return_decision"',
    'correlation_id = %context.correlation_id',
    'tenant_id_non_nil = !tenant_id.is_nil()',
    'actor_id_non_nil = !actor_id.is_nil()',
    'order_id_non_nil = !order_id.is_nil()',
    'owner_code_length = error.code.chars().count()',
    'retryable = error.retryable',
    'public_code = code',
    'status = %status',
  ], 'Payment owner bounded diagnostics'],
]) {
  for (const value of values) requireText(source, value, label);
}
for (const source of [orderPortMapper, paymentPortMapper]) {
  for (const value of ['error = ?error', 'error.message', 'error.to_string()', 'internal_message']) {
    forbidText(source, value, 'owner raw diagnostics');
  }
}

for (const [value, label] of [
  ['Arc<dyn OrderPostOrderCommandPort>', 'Order owner dependency'],
  ['Arc<dyn PaymentAdminReadPort>', 'Payment owner read dependency'],
  ['PaymentRead(PortError)', 'typed Payment read error'],
  ['base_context.tenant_id != tenant_id.to_string()', 'tenant context admission'],
  ['input.validate()', 'legacy input validation'],
  ['normalize_decision_action(&input.decision.action)', 'legacy action normalization'],
  ['validate_decision_shape(&action, &input.decision)', 'legacy action shape validation'],
  ['.create_return(', 'typed Order return creation'],
  ['.create_change(', 'typed Order change creation'],
  ['.complete_return_decision(', 'typed completion helper'],
  ['"return_only"', 'return-only branch'],
  ['"refund"', 'refund branch'],
  ['"exchange"', 'exchange branch'],
  ['"claim"', 'claim branch'],
]) requireText(ownerDecisionMethod, value, label);

for (const [value, label] of [
  ['payment_read_context_for(base_context, "list_captured_collections", order_id)', 'Payment read context'],
  ['.list_payment_collection_projections(', 'Payment owner read call'],
  ['ListPaymentCollectionProjectionsRequest {', 'Payment owner read request'],
  ['page: 1', 'legacy page'],
  ['per_page: 1', 'legacy page size'],
  ['status: Some("captured".to_string())', 'legacy captured filter'],
  ['order_id: Some(order_id)', 'legacy order filter'],
  ['cart_id: None', 'legacy cart filter'],
  ['customer_id: None', 'legacy customer filter'],
  ['PaymentOrchestrationService::new(self.db.clone())', 'preserved Payment execution orchestration'],
]) requireText(refundMethod, value, label);
for (const value of ['PaymentService::new(', 'ListPaymentCollectionsInput']) {
  forbidText(ownerDecision, value, 'return-decision direct Payment lookup');
}
requireText(ownerDecision, 'context.idempotency_key = None;', 'read-only context strips write idempotency');

for (const [value, label] of [
  ['payment_admin_read_runtime: rustok_payment::PaymentAdminReadRuntime', 'Commerce Payment admin read runtime field'],
  ['fn payment_admin_read_port(', 'Commerce Payment admin read accessor'],
  ['.shared_get::<rustok_payment::PaymentAdminReadRuntime>()', 'Commerce host Payment admin read selection'],
]) requireText(commerceRuntime, value, label);

for (const [value, label] of [
  ['pub trait PaymentAdminReadPort', 'Payment admin read capability'],
  ['async fn list_payment_collection_projections(', 'Payment collection list capability'],
  ['ListPaymentCollectionsInput {', 'Payment owner legacy-compatible list projection'],
  ['PaymentService::new(db)', 'Payment concrete service stays inside owner adapter'],
]) requireText(paymentAdminRead, value, label);

for (const [value, label] of [
  ['async fn complete_return(', 'Order complete-return capability'],
  ['context.require_policy(PortCallPolicy::write())?', 'Order write admission'],
]) requireText(ownerCommand, value, label);

// These gaps remain deliberately outside this bounded REST read cutover.
requireText(
  graphql,
  '.create_return_decision(',
  'mounted GraphQL return-decision compatibility call remains',
);
requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'broad ecommerce topology P0 remains open',
);

for (const [value, label] of [
  ['# Commerce REST admin return-decision Payment owner-read cutover', 'record title'],
  ['Status: `source_complete_validation_pending`', 'record validation status'],
  ['PaymentAdminReadPort', 'record Payment owner boundary'],
  ['list_payment_collection_projections', 'record Payment read operation'],
  ['PaymentOrchestrationService', 'record preserved execution boundary'],
  ['broad ecommerce topology P0', 'record broad invariant'],
]) requireText(record, value, label);

if (failures.length > 0) {
  console.error('Commerce REST admin return-decision owner-port verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ mounted REST admin return decision uses host-selected Order command and Payment read owner ports',
);
