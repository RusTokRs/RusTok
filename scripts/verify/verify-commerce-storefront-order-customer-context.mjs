#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const controller = read('crates/rustok-commerce/src/controllers/store/orders.rs');
const webErrors = read('crates/rustok-web/src/lib.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};

const customerMapper = between(
  controller,
  'fn map_storefront_customer_port_error(',
  'fn map_storefront_order_error(',
  'storefront order customer mapper',
);
const customerLookup = between(
  controller,
  'async fn current_storefront_customer_id(',
  'async fn ensure_customer_owns_order(',
  'storefront order customer lookup',
);
const ownership = between(
  controller,
  'async fn ensure_customer_owns_order(',
  '/// Get current storefront customer',
  'storefront order ownership helper',
);
const getMe = between(
  controller,
  'pub async fn get_me(',
  '/// Get customer-owned storefront order',
  'storefront current-customer route',
);

for (const [value, label] of [
  ['use rustok_api::{PortContext, PortErrorKind};', 'retained context imports'],
  ['const STOREFRONT_ORDER_CUSTOMER_OWNER: &str = "rustok_customer";', 'truthful customer owner'],
  [
    'const STOREFRONT_ORDER_CUSTOMER_OWNER_OPERATION: &str = "read_customer_projection_by_user";',
    'exact customer owner operation',
  ],
  [
    'const STOREFRONT_ORDER_CUSTOMER_BOUNDARY: &str = "commerce_storefront_order_http";',
    'order customer boundary',
  ],
]) requireText(controller, value, label);

for (const [value, label] of [
  ['error: PortError', 'original port error input'],
  ['context: &PortContext', 'retained context input'],
  ['user_id: Uuid', 'user identity input'],
  ["consumer_operation: &'static str", 'consumer operation input'],
  ['let public = port_error_to_http_error(error.clone());', 'unchanged safe HTTP mapping'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'technical severity classification',
  ],
  ['tracing::error!(', 'technical error severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['error = ?error', 'original error evidence'],
  ['owner = STOREFRONT_ORDER_CUSTOMER_OWNER', 'owner field'],
  ['owner_operation = STOREFRONT_ORDER_CUSTOMER_OWNER_OPERATION', 'owner operation field'],
  ['consumer_operation,', 'consumer operation field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['user_id = %user_id', 'user context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['internal_code = %error.code', 'internal owner code'],
  ['internal_message = %error.message', 'internal owner message'],
  ['error_kind = ?error.kind', 'typed error kind'],
  ['retryable = error.retryable', 'retryability'],
  ['public_code = %public.code', 'public code'],
  ['status = %public.status', 'public status'],
  ['boundary = STOREFRONT_ORDER_CUSTOMER_BOUNDARY', 'boundary field'],
  ['"storefront customer read failed"', 'technical event'],
  ['"storefront customer read was rejected"', 'ordinary rejection event'],
  ['\n    public\n}', 'mapped HTTP return'],
]) requireText(customerMapper, value, label);

const publicIndex = customerMapper.indexOf('let public = port_error_to_http_error(error.clone());');
const diagnosticIndex = customerMapper.indexOf('match &error.kind');
const returnIndex = customerMapper.lastIndexOf('\n    public');
if (!(publicIndex >= 0 && publicIndex < diagnosticIndex && diagnosticIndex < returnIndex)) {
  failures.push('customer error must be mapped, diagnosed, and then returned in order');
}

for (const [content, values, label] of [
  [
    customerLookup,
    [
      'let customer_context = super::storefront_customer_port_context(tenant_id, auth.user_id);',
      'read_customer_projection_by_user(',
      'customer_context.clone(),',
      'CustomerUserProjectionRequest {',
      'user_id: auth.user_id,',
      'Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None)',
      '&customer_context,',
      'auth.user_id,',
      'operation,',
    ],
    'order-access customer lookup',
  ],
  [
    getMe,
    [
      'super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;',
      'let customer_context = super::storefront_customer_port_context(tenant.id, auth.user_id);',
      'read_customer_projection_by_user(',
      'customer_context.clone(),',
      'CustomerUserProjectionRequest {',
      'user_id: auth.user_id,',
      'map_storefront_customer_port_error(error, &customer_context, auth.user_id, "get_me")',
      'Ok(Json(customer))',
    ],
    'current-customer route',
  ],
]) {
  for (const value of values) requireText(content, value, label);
}

for (const [value, label] of [
  ['current_storefront_customer_id(runtime, tenant_id, auth, operation)', 'ownership lookup reuse'],
  ['.get_order(tenant_id, order_id)', 'order ownership read'],
  ['order.customer_id != Some(customer_id)', 'ownership comparison'],
  ['"commerce_store_customer_required"', 'customer-required envelope'],
  ['"commerce_store_order_access_denied"', 'access-denied envelope'],
  ['Ok(customer_id)', 'verified customer identity return'],
]) requireText(ownership, value, label);

const contextBindings = controller.match(/let customer_context = super::storefront_customer_port_context\(/g) ?? [];
if (contextBindings.length !== 2) {
  failures.push(`expected two retained customer contexts, found ${contextBindings.length}`);
}
const contextClones = controller.match(/customer_context\.clone\(\),/g) ?? [];
if (contextClones.length !== 2) {
  failures.push(`expected two customer context delegation clones, found ${contextClones.length}`);
}
const mapperUses = controller.match(/map_storefront_customer_port_error\(/g) ?? [];
if (mapperUses.length !== 3) {
  failures.push(`expected customer mapper definition plus two uses, found ${mapperUses.length}`);
}

for (const value of [
  'read_customer_projection_by_user(\n            super::storefront_customer_port_context(',
  'map_storefront_customer_port_error(\n            error, operation, tenant_id',
  'map_storefront_customer_port_error(error, "get_me", tenant.id)',
]) forbidText(controller, value, 'context-dropping customer mapping');

for (const value of [
  'PortErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE',
  'PortErrorKind::Timeout => StatusCode::GATEWAY_TIMEOUT',
  'PortErrorKind::InvariantViolation => StatusCode::INTERNAL_SERVER_ERROR',
  '"The requested service is temporarily unavailable"',
  '"The requested operation could not be completed"',
]) requireText(webErrors, value, 'shared safe HTTP contract');

if (failures.length > 0) {
  console.error('Commerce storefront order customer-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Storefront order customer reads retain the delegated PortContext and preserve safe HTTP envelopes and ownership behavior',
);
