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
const shippingOwner = read('crates/rustok-fulfillment/src/shipping_option_read.rs');
const lifecycleOwner = read('crates/rustok-fulfillment/src/fulfillment_read.rs');
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
const fulfillmentLookup = between(
  shim,
  'pub async fn get_fulfillment(',
  'pub async fn list_fulfillments(',
  'fulfillment lookup facade',
);
const fulfillmentList = between(
  shim,
  'pub async fn list_fulfillments(',
  'pub async fn find_by_order(',
  'fulfillment list facade',
);
const fulfillmentLatest = between(
  shim,
  'pub async fn find_by_order(',
  'fn shipping_option_query_context(',
  'fulfillment latest-by-order facade',
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
  '    }\n\n    use self::rustok_api_shim as rustok_api;',
  'fulfillment port boundary',
);

for (const [value, label] of [
  ['mod rustok_fulfillment_shim {', 'private fulfillment facade'],
  ['use self::rustok_fulfillment_shim as rustok_fulfillment;', 'safe fulfillment routing'],
  ['shipping_option_reads: Arc<dyn ShippingOptionReadPort>', 'storefront read port field'],
  [
    'shipping_option_admin_reads: Arc<dyn ShippingOptionAdminReadPort>',
    'administrative read port field',
  ],
  ['fulfillment_reads: Arc<dyn FulfillmentReadPort>', 'lifecycle read port field'],
  ['pub async fn list_all_shipping_options(', 'administrative all-options facade'],
  ['pub async fn get_fulfillment(', 'fulfillment lookup facade'],
  ['pub async fn list_fulfillments(', 'fulfillment list facade'],
  ['pub async fn find_by_order(', 'order fulfillment facade'],
  ['FulfillmentResult<Option<FulfillmentResponse>>', 'order fulfillment typed result'],
]) requireText(facade, value, label);

for (const [value, label] of [
  [
    'shipping_option_read_runtime_for_current_graphql_scope(',
    'shipping resolver-scoped runtime resolution',
  ],
  ['shipping_option_runtime.shipping_option_read_port()', 'host storefront read port'],
  [
    'shipping_option_runtime\n                        .shipping_option_admin_read_port()',
    'host administrative read port',
  ],
  [
    'fulfillment_lifecycle_read_runtime_for_current_graphql_scope(db)',
    'lifecycle resolver-scoped runtime resolution',
  ],
  [
    'fulfillment_lifecycle_runtime.fulfillment_read_port()',
    'host lifecycle read port',
  ],
]) requireText(constructor, value, label);
for (const value of [
  '::rustok_fulfillment::FulfillmentService::new(db)',
  'in_process_shipping_option_read_port',
  'in_process_shipping_option_admin_read_port',
  'in_process_fulfillment_read_port',
]) forbidText(shim, value, 'private facade must not construct concrete read implementations');

for (const [source, value, label] of [
  [optionLookup, 'FulfillmentResult<ShippingOptionResponse>', 'optional option lookup result'],
  [optionLookup, '.read_shipping_option_projection(', 'owner option lookup'],
  [optionLookup, 'ReadShippingOptionProjectionRequest {', 'typed option lookup request'],
  [optionLookup, 'map_shipping_option_lookup_port_error(', 'option lookup adapter'],
  [optionList, 'Result<Vec<ShippingOptionResponse>, BoundaryError>', 'storefront option list result'],
  [optionList, '.list_shipping_option_projections(', 'owner storefront option list'],
  [optionList, 'ListShippingOptionProjectionsRequest {', 'typed storefront option request'],
  [optionList, 'map_shipping_option_port_error(', 'storefront option boundary mapper'],
  [
    optionAdminList,
    'Result<Vec<ShippingOptionResponse>, ShippingOptionAdminQueryError>',
    'typed admin option result',
  ],
  [optionAdminList, '.list_all_shipping_option_projections(', 'owner admin option list'],
  [optionAdminList, 'ListAllShippingOptionProjectionsRequest {', 'typed admin option request'],
  [
    optionAdminList,
    'ShippingOptionAdminQueryError(map_shipping_option_port_error(',
    'admin option boundary adapter',
  ],
  [fulfillmentLookup, 'FulfillmentResult<FulfillmentResponse>', 'fulfillment lookup result'],
  [fulfillmentLookup, '.read_fulfillment_projection(', 'owner fulfillment lookup'],
  [
    fulfillmentLookup,
    'ReadFulfillmentProjectionRequest { fulfillment_id: id }',
    'typed fulfillment lookup request',
  ],
  [fulfillmentLookup, 'map_fulfillment_port_error(', 'fulfillment lookup adapter'],
  [fulfillmentList, 'ListFulfillmentsInput {', 'compatibility list input destructuring'],
  [fulfillmentList, '.list_fulfillment_projections(', 'owner fulfillment list'],
  [fulfillmentList, 'ListFulfillmentProjectionsRequest {', 'typed fulfillment list request'],
  [fulfillmentList, 'Ok((page_result.items, page_result.total))', 'list result compatibility'],
  [
    fulfillmentLatest,
    '.find_latest_fulfillment_by_order_projection(',
    'owner latest fulfillment by order',
  ],
  [
    fulfillmentLatest,
    'FindLatestFulfillmentByOrderProjectionRequest { order_id }',
    'typed latest fulfillment request',
  ],
]) requireText(source, value, label);

