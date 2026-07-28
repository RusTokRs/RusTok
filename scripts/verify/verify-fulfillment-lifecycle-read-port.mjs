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

const ownerRoot = read('crates/rustok-fulfillment/src/lib.rs');
const ownerSource = read('crates/rustok-fulfillment/src/fulfillment_read.rs');
const commerceRuntime = read('crates/rustok-commerce/src/graphql_runtime.rs');
const hostRuntime = read('apps/server/src/services/commerce_provider_runtime.rs');
const commerceHttp = read('crates/rustok-commerce/src/controllers/mod.rs');
const compatibilityFacade = readCommerceSafeQuerySource(read);
const fulfillmentShim = readCommerceFulfillmentQueryShimSource(read);
const adminRest = read('crates/rustok-commerce/src/controllers/admin/fulfillments.rs');
const evidence = JSON.parse(
  read('crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-read-port-source.json'),
);
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

for (const [source, value, label] of [
  [ownerRoot, 'mod fulfillment_read;', 'private owner module'],
  [ownerRoot, 'FulfillmentReadPort,', 'owner port export'],
  [ownerRoot, 'InProcessFulfillmentReadPort,', 'in-process adapter export'],
  [ownerRoot, 'FulfillmentProjectionPage,', 'page export'],
  [ownerRoot, 'ReadFulfillmentProjectionRequest,', 'read request export'],
  [ownerRoot, 'ListFulfillmentProjectionsRequest,', 'list request export'],
  [ownerRoot, 'FindLatestFulfillmentByOrderProjectionRequest,', 'latest request export'],
  [ownerRoot, 'in_process_fulfillment_read_port,', 'root factory export'],
  [ownerSource, 'pub trait FulfillmentReadPort: Send + Sync {', 'owner trait'],
  [ownerSource, 'impl FulfillmentReadPort for InProcessFulfillmentReadPort', 'adapter implementation'],
  [ownerSource, 'context.require_policy(PortCallPolicy::read())?', 'read policy'],
  [ownerSource, '.get_fulfillment(tenant_id, request.fulfillment_id)', 'single delegation'],
  [ownerSource, '.list_fulfillments(', 'list delegation'],
  [ownerSource, '.find_by_order(tenant_id, request.order_id)', 'latest delegation'],
  [ownerSource, 'PortError::new(kind, code, message, retryable)', 'stable owner error'],
]) requireText(source, value, label);

for (const value of [
  'FulfillmentError::Validation(_)',
  'FulfillmentError::ShippingOptionNotFound(_)',
  'FulfillmentError::FulfillmentNotFound(_)',
  'FulfillmentError::InvalidTransition { .. }',
  'FulfillmentError::Database(_)',
  'PortErrorKind::Validation',
  'PortErrorKind::NotFound',
  'PortErrorKind::Conflict',
  'PortErrorKind::Unavailable',
]) requireText(ownerSource, value, 'complete owner error mapping');

for (const [source, value, label] of [
  [commerceRuntime, 'pub struct CommerceFulfillmentLifecycleReadRuntime {', 'Commerce runtime'],
  [commerceRuntime, 'fulfillment_reads: Arc<dyn FulfillmentReadPort>', 'runtime port field'],
  [commerceRuntime, 'Self::new(in_process_fulfillment_read_port(db))', 'runtime in-process factory'],
  [commerceRuntime, 'CURRENT_COMMERCE_FULFILLMENT_LIFECYCLE_READ_RUNTIME', 'GraphQL task-local'],
  [commerceRuntime, 'runtime_data.fulfillment_lifecycle_read_runtime()', 'resolver scope value'],
  [commerceRuntime, '.shared_get::<CommerceFulfillmentLifecycleReadRuntime>()', 'GraphQL host override'],
  [hostRuntime, '.shared_get::<rustok_commerce::graphql_runtime::CommerceFulfillmentLifecycleReadRuntime>()', 'server runtime reuse'],
  [hostRuntime, 'rustok_commerce::graphql_runtime::CommerceFulfillmentLifecycleReadRuntime::in_process(', 'server in-process composition'],
  [hostRuntime, 'server.shared_insert(runtime.clone());', 'server cache'],
  [hostRuntime, 'host.with_shared_value(runtime)', 'host attachment'],
  [commerceHttp, 'fn fulfillment_read_port(', 'HTTP port getter'],
  [commerceHttp, '.shared_get::<crate::graphql_runtime::CommerceFulfillmentLifecycleReadRuntime>()', 'HTTP host lookup'],
  [commerceHttp, 'Commerce HTTP routes require CommerceFulfillmentLifecycleReadRuntime in HostRuntimeContext', 'HTTP fail-closed requirement'],
]) requireText(source, value, label);

