#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const owner = read('crates/rustok-order/src/order_read.rs');
const exports = read('crates/rustok-order/src/lib.rs');
const graphqlRuntime = read('crates/rustok-commerce/src/graphql_runtime.rs');
const graphqlOrderShim = read(
  'crates/rustok-commerce/src/graphql/safe_query/source/rustok_order_shim.rs',
);
const httpRuntime = read('crates/rustok-commerce/src/controllers/mod.rs');
const adminOrders = read('crates/rustok-commerce/src/controllers/admin/orders.rs');
const storefrontOrders = read('crates/rustok-commerce/src/controllers/store/orders.rs');
const serverRuntime = read('apps/server/src/services/commerce_provider_runtime.rs');
const adminFixtures = read('crates/rustok-commerce/src/controllers/admin/tests/mod.rs');
const storefrontFixtures = read('crates/rustok-commerce/src/controllers/store/tests/mod.rs');
const fulfillmentFailureContract = read(
  'crates/rustok-commerce/tests/fulfillment_read_port_failure_contract.rs',
);
const evidence = JSON.parse(
  read('crates/rustok-order/contracts/evidence/order-read-port-source.json'),
);
const note = read('crates/rustok-order/docs/order-read-port.md');
const orderPlan = read('crates/rustok-order/docs/implementation-plan.md');
const commercePlan = read('crates/rustok-commerce/docs/implementation-plan.md');
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
  [owner, 'pub trait OrderReadPort: Send + Sync {', 'owner read trait'],
  [owner, 'async fn read_order_projection(', 'detail operation'],
  [owner, 'async fn list_order_projections(', 'list operation'],
  [owner, 'pub struct ReadOrderProjectionRequest {', 'detail request'],
  [owner, 'pub struct ListOrderProjectionsRequest {', 'list request'],
  [owner, 'pub struct OrderProjectionPage {', 'page projection'],
  [owner, 'pub struct InProcessOrderReadPort {', 'in-process adapter'],
  [owner, 'pub fn in_process_order_read_port(', 'root factory'],
  [owner, 'context.require_policy(PortCallPolicy::read())?', 'read policy'],
  [owner, 'Uuid::parse_str(&context.tenant_id)', 'tenant parsing'],
  [owner, '.get_order_with_locale_fallback(', 'owner detail delegation'],
  [owner, '.list_orders_with_locale_fallback(', 'owner list delegation'],
  [owner, 'context.locale.as_str()', 'requested locale propagation'],
  [owner, 'request.tenant_default_locale.as_deref()', 'fallback locale propagation'],
  [owner, 'OrderError::Validation(_)', 'validation mapping'],
  [owner, 'OrderError::OrderNotFound(_)', 'order not-found mapping'],
  [owner, 'OrderError::OrderReturnNotFound(_)', 'return not-found mapping'],
  [owner, 'OrderError::OrderChangeNotFound(_)', 'change not-found mapping'],
  [owner, 'OrderError::InvalidTransition { .. }', 'transition mapping'],
  [owner, 'OrderError::Database(_)', 'database mapping'],
  [owner, 'OrderError::Core(_)', 'core mapping'],
  [owner, 'PortErrorKind::InvariantViolation', 'core fail-closed kind'],
  [owner, 'PortError::new(kind, code, message, retryable)', 'stable port error'],
  [owner, 'boundary = "order_read_port"', 'owner diagnostic boundary'],
  [exports, 'mod order_read;', 'private owner module'],
  [exports, 'OrderReadPort,', 'root trait export'],
  [exports, 'InProcessOrderReadPort,', 'root adapter export'],
  [exports, 'in_process_order_read_port,', 'root factory export'],
  [graphqlRuntime, 'pub struct CommerceOrderReadRuntime {', 'Commerce order runtime'],
  [graphqlRuntime, 'order_reads: Arc<dyn OrderReadPort>', 'runtime owner port'],
  [graphqlRuntime, 'in_process_order_read_port(db, event_bus)', 'runtime in-process factory'],
  [graphqlRuntime, 'pub fn order_read_port(&self)', 'runtime port getter'],
  [graphqlRuntime, 'order_read_runtime: CommerceOrderReadRuntime,', 'GraphQL runtime data field'],
  [graphqlRuntime, '.shared_get::<CommerceOrderReadRuntime>()', 'GraphQL host runtime requirement'],
  [graphqlRuntime, 'static CURRENT_COMMERCE_ORDER_READ_RUNTIME:', 'GraphQL order task-local runtime'],
  [graphqlRuntime, 'static CURRENT_COMMERCE_ORDER_READ_CALL_CONTEXT:', 'GraphQL order call context'],
  [graphqlRuntime, 'ctx.data_opt::<AuthContext>()', 'GraphQL validated actor source'],
  [graphqlRuntime, 'PortActor::user(auth.user_id.to_string())', 'GraphQL user actor'],
  [graphqlRuntime, 'ctx.data_opt::<RequestContext>()', 'GraphQL resolved request source'],
  [graphqlRuntime, 'request.channel_slug.clone()', 'GraphQL channel slug source'],
  [graphqlRuntime, 'runtime_data.order_read_runtime()', 'GraphQL scoped host runtime'],
  [graphqlRuntime, 'pub(crate) fn order_read_runtime_for_current_graphql_scope(', 'GraphQL runtime accessor'],
  [graphqlRuntime, 'pub(crate) fn order_read_call_context_for_current_graphql_scope()', 'GraphQL call context accessor'],
  [graphqlOrderShim, 'order_read_runtime_for_current_graphql_scope(', 'GraphQL shim scoped runtime lookup'],
  [graphqlOrderShim, 'order_read_call_context_for_current_graphql_scope()', 'GraphQL shim call context lookup'],
  [graphqlOrderShim, 'call_context.actor()', 'GraphQL actor propagation'],
  [graphqlOrderShim, 'context.with_channel(channel)', 'GraphQL channel propagation'],
  [graphqlRuntime, 'commerce GraphQL requires CommerceOrderReadRuntime in host composition', 'GraphQL fail-closed message'],
  [httpRuntime, 'order_read_runtime: crate::graphql_runtime::CommerceOrderReadRuntime,', 'HTTP runtime field'],
  [httpRuntime, 'fn order_read_port(&self)', 'HTTP port getter'],
  [httpRuntime, '.shared_get::<crate::graphql_runtime::CommerceOrderReadRuntime>()', 'HTTP host requirement'],
  [httpRuntime, 'Commerce HTTP routes require CommerceOrderReadRuntime in HostRuntimeContext', 'HTTP fail-closed message'],
  [serverRuntime, '.shared_get::<rustok_commerce::graphql_runtime::CommerceOrderReadRuntime>()', 'server runtime reuse'],
  [serverRuntime, 'CommerceOrderReadRuntime::in_process(', 'server in-process composition'],
  [serverRuntime, 'server.shared_insert(runtime.clone());', 'server runtime cache'],
  [serverRuntime, 'host.with_shared_value(runtime)', 'host runtime attachment'],
  [adminOrders, 'fn admin_order_read_port_context(', 'admin PortContext builder'],
  [adminOrders, 'PortActor::user(auth.user_id.to_string())', 'admin actor propagation'],
  [adminOrders, '.with_deadline(std::time::Duration::from_secs(2))', 'admin deadline'],
  [adminOrders, 'request_context.channel_slug.as_deref()', 'admin channel propagation'],
  [adminOrders, 'fn map_admin_order_port_error(', 'admin PortError mapper'],
  [adminOrders, '.list_order_projections(', 'admin list port call'],
  [adminOrders, '.read_order_projection(', 'admin detail port call'],
  [adminOrders, 'tenant_default_locale: Some(tenant.default_locale.clone())', 'admin locale fallback'],
  [adminOrders, 'data: page.items,', 'admin list envelope'],
  [adminOrders, 'page.total,', 'admin owner total'],
  [adminOrders, 'find_latest_collection_by_order(tenant.id, id)', 'unchanged payment aggregation'],
  [adminOrders, 'find_by_order(tenant.id, id)', 'unchanged fulfillment aggregation'],
  [storefrontOrders, 'fn storefront_order_read_port_context(', 'storefront PortContext builder'],
  [storefrontOrders, 'PortActor::user(auth.user_id.to_string())', 'storefront actor propagation'],
  [storefrontOrders, 'request_context.channel_slug.as_deref()', 'storefront channel propagation'],
  [storefrontOrders, 'async fn read_storefront_order_projection(', 'storefront shared read helper'],
  [storefrontOrders, 'runtime\n        .order_read_port()', 'storefront host-selected port'],
  [storefrontOrders, '.read_order_projection(', 'storefront detail port call'],
  [storefrontOrders, 'tenant_default_locale: Some(tenant_default_locale.to_string())', 'storefront locale fallback'],
  [storefrontOrders, 'fn map_storefront_order_port_error(', 'storefront PortError mapper'],
  [adminFixtures, 'CommerceOrderReadRuntime::in_process(', 'admin fixture runtime'],
  [storefrontFixtures, 'CommerceOrderReadRuntime::in_process(', 'storefront fixture runtime'],
  [fulfillmentFailureContract, 'CommerceOrderReadRuntime::in_process(', 'manual host fixture runtime'],
  [fulfillmentFailureContract, '.with_shared_value(event_bus.clone())', 'manual host fixture shared event bus'],
  [note, 'Status: owner port and host runtime published; admin REST, mounted GraphQL, and storefront HTTP detail/ownership cut over, unvalidated.', 'owner note status'],
  [orderPlan, 'CommerceOrderReadRuntime', 'order plan runtime checkpoint'],
  [commercePlan, 'CommerceOrderReadRuntime', 'commerce plan runtime checkpoint'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['error.message', 'owner message control flow'],
  ['error.to_string()', 'owner error string control flow'],
  ['format!("{error}")', 'formatted owner error control flow'],
  ['PortError::new(kind, code, error', 'raw owner error publication'],
]) forbidText(owner, value, label);