for (const [value, label] of [
  ['pub(crate) struct ShippingOptionAdminQueryError(BoundaryError);', 'typed admin option error'],
  ['pub(crate) fn to_string(self) -> BoundaryError', 'non-string source bridge'],
  ['self.0', 'boundary preservation'],
  ['fn fulfillment_query_context(', 'lifecycle context builder'],
  [
    'PortActor::service("rustok-commerce.graphql-query-fulfillments")',
    'lifecycle service actor',
  ],
  [
    'graphql-fulfillment-lifecycle:{query_field}:{operation}:{resource}',
    'lifecycle correlation context',
  ],
  ['with_deadline(std::time::Duration::from_secs(2))', 'read deadline context'],
  ['fn map_fulfillment_port_error(', 'lifecycle compatibility mapper'],
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
  ['correlation_id = %context.correlation_id', 'correlation logging'],
  ['deadline_ms = ?context.deadline_ms', 'deadline logging'],
  ['owner_code = %error.code', 'stable owner code'],
  ['BoundaryError::Public {', 'typed public boundary'],
]) requireText(portBoundary, value, label);
for (const value of ['impl std::fmt::Display', 'impl Display', 'format!("{}", self.0)']) {
  forbidText(shim, value, 'admin boundary must not serialize through text');
}

for (const [value, label] of [
  ['pub struct CommerceShippingOptionReadRuntime {', 'host shipping runtime'],
  ['pub struct CommerceFulfillmentLifecycleReadRuntime {', 'host lifecycle runtime'],
  ['tokio::task_local! {', 'async task-local scope'],
  ['CURRENT_COMMERCE_SHIPPING_OPTION_READ_RUNTIME', 'shipping scoped runtime identity'],
  [
    'CURRENT_COMMERCE_FULFILLMENT_LIFECYCLE_READ_RUNTIME',
    'lifecycle scoped runtime identity',
  ],
  ['pub struct CommerceShippingOptionReadScope;', 'shared GraphQL scope extension'],
  ['impl ExtensionFactory for CommerceShippingOptionReadScope', 'extension factory'],
  ['ctx.data_opt::<CommerceGraphqlRuntimeData>()', 'schema runtime lookup'],
  ['runtime_data.shipping_option_read_runtime()', 'shipping runtime scope value'],
  [
    'runtime_data.fulfillment_lifecycle_read_runtime()',
    'lifecycle runtime scope value',
  ],
  ['.try_with(Clone::clone)', 'facade runtime lookup'],
  ['CommerceShippingOptionReadRuntime::in_process(db)', 'shipping standalone fallback'],
  [
    'CommerceFulfillmentLifecycleReadRuntime::in_process(db)',
    'lifecycle standalone fallback',
  ],
  [
    '.shared_get::<CommerceShippingOptionReadRuntime>()',
    'manifest shipping runtime host requirement',
  ],
  [
    '.shared_get::<CommerceFulfillmentLifecycleReadRuntime>()',
    'manifest lifecycle runtime host selection',
  ],
]) requireText(graphqlRuntime, value, label);

for (const [value, label] of [
  [
    'shared_get::<rustok_commerce::graphql_runtime::CommerceShippingOptionReadRuntime>()',
    'server shipping runtime reuse',
  ],
  [
    'shared_get::<rustok_commerce::graphql_runtime::CommerceFulfillmentLifecycleReadRuntime>()',
    'server lifecycle runtime reuse',
  ],
  ['server.shared_insert(runtime.clone());', 'server runtime cache'],
  ['host.with_shared_value(runtime)', 'host runtime attachment'],
]) requireText(serverComposition, value, label);

