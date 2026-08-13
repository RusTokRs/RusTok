#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const mutationsMod = read('crates/rustok-commerce/src/graphql/mutations/mod.rs');
const graphql = read('crates/rustok-commerce/src/graphql/mutations/fulfillment.rs');
const graphqlRuntime = read('crates/rustok-commerce/src/graphql_runtime.rs');
const orchestration = read('crates/rustok-commerce/src/services/order_change_orchestration.rs');
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

const applyMutation = between(
  graphql,
  'async fn apply_order_change(',
  'async fn cancel_order_change(',
  'GraphQL applyOrderChange mutation',
);
const runtimeFactory = between(
  graphqlRuntime,
  'pub(crate) fn order_change_orchestration_from_context(',
  'pub(crate) fn return_completion_orchestration_from_context(',
  'GraphQL order-change runtime factory',
);
const ownerMethod = between(
  orchestration,
  'pub async fn apply_order_change_with_owner_ports(',
  '\n    }\n}',
  'mounted order-change owner orchestration',
);
const paymentCompat = between(
  orchestration,
  'async fn create_exchange_difference_refund_compat(',
  '/// Routes order-change application through the correct post-order workflow.',
  'explicit Payment compatibility seam',
);

requireText(mutationsMod, 'pub mod fulfillment;', 'mounted fulfillment mutation module');
for (const [value, label] of [
  ['[Permission::ORDERS_UPDATE]', 'orders:update admission'],
  ['current_tenant_scope(ctx, Some(tenant_id), "Apply order change")', 'tenant scope admission'],
  ['order_change_read_context(ctx, tenant_id, id)?', 'Order read context'],
  ['order_post_order_command_context(ctx, tenant_id, id, "apply_order_change")?', 'Order command context'],
  ['order_change_orchestration_from_context(ctx, db.clone(), event_bus.clone())', 'runtime orchestration factory'],
  ['.apply_order_change_with_owner_ports(', 'shared owner orchestration entrypoint'],
  ['read_context.clone()', 'read context forwarding'],
  ['command_context.clone()', 'command context forwarding'],
]) requireText(applyMutation, value, label);
for (const value of [
  '.apply_order_change(tenant_id, id, difference_refund, metadata)',
  'OrderService::new(',
  '.get_order_change(',
  'match order_change.change_type.as_str()',
  '.apply_exchange_order_change(',
  '.apply_claim_order_change(',
]) forbidText(applyMutation, value, 'mounted GraphQL concrete/dispatch dependency');

for (const [value, label] of [
  ['ctx.data_opt::<CommerceGraphqlRuntimeData>()', 'host runtime lookup'],
  ['Some(runtime) => crate::OrderChangeOrchestrationService::from_order_ports(', 'host-composed Order service'],
  ['runtime.order_read_runtime().order_read_port()', 'host-selected Order read'],
  ['runtime.order_post_order_command_runtime().command_port()', 'host-selected Order command'],
  ['None => crate::OrderChangeOrchestrationService::new(db, event_bus)', 'embedded compatibility fallback'],
  ['with_payment_provider_registry(payment_provider_registry_from_context(ctx))', 'Payment registry preservation'],
]) requireText(runtimeFactory, value, label);

for (const [value, label] of [
  ['.read_order_change_projection(', 'Order owner read'],
  ['.apply_change(', 'Order owner apply'],
  ['create_exchange_difference_refund_compat(', 'explicit remaining Payment seam'],
]) requireText(ownerMethod, value, label);
for (const value of [
  'OrderService::new(',
  '.get_order_change(',
  'PostOrderOrchestrationService::new(',
  '.apply_exchange_order_change(',
  '.apply_claim_order_change(',
]) forbidText(ownerMethod, value, 'mounted GraphQL Order concrete/re-entry dependency');
const applyCalls = ownerMethod.match(/\.apply_change\(/g) ?? [];
if (applyCalls.length < 3) {
  failures.push(`mounted GraphQL owner orchestration: expected exchange, claim, and default Order apply calls, found ${applyCalls.length}`);
}

for (const [value, label] of [
  ['PaymentService::new(db.clone())', 'remaining Payment collection compatibility'],
  ['PaymentOrchestrationService::new(db.clone())', 'remaining Payment provider compatibility'],
  ['status: Some("captured".to_string())', 'captured collection semantics'],
  ['order_id: Some(order_id)', 'order collection filter'],
  ['.create_refund(', 'difference refund execution'],
]) requireText(paymentCompat, value, label);
for (const value of ['OrderService::new(', '.apply_order_change(']) {
  forbidText(paymentCompat, value, 'Payment compatibility seam must not re-enter Order');
}

requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'broad ecommerce topology P0 remains open',
);

if (failures.length > 0) {
  console.error('Commerce GraphQL order-change owner-port verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ mounted GraphQL applyOrderChange uses host-selected Order owner ports; remaining Payment compatibility is explicit and cannot re-enter Order',
);