const listRoute = between(
  adminOrders,
  'pub async fn list_orders(',
  '#[utoipa::path(\n    get,\n    path = "/admin/orders/{id}"',
  'admin list route',
);
const showRoute = between(
  adminOrders,
  'pub async fn show_order(',
  'fn map_order_detail_payment_error(',
  'admin detail route',
);
for (const [route, label] of [
  [listRoute, 'admin list route'],
  [showRoute, 'admin detail route'],
]) {
  forbidText(route, 'OrderService::new', `${label} concrete owner construction`);
  forbidText(route, '.list_orders_with_locale_fallback(', `${label} concrete list call`);
  forbidText(route, '.get_order_with_locale_fallback(', `${label} concrete detail call`);
  requireText(route, 'runtime\n        .order_read_port()', `${label} injected owner port`);
}

const storefrontOwnership = between(
  storefrontOrders,
  'async fn ensure_customer_owns_order(',
  '/// Get current storefront customer',
  'storefront ownership helper',
);
const storefrontDetail = between(
  storefrontOrders,
  'pub async fn get_order(',
  '/// Create a return request',
  'storefront detail route',
);
for (const [route, label] of [
  [storefrontOwnership, 'storefront ownership helper'],
  [storefrontDetail, 'storefront detail route'],
]) {
  forbidText(route, 'OrderService::new', `${label} concrete owner construction`);
  forbidText(route, '.get_order(', `${label} concrete detail call`);
  forbidText(route, '.get_order_with_locale_fallback(', `${label} concrete locale detail call`);
  requireText(route, 'read_storefront_order_projection(', `${label} shared owner read`);
}