const constructor = between(
  fulfillmentShim,
  'pub fn new(db: DatabaseConnection) -> Self {',
  'pub async fn get_shipping_option(',
  'GraphQL facade constructor',
);
const lookup = between(
  fulfillmentShim,
  'pub async fn get_fulfillment(',
  'pub async fn list_fulfillments(',
  'GraphQL fulfillment lookup',
);
const list = between(
  fulfillmentShim,
  'pub async fn list_fulfillments(',
  'pub async fn find_by_order(',
  'GraphQL fulfillment list',
);
const latest = between(
  fulfillmentShim,
  'pub async fn find_by_order(',
  'fn shipping_option_query_context(',
  'GraphQL latest-by-order',
);

for (const [source, value, label] of [
  [fulfillmentShim, 'fulfillment_reads: Arc<dyn FulfillmentReadPort>', 'facade owner port'],
  [constructor, 'fulfillment_lifecycle_read_runtime_for_current_graphql_scope(db)', 'scoped runtime resolution'],
  [constructor, 'fulfillment_reads: fulfillment_lifecycle_runtime.fulfillment_read_port()', 'scoped port injection'],
  [lookup, '.read_fulfillment_projection(', 'GraphQL owner lookup'],
  [lookup, 'ReadFulfillmentProjectionRequest { fulfillment_id: id }', 'typed lookup request'],
  [list, '.list_fulfillment_projections(', 'GraphQL owner list'],
  [list, 'ListFulfillmentProjectionsRequest {', 'typed list request'],
  [list, 'Ok((page_result.items, page_result.total))', 'list envelope'],
  [latest, '.find_latest_fulfillment_by_order_projection(', 'GraphQL latest operation'],
  [latest, 'FindLatestFulfillmentByOrderProjectionRequest { order_id }', 'typed latest request'],
  [fulfillmentShim, 'pub enum FulfillmentError {', 'typed shim error'],
  [fulfillmentShim, 'Public(BoundaryError)', 'typed public error variant'],
  [fulfillmentShim, 'Self::Public(error) => error', 'typed boundary restoration'],
  [fulfillmentShim, 'if matches!(&error.kind, PortErrorKind::NotFound)', 'optional not-found branch'],
  [fulfillmentShim, 'FulfillmentError::Public(BoundaryError::Public {', 'typed error mapping'],
  [fulfillmentShim, 'with_deadline(std::time::Duration::from_secs(2))', 'GraphQL deadline'],
  [fulfillmentShim, 'PortActor::service("rustok-commerce.graphql-query-fulfillments")', 'GraphQL actor'],
  [fulfillmentShim, 'graphql-fulfillment-lifecycle:{query_field}:{operation}:{resource}', 'GraphQL correlation'],
]) requireText(source, value, label);

for (const value of [
  '"FULFILLMENT_REQUEST_INVALID"',
  '"FULFILLMENT_RESOURCE_NOT_FOUND"',
  '"FULFILLMENT_STATE_CONFLICT"',
  '"FULFILLMENT_TEMPORARILY_UNAVAILABLE"',
  '"FULFILLMENT_ACCESS_DENIED"',
  '"FULFILLMENT_OPERATION_FAILED"',
]) requireText(fulfillmentShim, value, 'typed GraphQL public policy');

