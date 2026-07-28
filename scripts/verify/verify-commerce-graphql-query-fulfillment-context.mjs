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
const owner = read('crates/rustok-fulfillment/src/shipping_option_read.rs');
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
  ['inner: ::rustok_fulfillment::FulfillmentService', 'legacy fulfillment delegate'],
  ['shipping_option_reads: Arc<dyn ShippingOptionReadPort>', 'shipping option read port field'],
  [
    'shipping_option_admin_reads: Arc<dyn ShippingOptionAdminReadPort>',
    'administrative shipping option read port field',
  ],
  ['::rustok_fulfillment::FulfillmentService::new(db.clone())', 'legacy constructor isolation'],
  [
    'shipping_option_reads: in_process_shipping_option_read_port(db.clone())',
    'canonical storefront read factory',
  ],
  [
    'shipping_option_admin_reads: in_process_shipping_option_admin_read_port(db)',
    'canonical administrative read factory',
  ],
  ['pub async fn list_all_shipping_options(', 'administrative all-options facade'],
  ['pub async fn find_by_order(', 'order fulfillment facade'],
  ['FulfillmentResult<Option<FulfillmentResponse>>', 'order fulfillment typed result'],
  ['"shipping_options"', 'admin shipping query field'],
  ['"list_all_shipping_option_projections"', 'admin shipping owner operation'],
  ['"order"', 'admin order query field'],
  ['"find_by_order"', 'order fulfillment owner operation'],
]) {
  requireText(facade, value, label);
}

for (const [value, label] of [
  ['ShippingOptionReadPort', 'owner storefront read trait'],
  ['ShippingOptionAdminReadPort', 'owner administrative read trait'],
  ['ListShippingOptionProjectionsRequest', 'list projection request'],
  ['ListAllShippingOptionProjectionsRequest', 'admin list projection request'],
  ['ReadShippingOptionProjectionRequest', 'lookup projection request'],
  ['in_process_shipping_option_read_port', 'root storefront read factory'],
  ['in_process_shipping_option_admin_read_port', 'root administrative read factory'],
  ['PortActor, PortContext, PortError, PortErrorKind', 'port context imports'],
  ['fn shipping_option_query_context(', 'query context builder'],
  ['PortActor::service("rustok-commerce.graphql-query-shipping-options")', 'query service actor'],
  ['format!("graphql-fulfillment:{query_field}:{resource}")', 'query correlation'],
  ['.with_deadline(std::time::Duration::from_secs(2))', 'query deadline'],
]) {
  requireText(shim, value, label);
}

for (const [source, value, label] of [
  [optionLookup, 'FulfillmentResult<ShippingOptionResponse>', 'optional lookup compatibility result'],
  [optionLookup, '.read_shipping_option_projection(', 'owner option lookup'],
  [optionLookup, 'ReadShippingOptionProjectionRequest {', 'typed lookup request'],
  [optionLookup, 'context.clone(),', 'lookup context delegation'],
  [
    optionLookup,
    'map_shipping_option_lookup_port_error(',
    'lookup compatibility adapter',
  ],
  [
    optionLookup,
    '"read_shipping_option_projection"',
    'lookup owner operation',
  ],
  [optionList, 'Result<Vec<ShippingOptionResponse>, BoundaryError>', 'direct list boundary result'],
  [optionList, '.list_shipping_option_projections(', 'owner option list'],
  [optionList, 'ListShippingOptionProjectionsRequest {', 'typed list request'],
  [optionList, 'context.clone(),', 'list context delegation'],
  [optionList, 'map_shipping_option_port_error(', 'list boundary mapper'],
  [
    optionList,
    '"list_shipping_option_projections"',
    'list owner operation',
  ],
  [
    optionAdminList,
    'Result<Vec<ShippingOptionResponse>, ShippingOptionAdminQueryError>',
    'typed admin adapter result',
  ],
  [optionAdminList, '.list_all_shipping_option_projections(', 'owner admin option list'],
  [optionAdminList, 'ListAllShippingOptionProjectionsRequest {', 'typed admin list request'],
  [optionAdminList, 'context.clone(),', 'admin list context delegation'],
  [
    optionAdminList,
    'ShippingOptionAdminQueryError(map_shipping_option_port_error(',
    'admin boundary adapter',
  ],
  [
    optionAdminList,
    '"list_all_shipping_option_projections"',
    'admin list owner operation',
  ],
]) {
  requireText(source, value, label);
}

for (const [value, label] of [
  ['pub(crate) struct ShippingOptionAdminQueryError(BoundaryError);', 'typed admin query error'],
  ['pub(crate) fn to_string(self) -> BoundaryError', 'non-string source compatibility bridge'],
  ['self.0', 'boundary preservation'],
]) {
  requireText(shim, value, label);
}
for (const value of ['impl std::fmt::Display', 'impl Display', 'format!("{}", self.0)']) {
  forbidText(shim, value, 'admin boundary must not serialize through text');
}

