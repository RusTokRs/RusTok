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

const owner = read('crates/rustok-fulfillment/src/shipping_option_admin_command.rs');
const lib = read('crates/rustok-fulfillment/src/lib.rs');
const runtime = read('crates/rustok-commerce/src/controllers/mod.rs');
const consumer = read('crates/rustok-commerce/src/controllers/admin/shipping.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-fulfillment/docs/shipping-option-admin-command-owner-port-2026-08-09.md',
);
const cutoverRecord = read(
  'crates/rustok-commerce/docs/admin-shipping-option-command-owner-port-cutover-2026-08-09.md',
);

for (const marker of [
  'pub trait ShippingOptionAdminCommandPort',
  'async fn create_shipping_option(',
  'async fn update_shipping_option(',
  'async fn deactivate_shipping_option(',
  'async fn reactivate_shipping_option(',
  'pub struct ShippingOptionAdminCommandRuntime',
  'pub fn in_process(db: DatabaseConnection) -> Self',
  'service: FulfillmentService',
  'FulfillmentService::new(db)',
  'context.require_policy(PortCallPolicy::write())',
  'Uuid::parse_str(&context.tenant_id)',
  'PortError::validation(',
  'PortError::not_found(',
  'PortError::conflict(',
  'PortError::unavailable(',
  'fulfillment.shipping_option_not_found',
  'fulfillment.database_unavailable',
  'error_variant = fulfillment_error_variant(&error)',
]) need(owner, marker, 'shipping option owner command capability');

for (const marker of [
  'context.require_write_semantics()',
  'error = ?error',
  'error.to_string()',
]) forbid(owner, marker, 'shipping option owner bounded replay/diagnostic contract');

for (const marker of [
  'mod shipping_option_admin_command;',
  'ShippingOptionAdminCommandPort, ShippingOptionAdminCommandRuntime',
  'in_process_shipping_option_admin_command_port',
]) need(lib, marker, 'fulfillment public export');

for (const marker of [
  'shipping_option_admin_command_runtime: rustok_fulfillment::ShippingOptionAdminCommandRuntime',
  'fn shipping_option_admin_command_port(',
  'shared_get::<rustok_fulfillment::ShippingOptionAdminCommandRuntime>()',
]) need(runtime, marker, 'commerce HTTP runtime selection');

for (const marker of [
  '.shipping_option_admin_command_port()',
  '.create_shipping_option(command_context.clone(), request)',
  '.update_shipping_option(command_context.clone(), request)',
  '.deactivate_shipping_option(command_context.clone(), request)',
  '.reactivate_shipping_option(command_context.clone(), request)',
  'admin_shipping_option_command_idempotency_key(',
  'admin_shipping_option_command_port_context(',
]) need(consumer, marker, 'mounted consumer owner-port cutover');

for (const marker of [
  'FulfillmentService::new(runtime.db_clone())\n        .create_shipping_option',
  'FulfillmentService::new(runtime.db_clone())\n        .update_shipping_option',
  'FulfillmentService::new(runtime.db_clone())\n        .deactivate_shipping_option',
  'FulfillmentService::new(runtime.db_clone())\n        .reactivate_shipping_option',
]) forbid(consumer, marker, 'mounted consumer concrete owner construction');

need(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,',
  'canonical ecommerce topology P0 remains open',
);

for (const marker of [
  'Status: `source_complete_unvalidated`',
  '`ShippingOptionAdminCommandPort`',
  '`ShippingOptionAdminCommandRuntime`',
  'does **not** claim durable idempotent replay',
  'no tests, Cargo commands, Node verifiers, formatter',
]) need(record, marker, 'owner capability source record');

for (const marker of [
  'Status: `source_complete_unvalidated`',
  'Those handlers no longer construct `rustok_fulfillment::FulfillmentService`',
  'does **not** claim',
]) need(cutoverRecord, marker, 'consumer cutover source record');

if (failures.length > 0) {
  console.error('[verify-fulfillment-shipping-option-admin-command-owner-port] FAIL');
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

console.log('[verify-fulfillment-shipping-option-admin-command-owner-port] PASS');
