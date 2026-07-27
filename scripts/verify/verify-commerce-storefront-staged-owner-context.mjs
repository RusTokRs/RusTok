#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-commerce/src/storefront_staged_checkout_runtime.rs', root),
  'utf8',
);
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

const completion = between(
  source,
  'pub async fn complete_storefront_checkout_input(',
  'async fn resolve_customer_id(',
  'staged checkout completion',
);
const customerLookup = between(
  source,
  'async fn resolve_customer_id(',
  'fn cart_context(',
  'staged customer lookup',
);
const ownerMapper = between(
  source,
  'fn map_owner_port_error(',
  'fn map_checkout_error(',
  'staged owner-port mapper',
);
const recoveryMapper = source.slice(source.indexOf('fn map_checkout_error('));

for (const [value, label] of [
  [
    'const STOREFRONT_STAGED_CHECKOUT_CART_OWNER: &str = "rustok_cart";',
    'truthful cart owner',
  ],
  [
    'const STOREFRONT_STAGED_CHECKOUT_CUSTOMER_OWNER: &str = "rustok_customer";',
    'truthful customer owner',
  ],
  [
    'const STOREFRONT_STAGED_CHECKOUT_BOUNDARY: &str = "commerce_storefront_staged_checkout_runtime";',
    'stable runtime boundary',
  ],
]) requireText(source, value, label);

for (const [content, values, label] of [
  [
    completion,
    [
      'let cart_port_context = cart_context(',
      'read_storefront_cart(',
      'cart_port_context.clone(),',
      '&cart_port_context,',
      'STOREFRONT_STAGED_CHECKOUT_CART_OWNER,',
      '"read_storefront_cart",',
      'StorefrontStagedCheckoutRuntimeError::CartAccess,',
    ],
    'cart owner delegation',
  ],
  [
    customerLookup,
    [
      'let context = PortContext::new(',
      'read_customer_projection_by_user(',
      'context.clone(),',
      'Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None)',
      '&context,',
      'STOREFRONT_STAGED_CHECKOUT_CUSTOMER_OWNER,',
      '"read_customer_projection_by_user",',
      'StorefrontStagedCheckoutRuntimeError::CartAccess,',
    ],
    'customer owner delegation',
  ],
]) {
  for (const value of values) requireText(content, value, label);
}

for (const [value, label] of [
  ['context: &PortContext', 'retained context input'],
  ["owner: &'static str", 'truthful owner input'],
  ["operation: &'static str", 'exact operation input'],
  ['error: PortError', 'original port error input'],
  ['fallback: StorefrontStagedCheckoutRuntimeError', 'preserved fallback input'],
  ['let public = match &error.kind', 'public classification before diagnostics'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout =>',
    'unchanged temporary classification',
  ],
  ['StorefrontStagedCheckoutRuntimeError::TemporarilyUnavailable', 'temporary public outcome'],
  ['_ => fallback,', 'unchanged non-temporary fallback'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'technical severity classification',
  ],
  ['tracing::error!(', 'technical severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['error = ?error', 'original error evidence'],
  ['owner,', 'truthful owner field'],
  ['operation,', 'exact operation field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['internal_code = %error.code', 'owner error code'],
  ['internal_message = %error.message', 'owner internal message'],
  ['error_kind = ?error.kind', 'typed owner kind'],
  ['owner_retryable = error.retryable', 'owner retryability'],
  ['public_code = public.public_code()', 'mapped public code'],
  ['public_retryable = public.retryable()', 'mapped public retryability'],
  ['boundary = STOREFRONT_STAGED_CHECKOUT_BOUNDARY', 'runtime boundary field'],
  ['"storefront checkout owner port failed"', 'technical event'],
  ['"storefront checkout owner port was rejected"', 'rejection event'],
  ['\n    public\n}', 'mapped public return'],
]) requireText(ownerMapper, value, label);

const publicIndex = ownerMapper.indexOf('let public = match &error.kind');
const diagnosticsIndex = ownerMapper.indexOf('match &error.kind', publicIndex + 1);
const returnIndex = ownerMapper.lastIndexOf('\n    public');
if (!(publicIndex >= 0 && publicIndex < diagnosticsIndex && diagnosticsIndex < returnIndex)) {
  failures.push('owner error must be classified, diagnosed, and then returned in order');
}

const mapperCalls = source.match(/map_owner_port_error\(/g) ?? [];
if (mapperCalls.length !== 3) {
  failures.push(`expected owner mapper definition plus two uses, found ${mapperCalls.length}`);
}
const cartOwnerUses = source.match(/STOREFRONT_STAGED_CHECKOUT_CART_OWNER/g) ?? [];
if (cartOwnerUses.length !== 2) {
  failures.push(`expected cart owner constant plus one use, found ${cartOwnerUses.length}`);
}
const customerOwnerUses = source.match(/STOREFRONT_STAGED_CHECKOUT_CUSTOMER_OWNER/g) ?? [];
if (customerOwnerUses.length !== 2) {
  failures.push(`expected customer owner constant plus one use, found ${customerOwnerUses.length}`);
}

for (const [value, label] of [
  ['checkout_input.shipping_selections.clone()', 'shipping selection preservation'],
  ['with_payment_provider_registry(payment_provider_registry.clone())', 'provider registry composition'],
  ['RecoveringStagedCheckoutService::new(staged, compensation)', 'recovery composition'],
  ['map_checkout_error(&cart_port_context, cart_id, actor_id, error)', 'recovery mapper use'],
]) requireText(completion, value, label);

for (const [value, label] of [
  ['owner = STOREFRONT_STAGED_CHECKOUT_OWNER', 'recovery owner identity'],
  ['operation = "complete_storefront_checkout"', 'recovery operation'],
  ['StorefrontStagedCheckoutRuntimeError::ReconciliationRequired', 'reconciliation outcome'],
  ['StorefrontStagedCheckoutRuntimeError::CompensationPending', 'compensation outcome'],
  ['StorefrontStagedCheckoutRuntimeError::CheckoutFailed', 'checkout failure outcome'],
]) requireText(recoveryMapper, value, label);

forbidText(
  source,
  `map_owner_port_error(
                &cart_port_context,
                "read_storefront_cart",`,
  'owner-free cart mapper call',
);
forbidText(
  source,
  `map_owner_port_error(
            &context,
            "read_customer_projection_by_user",`,
  'owner-free customer mapper call',
);
forbidText(ownerMapper, 'tracing::error!(\n        error = ?error,', 'single-severity legacy mapper');
forbidText(ownerMapper, 'match error.kind {', 'post-diagnostic legacy classification');

if (failures.length > 0) {
  console.error('Commerce storefront staged owner-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Mounted storefront staged cart/customer owner-port failures retain truthful owner context and preserve runtime outcomes',
);
