#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const routing = read('crates/modules/rustok-commerce/src/graphql/mutations/mod.rs');
const shippingContext = read('crates/modules/rustok-commerce/src/graphql/mutations/shipping_option_read_context.rs');
const typedHelper = read('crates/modules/rustok-commerce/src/graphql/mutations/typed_shipping_option_helper.rs');
const storefrontShipping = read('crates/modules/rustok-commerce/src/storefront_shipping.rs');
const graphqlRuntime = read('crates/modules/rustok-commerce/src/graphql_runtime.rs');
const cartMutation = read('crates/modules/rustok-commerce/src/graphql/mutations/cart.rs');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['#[path = "shipping_option_read_context.rs"]\nmod shipping_option_read_context;', 'shipping option read context routing'],
  ['#[path = "typed_shipping_option_helper.rs"]\nmod typed_shipping_option_helper;', 'typed shipping option helper routing'],
]) {
  requireText(routing, value, label);
}

for (const [value, label] of [
  ['pub(crate) fn storefront_shipping_option_read_port(', 'shipping option read port factory'],
  ['shipping_option_read_runtime_for_current_graphql_scope(db)', 'host-selected GraphQL runtime lookup'],
  ['.shipping_option_read_port()', 'host-selected shipping-option capability'],
  ['pub(crate) fn storefront_shipping_option_read_context(', 'shipping option context constructor'],
  ['PortActor::service("rustok-commerce.storefront-shipping")', 'stable service actor identity'],
  ['.with_deadline(std::time::Duration::from_secs(2))', 'bounded read deadline'],
  ['context.clone().with_channel(channel)', 'channel propagation'],
]) {
  requireText(shippingContext, value, label);
}

for (const [value, label] of [
  ['pub(crate) async fn validate_selected_shipping_option(', 'typed shipping option validation entrypoint'],
  ['super::shipping_option_read_context::storefront_shipping_option_read_context(', 'delegates to shipping context'],
  ['super::shipping_option_read_context::storefront_shipping_option_read_port(', 'delegates to shipping port'],
  ['validate_storefront_shipping_option_selection(', 'calls domain selection validation'],
  ['"Selected shipping option is invalid"', 'unchanged public message'],
  ['"SHIPPING_OPTION_INVALID"', 'unchanged public code'],
  ['false,', 'unchanged public retryability'],
]) {
  requireText(typedHelper, value, label);
}

for (const [value, label] of [
  ['::rustok_fulfillment::FulfillmentService::new(', 'concrete fulfillment owner construction'],
  ['FulfillmentService::new(', 'direct fulfillment service construction'],
]) {
  forbidText(typedHelper, value, label);
}

for (const [value, label] of [
  ['static CURRENT_COMMERCE_SHIPPING_OPTION_READ_RUNTIME:', 'shipping-option task-local runtime'],
  ['runtime_data.shipping_option_read_runtime()', 'mounted host runtime scope'],
  ['pub(crate) fn shipping_option_read_runtime_for_current_graphql_scope(', 'scoped runtime accessor'],
  ['pub(crate) fn fulfillment_read_call_context_for_current_graphql_scope()', 'scoped channel accessor source'],
]) {
  requireText(graphqlRuntime, value, label);
}

for (const [value, label] of [
  ['pub(crate) async fn validate_storefront_shipping_option_selection(', 'storefront shipping selection validation'],
  ['ReadShippingOptionProjectionRequest {', 'typed owner read request'],
  ['.read_shipping_option_projection(', 'typed owner read call'],
  ['requested_locale.map(str::to_owned)', 'requested locale propagation'],
  ['tenant_default_locale.map(str::to_owned)', 'tenant fallback locale propagation'],
]) {
  requireText(storefrontShipping, value, label);
}

for (const [value, label] of [
  ['use super::typed_shipping_option_helper::validate_selected_shipping_option;', 'cart mutation imports typed helper'],
  ['validate_selected_shipping_option(', 'cart mutation calls typed helper'],
]) {
  requireText(cartMutation, value, label);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL cart shipping option owner-port verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Mounted GraphQL cart shipping-option validation resolves the host-selected Fulfillment owner port with bounded context and preserves the public SHIPPING_OPTION_INVALID envelope',
);
