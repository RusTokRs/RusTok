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

const runtime = read('crates/rustok-commerce/src/controllers/mod.rs');
const shipping = read('crates/rustok-commerce/src/controllers/admin/shipping.rs');
const owner = read('crates/rustok-fulfillment/src/shipping_option_admin_command.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-commerce/docs/admin-shipping-option-command-owner-port-cutover-2026-08-09.md',
);

for (const marker of [
  'shipping_option_admin_command_runtime: rustok_fulfillment::ShippingOptionAdminCommandRuntime',
  'fn shipping_option_admin_command_port(',
  'shared_get::<rustok_fulfillment::ShippingOptionAdminCommandRuntime>()',
  'rustok_fulfillment::ShippingOptionAdminCommandRuntime::in_process(',
]) need(runtime, marker, 'commerce HTTP runtime composition');

for (const marker of [
  'CreateAdminShippingOptionRequest',
  'UpdateAdminShippingOptionRequest',
  'DeactivateAdminShippingOptionRequest',
  'ReactivateAdminShippingOptionRequest',
  'admin_shipping_option_command_idempotency_key(',
  '.with_idempotency_key(idempotency_key)',
  '.with_deadline(std::time::Duration::from_secs(2))',
  '.shipping_option_admin_command_port()',
  '.create_shipping_option(command_context.clone(), request)',
  '.update_shipping_option(command_context.clone(), request)',
  '.deactivate_shipping_option(command_context.clone(), request)',
  '.reactivate_shipping_option(command_context.clone(), request)',
  'validate_shipping_option_profile_inputs(',
  'map_admin_shipping_option_port_error(',
]) need(shipping, marker, 'mounted shipping-option command cutover');

for (const marker of [
  'FulfillmentService::new(runtime.db_clone())\n        .create_shipping_option',
  'FulfillmentService::new(runtime.db_clone())\n        .update_shipping_option',
  'FulfillmentService::new(runtime.db_clone())\n        .deactivate_shipping_option',
  'FulfillmentService::new(runtime.db_clone())\n        .reactivate_shipping_option',
  'use rustok_fulfillment::error::FulfillmentError;',
]) forbid(shipping, marker, 'mounted shipping-option direct owner construction');

for (const marker of [
  'context.require_policy(PortCallPolicy::write())',
  'pub trait ShippingOptionAdminCommandPort',
]) need(owner, marker, 'fulfillment owner write admission');

need(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,',
  'canonical ecommerce topology P0 remains open',
);

for (const marker of [
  'Status: `source_complete_unvalidated`',
  '`ShippingOptionAdminCommandPort`',
  'Those handlers no longer construct `rustok_fulfillment::FulfillmentService`',
  'does **not** claim',
  'no tests, Cargo commands, Node verifiers, formatter',
]) need(record, marker, 'dated source record');

if (failures.length > 0) {
  console.error('[verify-commerce-admin-shipping-option-command-owner-port-cutover] FAIL');
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

console.log('[verify-commerce-admin-shipping-option-command-owner-port-cutover] PASS');
