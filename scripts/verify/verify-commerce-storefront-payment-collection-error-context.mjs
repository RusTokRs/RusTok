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
const paymentError = read('crates/rustok-payment/src/error.rs');
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

const route = between(
  checkout,
  'pub async fn create_payment_collection(',
  '/// Complete storefront cart checkout',
  'payment collection route',
);
const policy = between(
  checkout,
  'fn payment_collection_error_policy(',
  'fn payment_collection_http_error(',
  'payment collection policy',
);
const mapper = checkout.slice(
  checkout.indexOf('fn payment_collection_http_error('),
);

for (const [value, label] of [
  [
    'const STOREFRONT_PAYMENT_COLLECTION_OWNER: &str = "rustok_payment.storefront_payment_collections";',
    'payment owner constant',
  ],
  [
    'const STOREFRONT_PAYMENT_COLLECTION_BOUNDARY: &str =',
    'payment boundary constant',
  ],
  [
    '"commerce_storefront_payment_collection_http";',
    'payment boundary value',
  ],
  [
    'type StorefrontPaymentCollectionHttpPolicy = (',
    'payment policy type',
  ],
  [
    'struct StorefrontPaymentCollectionErrorContext<\'a> {',
    'typed payment context',
  ],
  ['tenant_id: Uuid,', 'tenant context field'],
  ['actor_id: Uuid,', 'actor context field'],
  ['cart_id: Uuid,', 'cart context field'],
  ['customer_id: Option<Uuid>,', 'customer context field'],
  ['channel_id: Option<Uuid>,', 'channel id context field'],
  ['channel_slug: Option<&\'a str>,', 'channel slug context field'],
  ['locale: &\'a str,', 'locale context field'],
  ['operation: &\'static str,', 'operation context field'],
  ['channel_id: request_context.channel_id,', 'channel id adoption'],
  [
    'channel_slug: request_context.channel_slug.as_deref(),',
    'channel slug adoption',
  ],
  ['locale: request_context.locale.as_str(),', 'locale adoption'],
]) requireText(checkout, value, label);

for (const [value, label] of [
  [
    'super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;',
    'storefront channel guard',
  ],
  [
    'let actor_id = super::checkout_actor_id(auth.0.as_ref());',
    'actor resolution',
  ],
  [
    'super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;',
    'customer resolution',
  ],
  ['in_process_cart_storefront_port(runtime.db_clone())', 'cart storefront port'],
  ['CartStorefrontReadRequest {', 'cart read request'],
  ['cart_id: input.cart_id,', 'input cart identity'],
  ['super::ensure_store_cart_access(&cart, customer_id)?;', 'cart access guard'],
  [
    'super::ensure_cart_allows_payment_collection(&cart)?;',
    'payment collection lifecycle guard',
  ],
  [
    'super::reprice_storefront_cart_line_items_for_db(',
    'cart repricing',
  ],
  [
    'super::resolve_context_from_cart_for_db(runtime.db(), tenant.id, &request_context, &cart)',
    'cart context resolution',
  ],
  [
    '.find_reusable_collection_by_cart(tenant.id, cart.id)',
    'reusable collection lookup',
  ],
  ['.create_collection(', 'collection creation'],
  ['cart_id: Some(cart.id),', 'created collection cart id'],
  ['order_id: None,', 'created collection order contract'],
  ['customer_id: cart.customer_id,', 'created collection customer id'],
  ['currency_code: cart.currency_code.clone(),', 'currency forwarding'],
  ['amount: cart.total_amount,', 'amount forwarding'],
  ['super::cart_context_metadata(&cart, &context)', 'metadata forwarding'],
  ['return Ok((StatusCode::OK, Json(existing)));', 'reusable response'],
  ['Ok((StatusCode::CREATED, Json(collection)))', 'created response'],
  [
    '"find_reusable_collection_by_cart"',
    'reusable lookup operation',
  ],
  ['"create_collection"', 'create operation'],
]) requireText(route, value, label);

