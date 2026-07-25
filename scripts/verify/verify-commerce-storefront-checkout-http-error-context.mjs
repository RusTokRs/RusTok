#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const checkout = read('crates/rustok-commerce/src/controllers/store/checkout.rs');
const stagedRuntime = read(
  'crates/rustok-commerce/src/storefront_staged_checkout_runtime.rs',
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

const paymentCollectionRoute = between(
  checkout,
  'pub async fn create_payment_collection(',
  '/// Complete storefront cart checkout',
  'payment collection route',
);
const completionRoute = between(
  checkout,
  'pub async fn complete_cart_checkout(',
  'fn required_idempotency_key(',
  'checkout completion route',
);
const idempotencyHelper = between(
  checkout,
  'fn required_idempotency_key(',
  'fn storefront_checkout_error_policy(',
  'idempotency helper',
);
const policy = between(
  checkout,
  'fn storefront_checkout_error_policy(',
  'fn storefront_checkout_http_error(',
  'checkout HTTP policy',
);
const mapper = between(
  checkout,
  'fn storefront_checkout_http_error(',
  'fn payment_collection_http_error(',
  'checkout HTTP mapper',
);

for (const [value, label] of [
  [
    'const STOREFRONT_CHECKOUT_OWNER: &str = "rustok_commerce.storefront_staged_checkout_runtime";',
    'owner constant',
  ],
  [
    'const STOREFRONT_CHECKOUT_BOUNDARY: &str = "commerce_storefront_checkout_http";',
    'boundary constant',
  ],
  ['type StorefrontCheckoutHttpPolicy = (StatusCode, &\'static str);', 'policy type'],
  ['struct StorefrontCheckoutErrorContext<\'a> {', 'typed route context'],
  ['tenant_id: Uuid,', 'tenant context field'],
  ['actor_id: Uuid,', 'actor context field'],
  ['cart_id: Uuid,', 'cart context field'],
  ['channel_id: Option<Uuid>,', 'channel ID context field'],
  ['channel_slug: Option<&\'a str>,', 'channel slug context field'],
  ['locale: &\'a str,', 'locale context field'],
  ['operation: &\'static str,', 'operation context field'],
  ['channel_id: request_context.channel_id,', 'channel ID adoption'],
  [
    'channel_slug: request_context.channel_slug.as_deref(),',
    'channel slug adoption',
  ],
  ['locale: request_context.locale.as_str(),', 'locale adoption'],
]) requireText(checkout, value, label);

for (const [value, label] of [
  ['StorefrontStagedCheckoutRuntimeError::Validation(_)', 'validation variant'],
  ['StatusCode::BAD_REQUEST', 'validation status'],
  ['"validation"', 'validation kind'],
  ['StorefrontStagedCheckoutRuntimeError::CartAccess', 'cart-access variant'],
  ['StatusCode::NOT_FOUND', 'cart-access status'],
  ['"cart_access"', 'cart-access kind'],
  [
    'StorefrontStagedCheckoutRuntimeError::AuthenticationRequired',
    'authentication variant',
  ],
  ['StatusCode::UNAUTHORIZED', 'authentication status'],
  ['"authentication_required"', 'authentication kind'],
  [
    'StorefrontStagedCheckoutRuntimeError::TemporarilyUnavailable',
    'temporary variant',
  ],
  ['StatusCode::SERVICE_UNAVAILABLE', 'temporary status'],
  ['"temporarily_unavailable"', 'temporary kind'],
  ['StorefrontStagedCheckoutRuntimeError::CheckoutFailed', 'failure variant'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'failure status'],
  ['"checkout_failed"', 'failure kind'],
  [
    'StorefrontStagedCheckoutRuntimeError::CompensationPending',
    'compensation variant',
  ],
  ['"compensation_pending"', 'compensation kind'],
  [
    'StorefrontStagedCheckoutRuntimeError::ReconciliationRequired',
    'reconciliation variant',
  ],
  ['"reconciliation_required"', 'reconciliation kind'],
  ['StatusCode::CONFLICT', 'conflict status'],
]) requireText(policy, value, label);

for (const [value, label] of [
  ['error = ?error', 'typed runtime cause log'],
  ['owner = STOREFRONT_CHECKOUT_OWNER', 'owner log'],
  ['tenant_id = %context.tenant_id', 'tenant log'],
  ['actor_id = %context.actor_id', 'actor log'],
  ['cart_id = %context.cart_id', 'cart log'],
  ['channel_id = ?context.channel_id', 'channel ID log'],
  ['channel = ?context.channel_slug', 'channel slug log'],
  ['locale = %context.locale', 'locale log'],
  ['operation = %context.operation', 'operation log'],
  ['error_kind,', 'error-kind log'],
  ['public_code = code', 'public code log'],
  ['retryable = error.retryable()', 'retryability log'],
  ['status = %status', 'status log'],
  ['boundary = STOREFRONT_CHECKOUT_BOUNDARY', 'boundary log'],
  ['HttpError::new(status, code, message)', 'stable public envelope'],
]) requireText(mapper, value, label);

for (const [value, label] of [
  [
    'super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;',
    'channel guard',
  ],
  [
    'let idempotency_key = required_idempotency_key(&headers)?;',
    'idempotency extraction',
  ],
  [
    'let actor_id = super::checkout_actor_id(auth.0.as_ref());',
    'actor resolution',
  ],
  ['shipping_option_id: input.shipping_option_id,', 'shipping option forwarding'],
  ['shipping_selections: input.shipping_selections.map(|items| {', 'shipping selections'],
  ['region_id: input.region_id,', 'region forwarding'],
  ['country_code: input.country_code,', 'country forwarding'],
  ['locale: input.locale,', 'checkout locale forwarding'],
  ['create_fulfillment: input.create_fulfillment,', 'fulfillment flag forwarding'],
  ['metadata: input.metadata,', 'metadata forwarding'],
  [
    'runtime.payment_provider_registry(),',
    'payment provider registry forwarding',
  ],
  ['tenant.id,', 'tenant forwarding'],
  ['&request_context,', 'request context forwarding'],
  ['auth.0,', 'auth forwarding'],
  ['idempotency_key,', 'idempotency forwarding'],
  ['checkout_input,', 'checkout input forwarding'],
  ['StorefrontCheckoutErrorContext::new(', 'typed HTTP context'],
  ['actor_id,', 'actor context forwarding'],
  ['cart_id,', 'cart context forwarding'],
  ['"complete_cart_checkout"', 'route operation'],
  ['Ok(Json(response))', 'response contract'],
]) requireText(completionRoute, value, label);

for (const [value, label] of [
  ['const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";', 'header constant'],
  ['const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 191;', 'maximum length'],
  ['"idempotency_key_required"', 'required-key code'],
  ['"Idempotency-Key header is required for checkout"', 'required-key message'],
  ['"idempotency_key_invalid"', 'invalid-key code'],
  ['"Idempotency-Key header must be valid ASCII"', 'ASCII message'],
  [
    'format!("Idempotency-Key must contain 1 to {MAX_IDEMPOTENCY_KEY_LENGTH} characters")',
    'length message',
  ],
]) requireText(`${checkout}\n${idempotencyHelper}`, value, label);

for (const [value, label] of [
  ['.find_reusable_collection_by_cart(tenant.id, cart.id)', 'reusable collection lookup'],
  ['payment_collection_http_error(', 'payment collection mapper call'],
  ['.create_collection(', 'collection creation'],
  ['Ok((StatusCode::CREATED, Json(collection)))', 'collection response'],
]) requireText(paymentCollectionRoute, value, label);

for (const [value, label] of [
  ['pub enum StorefrontStagedCheckoutRuntimeError {', 'runtime error enum'],
  ['Validation(String),', 'runtime validation source variant'],
  ['CartAccess,', 'runtime cart source variant'],
  ['AuthenticationRequired,', 'runtime auth source variant'],
  ['TemporarilyUnavailable,', 'runtime temporary source variant'],
  ['CheckoutFailed,', 'runtime failure source variant'],
  ['CompensationPending,', 'runtime compensation source variant'],
  ['ReconciliationRequired,', 'runtime reconciliation source variant'],
  ['pub const fn public_code(&self)', 'runtime public-code contract'],
  ['pub const fn public_message(&self)', 'runtime public-message contract'],
  ['pub const fn retryable(&self)', 'runtime retryability contract'],
]) requireText(stagedRuntime, value, label);

const mapperUses =
  completionRoute.match(
    /storefront_checkout_http_error\(\s+StorefrontCheckoutErrorContext::new\(/g,
  ) ?? [];
if (mapperUses.length !== 1) {
  failures.push(
    `expected one context-aware storefront checkout HTTP mapper callsite, found ${mapperUses.length}`,
  );
}

for (const value of [
  'storefront_checkout_http_error(error)',
  'code = error.public_code()',
  'tracing::error!(',
  'error.to_string()',
  'err.to_string()',
  'format!("Checkout',
]) forbidText(completionRoute, value, 'stale or unsafe completion-route mapping');

for (const value of [
  'error.to_string()',
  'err.to_string()',
  'HttpError::internal(error',
  'HttpError::bad_request(error',
]) forbidText(mapper, value, 'unsafe checkout HTTP mapper');

if (failures.length > 0) {
  console.error('Commerce storefront checkout HTTP error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Storefront checkout HTTP failures retain typed route context and stable public contracts',
);
