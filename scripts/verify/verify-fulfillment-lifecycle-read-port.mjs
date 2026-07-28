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

forbidText(
  hostRuntime,
  'CommerceFulfillmentLifecycleReadRuntime',
  'default server lifecycle runtime composition before consumer cutover',
);

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
  [adminList, 'FulfillmentService::new(runtime.db_clone())', 'retained admin list service'],
  [adminList, '.list_fulfillments(', 'retained admin list operation'],
  [adminShow, 'FulfillmentService::new(runtime.db_clone())', 'retained admin show service'],
  [adminShow, '.get_fulfillment(tenant.id, id)', 'retained admin show operation'],
]) requireText(source, value, label);

for (const value of [
  'error = %message',
  'message = %',
  'error.message',
]) forbidText(ownerSource, value, 'owner message exposure');

if (evidence.status !== 'source_ready_unvalidated') {
  failures.push(`evidence status: expected source_ready_unvalidated, found ${evidence.status}`);
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
if (evidence.runtime_publication?.server_cache_composed !== false) {
  failures.push('evidence must retain default server cache composition as false');
}
if (evidence.runtime_publication?.default_server_attachment !== false) {
  failures.push('evidence must retain default server attachment as false');
}
if (evidence.consumer_cutover?.graphql_compatibility_facade !== false) {
  failures.push('evidence must retain GraphQL facade cutover as false');
}
if (evidence.consumer_cutover?.admin_rest_list_show !== false) {
  failures.push('evidence must retain admin REST list/show cutover as false');
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
  '✔ fulfillment lifecycle reads are owner-published with a host-selectable Commerce runtime while default server and consumer cutovers remain explicitly open',
);
