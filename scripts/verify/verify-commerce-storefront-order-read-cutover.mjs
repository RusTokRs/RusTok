#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const runtime = read('crates/rustok-commerce/src/controllers/mod.rs');
const storefrontOrders = read('crates/rustok-commerce/src/controllers/store/orders.rs');
const evidence = JSON.parse(
  read('crates/rustok-order/contracts/evidence/order-read-port-source.json'),
);
const note = read('crates/rustok-order/docs/order-read-port.md');
const orderPlan = read('crates/rustok-order/docs/implementation-plan.md');
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
  [runtime, 'fn order_read_port(&self)', 'HTTP runtime owner port accessor'],
  [storefrontOrders, 'fn storefront_order_read_port_context(', 'storefront read context'],
  [storefrontOrders, 'PortActor::user(auth.user_id.to_string())', 'authenticated actor'],
  [storefrontOrders, 'request_context.locale.as_str()', 'request locale'],
  [storefrontOrders, 'request_context.channel_slug.as_deref()', 'resolved channel'],
  [storefrontOrders, '.with_deadline(std::time::Duration::from_secs(2))', 'read deadline'],
  [storefrontOrders, 'async fn read_storefront_order_projection(', 'shared read helper'],
  [storefrontOrders, 'runtime\n        .order_read_port()', 'host-selected owner port'],
  [storefrontOrders, '.read_order_projection(', 'typed owner detail call'],
  [storefrontOrders, 'ReadOrderProjectionRequest {', 'typed detail request'],
  [storefrontOrders, 'tenant_default_locale: Some(tenant_default_locale.to_string())', 'locale fallback'],
  [storefrontOrders, 'fn map_storefront_order_port_error(', 'safe PortError mapper'],
  [storefrontOrders, 'internal_code = %error.code', 'stable internal diagnostics'],
  [storefrontOrders, 'if order.customer_id != Some(customer_id)', 'ownership comparison'],
  [note, 'Status: owner port and host runtime published; complete order projections plus storefront return/change lists cut over, unvalidated.', 'owner note status'],
  [orderPlan, 'storefront HTTP', 'order plan storefront checkpoint'],
]) requireText(source, value, label);

const ownershipHelper = between(
  storefrontOrders,
  'async fn ensure_customer_owns_order(',
  '/// Get current storefront customer',
  'storefront ownership helper',
);
const getOrderRoute = between(
  storefrontOrders,
  'pub async fn get_order(',
  '/// Create a return request',
  'storefront get-order route',
);
for (const [source, label] of [
  [ownershipHelper, 'storefront ownership helper'],
  [getOrderRoute, 'storefront get-order route'],
]) {
  requireText(source, 'read_storefront_order_projection(', `${label} shared owner read`);
  forbidText(source, 'OrderService::new', `${label} concrete owner construction`);
  forbidText(source, '.get_order(', `${label} concrete detail call`);
  forbidText(source, '.get_order_with_locale_fallback(', `${label} concrete locale detail call`);
}

for (const [value, label] of [
  ['.create_return(tenant.id, id, input)', 'return mutation remains owner service'],
  ['PaymentService::new(runtime.db_clone())', 'refund list remains payment service'],
]) requireText(storefrontOrders, value, label);

if (evidence.status !== 'storefront_post_order_reads_cutover_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (evidence.consumer_inventory?.commerce_storefront_detail_and_ownership !==
    'order_read_port_host_runtime') {
  failures.push('storefront detail/ownership must use the host-selected order read port');
}
if (evidence.consumer_inventory?.commerce_storefront_detail_and_ownership_cutover_completed !== true) {
  failures.push('storefront detail/ownership source cutover must be complete');
}
if (evidence.consumer_inventory?.complete_order_projection_consumer_cutover_completed !== true) {
  failures.push('complete order projection consumer cutover must remain complete');
}
if (evidence.consumer_inventory?.all_consumer_cutover_completed !== false) {
  failures.push('post-order GraphQL/admin consumers must remain open');
}
if (evidence.consumer_inventory?.cutover_required !== true) {
  failures.push('post-order consumer cutover must remain pending');
}
if (evidence.context?.storefront_actor_source !== 'validated_auth_context_user') {
  failures.push('storefront actor source mismatch');
}
if (evidence.context?.storefront_channel_source !== 'resolved_request_context_channel_slug') {
  failures.push('storefront channel source mismatch');
}
if (evidence.errors?.storefront_public_envelopes_preserved !== true) {
  failures.push('storefront public envelopes must be preserved');
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
  console.error('Commerce storefront order read cutover verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Storefront order detail and shared ownership reads use the host-selected typed owner port while mutations and payment policy remain unchanged',
);
