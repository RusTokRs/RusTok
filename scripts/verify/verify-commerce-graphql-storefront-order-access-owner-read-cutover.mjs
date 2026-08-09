#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const read = (path) => readFileSync(path, 'utf8');
const failures = [];
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

const helper = read('crates/rustok-commerce/src/graphql/mutations/safe_order_helpers.rs');
const runtime = read('crates/rustok-commerce/src/graphql_runtime.rs');
const owner = read('crates/rustok-order/src/order_read.rs');
const orderService = read('crates/rustok-order/src/services/order.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-commerce/docs/graphql-storefront-order-access-owner-read-cutover-2026-08-09.md',
);

for (const marker of [
  'TenantContext',
  'use rustok_order::ReadOrderProjectionRequest;',
  'order_read_runtime_for_current_graphql_scope(',
  '.order_read_port()',
  '.read_order_projection(',
  'ReadOrderProjectionRequest {',
  'tenant_default_locale: None',
  'tenant.default_locale.as_str()',
  'PortActor::user(auth.user_id.to_string())',
  '.with_deadline(std::time::Duration::from_secs(2))',
  'request.channel_slug.as_deref()',
  'order.customer_id != Some(customer_id)',
  'owner_error_kind = ?error.kind',
  'owner_code_length = error.code.chars().count()',
  'ORDER_RESOURCE_NOT_FOUND',
  'ORDER_TEMPORARILY_UNAVAILABLE',
]) need(helper, marker, 'storefront order access helper');

for (const marker of [
  'OrderService::new(db.clone(), event_bus.clone())',
  'use rustok_order::{OrderError, OrderService};',
]) forbid(helper, marker, 'storefront order access concrete path');

for (const marker of [
  'pub struct CommerceOrderReadRuntime',
  'pub(crate) fn order_read_runtime_for_current_graphql_scope(',
  'CURRENT_COMMERCE_ORDER_READ_RUNTIME',
]) need(runtime, marker, 'commerce host-selected order runtime');

for (const marker of [
  'pub trait OrderReadPort',
  'async fn read_order_projection(',
  'pub struct ReadOrderProjectionRequest',
  'context.require_policy(PortCallPolicy::read())?',
]) need(owner, marker, 'order owner read contract');

for (const marker of [
  'pub async fn get_order(&self, tenant_id: Uuid, order_id: Uuid)',
  'let default_locale = load_tenant_default_locale(&self.db, tenant_id).await?;',
  'self.get_order_with_locale_fallback(tenant_id, order_id, default_locale.as_str(), None)',
]) need(orderService, marker, 'legacy storefront access locale parity source');

need(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,',
  'canonical broad topology P0 remains open',
);

for (const marker of [
  'Status: `source_complete_unvalidated`',
  '`CommerceOrderReadRuntime` / `OrderReadPort`',
  'tenant default locale from `TenantContext`',
  'matches the former `OrderService::get_order` behavior',
  'Raw owner/backend messages are not logged or exposed',
  'broad implementation-plan item',
  'no tests, Cargo commands, Node verifiers, formatter',
]) need(record, marker, 'dated source record');

if (failures.length > 0) {
  console.error('[verify-commerce-graphql-storefront-order-access-owner-read-cutover] FAIL');
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

console.log('[verify-commerce-graphql-storefront-order-access-owner-read-cutover] PASS');