for (const value of [
  'inner: ::rustok_fulfillment::FulfillmentService',
  '::rustok_fulfillment::FulfillmentService::new(db)',
  'self.inner',
  'DbErr::Custom("fulfillment storage is temporarily unavailable"',
  'FulfillmentError::Validation("fulfillment query is not permitted"',
]) forbidText(compatibilityFacade, value, 'GraphQL concrete or downgraded delegate');

const adminList = between(
  adminRest,
  'pub async fn list_fulfillments(',
  '/// Create admin fulfillment',
  'admin list',
);
const adminShow = between(
  adminRest,
  'pub async fn show_fulfillment(',
  '/// Ship admin fulfillment',
  'admin show',
);
for (const [source, value, label] of [
  [adminList, 'request_context: RequestContext', 'admin list context'],
  [adminList, '.fulfillment_read_port()', 'admin list port'],
  [adminList, '.list_fulfillment_projections(', 'admin list operation'],
  [adminList, 'ListFulfillmentProjectionsRequest {', 'admin list request'],
  [adminList, 'data: page.items', 'admin list projection'],
  [adminList, 'page.total', 'admin list total'],
  [adminShow, 'request_context: RequestContext', 'admin show context'],
  [adminShow, '.fulfillment_read_port()', 'admin show port'],
  [adminShow, '.read_fulfillment_projection(', 'admin show operation'],
]) requireText(source, value, label);
for (const value of ['FulfillmentService::new(', '.list_fulfillments(', '.get_fulfillment(']) {
  forbidText(adminList + adminShow, value, 'admin concrete read');
}

if (evidence.status !== 'source_cutover_unvalidated') failures.push('evidence status mismatch');
if (evidence.owner?.port !== 'FulfillmentReadPort') failures.push('evidence owner mismatch');
if (evidence.runtime_publication?.runtime !== 'CommerceFulfillmentLifecycleReadRuntime') {
  failures.push('evidence runtime mismatch');
}
for (const [value, label] of [
  [evidence.runtime_publication?.server_cache_composed, 'server cache'],
  [evidence.runtime_publication?.commerce_http_runtime_required, 'HTTP runtime'],
  [evidence.runtime_publication?.graphql_task_local_scoped, 'GraphQL scope'],
  [evidence.admin_rest?.filters_preserved, 'admin filters'],
  [evidence.admin_rest?.public_error_policy_preserved, 'admin errors'],
  [evidence.graphql?.optional_lookup_not_found_preserved, 'GraphQL optional lookup'],
  [evidence.graphql?.pagination_total_and_filters_preserved, 'GraphQL list'],
  [evidence.graphql?.typed_port_error_extensions_preserved, 'GraphQL typed extensions'],
  [evidence.consumer_cutover?.graphql_compatibility_facade, 'GraphQL cutover'],
  [evidence.consumer_cutover?.admin_rest_list_show, 'REST cutover'],
  [evidence.consumer_cutover?.concrete_delegate_removed, 'delegate removal'],
]) {
  if (value !== true) failures.push(`evidence must record ${label}`);
}
if (evidence.validation?.runtime_parity_proven !== false) {
  failures.push('runtime parity must remain unproven');
}
for (const key of ['tests_run', 'cargo_run', 'format_run', 'verifiers_run', 'workflow_checks_run', 'ci_run']) {
  if (evidence.validation?.[key] !== false) failures.push(`validation.${key} must be false`);
}

if (failures.length > 0) {
  console.error('Fulfillment lifecycle read-port source verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ fulfillment lifecycle reads are owner-published, host-composed, typed across GraphQL/admin REST, and free of concrete Commerce read delegates',
);