for (const [value, label] of [
  [
    'use rustok_commerce::graphql_runtime::CommerceShippingOptionReadScope;',
    'server shared extension import',
  ],
  [
    'let builder = builder.extension(CommerceShippingOptionReadScope);',
    'server shared extension mount',
  ],
]) requireText(serverSchema, value, label);
requireText(commerceCargo, 'tokio.workspace = true', 'task-local runtime dependency');

for (const [value, label] of [
  ['use rustok_fulfillment::FulfillmentService;', 'unchanged query facade import'],
  ['.get_fulfillment(tenant_id, id)', 'query fulfillment lookup facade call'],
  ['.list_fulfillments(', 'query fulfillment list facade call'],
  ['.find_by_order(tenant_id, id)', 'query latest-by-order facade call'],
  [
    'Err(rustok_fulfillment::error::FulfillmentError::FulfillmentNotFound(_))',
    'optional fulfillment lookup none',
  ],
  ['Err(err) => return Err(err.to_string().into())', 'lookup fail-closed conversion'],
]) requireText(query, value, label);
for (const [value, label] of [
  ['.list_all_shipping_options(', 'admin option facade call'],
  ['.map_err(|err| async_graphql::Error::new(err.to_string()))?', 'typed admin source bridge'],
  ['active: None,', 'admin option active filter'],
  ['items.retain(|option| option.active == active);', 'admin option filtering'],
]) requireText(adminQuery, value, label);
forbidText(
  query,
  '::rustok_fulfillment::FulfillmentService',
  'query source must remain facade-routed',
);

const concreteConstructors =
  facade.match(/::rustok_fulfillment::FulfillmentService::new\(db\)/g) ?? [];
if (concreteConstructors.length !== 0) {
  failures.push(`expected zero concrete lifecycle constructors, found ${concreteConstructors.length}`);
}
const shippingRuntimeFactories =
  graphqlRuntime.match(/in_process_shipping_option_read_port\(db\.clone\(\)\)/g) ?? [];
if (shippingRuntimeFactories.length !== 1) {
  failures.push(`expected one standalone storefront factory, found ${shippingRuntimeFactories.length}`);
}
const adminRuntimeFactories =
  graphqlRuntime.match(/in_process_shipping_option_admin_read_port\(db\)/g) ?? [];
if (adminRuntimeFactories.length !== 1) {
  failures.push(`expected one standalone admin factory, found ${adminRuntimeFactories.length}`);
}
const lifecycleRuntimeFactories =
  graphqlRuntime.match(/in_process_fulfillment_read_port\(db\)/g) ?? [];
if (lifecycleRuntimeFactories.length !== 1) {
  failures.push(`expected one standalone lifecycle factory, found ${lifecycleRuntimeFactories.length}`);
}

for (const [source, value, label] of [
  [shippingOwner, 'pub trait ShippingOptionReadPort: Send + Sync {', 'owner storefront read port'],
  [shippingOwner, 'pub trait ShippingOptionAdminReadPort: Send + Sync {', 'owner admin read port'],
  [shippingOwner, 'async fn list_all_shipping_option_projections(', 'owner admin operation'],
  [lifecycleOwner, 'pub trait FulfillmentReadPort: Send + Sync {', 'owner lifecycle read port'],
  [lifecycleOwner, 'async fn read_fulfillment_projection(', 'owner lifecycle lookup'],
  [lifecycleOwner, 'async fn list_fulfillment_projections(', 'owner lifecycle list'],
  [
    lifecycleOwner,
    'async fn find_latest_fulfillment_by_order_projection(',
    'owner lifecycle latest-by-order',
  ],
  [lifecycleOwner, 'context.require_policy(PortCallPolicy::read())?', 'owner lifecycle read policy'],
  [lifecycleOwner, 'PortError::new(kind, code, message, retryable)', 'owner lifecycle stable error'],
]) requireText(source, value, label);

if (failures.length > 0) {
  console.error('Commerce GraphQL query fulfillment context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Mounted Commerce GraphQL shipping-option and fulfillment lifecycle reads use host-composed owner ports through one resolver-scoped runtime bridge with retained compatibility envelopes',
);
