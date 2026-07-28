#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

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
const compatibilityFacade = read('crates/rustok-commerce/src/graphql/safe_query.rs');
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
  [
    ownerRoot,
    'FindLatestFulfillmentByOrderProjectionRequest,',
    'latest-by-order request export',
  ],
  [ownerRoot, 'in_process_fulfillment_read_port,', 'root factory export'],
  [ownerSource, 'pub trait FulfillmentReadPort: Send + Sync {', 'owner trait'],
  [ownerSource, 'pub struct InProcessFulfillmentReadPort {', 'in-process adapter'],
  [
    ownerSource,
    'impl FulfillmentReadPort for InProcessFulfillmentReadPort',
    'adapter implementation',
  ],
  [ownerSource, 'pub fn in_process_fulfillment_read_port(', 'root factory'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['async fn read_fulfillment_projection(', 'single read operation'],
  ['async fn list_fulfillment_projections(', 'list operation'],
  [
    'async fn find_latest_fulfillment_by_order_projection(',
    'latest-by-order operation',
  ],
  ['pub fulfillment_id: Uuid', 'fulfillment identity request'],
  ['pub page: u64', 'page request'],
  ['pub per_page: u64', 'per-page request'],
  ['pub status: Option<String>', 'status request'],
  ['pub order_id: Option<Uuid>', 'list order request'],
  ['pub customer_id: Option<Uuid>', 'customer request'],
  ['pub items: Vec<FulfillmentResponse>', 'page items'],
  ['pub total: u64', 'page total'],
  ['context.require_policy(PortCallPolicy::read())?', 'read policy'],
  ['.get_fulfillment(tenant_id, request.fulfillment_id)', 'single owner delegation'],
  ['.list_fulfillments(', 'list owner delegation'],
  ['.find_by_order(tenant_id, request.order_id)', 'latest owner delegation'],
  ['FulfillmentError::Validation(_)', 'validation mapping'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping option mapping'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment mapping'],
  ['FulfillmentError::InvalidTransition { .. }', 'conflict mapping'],
  ['FulfillmentError::Database(_)', 'database mapping'],
  ['PortErrorKind::Validation', 'validation kind'],
  ['PortErrorKind::NotFound', 'not-found kind'],
  ['PortErrorKind::Conflict', 'conflict kind'],
  ['PortErrorKind::Unavailable', 'unavailable kind'],
  ['"fulfillment_lifecycle_read_port"', 'owner boundary'],
  ['PortError::new(kind, code, message, retryable)', 'stable error construction'],
]) requireText(ownerSource, value, label);

for (const [value, label] of [
  ['pub struct CommerceFulfillmentLifecycleReadRuntime {', 'Commerce runtime'],
  ['fulfillment_reads: Arc<dyn FulfillmentReadPort>', 'runtime port field'],
  ['pub fn new(fulfillment_reads: Arc<dyn FulfillmentReadPort>)', 'runtime constructor'],
  ['Self::new(in_process_fulfillment_read_port(db))', 'runtime in-process factory'],
  ['pub fn fulfillment_read_port(&self)', 'runtime getter'],
  [
    'fulfillment_lifecycle_read_runtime: CommerceFulfillmentLifecycleReadRuntime',
    'GraphQL runtime-data field',
  ],
  [
    'pub fn fulfillment_lifecycle_read_runtime(&self)',
    'GraphQL runtime-data getter',
  ],
  [
    '.shared_get::<CommerceFulfillmentLifecycleReadRuntime>()',
    'GraphQL host override',
  ],
  [
    'CommerceFulfillmentLifecycleReadRuntime::in_process(inputs.db_clone())',
    'GraphQL in-process compatibility fallback',
  ],
]) requireText(commerceRuntime, value, label);