for (const [value, label] of [
  ['PaymentError::Validation(_)', 'validation variant'],
  ['"payment_request_invalid"', 'validation code'],
  ['"Payment collection request is invalid"', 'validation message'],
  ['"validation"', 'validation kind'],
  ['PaymentError::PaymentCollectionNotFound(_)', 'collection missing variant'],
  ['PaymentError::PaymentNotFound(_)', 'payment missing variant'],
  ['PaymentError::RefundNotFound(_)', 'refund missing variant'],
  ['StatusCode::NOT_FOUND', 'missing status'],
  ['"payment_resource_not_found"', 'missing code'],
  ['"Payment resource was not found"', 'missing message'],
  ['"not_found"', 'missing kind'],
  ['PaymentError::InvalidTransition { .. }', 'transition variant'],
  ['"payment_state_conflict"', 'transition code'],
  [
    '"Payment lifecycle conflicts with the requested operation"',
    'transition message',
  ],
  ['"state_conflict"', 'transition kind'],
  ['PaymentError::ProviderUnavailable { .. }', 'provider unavailable variant'],
  ['"provider_unavailable"', 'provider unavailable kind'],
  ['PaymentError::ProviderConfiguration { .. }', 'provider configuration variant'],
  ['"provider_configuration"', 'provider configuration kind'],
  ['"payment_temporarily_unavailable"', 'temporary code'],
  ['"Payment service is temporarily unavailable"', 'temporary message'],
  ['PaymentError::ProviderRejected { .. }', 'provider rejected variant'],
  ['"payment_provider_rejected"', 'provider rejected code'],
  [
    '"Payment provider rejected the requested operation"',
    'provider rejected message',
  ],
  ['"provider_rejected"', 'provider rejected kind'],
  [
    'PaymentError::ProviderInvalidResponse { .. }',
    'invalid provider response variant',
  ],
  ['"provider_invalid_response"', 'invalid provider response kind'],
  [
    'PaymentError::ProviderOutcomeUnknown { .. }',
    'unknown provider outcome variant',
  ],
  ['"provider_outcome_unknown"', 'unknown provider outcome kind'],
  ['"payment_reconciliation_required"', 'reconciliation code'],
  [
    '"Payment operation requires reconciliation"',
    'reconciliation message',
  ],
  ['PaymentError::Database(_)', 'database variant'],
  ['"payment_storage_unavailable"', 'database code'],
  ['"database"', 'database kind'],
  ['StatusCode::BAD_REQUEST', 'bad request status'],
  ['StatusCode::CONFLICT', 'conflict status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'unavailable status'],
]) requireText(policy, value, label);

for (const [value, label] of [
  ['error = ?error', 'typed payment cause log'],
  ['owner = STOREFRONT_PAYMENT_COLLECTION_OWNER', 'owner log'],
  ['tenant_id = %context.tenant_id', 'tenant log'],
  ['actor_id = %context.actor_id', 'actor log'],
  ['cart_id = %context.cart_id', 'cart log'],
  ['customer_id = ?context.customer_id', 'customer log'],
  ['channel_id = ?context.channel_id', 'channel id log'],
  ['channel = ?context.channel_slug', 'channel slug log'],
  ['locale = %context.locale', 'locale log'],
  ['operation = %context.operation', 'operation log'],
  ['error_kind,', 'error kind log'],
  ['public_code = code', 'public code log'],
  ['status = %status', 'status log'],
  ['boundary = STOREFRONT_PAYMENT_COLLECTION_BOUNDARY', 'boundary log'],
  ['HttpError::new(status, code, message)', 'stable public envelope'],
]) requireText(mapper, value, label);

for (const value of [
  'Validation(String),',
  'PaymentCollectionNotFound(Uuid),',
  'PaymentNotFound(Uuid),',
  'RefundNotFound(Uuid),',
  'InvalidTransition { from: String, to: String },',
  'ProviderUnavailable {',
  'ProviderRejected {',
  'ProviderInvalidResponse {',
  'ProviderOutcomeUnknown {',
  'ProviderConfiguration { provider_id: String },',
  'Database(#[from] DbErr),',
]) requireText(paymentError, value, 'payment source enum');

const mapperUses =
  route.match(
    /payment_collection_http_error\(\s+StorefrontPaymentCollectionErrorContext::new\(/g,
  ) ?? [];
if (mapperUses.length !== 2) {
  failures.push(
    `expected two context-aware payment collection mapper callsites, found ${mapperUses.length}`,
  );
}

for (const value of [
  'payment_collection_http_error(tenant.id, cart.id',
  'payment_collection_http_error(\n                tenant.id',
  'error.to_string()',
  'err.to_string()',
  'HttpError::bad_request(error',
  'HttpError::internal(error',
]) forbidText(checkout, value, 'stale or unsafe payment collection mapping');

if (failures.length > 0) {
  console.error('Commerce storefront payment collection error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Storefront payment collection failures retain typed route context and stable public contracts',
);
