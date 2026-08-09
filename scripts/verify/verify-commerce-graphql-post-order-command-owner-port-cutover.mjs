#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const read = (path) => readFileSync(path, 'utf8');
const expect = (condition, message) => {
  if (!condition) throw new Error(message);
};

const mutationPath = 'crates/rustok-commerce/src/graphql/mutations/fulfillment.rs';
const runtimePath = 'crates/rustok-commerce/src/graphql_runtime.rs';
const hostPath = 'apps/server/src/services/commerce_provider_runtime.rs';
const ownerPath = 'crates/rustok-order/src/post_order_command.rs';
const planPath = 'crates/rustok-commerce/docs/implementation-plan.md';
const recordPath =
  'crates/rustok-commerce/docs/graphql-post-order-command-owner-port-cutover-2026-08-09.md';

const mutations = read(mutationPath);
const runtime = read(runtimePath);
const host = read(hostPath);
const owner = read(ownerPath);
const plan = read(planPath);
const record = read(recordPath);

const body = (name) => {
  const start = mutations.indexOf(`async fn ${name}(`);
  expect(start >= 0, `missing GraphQL mutation ${name}`);
  const next = mutations.indexOf('\n    async fn ', start + 1);
  return mutations.slice(start, next >= 0 ? next : mutations.length);
};

expect(owner.includes('pub trait OrderPostOrderCommandPort'), 'owner post-order port is missing');
expect(owner.includes('pub struct OrderPostOrderCommandRuntime'), 'owner post-order runtime is missing');
expect(
  runtime.includes('order_post_order_command_runtime: OrderPostOrderCommandRuntime'),
  'Commerce GraphQL runtime data does not carry the post-order runtime',
);
expect(
  runtime.includes('shared_get::<OrderPostOrderCommandRuntime>()'),
  'mounted schema does not require host-selected post-order runtime',
);
expect(
  runtime.includes('order_post_order_command_runtime_from_context'),
  'GraphQL compatibility runtime helper is missing',
);
expect(
  host.includes('shared_get::<rustok_order::OrderPostOrderCommandRuntime>()'),
  'server host does not preserve a supplied post-order runtime',
);
expect(
  host.includes('rustok_order::OrderPostOrderCommandRuntime::in_process'),
  'server host does not compose the Order-owned in-process fallback',
);

const storefrontCreate = body('create_storefront_order_return');
const createChange = body('create_order_change');
const cancelChange = body('cancel_order_change');
const createReturn = body('create_order_return');
const cancelReturn = body('cancel_order_return');

for (const [name, source] of [
  ['create_storefront_order_return', storefrontCreate],
  ['create_order_change', createChange],
  ['cancel_order_change', cancelChange],
  ['create_order_return', createReturn],
  ['cancel_order_return', cancelReturn],
]) {
  expect(
    source.includes('order_post_order_command_runtime_from_context'),
    `${name} does not resolve the owner runtime`,
  );
  expect(!source.includes('OrderService::new'), `${name} still constructs OrderService`);
  expect(
    source.includes('post_order_owner_graphql_error'),
    `${name} does not use the bounded owner error envelope`,
  );
}

expect(storefrontCreate.includes('ensure_storefront_order_access'), 'storefront ownership gate was removed');
expect(storefrontCreate.includes('.create_return('), 'storefront return is not routed through owner create_return');
expect(createChange.includes('.create_change('), 'order change create is not routed through owner port');
expect(cancelChange.includes('.cancel_change('), 'order change cancel is not routed through owner port');
expect(createReturn.includes('.create_return('), 'order return create is not routed through owner port');
expect(cancelReturn.includes('.cancel_return('), 'order return cancel is not routed through owner port');

expect(
  body('apply_order_change').includes('order_change_orchestration_from_context'),
  'cross-domain order-change orchestration was unexpectedly removed',
);
expect(
  body('create_order_return_decision').includes('post_order_orchestration_from_context'),
  'return-decision orchestration was unexpectedly removed',
);
expect(
  body('complete_order_return').includes('return_completion_orchestration_from_context'),
  'return-completion orchestration was unexpectedly removed',
);

expect(
  mutations.includes('owner_code_length = error.code.chars().count()'),
  'bounded owner diagnostic code-length marker is missing',
);
expect(
  !mutations.includes('owner_message = %error.message'),
  'raw owner message leaked into GraphQL diagnostics',
);
expect(
  plan.includes('- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,'),
  'broad ecommerce topology P0 was closed prematurely',
);
expect(record.includes('Status: `source_complete_unvalidated`'), 'source record status is not truthful');
expect(record.includes('no tests, Cargo commands, Node verifiers'), 'source record does not state validation was skipped');

console.log('commerce GraphQL post-order owner-port source guard passed');
