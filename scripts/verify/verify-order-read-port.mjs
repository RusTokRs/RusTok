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
  [owner, 'async fn read_order_projection(', 'order detail operation'],
  [owner, 'async fn list_order_projections(', 'order list operation'],
  [owner, 'async fn read_order_return_projection(', 'return detail operation'],
  [owner, 'async fn list_order_return_projections(', 'return list operation'],
  [owner, 'async fn read_order_change_projection(', 'change detail operation'],
  [owner, 'async fn list_order_change_projections(', 'change list operation'],
  [owner, 'pub struct ReadOrderProjectionRequest {', 'order detail request'],
  [owner, 'pub struct ListOrderProjectionsRequest {', 'order list request'],
  [owner, 'pub struct ReadOrderReturnProjectionRequest {', 'return detail request'],
  [owner, 'pub struct ListOrderReturnProjectionsRequest {', 'return list request'],
  [owner, 'pub struct ReadOrderChangeProjectionRequest {', 'change detail request'],
  [owner, 'pub struct ListOrderChangeProjectionsRequest {', 'change list request'],
  [owner, 'pub struct OrderProjectionPage {', 'order page'],
  [owner, 'pub struct OrderReturnProjectionPage {', 'return page'],
  [owner, 'pub struct OrderChangeProjectionPage {', 'change page'],
  [owner, 'pub struct InProcessOrderReadPort {', 'in-process adapter'],
  [owner, 'pub fn in_process_order_read_port(', 'root factory'],
  [owner, 'context.require_policy(PortCallPolicy::read())?', 'read policy'],
  [owner, 'Uuid::parse_str(&context.tenant_id)', 'tenant parsing'],
  [owner, '.get_order_with_locale_fallback(', 'order detail delegation'],
  [owner, '.list_orders_with_locale_fallback(', 'order list delegation'],
  [owner, '.get_return(tenant_id, request.return_id)', 'return detail delegation'],
  [owner, '.list_returns(', 'return list delegation'],
  [owner, '.get_order_change(tenant_id, request.change_id)', 'change detail delegation'],
  [owner, '.list_order_changes(', 'change list delegation'],
  [owner, 'OrderError::Validation(_)', 'validation mapping'],
  [owner, 'OrderError::OrderNotFound(_)', 'order not-found mapping'],
  [owner, 'OrderError::OrderReturnNotFound(_)', 'return not-found mapping'],
  [owner, 'OrderError::OrderChangeNotFound(_)', 'change not-found mapping'],
  [owner, 'OrderError::Database(_)', 'database mapping'],
  [owner, 'OrderError::Core(_)', 'core mapping'],
  [owner, 'PortError::new(kind, code, message, retryable)', 'stable port error'],
  [exports, 'ListOrderReturnProjectionsRequest', 'return contract export'],
  [exports, 'ListOrderChangeProjectionsRequest', 'change contract export'],
  [graphqlRuntime, 'pub struct CommerceOrderReadRuntime {', 'Commerce order runtime'],
  [graphqlRuntime, 'order_reads: Arc<dyn OrderReadPort>', 'runtime owner port'],
  [graphqlRuntime, 'runtime_data.order_read_runtime()', 'GraphQL scoped runtime'],
  [graphqlRuntime, 'ctx.data_opt::<AuthContext>()', 'GraphQL actor source'],
  [graphqlRuntime, 'ctx.data_opt::<RequestContext>()', 'GraphQL request source'],
  [graphqlOrderShim, 'order_read_runtime_for_current_graphql_scope(', 'GraphQL runtime lookup'],
  [httpRuntime, 'fn order_read_port(&self)', 'HTTP port getter'],
  [serverRuntime, 'CommerceOrderReadRuntime::in_process(', 'server composition'],
  [adminOrders, '.list_order_projections(', 'admin list port call'],
  [adminOrders, '.read_order_projection(', 'admin detail port call'],
  [storefrontOrders, '.read_order_projection(', 'storefront detail port call'],
  [storefrontOrders, '.list_order_return_projections(', 'storefront return port call'],
  [storefrontOrders, '.list_order_change_projections(', 'storefront change port call'],
  [storefrontOrders, 'ListOrderReturnProjectionsRequest {', 'storefront return request'],
  [storefrontOrders, 'ListOrderChangeProjectionsRequest {', 'storefront change request'],
  [storefrontOrders, 'data: page.items,', 'storefront typed page items'],
  [storefrontOrders, 'page.total', 'storefront typed page total'],
  [note, 'complete order projections plus storefront return/change lists cut over', 'owner note status'],
  [orderPlan, 'OrderReadPort', 'order plan checkpoint'],
  [commercePlan, 'OrderReadPort', 'commerce plan checkpoint'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['error.message', 'owner message control flow'],
  ['error.to_string()', 'owner error string control flow'],
  ['format!("{error}")', 'formatted owner error control flow'],
  ['PortError::new(kind, code, error', 'raw owner error publication'],
]) forbidText(owner, value, label);

