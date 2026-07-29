#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/rustok-commerce/src/graphql/safe_query/source.rs');
const shim = read(
  'crates/rustok-commerce/src/graphql/safe_query/source/rustok_order_shim.rs',
);
const query = read('crates/rustok-commerce/src/graphql/query.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
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
  [shim, 'order_reads: Arc<dyn OrderReadPort>', 'typed owner read dependency'],
  [shim, 'CommerceOrderReadRuntime::in_process(', 'explicit compatibility runtime'],
  [shim, '.read_order_projection(', 'detail owner port call'],
  [shim, '.list_order_projections(', 'list owner port call'],
  [shim, 'ReadOrderProjectionRequest {', 'detail typed request'],
  [shim, 'ListOrderProjectionsRequest {', 'list typed request'],
  [shim, '.with_deadline(std::time::Duration::from_secs(2))', 'read deadline'],
  [shim, 'self.inner.get_order_change(', 'unchanged order-change delegate'],
  [shim, 'self.inner.list_order_changes(', 'unchanged order-change list delegate'],
  [shim, 'self.inner.get_return(', 'unchanged return delegate'],
  [shim, 'self.inner.list_returns(', 'unchanged return list delegate'],
  [query, 'use rustok_order::OrderService;', 'legacy included source import'],
  [plan, 'GraphQL safe-query order detail/list compatibility facade', 'master-plan checkpoint'],
]) requireText(value, text, label);

for (const [text, label] of [
  ['self.inner.get_order_with_locale_fallback(', 'concrete detail delegation'],
  ['self.inner.list_orders_with_locale_fallback(', 'concrete list delegation'],
]) forbidText(shim, text, label);

if (failures.length > 0) {
  console.error('Commerce GraphQL order read shim verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL safe-query order detail/list reads use the typed owner port through an explicit in-process compatibility facade; host-selected runtime scoping remains open',
);
