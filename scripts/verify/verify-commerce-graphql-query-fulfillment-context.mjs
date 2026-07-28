#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const query = read('crates/rustok-commerce/src/graphql/query.rs');
const facade = read('crates/rustok-commerce/src/graphql/safe_query.rs');
const graphqlRuntime = read('crates/rustok-commerce/src/graphql_runtime.rs');
const commerceCargo = read('crates/rustok-commerce/Cargo.toml');
const owner = read('crates/rustok-fulfillment/src/shipping_option_read.rs');
const serverComposition = read('apps/server/src/services/commerce_provider_runtime.rs');
const serverSchema = read('apps/server/src/graphql/schema.rs');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
};

const shim = between(
  facade,
  'mod rustok_fulfillment_shim {',
  '    use self::rustok_api_shim as rustok_api;',
  'private fulfillment facade',
);
const constructor = between(
  shim,
  'pub fn new(db: DatabaseConnection) -> Self {',
  'pub async fn get_shipping_option(',
  'fulfillment facade constructor',
);
const optionLookup = between(
  shim,
  'pub async fn get_shipping_option(',
  'pub async fn list_shipping_options(',
  'shipping option lookup facade',
);
const optionList = between(
  shim,
  'pub async fn list_shipping_options(',
  'pub async fn list_all_shipping_options(',
  'shipping option list facade',
);
const optionAdminList = between(
  shim,
  'pub async fn list_all_shipping_options(',
  'pub async fn get_fulfillment(',
  'administrative shipping option list facade',
);
const adminQuery = between(
  query,
  'async fn shipping_options(',
  'async fn shipping_profiles(',
  'administrative shipping options query',
);
const portBoundary = between(
  shim,
  'fn map_shipping_option_lookup_port_error(',
  '#[allow(clippy::too_many_arguments)]\n        fn log_fulfillment_query_error(',
  'shipping option port boundary',
);

for (const [value, label] of [
  ['mod rustok_fulfillment_shim {', 'private fulfillment facade'],
  ['use self::rustok_fulfillment_shim as rustok_fulfillment;', 'safe fulfillment routing'],
  ['inner: ::rustok_fulfillment::FulfillmentService', 'legacy lifecycle delegate'],
  ['shipping_option_reads: Arc<dyn ShippingOptionReadPort>', 'storefront read port field'],
  [
    'shipping_option_admin_reads: Arc<dyn ShippingOptionAdminReadPort>',
    'administrative read port field',
  ],
  ['pub async fn list_all_shipping_options(', 'administrative all-options facade'],
  ['pub async fn find_by_order(', 'order fulfillment facade'],
  ['FulfillmentResult<Option<FulfillmentResponse>>', 'order fulfillment typed result'],
]) {
  requireText(facade, value, label);
}

for (const [value, label] of [
  [
    'shipping_option_read_runtime_for_current_graphql_scope(',
    'resolver-scoped runtime resolution',
  ],
  ['shipping_option_runtime.shipping_option_read_port()', 'host storefront read port'],
  [
    'shipping_option_runtime\n                        .shipping_option_admin_read_port()',
    'host administrative read port',
  ],
  ['::rustok_fulfillment::FulfillmentService::new(db)', 'isolated lifecycle constructor'],
]) {
  requireText(constructor, value, label);
}
for (const value of [
  'in_process_shipping_option_read_port',
  'in_process_shipping_option_admin_read_port',
]) {
  forbidText(shim, value, 'private facade must not construct shipping-option ports');
}

