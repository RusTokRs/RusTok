#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import {
  readCommerceFulfillmentQueryShimSource,
  readCommerceSafeQuerySource,
} from './lib/commerce-safe-query-source.mjs';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const query = read('crates/rustok-commerce/src/graphql/query.rs');
const facade = readCommerceSafeQuerySource(read);
const shim = readCommerceFulfillmentQueryShimSource(read);
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

const constructor = between(
  shim,
  'pub fn new(db: DatabaseConnection) -> Self {',
  'pub async fn get_shipping_option(',
  'facade constructor',
);
const optionLookup = between(
  shim,
  'pub async fn get_shipping_option(',
  'pub async fn list_shipping_options(',
  'option lookup',
);
const optionList = between(
  shim,
  'pub async fn list_shipping_options(',
  'pub async fn list_all_shipping_options(',
  'option list',
);
const optionAdminList = between(
  shim,
  'pub async fn list_all_shipping_options(',
  'pub async fn get_fulfillment(',
  'admin option list',
);
const fulfillmentLookup = between(
  shim,
  'pub async fn get_fulfillment(',
  'pub async fn list_fulfillments(',
  'fulfillment lookup',
);
const fulfillmentList = between(
  shim,
  'pub async fn list_fulfillments(',
  'pub async fn find_by_order(',
  'fulfillment list',
);
const fulfillmentLatest = between(
  shim,
  'pub async fn find_by_order(',
  'fn shipping_option_query_context(',
  'fulfillment latest',
);
const shippingOptionContext = between(
  shim,
  'fn shipping_option_query_context(',
  'fn fulfillment_query_context(',
  'shipping option context',
);
const fulfillmentContext = between(
  shim,
  'fn fulfillment_query_context(',
  'fn with_current_graphql_public_channel(',
  'fulfillment lifecycle context',
);
const channelContextHelper = between(
  shim,
  'fn with_current_graphql_public_channel(',
  '#[allow(clippy::too_many_arguments)]\nfn map_shipping_option_lookup_port_error(',
  'fulfillment channel context helper',
);
const fulfillmentCallContext = between(
  graphqlRuntime,
  'pub(crate) struct CommerceFulfillmentReadCallContext {',
  'tokio::task_local! {',
  'fulfillment request call context',
);
const adminQuery = between(
  query,
  'async fn shipping_options(',
  'async fn shipping_profiles(',
  'admin shipping query',
);
const portBoundary = shim.slice(shim.indexOf('fn map_shipping_option_lookup_port_error('));
if (!portBoundary) failures.push('unable to isolate fulfillment port boundary');