for (const [value, label] of [
  [
    '.shared_get::<rustok_commerce::graphql_runtime::CommerceFulfillmentLifecycleReadRuntime>()',
    'default server lifecycle runtime reuse',
  ],
  [
    'rustok_commerce::graphql_runtime::CommerceFulfillmentLifecycleReadRuntime::in_process(',
    'default server in-process composition',
  ],
  ['server.shared_insert(runtime.clone());', 'default server runtime cache'],
  ['host.with_shared_value(runtime)', 'default host runtime attachment'],
]) requireText(hostRuntime, value, label);

for (const [value, label] of [
  [
    'fulfillment_lifecycle_read_runtime:\n        crate::graphql_runtime::CommerceFulfillmentLifecycleReadRuntime',
    'Commerce HTTP runtime field',
  ],
  ['fn fulfillment_read_port(', 'Commerce HTTP port getter'],
  [
    '.shared_get::<crate::graphql_runtime::CommerceFulfillmentLifecycleReadRuntime>()',
    'Commerce HTTP host lookup',
  ],
  [
    'Commerce HTTP routes require CommerceFulfillmentLifecycleReadRuntime in HostRuntimeContext',
    'Commerce HTTP fail-closed message',
  ],
  ['fulfillment_lifecycle_read_runtime,', 'Commerce HTTP runtime initialization'],
]) requireText(commerceHttp, value, label);

for (const [value, label] of [
  ['inner: ::rustok_fulfillment::FulfillmentService,', 'retained concrete facade field'],
  [
    'inner: ::rustok_fulfillment::FulfillmentService::new(db)',
    'retained concrete facade construction',
  ],
  [
    'self.inner\n                    .get_fulfillment(',
    'retained GraphQL single-read consumer',
  ],
  [
    'self.inner\n                    .list_fulfillments(',
    'retained GraphQL list consumer',
  ],
  [
    'self.inner\n                    .find_by_order(',
    'retained GraphQL latest-by-order consumer',
  ],
]) requireText(compatibilityFacade, value, label);

const adminList = between(
  adminRest,
  'pub async fn list_fulfillments(',
  '/// Create admin fulfillment',
  'admin fulfillment list',
);
const adminShow = between(
  adminRest,
  'pub async fn show_fulfillment(',
  '/// Ship admin fulfillment',
  'admin fulfillment show',
);
for (const [source, value, label] of [
  [adminList, 'request_context: RequestContext', 'admin list request context'],
  [adminList, '.fulfillment_read_port()', 'admin list runtime port'],
  [adminList, '.list_fulfillment_projections(', 'admin list owner operation'],
  [adminList, 'ListFulfillmentProjectionsRequest {', 'admin list typed request'],
  [adminList, 'status: params.status', 'admin list status filter'],
  [adminList, 'order_id: params.order_id', 'admin list order filter'],
  [adminList, 'customer_id: params.customer_id', 'admin list customer filter'],
  [adminList, 'data: page.items', 'admin list owner projection'],
  [adminList, 'page.total', 'admin list owner total'],
  [adminShow, 'request_context: RequestContext', 'admin show request context'],
  [adminShow, '.fulfillment_read_port()', 'admin show runtime port'],
  [adminShow, '.read_fulfillment_projection(', 'admin show owner operation'],
  [adminShow, 'ReadFulfillmentProjectionRequest { fulfillment_id: id }', 'admin show typed request'],
]) requireText(source, value, label);

for (const [source, value, label] of [
  [adminList, 'FulfillmentService::new(', 'admin list concrete service'],
  [adminList, '.list_fulfillments(', 'admin list concrete operation'],
  [adminShow, 'FulfillmentService::new(', 'admin show concrete service'],
  [adminShow, '.get_fulfillment(', 'admin show concrete operation'],
]) forbidText(source, value, label);

