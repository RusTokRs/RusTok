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

const server = read('apps/server/src/services/commerce_provider_runtime.rs');
const graphqlRuntime = read('crates/rustok-commerce/src/graphql_runtime.rs');
const httpRuntime = read('crates/rustok-commerce/src/controllers/mod.rs');
const routing = read('crates/rustok-commerce/src/graphql/mutations/mod.rs');
const checkout = read('crates/rustok-commerce/src/graphql/mutations/checkout.rs');
const safeCheckout = read('crates/rustok-commerce/src/graphql/mutations/safe_checkout.rs');
const owner = read('crates/rustok-fulfillment/src/shipping_option_admin_command.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-commerce/docs/graphql-shipping-option-command-owner-port-cutover-2026-08-09.md',
);

const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

for (const marker of [
  'shared_get::<rustok_fulfillment::ShippingOptionAdminCommandRuntime>()',
  'server.shared_get::<rustok_fulfillment::ShippingOptionAdminCommandRuntime>()',
  'rustok_fulfillment::ShippingOptionAdminCommandRuntime::in_process(',
  'server.shared_insert(runtime.clone());',
  'host.with_shared_value(runtime)',
]) need(server, marker, 'server host composition');

for (const marker of [
  'ShippingOptionAdminCommandRuntime',
  'shipping_option_admin_command_runtime: ShippingOptionAdminCommandRuntime',
  'pub fn shipping_option_admin_command_runtime(&self) -> ShippingOptionAdminCommandRuntime',
  '.shared_get::<ShippingOptionAdminCommandRuntime>()',
  'commerce GraphQL requires ShippingOptionAdminCommandRuntime in host composition',
  'pub(crate) fn shipping_option_admin_command_runtime_from_context(',
  '.map(CommerceGraphqlRuntimeData::shipping_option_admin_command_runtime)',
  'ShippingOptionAdminCommandRuntime::in_process(db)',
]) need(graphqlRuntime, marker, 'GraphQL runtime composition');

for (const marker of [
  'shipping_option_admin_command_runtime: rustok_fulfillment::ShippingOptionAdminCommandRuntime',
  'shared_get::<rustok_fulfillment::ShippingOptionAdminCommandRuntime>()',
  'fn shipping_option_admin_command_port(',
]) need(httpRuntime, marker, 'REST runtime shares owner capability');

need(
  routing,
  '#[path = "safe_checkout.rs"]\npub mod checkout;',
  'mounted safe checkout routing',
);

for (const marker of [
  'CreateAdminShippingOptionRequest',
  'UpdateAdminShippingOptionRequest',
  'DeactivateAdminShippingOptionRequest',
  'ReactivateAdminShippingOptionRequest',
  'fn shipping_option_command_context(',
  'PortActor::user(auth.user_id.to_string())',
  '.with_idempotency_key(Uuid::new_v4().to_string())',
  '.with_deadline(std::time::Duration::from_secs(2))',
  'request.channel_slug.as_deref()',
  'shipping_option_admin_command_runtime_from_context(',
  '.create_shipping_option(command_context.clone(), request)',
  '.update_shipping_option(command_context.clone(), request)',
  '.deactivate_shipping_option(command_context.clone(), request)',
  '.reactivate_shipping_option(command_context.clone(), request)',
  'validate_shipping_option_profile_inputs(',
  'checkout_boundary::shipping_option_port_error(',
]) need(checkout, marker, 'mounted shipping-option command cutover');

for (const marker of [
  'FulfillmentService::new(',
  'use rustok_fulfillment::FulfillmentService;',
]) forbid(checkout, marker, 'mounted GraphQL concrete Fulfillment owner construction');

for (const marker of [
  'use ::rustok_api::{PortContext, PortError, PortErrorKind};',
  'fn shipping_option_port_error_envelope(',
  'pub(crate) fn shipping_option_port_error(',
  'PortErrorKind::Validation',
  'PortErrorKind::NotFound if error.code == "fulfillment.shipping_option_not_found"',
  'PortErrorKind::Conflict',
  'PortErrorKind::Unavailable | PortErrorKind::Timeout',
  'PortErrorKind::Forbidden',
  'PortErrorKind::InvariantViolation',
  '"SHIPPING_OPTION_REQUEST_INVALID"',
  '"SHIPPING_OPTION_NOT_FOUND"',
  '"SHIPPING_OPTION_STATE_CONFLICT"',
  '"SHIPPING_OPTION_TEMPORARILY_UNAVAILABLE"',
  '"SHIPPING_OPTION_OPERATION_FAILED"',
  'owner_error_kind = ?error.kind',
  'owner_code_length = error.code.chars().count()',
  'boundary = CHECKOUT_ERROR_BOUNDARY',
]) need(safeCheckout, marker, 'bounded GraphQL owner error boundary');

for (const marker of [
  'FulfillmentError',
  'owner_message = %error.message',
  'message = %error.message',
  'error.to_string()',
]) forbid(safeCheckout, marker, 'raw owner error leakage');

for (const marker of [
  'pub trait ShippingOptionAdminCommandPort',
  'context.require_policy(PortCallPolicy::write())',
]) need(owner, marker, 'Fulfillment owner command admission');

need(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'canonical topology item remains open',
);

for (const marker of [
  '# Commerce GraphQL shipping-option command owner-port cutover',
  'Status: `source_complete_unvalidated`',
  '`createShippingOption`',
  '`updateShippingOption`',
  '`deactivateShippingOption`',
  '`reactivateShippingOption`',
  'does **not** claim',
  '`createStorefrontPaymentCollection` in the same resolver source still constructs `PaymentService`',
  'no tests, Cargo commands, Node verifiers, formatter',
]) need(record, marker, 'truthful source record');

if (failures.length > 0) {
  console.error('Commerce GraphQL shipping-option command owner-port cutover verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL shipping-option writes use the host-composed Fulfillment owner command runtime with bounded public errors',
);
