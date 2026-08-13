#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const routing = read('crates/rustok-commerce/src/graphql/mutations/mod.rs');
const facade = read('crates/rustok-commerce/src/graphql/mutations/safe_legacy_helpers.rs');
const helperSource = read('crates/rustok-commerce/src/graphql/mutations/helpers.rs');
const graphqlRuntime = read('crates/rustok-commerce/src/graphql_runtime.rs');
const publicFacade = read('crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['#[path = "safe_legacy_helpers.rs"]\nmod legacy_helpers;', 'safe legacy helper routing'],
]) {
  requireText(routing, value, label);
}
for (const value of ['#[path = "helpers.rs"]\nmod legacy_helpers;']) {
  forbidText(routing, value, 'unsafe legacy helper routing');
}

for (const [value, label] of [
  ['mod rustok_fulfillment_shim {', 'fulfillment compatibility shim'],
  ['shipping_option_reads: Arc<dyn ShippingOptionReadPort>', 'typed shipping-option owner port'],
  ['shipping_option_read_runtime_for_current_graphql_scope(', 'host-selected GraphQL runtime lookup'],
  ['.shipping_option_read_port()', 'host-selected shipping-option capability'],
  ['pub async fn get_shipping_option(', 'shipping option compatibility method'],
  ['ReadShippingOptionProjectionRequest {', 'typed owner read request'],
  ['.read_shipping_option_projection(', 'typed owner read call'],
  ['PortActor::service(', 'service actor construction'],
  ['"rustok-commerce.graphql-cart-shipping-option"', 'stable service actor identity'],
  ['.with_deadline(std::time::Duration::from_secs(2))', 'bounded read deadline'],
  ['fulfillment_read_call_context_for_current_graphql_scope()', 'trusted scoped channel lookup'],
  ['.channel()', 'scoped channel accessor'],
  ['.with_channel(channel)', 'channel propagation'],
  ['requested_locale.map(str::to_owned)', 'requested locale propagation'],
  ['tenant_default_locale.map(str::to_owned)', 'tenant fallback locale propagation'],
  ['use self::rustok_fulfillment_shim as rustok_fulfillment;', 'fulfillment shim alias'],
  ['include!("helpers.rs");', 'unchanged legacy helper inclusion'],
]) {
  requireText(facade, value, label);
}

for (const [value, label] of [
  ['inner: ::rustok_fulfillment::FulfillmentService', 'stored concrete fulfillment service'],
  ['::rustok_fulfillment::FulfillmentService::new(', 'concrete fulfillment owner construction'],
  ['FulfillmentError::', 'concrete fulfillment error remapping'],
  ['error = ?error', 'raw owner error logging'],
]) {
  forbidText(facade, value, label);
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
  ['use rustok_fulfillment::FulfillmentService;', 'legacy helper facade import'],
  ['let option = FulfillmentService::new(db.clone())', 'legacy helper routed constructor call'],
  ['.get_shipping_option(', 'legacy shipping option compatibility call'],
]) {
  requireText(helperSource, value, label);
}

const concreteOwnerConstructors =
  facade.match(/::rustok_fulfillment::FulfillmentService::new\(/g) ?? [];
if (concreteOwnerConstructors.length !== 0) {
  failures.push(
    `expected no concrete fulfillment owner constructors in the mounted facade, found ${concreteOwnerConstructors.length}`,
  );
}
const legacyConstructors = helperSource.match(/FulfillmentService::new\(/g) ?? [];
if (legacyConstructors.length !== 1) {
  failures.push(
    `expected one legacy helper constructor routed through the typed facade, found ${legacyConstructors.length}`,
  );
}

for (const [value, label] of [
  ['"validate_selected_shipping_option"', 'public operation mapping'],
  ['"Selected shipping option is invalid"', 'unchanged public message'],
  ['"SHIPPING_OPTION_INVALID"', 'unchanged public code'],
  ['false,', 'unchanged public retryability'],
]) {
  requireText(publicFacade, value, label);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL cart shipping option owner-port verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Mounted GraphQL cart shipping-option validation resolves the host-selected Fulfillment owner port with bounded context and preserves the public SHIPPING_OPTION_INVALID envelope',
);
