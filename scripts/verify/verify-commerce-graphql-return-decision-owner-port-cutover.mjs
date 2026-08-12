#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const graphql = read('crates/rustok-commerce/src/graphql/mutations/fulfillment.rs');
const graphqlRuntime = read('crates/rustok-commerce/src/graphql_runtime.rs');
const ownerDecision = read(
  'crates/rustok-commerce/src/services/return_decision_owner_orchestration.rs',
);
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
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

const mutation = between(
  graphql,
  '    async fn create_order_return_decision(',
  '    async fn complete_order_return(',
  'GraphQL return-decision mutation',
);
const paymentMapper = between(
  graphql,
  'fn payment_owner_graphql_error(',
  'fn order_owner_graphql_error(',
  'GraphQL Payment owner mapper',
);
const returnDecisionMapper = between(
  graphql,
  'fn return_decision_graphql_error(',
  'fn order_change_graphql_error(',
  'GraphQL return-decision mapper',
);
const runtimeHelper = between(
  graphqlRuntime,
  'pub(crate) fn return_decision_owner_orchestration_from_context(',
  'pub(crate) fn order_change_orchestration_from_context(',
  'GraphQL return-decision runtime helper',
);

for (const [value, label] of [
  ['return_decision_owner_orchestration_from_context(', 'owner orchestration selection'],
  ['order_post_order_command_context(', 'typed owner context'],
  ['.create_return_decision(', 'owner-backed decision call'],
  ['context.clone(),', 'bounded context passed to owner orchestration'],
  ['ReturnDecisionOwnerOrchestrationError', 'typed owner orchestration error'],
  ['return_decision_graphql_error(', 'bounded GraphQL owner error mapping'],
]) requireText(mutation, value, label);
for (const value of [
  'post_order_orchestration_from_context(',
  'PostOrderOrchestrationService::new(',
  'PaymentService::new(',
  'OrderService::new(',
]) forbidText(mutation, value, 'mounted GraphQL return-decision direct dependency');

for (const [value, label] of [
  ['runtime.order_post_order_command_runtime().command_port()', 'host-selected Order command port'],
  ['runtime.payment_read_runtime().admin_read_port()', 'host-selected Payment admin-read port'],
  ['ReturnDecisionOwnerOrchestrationService::new(', 'owner orchestration construction'],
  ['with_payment_provider_registry(payment_provider_registry_from_context(ctx))', 'host-selected Payment provider registry'],
  ['OrderPostOrderCommandRuntime::in_process(', 'embedded-schema Order compatibility fallback'],
  ['CommercePaymentReadRuntime::in_process(', 'embedded-schema Payment compatibility fallback'],
]) requireText(runtimeHelper, value, label);

for (const [value, label] of [
  ['owner = "rustok_payment.admin_read"', 'Payment owner diagnostic identity'],
  ['owner_operation = "list_payment_collection_projections"', 'Payment owner operation'],
  ['consumer_operation = "create_order_return_decision"', 'GraphQL consumer operation'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner code diagnostic'],
  ['public_code = code', 'public code diagnostic'],
  ['retryable', 'retryability diagnostic'],
]) requireText(paymentMapper, value, label);
for (const value of ['error = ?error', 'error.message', 'error.to_string()', 'internal_message']) {
  forbidText(paymentMapper, value, 'Payment owner raw diagnostics');
}

for (const [value, label] of [
  ['ReturnDecisionOwnerOrchestrationError::OrderCommand(source)', 'Order owner error branch'],
  ['post_order_owner_graphql_error(', 'Order owner bounded mapping'],
  ['ReturnDecisionOwnerOrchestrationError::PaymentRead(source)', 'Payment owner read error branch'],
  ['payment_owner_graphql_error(', 'Payment owner bounded mapping'],
  ['ReturnDecisionOwnerOrchestrationError::PostOrder(source)', 'preserved Payment execution/validation branch'],
  ['post_order_graphql_error(', 'preserved legacy bounded public mapping'],
]) requireText(returnDecisionMapper, value, label);

for (const [value, label] of [
  ['Arc<dyn OrderPostOrderCommandPort>', 'Order owner dependency'],
  ['Arc<dyn PaymentAdminReadPort>', 'Payment owner read dependency'],
  ['PaymentOrchestrationService::new(self.db.clone())', 'preserved Payment execution orchestration'],
]) requireText(ownerDecision, value, label);

requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'broad ecommerce topology P0 remains open',
);

if (failures.length > 0) {
  console.error('Commerce GraphQL return-decision owner-port verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ mounted GraphQL return decision uses host-selected Order command and Payment read owner ports',
);
