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
const paymentErrors = read('crates/rustok-payment/src/error.rs');
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

const policy = between(
  controller,
  'fn storefront_order_payment_error_policy(',
  'fn map_storefront_payment_error(',
  'storefront order payment policy',
);
const mapper = between(
  controller,
  'fn map_storefront_payment_error(',
  'async fn current_storefront_customer_id(',
  'storefront order payment mapper',
);
const ownership = between(
  controller,
  'async fn ensure_customer_owns_order(',
  '/// Get current storefront customer',
  'order ownership helper',
);
const refundRoute = between(
  controller,
  'pub async fn list_order_refunds(',
  '/// List order changes for the current customer',
  'order refund route',
);

for (const [value, label] of [
  [
    'const STOREFRONT_ORDER_PAYMENT_OWNER: &str = "rustok_payment.storefront_order_refunds";',
    'payment owner constant',
  ],
  [
    'const STOREFRONT_ORDER_PAYMENT_BOUNDARY: &str = "commerce_storefront_order_http";',
    'payment boundary constant',
  ],
  ['type StorefrontOrderPaymentHttpPolicy = (', 'payment policy type'],
  [
    'struct StorefrontOrderPaymentErrorContext<\'a> {',
    'typed payment context',
  ],
  ['tenant_id: Uuid,', 'tenant context field'],
  ['actor_id: Uuid,', 'actor context field'],
  ['customer_id: Uuid,', 'customer context field'],
  ['order_id: Uuid,', 'order context field'],
  ['payment_collection_id: Option<Uuid>,', 'collection context field'],
  ['refund_id: Option<Uuid>,', 'refund context field'],
  ['channel_id: Option<Uuid>,', 'channel ID context field'],
  ['channel_slug: Option<&\'a str>,', 'channel slug context field'],
  ['locale: &\'a str,', 'locale context field'],
  ['operation: &\'static str,', 'operation context field'],
  ['payment_collection_id: None,', 'empty collection identity'],
  ['refund_id: None,', 'empty refund identity'],
  ['channel_id: request_context.channel_id,', 'channel ID adoption'],
  [
    'channel_slug: request_context.channel_slug.as_deref(),',
    'channel slug adoption',
  ],
  ['locale: request_context.locale.as_str(),', 'locale adoption'],
]) requireText(controller, value, label);

for (const [value, label] of [
  ['PaymentError::Validation(_)', 'validation variant'],
  ['StatusCode::BAD_REQUEST', 'validation status'],
  ['"commerce_store_payment_invalid"', 'validation code'],
  ['"Payment request is invalid"', 'validation message'],
  ['"validation"', 'validation kind'],
  [
    'PaymentError::PaymentCollectionNotFound(payment_collection_id)',
    'collection not-found variant',
  ],
  [
    'context.payment_collection_id = Some(*payment_collection_id);',
    'collection identity adoption',
  ],
  [
    'PaymentError::PaymentNotFound(payment_collection_id)',
    'payment not-found variant',
  ],
  ['"payment_not_found"', 'payment not-found kind'],
  ['PaymentError::RefundNotFound(refund_id)', 'refund not-found variant'],
  ['context.refund_id = Some(*refund_id);', 'refund identity adoption'],
  ['StatusCode::NOT_FOUND', 'not-found status'],
  ['"commerce_store_payment_not_found"', 'not-found code'],
  ['"Payment resource was not found"', 'not-found message'],
  ['PaymentError::InvalidTransition { .. }', 'transition variant'],
  ['"state_conflict"', 'transition kind'],
  ['PaymentError::ProviderRejected { .. }', 'provider rejected variant'],
  ['"provider_rejected"', 'provider rejected kind'],
  ['StatusCode::CONFLICT', 'conflict status'],
  ['"commerce_store_payment_state_conflict"', 'state conflict code'],
  [
    '"Payment operation conflicts with the current state"',
    'state conflict message',
  ],
  ['PaymentError::ProviderUnavailable { .. }', 'provider unavailable variant'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'unavailable status'],
  [
    '"commerce_store_payment_provider_unavailable"',
    'provider unavailable code',
  ],
  [
    '"Payment provider is temporarily unavailable"',
    'provider unavailable message',
  ],
  [
    'PaymentError::ProviderInvalidResponse { .. }',
    'invalid provider response variant',
  ],
  ['StatusCode::BAD_GATEWAY', 'bad gateway status'],
  [
    '"commerce_store_payment_provider_invalid_response"',
    'invalid provider response code',
  ],
  [
    '"Payment provider returned an invalid response"',
    'invalid provider response message',
  ],
  [
    'PaymentError::ProviderOutcomeUnknown { .. }',
    'unknown provider outcome variant',
  ],
  [
    '"commerce_store_payment_reconciliation_required"',
    'reconciliation code',
  ],
  ['"Payment state requires reconciliation"', 'reconciliation message'],
  [
    'PaymentError::ProviderConfiguration { .. }',
    'provider configuration variant',
  ],
  [
    '"commerce_store_payment_provider_not_configured"',
    'provider configuration code',
  ],
  [
    '"Payment provider is not configured for this tenant"',
    'provider configuration message',
  ],
  ['PaymentError::Database(_)', 'database variant'],
  ['"commerce_store_payment_unavailable"', 'database code'],
  ['"Payment service is temporarily unavailable"', 'database message'],
  ['"database"', 'database kind'],
]) requireText(policy, value, label);