const adminList = between(
  adminOrders,
  'pub async fn list_orders(',
  '#[utoipa::path(\n    get,\n    path = "/admin/orders/{id}"',
  'admin list route',
);
const adminDetail = between(
  adminOrders,
  'pub async fn show_order(',
  'fn map_order_detail_payment_error(',
  'admin detail route',
);
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
const storefrontReturns = between(
  storefrontOrders,
  'pub async fn list_order_returns(',
  '/// List refunds',
  'storefront return list route',
);
const storefrontChanges = storefrontOrders.slice(
  storefrontOrders.indexOf('pub async fn list_order_changes('),
);

for (const [route, label] of [
  [adminList, 'admin list route'],
  [adminDetail, 'admin detail route'],
  [storefrontOwnership, 'storefront ownership helper'],
  [storefrontDetail, 'storefront detail route'],
  [storefrontReturns, 'storefront return list route'],
  [storefrontChanges, 'storefront change list route'],
]) {
  forbidText(route, '.get_order_with_locale_fallback(', `${label} concrete order detail`);
}
for (const [route, value, label] of [
  [storefrontReturns, 'OrderService::new', 'storefront return concrete service'],
  [storefrontReturns, '.list_returns(', 'storefront concrete return list'],
  [storefrontChanges, 'OrderService::new', 'storefront change concrete service'],
  [storefrontChanges, '.list_order_changes(', 'storefront concrete change list'],
]) forbidText(route, value, label);

for (const [source, value, label] of [
  [adminOrders, '.mark_paid(', 'mark-paid mutation remains owner service'],
  [adminOrders, '.ship_order(', 'ship mutation remains owner service'],
  [adminOrders, '.deliver_order(', 'deliver mutation remains owner service'],
  [adminOrders, '.cancel_order(', 'cancel mutation remains owner service'],
  [storefrontOrders, '.create_return(tenant.id, id, input)', 'return mutation remains owner service'],
  [storefrontOrders, 'PaymentService::new(runtime.db_clone())', 'refund list remains payment service'],
  [graphqlOrderShim, '.get_return(tenant_id, return_id)', 'GraphQL return detail remains concrete'],
  [graphqlOrderShim, '.list_returns(tenant_id, input)', 'GraphQL return list remains concrete'],
  [graphqlOrderShim, '.get_order_change(tenant_id, change_id)', 'GraphQL change detail remains concrete'],
  [graphqlOrderShim, '.list_order_changes(tenant_id, input)', 'GraphQL change list remains concrete'],
]) requireText(source, value, label);

const operationNames = evidence.operations?.map((operation) => operation.name).join(',');
if (evidence.status !== 'storefront_post_order_reads_cutover_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (operationNames !==
    'read_order_projection,list_order_projections,read_order_return_projection,list_order_return_projections,read_order_change_projection,list_order_change_projections') {
  failures.push(`evidence operation inventory mismatch: ${operationNames}`);
}
if (evidence.owner?.port !== 'OrderReadPort' ||
    evidence.owner?.adapter !== 'InProcessOrderReadPort') {
  failures.push('evidence owner boundary mismatch');
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
for (const [key, expected] of [
  ['commerce_admin_rest_cutover_completed', true],
  ['commerce_storefront_detail_and_ownership_cutover_completed', true],
  ['commerce_storefront_return_list_cutover_completed', true],
  ['commerce_storefront_order_change_list_cutover_completed', true],
  ['commerce_graphql_order_list_detail_cutover_completed', true],
  ['complete_order_projection_consumer_cutover_completed', true],
  ['post_order_consumer_cutover_completed', false],
  ['all_consumer_cutover_completed', false],
  ['cutover_required', true],
]) {
  if (evidence.consumer_inventory?.[key] !== expected) {
    failures.push(`evidence consumer_inventory.${key} must be ${expected}`);
  }
}
if (evidence.errors?.owner_message_control_flow !== false ||
    evidence.errors?.all_current_order_error_variants_mapped !== true ||
    evidence.errors?.storefront_public_envelopes_preserved !== true) {
  failures.push('evidence error policy mismatch');
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
  '✔ OrderReadPort publishes six typed projections; storefront order, return, and change reads are cut over while GraphQL/admin post-order reads and runtime evidence remain open',
);