for (const [source, value, label] of [
  [optionLookup, 'FulfillmentResult<ShippingOptionResponse>', 'optional lookup result'],
  [optionLookup, '.read_shipping_option_projection(', 'owner option lookup'],
  [optionLookup, 'ReadShippingOptionProjectionRequest {', 'typed lookup request'],
  [optionLookup, 'map_shipping_option_lookup_port_error(', 'lookup compatibility adapter'],
  [optionList, 'Result<Vec<ShippingOptionResponse>, BoundaryError>', 'storefront list result'],
  [optionList, '.list_shipping_option_projections(', 'owner storefront list'],
  [optionList, 'ListShippingOptionProjectionsRequest {', 'typed storefront request'],
  [optionList, 'map_shipping_option_port_error(', 'storefront boundary mapper'],
  [
    optionAdminList,
    'Result<Vec<ShippingOptionResponse>, ShippingOptionAdminQueryError>',
    'typed admin adapter result',
  ],
  [optionAdminList, '.list_all_shipping_option_projections(', 'owner admin list'],
  [optionAdminList, 'ListAllShippingOptionProjectionsRequest {', 'typed admin request'],
  [
    optionAdminList,
    'ShippingOptionAdminQueryError(map_shipping_option_port_error(',
    'admin boundary adapter',
  ],
]) {
  requireText(source, value, label);
}

for (const [value, label] of [
  ['pub(crate) struct ShippingOptionAdminQueryError(BoundaryError);', 'typed admin error'],
  ['pub(crate) fn to_string(self) -> BoundaryError', 'non-string source bridge'],
  ['self.0', 'boundary preservation'],
]) {
  requireText(shim, value, label);
}
for (const value of ['impl std::fmt::Display', 'impl Display', 'format!("{}", self.0)']) {
  forbidText(shim, value, 'admin boundary must not serialize through text');
}