for (const [value, label] of [
  ['fn admin_fulfillment_read_port_context(', 'admin read context builder'],
  ['PortActor::user(auth.user_id.to_string())', 'admin user actor'],
  ['request_context.locale.as_str()', 'admin locale propagation'],
  ['request_context.channel_slug.as_deref()', 'admin channel propagation'],
  ['commerce-admin-fulfillment:{operation}:{resource_id}', 'admin correlation id'],
  ['with_deadline(std::time::Duration::from_secs(2))', 'admin read deadline'],
  ['fn map_admin_fulfillment_port_error(', 'admin typed error mapper'],
  ['PortErrorKind::Forbidden', 'admin forbidden mapping'],
  ['PortErrorKind::Timeout', 'admin timeout mapping'],
  ['PortErrorKind::InvariantViolation', 'admin invariant mapping'],
  ['"commerce_admin_fulfillment_invalid"', 'admin validation public code'],
  ['"commerce_admin_not_found"', 'admin not-found public code'],
  ['"commerce_admin_fulfillment_state_conflict"', 'admin conflict public code'],
  ['"commerce_admin_fulfillment_storage_unavailable"', 'admin unavailable public code'],
  ['"commerce_admin_fulfillment_failed"', 'admin invariant public code'],
  ['internal_code = %error.code', 'admin stable owner code logging'],
]) requireText(adminRest, value, label);

for (const value of [
  'error = %message',
  'message = %',
  'error.message',
]) forbidText(ownerSource, value, 'owner message exposure');

if (evidence.status !== 'source_rest_cutover_unvalidated') {
  failures.push(
    `evidence status: expected source_rest_cutover_unvalidated, found ${evidence.status}`,
  );
}
if (evidence.owner?.port !== 'FulfillmentReadPort') {
  failures.push('evidence owner port must be FulfillmentReadPort');
}
if (evidence.runtime_publication?.runtime !== 'CommerceFulfillmentLifecycleReadRuntime') {
  failures.push('evidence runtime publication mismatch');
}
if (evidence.runtime_publication?.graphql_host_override_supported !== true) {
  failures.push('evidence must record GraphQL host override support');
}
if (evidence.runtime_publication?.graphql_manifest_fallback !== 'in_process') {
  failures.push('evidence must record the GraphQL in-process fallback');
}
if (evidence.runtime_publication?.server_cache_composed !== true) {
  failures.push('evidence must record default server cache composition');
}
if (evidence.runtime_publication?.default_server_attachment !== true) {
  failures.push('evidence must record default server attachment');
}
if (evidence.runtime_publication?.commerce_http_runtime_required !== true) {
  failures.push('evidence must record mandatory Commerce HTTP runtime injection');
}
if (evidence.runtime_publication?.external_adapter_preserved !== true) {
  failures.push('evidence must record preservation of external adapters');
}
if (evidence.admin_rest?.pagination_total_preserved !== true) {
  failures.push('evidence must record preserved admin pagination total');
}
if (evidence.admin_rest?.filters_preserved !== true) {
  failures.push('evidence must record preserved admin list filters');
}
if (evidence.admin_rest?.public_error_policy_preserved !== true) {
  failures.push('evidence must record preserved admin public error policy');
}
if (evidence.admin_rest?.concrete_service_read_construction !== false) {
  failures.push('evidence must forbid concrete admin REST read construction');
}
if (evidence.admin_rest?.mutation_service_construction_unchanged !== true) {
  failures.push('evidence must retain lifecycle mutation service construction');
}
if (evidence.consumer_cutover?.graphql_compatibility_facade !== false) {
  failures.push('evidence must retain GraphQL facade cutover as false');
}
if (evidence.consumer_cutover?.admin_rest_list_show !== true) {
  failures.push('evidence must record admin REST list/show cutover');
}
if (evidence.consumer_cutover?.concrete_delegate_removed !== false) {
  failures.push('evidence must retain concrete delegate removal as false');
}
for (const key of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must be false`);
  }
}

if (failures.length > 0) {
  console.error('Fulfillment lifecycle read-port source verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ fulfillment lifecycle reads are owner-published and host-composed; admin REST list/show consume the typed port while GraphQL cutover remains explicitly open',
);
