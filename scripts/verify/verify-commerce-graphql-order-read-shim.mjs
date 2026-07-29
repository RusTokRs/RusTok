#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const graphqlRuntime = read('crates/rustok-commerce/src/graphql_runtime.rs');
const source = read('crates/rustok-commerce/src/graphql/safe_query/source.rs');
const shim = read(
  'crates/rustok-commerce/src/graphql/safe_query/source/rustok_order_shim.rs',
);
const query = read('crates/rustok-commerce/src/graphql/query.rs');
const note = read('crates/rustok-commerce/docs/graphql-order-read-shim.md');
const failures = [];

const requireText = (value, text, label) => {
  if (!value.includes(text)) failures.push(`${label}: missing ${text}`);
};
const forbidText = (value, text, label) => {
  if (value.includes(text)) failures.push(`${label}: forbidden ${text}`);
};

for (const [value, text, label] of [
  [source, 'mod rustok_order_shim;', 'safe-query order shim module'],
  [source, 'use self::rustok_order_shim as rustok_order;', 'safe-query order alias'],
  [shim, 'pub(crate) mod dto {', 'order DTO passthrough'],
  [graphqlRuntime, 'static CURRENT_COMMERCE_ORDER_READ_RUNTIME:', 'order task-local runtime'],
  [graphqlRuntime, 'runtime_data.order_read_runtime()', 'host-selected order runtime scope'],
  [graphqlRuntime, 'pub(crate) fn order_read_runtime_for_current_graphql_scope(', 'order runtime scope accessor'],
  [shim, 'order_reads: Arc<dyn OrderReadPort>', 'typed owner read dependency'],
  [shim, 'order_read_runtime_for_current_graphql_scope(', 'scoped runtime lookup'],
  [shim, '.read_order_projection(', 'detail owner port call'],
  [shim, '.list_order_projections(', 'list owner port call'],
  [shim, 'ReadOrderProjectionRequest {', 'detail typed request'],
  [shim, 'ListOrderProjectionsRequest {', 'list typed request'],
  [shim, '.with_deadline(std::time::Duration::from_secs(2))', 'read deadline'],
  [shim, 'fn concrete_owner_service(&self)', 'deferred concrete compatibility factory'],
  [shim, '.get_order_change(tenant_id, change_id)', 'unchanged order-change delegate'],
  [shim, '.list_order_changes(tenant_id, input)', 'unchanged order-change list delegate'],
  [shim, '.get_return(tenant_id, return_id)', 'unchanged return delegate'],
  [shim, '.list_returns(tenant_id, input)', 'unchanged return list delegate'],
  [query, 'use rustok_order::OrderService;', 'legacy included source import'],
  [note, 'Status: host-runtime-scoped, unvalidated.', 'checkpoint status'],
  [note, 'Directly embedded schemas', 'embedded compatibility fallback'],
]) requireText(value, text, label);

for (const [text, label] of [
  ['CommerceOrderReadRuntime::in_process(', 'shim-local runtime construction'],
  ['.get_order_with_locale_fallback(tenant_id, order_id', 'concrete detail delegation'],
  ['.list_orders_with_locale_fallback(tenant_id, input', 'concrete list delegation'],
]) forbidText(shim, text, label);

if (failures.length > 0) {
  console.error('Commerce GraphQL order read shim verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL safe-query order detail/list reads use the host-selected typed owner runtime; embedded schemas retain an explicit in-process fallback',
);