for (const [value, label] of [
  ['.mark_paid(', 'mark-paid mutation remains owner service'],
  ['.ship_order(', 'ship mutation remains owner service'],
  ['.deliver_order(', 'deliver mutation remains owner service'],
  ['.cancel_order(', 'cancel mutation remains owner service'],
]) requireText(adminOrders, value, label);
for (const [value, label] of [
  ['.create_return(tenant.id, id, input)', 'storefront return mutation remains owner service'],
  ['.list_returns(', 'storefront return list remains owner service'],
  ['.list_order_changes(', 'storefront order-change list remains owner service'],
  ['PaymentService::new(runtime.db_clone())', 'storefront refund list remains payment service'],
]) requireText(storefrontOrders, value, label);

if (evidence.status !== 'storefront_http_cutover_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (evidence.owner?.port !== 'OrderReadPort') {
  failures.push('evidence owner port must be OrderReadPort');
}
if (evidence.owner?.adapter !== 'InProcessOrderReadPort') {
  failures.push('evidence adapter must be InProcessOrderReadPort');
}
if (evidence.operations?.map((operation) => operation.name).join(',') !==
    'read_order_projection,list_order_projections') {
  failures.push('evidence operation inventory mismatch');
}
if (evidence.runtime_composition?.type !== 'CommerceOrderReadRuntime') {
  failures.push('evidence runtime type must be CommerceOrderReadRuntime');
}
for (const key of [
  'server_runtime_reuse',
  'in_process_default_composed_once',
  'host_runtime_attachment',
  'commerce_http_required',
  'commerce_graphql_schema_data_required',
  'graphql_resolver_scope_published',
  'graphql_embedded_fallback_retained',
  'graphql_request_context_scope_published',
]) {
  if (evidence.runtime_composition?.[key] !== true) {
    failures.push(`evidence runtime_composition.${key} must be true`);
  }
}
if (evidence.context?.graphql_actor_source !== 'validated_auth_context_or_service_actor') {
  failures.push('GraphQL actor source must remain validated AuthContext or service actor');
}
if (evidence.context?.graphql_channel_source !== 'resolved_request_context_channel_slug') {
  failures.push('GraphQL channel source must remain the resolved request channel slug');
}
if (evidence.context?.graphql_embedded_context_fallback !== 'service_actor_without_channel') {
  failures.push('GraphQL embedded context fallback must not invent actor or channel attribution');
}
if (evidence.context?.storefront_actor_source !== 'validated_auth_context_user') {
  failures.push('storefront actor source must remain validated AuthContext user');
}
if (evidence.context?.storefront_channel_source !== 'resolved_request_context_channel_slug') {
  failures.push('storefront channel source must remain the resolved request channel slug');
}
if (evidence.errors?.owner_message_control_flow !== false) {
  failures.push('evidence must forbid owner-message control flow');
}
if (evidence.errors?.all_current_order_error_variants_mapped !== true) {
  failures.push('evidence must record complete current OrderError mapping');
}
if (evidence.errors?.storefront_public_envelopes_preserved !== true) {
  failures.push('storefront public envelopes must be preserved');
}
if (evidence.consumer_inventory?.commerce_admin_rest_list_detail !== 'order_read_port') {
  failures.push('admin REST list/detail must be recorded on order_read_port');
}
if (evidence.consumer_inventory?.commerce_admin_rest_cutover_completed !== true) {
  failures.push('admin REST source cutover must be complete');
}
if (evidence.consumer_inventory?.commerce_graphql_order_list_detail !== 'order_read_port_host_runtime_with_request_context') {
  failures.push('GraphQL list/detail must use the host-selected runtime with request context');
}
if (evidence.consumer_inventory?.commerce_graphql_order_list_detail_cutover_completed !== true) {
  failures.push('GraphQL list/detail source cutover must be complete');
}
if (evidence.consumer_inventory?.commerce_storefront_detail_and_ownership !== 'order_read_port_host_runtime') {
  failures.push('storefront detail/ownership must use the host-selected runtime');
}
if (evidence.consumer_inventory?.commerce_storefront_detail_and_ownership_cutover_completed !== true) {
  failures.push('storefront detail/ownership source cutover must be complete');
}
if (evidence.consumer_inventory?.runtime_composition_published !== true) {
  failures.push('runtime composition must be published');
}
if (evidence.consumer_inventory?.all_consumer_cutover_completed !== true) {
  failures.push('complete order projection consumer cutover must be recorded');
}
if (evidence.consumer_inventory?.cutover_required !== false) {
  failures.push('complete order projection consumer cutover must no longer be pending');
}
if (evidence.decision?.status_promotion !== false) {
  failures.push('source cutover must not promote status');
}
for (const key of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
  'runtime_parity_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

if (failures.length > 0) {
  console.error('Order read port source verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Order read runtime is host-composed and admin REST, mounted GraphQL, and storefront detail/ownership use typed owner ports while runtime evidence remains pending',
);