for (const [value, label] of [
  ['error = ?error', 'typed payment cause log'],
  ['owner = STOREFRONT_ORDER_PAYMENT_OWNER', 'owner log'],
  ['tenant_id = %context.tenant_id', 'tenant log'],
  ['actor_id = %context.actor_id', 'actor log'],
  ['customer_id = %context.customer_id', 'customer log'],
  ['order_id = %context.order_id', 'order log'],
  [
    'payment_collection_id = ?context.payment_collection_id',
    'collection identity log',
  ],
  ['refund_id = ?context.refund_id', 'refund identity log'],
  ['channel_id = ?context.channel_id', 'channel ID log'],
  ['channel = ?context.channel_slug', 'channel slug log'],
  ['locale = %context.locale', 'locale log'],
  ['operation = %context.operation', 'operation log'],
  ['error_kind,', 'error kind log'],
  ['public_code = code', 'public code log'],
  ['status = %status', 'status log'],
  ['boundary = STOREFRONT_ORDER_PAYMENT_BOUNDARY', 'boundary log'],
  ['HttpError::new(status, code, message)', 'stable public envelope'],
]) requireText(mapper, value, label);

for (const [value, label] of [
  [') -> HttpResult<Uuid> {', 'ownership helper return type'],
  ['Ok(customer_id)', 'verified customer return'],
  ['.get_order(tenant_id, order_id)', 'order ownership read'],
  ['order.customer_id != Some(customer_id)', 'ownership comparison'],
  ['"commerce_store_customer_required"', 'customer-required envelope'],
  ['"commerce_store_order_access_denied"', 'access-denied envelope'],
]) requireText(ownership, value, label);

for (const [value, label] of [
  [
    'super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;',
    'channel guard',
  ],
  ['let customer_id =', 'verified customer binding'],
  [
    'ensure_customer_owns_order(&runtime, tenant.id, &auth, id, "list_order_refunds_access")',
    'ownership helper call',
  ],
  ['PaymentService::new(runtime.db_clone())', 'payment service construction'],
  ['.list_refunds(', 'refund list call'],
  ['page: params.pagination.page,', 'page forwarding'],
  ['per_page: params.pagination.per_page,', 'per-page forwarding'],
  ['payment_collection_id: None,', 'collection filter contract'],
  ['order_id: Some(id),', 'order filter contract'],
  ['status: params.status,', 'status filter forwarding'],
  ['StorefrontOrderPaymentErrorContext::new(', 'typed mapper context'],
  ['tenant.id,', 'tenant context forwarding'],
  ['auth.user_id,', 'actor context forwarding'],
  ['customer_id,', 'customer context forwarding'],
  ['id,', 'order context forwarding'],
  ['&request_context,', 'request context forwarding'],
  ['"list_order_refunds"', 'route operation'],
  [
    'PaginationMeta::new(params.pagination.page, params.pagination.limit(), total)',
    'pagination response contract',
  ],
]) requireText(refundRoute, value, label);

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
]) requireText(paymentErrors, value, 'payment source enum');

const mapperUses = controller.match(/map_storefront_payment_error\(/g) ?? [];
if (mapperUses.length !== 2) {
  failures.push(
    `expected payment mapper definition plus one route use, found ${mapperUses.length}`,
  );
}

for (const value of [
  'map_storefront_payment_error(error, "list_order_refunds", tenant.id, id)',
  'error.to_string()',
  'err.to_string()',
  'error.message',
  'HttpError::bad_request(error',
  'HttpError::internal(error',
]) forbidText(controller, value, 'stale or unsafe refund mapping');

if (failures.length > 0) {
  console.error('Commerce storefront order-refund error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Storefront order refund failures retain typed route context and stable public contracts',
);
