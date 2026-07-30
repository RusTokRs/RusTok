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
  [graphqlRuntime, 'static CURRENT_COMMERCE_ORDER_READ_CALL_CONTEXT:', 'order request task-local context'],
  [graphqlRuntime, 'ctx.data_opt::<AuthContext>()', 'validated GraphQL actor source'],
  [graphqlRuntime, 'PortActor::user(auth.user_id.to_string())', 'authenticated user actor'],
  [graphqlRuntime, 'ctx.data_opt::<RequestContext>()', 'resolved GraphQL request context source'],
  [graphqlRuntime, 'request.channel_slug.clone()', 'resolved request channel slug'],
  [graphqlRuntime, 'request.locale.clone()', 'resolved request locale'],
  [graphqlRuntime, 'pub(crate) fn locale(&self) -> Option<&str>', 'scoped locale accessor'],
  [graphqlRuntime, 'runtime_data.order_read_runtime()', 'host-selected order runtime scope'],
  [graphqlRuntime, 'pub(crate) fn order_read_runtime_for_current_graphql_scope(', 'order runtime scope accessor'],
  [graphqlRuntime, 'pub(crate) fn order_read_call_context_for_current_graphql_scope()', 'order request context accessor'],
  [shim, 'order_reads: Arc<dyn OrderReadPort>', 'typed owner read dependency'],
  [shim, 'order_read_runtime_for_current_graphql_scope(', 'scoped runtime lookup'],
  [shim, 'order_read_call_context_for_current_graphql_scope()', 'scoped call context lookup'],
  [shim, 'call_context.actor()', 'PortContext actor propagation'],
  [shim, 'call_context.locale()', 'PortContext locale propagation'],
  [shim, 'context.with_channel(channel)', 'PortContext channel propagation'],
  [shim, '.read_order_projection(', 'order detail owner port call'],
  [shim, '.list_order_projections(', 'order list owner port call'],
  [shim, '.read_order_return_projection(', 'return detail owner port call'],
  [shim, '.list_order_return_projections(', 'return list owner port call'],
  [shim, '.read_order_change_projection(', 'change detail owner port call'],
  [shim, '.list_order_change_projections(', 'change list owner port call'],
  [shim, 'ReadOrderReturnProjectionRequest { return_id }', 'return detail typed request'],
  [shim, 'ListOrderReturnProjectionsRequest {', 'return list typed request'],
  [shim, 'ReadOrderChangeProjectionRequest { change_id }', 'change detail typed request'],
  [shim, 'ListOrderChangeProjectionsRequest {', 'change list typed request'],
  [shim, 'GraphqlOrderReadResource::Return(return_id)', 'return not-found resource mapping'],
  [shim, 'GraphqlOrderReadResource::Change(change_id)', 'change not-found resource mapping'],
  [shim, 'OrderError::OrderReturnNotFound(id)', 'return not-found compatibility error'],
  [shim, 'OrderError::OrderChangeNotFound(id)', 'change not-found compatibility error'],
  [shim, '.with_deadline(std::time::Duration::from_secs(2))', 'read deadline'],
  [query, 'use rustok_order::OrderService;', 'legacy included source import'],
  [note, 'Status: complete-and-post-order-reads-cut-over, unvalidated.', 'checkpoint status'],
  [note, 'validated `AuthContext`', 'authenticated actor note'],
  [note, '`RequestContext.channel_slug`', 'resolved channel note'],
  [note, 'effective locale', 'resolved locale note'],
  [note, 'stores only `Arc<dyn OrderReadPort>`', 'no concrete service storage note'],
]) requireText(value, text, label);

for (const [text, label] of [
  ['CommerceOrderReadRuntime::in_process(', 'shim-local runtime construction'],
  ['PortActor::service(', 'shim-local actor construction'],
  ['db: DatabaseConnection,\n    event_bus:', 'stored database/event-bus dependencies'],
  ['fn concrete_owner_service(&self)', 'concrete compatibility factory'],
  ['::rustok_order::OrderService::new', 'concrete owner construction'],
  ['.get_order_with_locale_fallback(tenant_id, order_id', 'concrete order detail delegation'],
  ['.list_orders_with_locale_fallback(tenant_id, input', 'concrete order list delegation'],
  ['.get_return(tenant_id, return_id)', 'concrete return detail delegation'],
  ['.list_returns(tenant_id, input)', 'concrete return list delegation'],
  ['.get_order_change(tenant_id, change_id)', 'concrete change detail delegation'],
  ['.list_order_changes(tenant_id, input)', 'concrete change list delegation'],
]) forbidText(shim, text, label);

if (failures.length > 0) {
  console.error('Commerce GraphQL order read shim verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL complete order, return, and order-change reads use the scoped host-selected owner port with validated actor/channel/locale context and no concrete owner service storage',
);