for (const [value, label] of [
  ['PortErrorKind::Validation', 'validation classification'],
  ['PortErrorKind::NotFound', 'not-found classification'],
  ['PortErrorKind::Conflict', 'conflict classification'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'availability classification'],
  ['PortErrorKind::Forbidden', 'forbidden classification'],
  ['PortErrorKind::InvariantViolation', 'invariant classification'],
  ['"FULFILLMENT_REQUEST_INVALID"', 'validation code'],
  ['"FULFILLMENT_RESOURCE_NOT_FOUND"', 'not-found code'],
  ['"FULFILLMENT_STATE_CONFLICT"', 'conflict code'],
  ['"FULFILLMENT_TEMPORARILY_UNAVAILABLE"', 'unavailable code'],
  ['"FULFILLMENT_ACCESS_DENIED"', 'forbidden code'],
  ['"FULFILLMENT_OPERATION_FAILED"', 'invariant code'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['owner_code = %error.code', 'stable owner code'],
  ['BoundaryError::Public {', 'typed public boundary'],
]) {
  requireText(portBoundary, value, label);
}

for (const [value, label] of [
  ['pub struct CommerceShippingOptionReadRuntime {', 'host read runtime'],
  ['shipping_option_reads: Arc<dyn ShippingOptionReadPort>', 'runtime storefront port'],
  [
    'shipping_option_admin_reads: Arc<dyn ShippingOptionAdminReadPort>',
    'runtime administrative port',
  ],
  ['pub fn new(', 'host-injectable runtime constructor'],
  ['pub fn in_process(db: DatabaseConnection) -> Self', 'standalone compatibility factory'],
  ['in_process_shipping_option_read_port(db.clone())', 'standalone storefront factory'],
  ['in_process_shipping_option_admin_read_port(db)', 'standalone admin factory'],
  ['tokio::task_local! {', 'async task-local scope'],
  ['CURRENT_COMMERCE_SHIPPING_OPTION_READ_RUNTIME', 'scoped runtime identity'],
  ['pub struct CommerceShippingOptionReadScope;', 'GraphQL scope extension'],
  ['impl ExtensionFactory for CommerceShippingOptionReadScope', 'extension factory'],
  ['ctx.data_opt::<CommerceGraphqlRuntimeData>()', 'schema runtime lookup'],
  ['.scope(', 'resolver task scope'],
  ['.try_with(Clone::clone)', 'facade runtime lookup'],
  ['CommerceShippingOptionReadRuntime::in_process(db)', 'standalone fallback'],
  [
    '.shared_get::<CommerceShippingOptionReadRuntime>()',
    'manifest runtime-data host requirement',
  ],
]) {
  requireText(graphqlRuntime, value, label);
}

for (const [value, label] of [
  [
    'shared_get::<rustok_commerce::graphql_runtime::CommerceShippingOptionReadRuntime>()',
    'server runtime reuse',
  ],
  [
    'CommerceShippingOptionReadRuntime::in_process(',
    'server in-process baseline composition',
  ],
  ['server.shared_insert(runtime.clone());', 'server runtime cache'],
  ['host.with_shared_value(runtime)', 'host runtime attachment'],
]) {
  requireText(serverComposition, value, label);
}

for (const [value, label] of [
  [
    'use rustok_commerce::graphql_runtime::CommerceShippingOptionReadScope;',
    'server extension import',
  ],
  [
    'let builder = builder.extension(CommerceShippingOptionReadScope);',
    'server extension mount',
  ],
]) {
  requireText(serverSchema, value, label);
}
requireText(commerceCargo, 'tokio.workspace = true', 'task-local runtime dependency');

for (const [value, label] of [
  ['use rustok_fulfillment::FulfillmentService;', 'unchanged query import'],
  ['let mut options = FulfillmentService::new(db.clone())', 'storefront facade constructor'],
  ['let fulfillment = FulfillmentService::new(db.clone())', 'lifecycle facade constructor'],
  ['Err(rustok_fulfillment::error::FulfillmentError::ShippingOptionNotFound(_))', 'optional none'],
  ['Err(err) => return Err(err.to_string().into())', 'lookup fail-closed conversion'],
]) {
  requireText(query, value, label);
}
for (const [value, label] of [
  ['.list_all_shipping_options(', 'admin facade call'],
  ['.map_err(|err| async_graphql::Error::new(err.to_string()))?', 'typed admin source bridge'],
  ['active: None,', 'admin active filter'],
  ['items.retain(|option| option.active == active);', 'admin active filtering'],
]) {
  requireText(adminQuery, value, label);
}
forbidText(
  query,
  '::rustok_fulfillment::FulfillmentService',
  'query source must remain facade-routed',
);

const concreteConstructors =
  facade.match(/::rustok_fulfillment::FulfillmentService::new\(db\)/g) ?? [];
if (concreteConstructors.length !== 1) {
  failures.push(`expected one remaining lifecycle constructor, found ${concreteConstructors.length}`);
}
const runtimeReadFactories =
  graphqlRuntime.match(/in_process_shipping_option_read_port\(db\.clone\(\)\)/g) ?? [];
if (runtimeReadFactories.length !== 1) {
  failures.push(`expected one standalone storefront factory, found ${runtimeReadFactories.length}`);
}
const runtimeAdminFactories =
  graphqlRuntime.match(/in_process_shipping_option_admin_read_port\(db\)/g) ?? [];
if (runtimeAdminFactories.length !== 1) {
  failures.push(`expected one standalone admin factory, found ${runtimeAdminFactories.length}`);
}

for (const [value, label] of [
  ['pub trait ShippingOptionReadPort: Send + Sync {', 'owner storefront read port'],
  ['pub trait ShippingOptionAdminReadPort: Send + Sync {', 'owner admin read port'],
  ['async fn list_all_shipping_option_projections(', 'owner admin operation'],
  ['context.require_policy(PortCallPolicy::read())?', 'owner read policy'],
  ['PortError::new(kind, code, message, retryable)', 'owner stable error'],
]) {
  requireText(owner, value, label);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL query fulfillment context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Mounted Commerce GraphQL shipping-option reads use host-composed fulfillment ports with resolver-scoped runtime data, retained typed envelopes, standalone compatibility, and isolated lifecycle reads',
);