for (const [source, value, label] of [
  [facade, 'mod rustok_fulfillment_shim;', 'private fulfillment facade'],
  [facade, 'use self::rustok_fulfillment_shim as rustok_fulfillment;', 'safe fulfillment routing'],
  [shim, 'shipping_option_reads: Arc<dyn ShippingOptionReadPort>', 'storefront port field'],
  [shim, 'shipping_option_admin_reads: Arc<dyn ShippingOptionAdminReadPort>', 'admin port field'],
  [shim, 'fulfillment_reads: Arc<dyn FulfillmentReadPort>', 'lifecycle port field'],
  [shim, 'pub enum FulfillmentError {', 'typed shim error'],
  [shim, 'Public(BoundaryError)', 'typed public variant'],
  [shim, 'pub(crate) fn to_string(self) -> BoundaryError', 'typed source bridge'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['shipping_option_read_runtime_for_current_graphql_scope(', 'shipping scoped runtime'],
  ['shipping_option_runtime.shipping_option_read_port()', 'storefront port injection'],
  ['shipping_option_runtime\n                .shipping_option_admin_read_port()', 'admin port injection'],
  ['fulfillment_lifecycle_read_runtime_for_current_graphql_scope(db)', 'lifecycle scoped runtime'],
  ['fulfillment_lifecycle_runtime.fulfillment_read_port()', 'lifecycle port injection'],
]) requireText(constructor, value, label);
for (const value of [
  '::rustok_fulfillment::FulfillmentService::new(db)',
  'in_process_shipping_option_read_port',
  'in_process_shipping_option_admin_read_port',
  'in_process_fulfillment_read_port',
]) forbidText(shim, value, 'private facade concrete read construction');

for (const [source, value, label] of [
  [optionLookup, 'FulfillmentResult<ShippingOptionResponse>', 'optional option result'],
  [optionLookup, '.read_shipping_option_projection(', 'owner option lookup'],
  [optionLookup, 'ReadShippingOptionProjectionRequest {', 'typed option request'],
  [optionList, 'Result<Vec<ShippingOptionResponse>, BoundaryError>', 'storefront option result'],
  [optionList, '.list_shipping_option_projections(', 'owner option list'],
  [optionAdminList, 'Result<Vec<ShippingOptionResponse>, ShippingOptionAdminQueryError>', 'admin option result'],
  [optionAdminList, '.list_all_shipping_option_projections(', 'owner admin list'],
  [fulfillmentLookup, 'FulfillmentResult<FulfillmentResponse>', 'fulfillment result'],
  [fulfillmentLookup, '.read_fulfillment_projection(', 'owner fulfillment lookup'],
  [fulfillmentLookup, 'ReadFulfillmentProjectionRequest { fulfillment_id: id }', 'typed fulfillment request'],
  [fulfillmentList, 'ListFulfillmentsInput {', 'list compatibility input'],
  [fulfillmentList, '.list_fulfillment_projections(', 'owner fulfillment list'],
  [fulfillmentList, 'ListFulfillmentProjectionsRequest {', 'typed list request'],
  [fulfillmentList, 'Ok((page_result.items, page_result.total))', 'list compatibility result'],
  [fulfillmentLatest, '.find_latest_fulfillment_by_order_projection(', 'owner latest operation'],
  [fulfillmentLatest, 'FindLatestFulfillmentByOrderProjectionRequest { order_id }', 'typed latest request'],
]) requireText(source, value, label);

for (const [source, value, label] of [
  [shippingOptionContext, 'with_current_graphql_public_channel(', 'shipping-option channel attachment'],
  [shippingOptionContext, 'PortActor::service("rustok-commerce.graphql-query-shipping-options")', 'shipping-option actor'],
  [fulfillmentContext, 'with_current_graphql_public_channel(', 'fulfillment channel attachment'],
  [fulfillmentContext, 'PortActor::service("rustok-commerce.graphql-query-fulfillments")', 'lifecycle actor'],
  [channelContextHelper, 'fulfillment_read_call_context_for_current_graphql_scope()', 'scoped channel read'],
  [channelContextHelper, 'Some(channel) => context.with_channel(channel)', 'port channel propagation'],
  [channelContextHelper, 'None => context', 'standalone no-channel fallback'],
]) requireText(source, value, label);
for (const value of [
  'RequestContext',
  'channel_slug',
  'public_channel_slug_from_request',
]) forbidText(channelContextHelper, value, 'private facade request-context ownership');
const channelHelperReferences = shim.match(/with_current_graphql_public_channel\(/g) ?? [];
if (channelHelperReferences.length !== 3) {
  failures.push(
    `expected one helper plus two fulfillment context applications, found ${channelHelperReferences.length}`,
  );
}

for (const [value, label] of [
  ['pub(crate) struct ShippingOptionAdminQueryError(BoundaryError);', 'typed admin error'],
  ['fn fulfillment_query_context(', 'lifecycle context builder'],
  ['graphql-fulfillment-lifecycle:{query_field}:{operation}:{resource}', 'lifecycle correlation'],
  ['with_deadline(std::time::Duration::from_secs(2))', 'read deadline'],
  ['fn map_fulfillment_port_error(', 'lifecycle mapper'],
  ['if matches!(&error.kind, PortErrorKind::NotFound)', 'optional not-found'],
  ['FulfillmentError::Public(BoundaryError::Public {', 'typed public mapping'],
  ['PortErrorKind::Forbidden', 'forbidden classification'],
  ['PortErrorKind::InvariantViolation', 'invariant classification'],
  ['"FULFILLMENT_ACCESS_DENIED"', 'forbidden code'],
  ['"FULFILLMENT_OPERATION_FAILED"', 'invariant code'],
  ['struct FulfillmentQueryDiagnosticError;', 'redacted diagnostic token'],
  ['formatter.write_str("redacted")', 'redacted diagnostic Debug'],
  ['fn fulfillment_query_context_facts(', 'bounded context facts'],
  ['fn optional_uuid_shape(', 'resource shape helper'],
  ['tenant_id_length = facts.tenant_id_length', 'tenant length logging'],
  ['actor_kind = facts.actor_kind', 'actor kind logging'],
  ['actor_id_length = facts.actor_id_length', 'actor length logging'],
  ['correlation_id_length = facts.correlation_id_length', 'correlation length logging'],
  ['channel_present = facts.channel_present', 'channel presence logging'],
  ['channel_length = ?facts.channel_length', 'channel length logging'],
  ['deadline_ms = ?facts.deadline_ms', 'deadline logging'],
  ['shipping_option_id_shape', 'shipping option shape logging'],
  ['fulfillment_id_shape', 'fulfillment shape logging'],
  ['order_id_shape', 'order shape logging'],
  ['owner_code = %error.code', 'owner code logging'],
  ['owner_message_presence', 'owner message presence logging'],
  ['owner_message_length', 'owner message length logging'],
  ['tracing::error!(', 'technical severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
]) requireText(portBoundary, value, label);
for (const value of [
  'error = ?error',
  'correlation_id = %context.correlation_id',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = %',
  'shipping_option_id = ?shipping_option_id',
  'fulfillment_id = ?fulfillment_id',
  'order_id = ?order_id',
  'owner_kind = ?error.kind',
  'public_message,',
]) forbidText(portBoundary, value, 'raw fulfillment query diagnostic');
for (const value of ['impl std::fmt::Display', 'impl Display', 'format!("{}", self.0)']) {
  forbidText(shim, value, 'typed boundary text serialization');
}

for (const [source, value, label] of [
  [graphqlRuntime, 'pub struct CommerceShippingOptionReadRuntime {', 'shipping runtime'],
  [graphqlRuntime, 'pub struct CommerceFulfillmentLifecycleReadRuntime {', 'lifecycle runtime'],
  [graphqlRuntime, 'pub(crate) struct CommerceFulfillmentReadCallContext {', 'fulfillment call context'],
  [fulfillmentCallContext, 'channel: Option<String>', 'bounded channel storage'],
  [fulfillmentCallContext, 'ctx.data_opt::<RequestContext>()', 'trusted request context'],
  [fulfillmentCallContext, '.and_then(crate::storefront_channel::public_channel_slug_from_request)', 'normalized public channel'],
  [fulfillmentCallContext, 'pub(crate) fn channel(&self) -> Option<&str>', 'channel accessor'],
  [graphqlRuntime, 'tokio::task_local! {', 'task-local scope'],
  [graphqlRuntime, 'CURRENT_COMMERCE_FULFILLMENT_LIFECYCLE_READ_RUNTIME', 'lifecycle scope identity'],
  [graphqlRuntime, 'CURRENT_COMMERCE_FULFILLMENT_READ_CALL_CONTEXT', 'fulfillment call scope identity'],
  [graphqlRuntime, 'CommerceFulfillmentReadCallContext::from_extension_context(ctx)', 'request channel derivation'],
  [graphqlRuntime, 'CURRENT_COMMERCE_FULFILLMENT_READ_CALL_CONTEXT.scope(', 'request channel scoping'],
  [graphqlRuntime, 'fulfillment_read_call_context_for_current_graphql_scope', 'scoped channel accessor'],
  [graphqlRuntime, 'pub struct CommerceShippingOptionReadScope;', 'shared extension'],
  [graphqlRuntime, 'runtime_data.fulfillment_lifecycle_read_runtime()', 'lifecycle scope value'],
  [graphqlRuntime, 'CommerceFulfillmentLifecycleReadRuntime::in_process(db)', 'standalone fallback'],
  [graphqlRuntime, '.shared_get::<CommerceFulfillmentLifecycleReadRuntime>()', 'manifest host selection'],
  [serverComposition, 'shared_get::<rustok_commerce::graphql_runtime::CommerceFulfillmentLifecycleReadRuntime>()', 'server runtime reuse'],
  [serverComposition, 'host.with_shared_value(runtime)', 'host attachment'],
  [serverSchema, 'use rustok_commerce::graphql_runtime::CommerceShippingOptionReadScope;', 'server extension import'],
  [serverSchema, 'let builder = builder.extension(CommerceShippingOptionReadScope);', 'server extension mount'],
  [commerceCargo, 'tokio.workspace = true', 'task-local dependency'],
]) requireText(source, value, label);
forbidText(
  fulfillmentCallContext,
  'request.channel_slug.clone()',
  'un-normalized fulfillment request channel',
);

for (const [value, label] of [
  ['use rustok_fulfillment::FulfillmentService;', 'unchanged facade import'],
  ['.get_fulfillment(tenant_id, id)', 'query lookup facade call'],
  ['.list_fulfillments(', 'query list facade call'],
  ['.find_by_order(tenant_id, id)', 'query latest facade call'],
  ['Err(rustok_fulfillment::error::FulfillmentError::FulfillmentNotFound(_))', 'optional lookup none'],
  ['Err(err) => return Err(err.to_string().into())', 'typed source bridge'],
]) requireText(query, value, label);
for (const [value, label] of [
  ['.list_all_shipping_options(', 'admin option facade call'],
  ['.map_err(|err| async_graphql::Error::new(err.to_string()))?', 'admin typed bridge'],
  ['active: None,', 'admin active filter'],
  ['items.retain(|option| option.active == active);', 'admin filtering'],
]) requireText(adminQuery, value, label);
forbidText(query, '::rustok_fulfillment::FulfillmentService', 'query concrete service path');

if ((facade.match(/::rustok_fulfillment::FulfillmentService::new\(db\)/g) ?? []).length !== 0) {
  failures.push('facade must not construct concrete fulfillment service');
}
if ((graphqlRuntime.match(/in_process_shipping_option_read_port\(db\.clone\(\)\)/g) ?? []).length !== 1) {
  failures.push('expected one standalone storefront factory');
}
if ((graphqlRuntime.match(/in_process_shipping_option_admin_read_port\(db\)/g) ?? []).length !== 1) {
  failures.push('expected one standalone admin factory');
}
if ((graphqlRuntime.match(/in_process_fulfillment_read_port\(db\)/g) ?? []).length !== 1) {
  failures.push('expected one standalone lifecycle factory');
}

for (const [source, value, label] of [
  [shippingOwner, 'pub trait ShippingOptionReadPort: Send + Sync {', 'owner storefront port'],
  [shippingOwner, 'pub trait ShippingOptionAdminReadPort: Send + Sync {', 'owner admin port'],
  [lifecycleOwner, 'pub trait FulfillmentReadPort: Send + Sync {', 'owner lifecycle port'],
  [lifecycleOwner, 'async fn read_fulfillment_projection(', 'owner lookup'],
  [lifecycleOwner, 'async fn list_fulfillment_projections(', 'owner list'],
  [lifecycleOwner, 'async fn find_latest_fulfillment_by_order_projection(', 'owner latest'],
  [lifecycleOwner, 'context.require_policy(PortCallPolicy::read())?', 'owner read policy'],
  [lifecycleOwner, 'PortError::new(kind, code, message, retryable)', 'owner stable error'],
]) requireText(source, value, label);

if (failures.length > 0) {
  console.error('Commerce GraphQL query fulfillment context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ mounted Commerce GraphQL shipping-option and fulfillment lifecycle reads retain host-scoped owner ports, normalized public channel context, typed failures, bounded diagnostics, optional not-found, and two-second policy',
);
