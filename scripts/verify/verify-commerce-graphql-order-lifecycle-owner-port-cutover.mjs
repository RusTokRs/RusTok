#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const paths = {
  mutations: 'crates/rustok-commerce/src/graphql/mutations/fulfillment.rs',
  graphqlRuntime: 'crates/rustok-commerce/src/graphql_runtime.rs',
  orderOwner: 'crates/rustok-order/src/admin_command.rs',
  hostRuntime: 'apps/server/src/services/commerce_provider_runtime.rs',
  plan: 'crates/rustok-commerce/docs/implementation-plan.md',
  document: 'crates/rustok-commerce/docs/graphql-order-lifecycle-owner-port-cutover-2026-08-09.md',
};

const mutations = read(paths.mutations);
const graphqlRuntime = read(paths.graphqlRuntime);
const orderOwner = read(paths.orderOwner);
const hostRuntime = read(paths.hostRuntime);
const plan = read(paths.plan);
const document = read(paths.document);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const sliceFunction = (name, nextName) => {
  const start = mutations.indexOf(`    async fn ${name}(`);
  const end = mutations.indexOf(`    async fn ${nextName}(`, start + 1);
  if (start < 0 || end <= start) {
    failures.push(`${paths.mutations}: invalid function boundary ${name} -> ${nextName}`);
    return '';
  }
  return mutations.slice(start, end);
};

for (const marker of [
  'order_admin_command_runtime_from_context',
  'OwnerMarkOrderPaidRequest',
  'OwnerShipOrderRequest',
  'OwnerDeliverOrderRequest',
  'OwnerCancelOrderRequest',
  'PortActor::user(auth.user_id.to_string())',
  '.with_idempotency_key(format!("graphql-order:{order_id}:{operation}"))',
  '.with_deadline(std::time::Duration::from_secs(2))',
  'request.channel_slug.as_deref()',
  'boundary = "commerce_graphql_order_command"',
  'owner_code_length = error.code.chars().count()',
  '"ORDER_REQUEST_INVALID"',
  '"ORDER_RESOURCE_NOT_FOUND"',
  '"ORDER_STATE_CONFLICT"',
  '"ORDER_TEMPORARILY_UNAVAILABLE"',
  '"ORDER_OPERATION_FAILED"',
]) requireText(mutations, marker, `${paths.mutations}: mounted Order owner command contract`);

const lifecycleSlices = [
  ['mark_order_paid', 'ship_order', '.mark_paid(', 'OwnerMarkOrderPaidRequest'],
  ['ship_order', 'deliver_order', '.ship(', 'OwnerShipOrderRequest'],
  ['deliver_order', 'cancel_order', '.deliver(', 'OwnerDeliverOrderRequest'],
  ['cancel_order', 'create_order_change', '.cancel(', 'OwnerCancelOrderRequest'],
];
for (const [name, nextName, ownerCall, requestType] of lifecycleSlices) {
  const source = sliceFunction(name, nextName);
  for (const marker of [
    'order_admin_command_runtime_from_context',
    '.command_port()',
    ownerCall,
    requestType,
    'order_command_context(',
    'order_owner_graphql_error(',
  ]) requireText(source, marker, `${paths.mutations}: ${name} owner cutover`);
  forbidText(source, 'OrderService::new', `${paths.mutations}: ${name} concrete OrderService`);
}

for (const forbidden of [
  'owner_message = %error.message',
  'message = %error.message',
  'error = ?error',
]) {
  const start = mutations.indexOf('fn order_owner_graphql_error(');
  const end = mutations.indexOf('fn post_order_graphql_error(', start);
  if (start >= 0 && end > start) {
    forbidText(
      mutations.slice(start, end),
      forbidden,
      `${paths.mutations}: bounded Order diagnostics`,
    );
  }
}

for (const marker of [
  'use rustok_order::{OrderAdminCommandRuntime, OrderReadPort, in_process_order_read_port};',
  'order_admin_command_runtime: OrderAdminCommandRuntime',
  'pub fn order_admin_command_runtime(&self) -> OrderAdminCommandRuntime',
  '.shared_get::<OrderAdminCommandRuntime>()',
  'commerce GraphQL requires OrderAdminCommandRuntime in host composition',
  'pub(crate) fn order_admin_command_runtime_from_context(',
  '.map(CommerceGraphqlRuntimeData::order_admin_command_runtime)',
  'OrderAdminCommandRuntime::in_process(db, event_bus)',
]) requireText(graphqlRuntime, marker, `${paths.graphqlRuntime}: Order command runtime composition`);

for (const marker of [
  'pub trait OrderAdminCommandPort: Send + Sync',
  'async fn mark_paid(',
  'async fn ship(',
  'async fn deliver(',
  'async fn cancel(',
  'pub struct OrderAdminCommandRuntime',
  'pub fn command_port(&self) -> Arc<dyn OrderAdminCommandPort>',
]) requireText(orderOwner, marker, `${paths.orderOwner}: Order owner command capability`);

for (const marker of [
  '.shared_get::<rustok_order::OrderAdminCommandRuntime>()',
  'rustok_order::OrderAdminCommandRuntime::in_process(',
  'host.with_shared_value(runtime)',
]) requireText(hostRuntime, marker, `${paths.hostRuntime}: host-composed Order command runtime`);

requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  `${paths.plan}: broad topology item remains open`,
);

for (const marker of [
  '# Commerce GraphQL order lifecycle owner-port cutover',
  'Status: `source_complete_unvalidated`',
  '`markOrderPaid`',
  '`shipOrder`',
  '`deliverOrder`',
  '`cancelOrder`',
  '`OrderAdminCommandPort`',
  'The broad canonical topology item remains open.',
  'no tests, Cargo commands, Node verifiers, formatter, mounted GraphQL scenarios, workflows, CI reruns, runtime calls, database scenarios, restart scenarios, or remote-adapter scenarios were executed',
]) requireText(document, marker, `${paths.document}: truthful source record`);

for (const marker of [
  'create_storefront_order_return',
  'create_order_change',
  'cancel_order_change',
  'create_order_return',
  'cancel_order_return',
  'OrderService::new',
]) requireText(mutations, marker, `${paths.mutations}: explicitly open post-order owner work`);

if (failures.length > 0) {
  console.error('Commerce GraphQL order lifecycle owner-port cutover verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('commerce GraphQL order lifecycle commands route through the host-selected Order owner port');