const projectionLookups = optionLookup.match(/\.read_shipping_option_projection\(/g) ?? [];
if (projectionLookups.length !== 1) {
  failures.push(`expected one shipping-option projection lookup, found ${projectionLookups.length}`);
}
const projectionLists = optionList.match(/\.list_shipping_option_projections\(/g) ?? [];
if (projectionLists.length !== 1) {
  failures.push(`expected one shipping-option projection list, found ${projectionLists.length}`);
}
const adminProjectionLists =
  optionAdminList.match(/\.list_all_shipping_option_projections\(/g) ?? [];
if (adminProjectionLists.length !== 1) {
  failures.push(`expected one administrative shipping-option projection list, found ${adminProjectionLists.length}`);
}

for (const [value, label] of [
  ['PortErrorKind::Validation', 'validation classification'],
  ['PortErrorKind::NotFound', 'not-found classification'],
  ['PortErrorKind::Conflict', 'conflict classification'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'availability classification'],
  ['PortErrorKind::Forbidden', 'forbidden classification'],
  ['PortErrorKind::InvariantViolation', 'invariant classification'],
  ['"Fulfillment query is invalid"', 'existing validation message'],
  ['"FULFILLMENT_REQUEST_INVALID"', 'existing validation code'],
  ['"Fulfillment resource was not found"', 'existing not-found message'],
  ['"FULFILLMENT_RESOURCE_NOT_FOUND"', 'existing not-found code'],
  ['"Fulfillment state conflicts with this query"', 'existing conflict message'],
  ['"FULFILLMENT_STATE_CONFLICT"', 'existing conflict code'],
  ['"Fulfillment data is temporarily unavailable"', 'existing unavailable message'],
  ['"FULFILLMENT_TEMPORARILY_UNAVAILABLE"', 'existing unavailable code'],
  ['FulfillmentError::ShippingOptionNotFound(shipping_option_id)', 'optional not-found compatibility'],
  ['FulfillmentError::InvalidTransition {', 'conflict compatibility'],
  ['DbErr::Custom("fulfillment storage is temporarily unavailable".to_string())', 'availability compatibility'],
  ['"OPTIONAL_NONE"', 'optional-none diagnostic policy'],
  ['"COMMERCE_QUERY_OPERATION_FAILED"', 'lookup fail-closed diagnostic policy'],
  ['fn port_error_kind_name(', 'stable kind classifier'],
  ['fn is_technical_port_error(', 'technical classifier'],
  ['fn log_shipping_option_port_error(', 'shared context logger'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['context_locale_length = context.locale.len()', 'locale context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['query_field,', 'query field context'],
  ['operation,', 'owner operation context'],
  ['shipping_option_id = ?shipping_option_id', 'option identity'],
  ['requested_locale_length = requested_locale.map(str::len)', 'requested locale length'],
  ['tenant_default_locale_length = tenant_default_locale.map(str::len)', 'default locale length'],
  ['error_kind,', 'stable kind diagnostic'],
  ['owner_code = %error.code', 'stable owner code'],
  ['owner_kind = ?error.kind', 'owner kind'],
  ['owner_retryable = error.retryable', 'owner retryability'],
  ['public_code,', 'public code'],
  ['public_retryable,', 'public retryability'],
  ['tracing::error!(', 'technical severity'],
  ['tracing::warn!(', 'ordinary severity'],
  ['BoundaryError::Public {', 'direct list boundary result'],
]) {
  requireText(portBoundary, value, label);
}

for (const [value, label] of [
  ['use rustok_fulfillment::FulfillmentService;', 'unchanged query import'],
  ['let mut options = FulfillmentService::new(db.clone())', 'storefront facade constructor'],
  ['.list_shipping_options(', 'storefront shipping option call'],
  ['let fulfillment = FulfillmentService::new(db.clone())', 'admin order facade constructor'],
  ['.find_by_order(tenant_id, id)', 'admin order fulfillment call'],
  ['Err(rustok_fulfillment::error::FulfillmentError::ShippingOptionNotFound(_))', 'optional not-found source contract'],
  ['Err(err) => return Err(err.to_string().into())', 'existing lookup fail-closed conversion'],
  ['.map_err(|err| async_graphql::Error::new(err.to_string()))?', 'unchanged mapped query source'],
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

const legacyConstructors =
  facade.match(/::rustok_fulfillment::FulfillmentService::new\(db\.clone\(\)\)/g) ?? [];
if (legacyConstructors.length !== 1) {
  failures.push(`expected one remaining concrete fulfillment constructor, found ${legacyConstructors.length}`);
}
const readFactories =
  facade.match(/in_process_shipping_option_read_port\(db\.clone\(\)\)/g) ?? [];
if (readFactories.length !== 1) {
  failures.push(`expected one shipping-option read factory, found ${readFactories.length}`);
}
const adminReadFactories =
  facade.match(/in_process_shipping_option_admin_read_port\(db\)/g) ?? [];
if (adminReadFactories.length !== 1) {
  failures.push(`expected one administrative shipping-option read factory, found ${adminReadFactories.length}`);
}

for (const value of [
  '.get_shipping_option(tenant_id, id, requested_locale, tenant_default_locale)',
  '.list_shipping_options(\n                        tenant_id,',
  '.list_all_shipping_options(tenant_id, requested_locale, tenant_default_locale)',
  'error.message',
]) {
  forbidText(
    optionLookup + optionList + optionAdminList + portBoundary,
    value,
    'shipping-option query port boundary',
  );
}

forbidText(
  query,
  '::rustok_fulfillment::FulfillmentService',
  'query source must remain facade-routed',
);

for (const [value, label] of [
  ['pub trait ShippingOptionReadPort: Send + Sync {', 'owner storefront read port'],
  ['pub trait ShippingOptionAdminReadPort: Send + Sync {', 'owner admin read port'],
  ['async fn list_all_shipping_option_projections(', 'owner admin list operation'],
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
  '✔ Commerce GraphQL shipping-option lookup, storefront list, and administrative list-all use fulfillment owner read ports with retained context, typed public envelopes, and isolated legacy lifecycle reads',
);
