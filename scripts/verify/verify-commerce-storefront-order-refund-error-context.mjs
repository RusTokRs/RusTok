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
const paymentOwner = read('crates/rustok-payment/src/order_read.rs');
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

const mapper = between(
  controller,
  'fn map_storefront_payment_port_error(',
  'async fn current_storefront_customer_id(',
  'storefront Payment refund mapper',
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
  ['const STOREFRONT_ORDER_PAYMENT_OWNER: &str = "rustok_payment.storefront_order_refunds";', 'Payment owner constant'],
  ['const STOREFRONT_ORDER_PAYMENT_BOUNDARY: &str = "commerce_storefront_order_http";', 'Payment boundary constant'],
  ['fn map_storefront_payment_port_error(', 'typed Payment port mapper'],
]) requireText(controller, value, label);

for (const [value, label] of [
  ['"payment.order_refund_provider_unavailable"', 'provider unavailable owner code'],
  ['"payment.order_refund_provider_invalid_response"', 'provider invalid response owner code'],
  ['"payment.order_refund_reconciliation_required"', 'reconciliation owner code'],
  ['"payment.order_refund_provider_not_configured"', 'provider configuration owner code'],
  ['StatusCode::BAD_REQUEST', 'validation status'],
  ['"commerce_store_payment_invalid"', 'validation public code'],
  ['StatusCode::NOT_FOUND', 'not-found status'],
  ['"commerce_store_payment_not_found"', 'not-found public code'],
  ['StatusCode::CONFLICT', 'conflict status'],
  ['"commerce_store_payment_state_conflict"', 'state-conflict public code'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'unavailable status'],
  ['"commerce_store_payment_provider_unavailable"', 'provider unavailable public code'],
  ['StatusCode::BAD_GATEWAY', 'bad gateway status'],
  ['"commerce_store_payment_provider_invalid_response"', 'provider invalid-response public code'],
  ['"commerce_store_payment_reconciliation_required"', 'reconciliation public code'],
  ['"commerce_store_payment_provider_not_configured"', 'provider configuration public code'],
  ['"commerce_store_payment_unavailable"', 'storage unavailable public code'],
  ['owner = STOREFRONT_ORDER_PAYMENT_OWNER', 'owner diagnostic'],
  ['owner_operation = "list_refunds_by_order"', 'owner operation diagnostic'],
  ['consumer_operation = "list_order_refunds"', 'consumer operation diagnostic'],
  ['correlation_id = %context.correlation_id', 'correlation diagnostic'],
  ['actor_id_non_nil = !actor_id.is_nil()', 'actor identity diagnostic'],
  ['customer_id_non_nil = !customer_id.is_nil()', 'customer identity diagnostic'],
  ['order_id_non_nil = !order_id.is_nil()', 'order identity diagnostic'],
  ['owner_error_kind = ?error.kind', 'owner kind diagnostic'],
  ['owner_code_length = error.code.chars().count()', 'bounded code diagnostic'],
  ['retryable = error.retryable', 'retryability diagnostic'],
  ['HttpError::new(status, code, message)', 'stable public envelope'],
]) requireText(mapper, value, label);

for (const value of ['error = ?error', 'error.message', 'error.to_string()', 'internal_message']) {
  forbidText(mapper, value, 'raw Payment owner diagnostic');
}

for (const [value, label] of [
  [') -> HttpResult<Uuid> {', 'ownership helper return type'],
  ['Ok(customer_id)', 'verified customer return'],
  ['read_storefront_order_projection(', 'owner Order ownership read'],
  ['order.customer_id != Some(customer_id)', 'ownership comparison'],
  ['"commerce_store_customer_required"', 'customer-required envelope'],
  ['"commerce_store_order_access_denied"', 'access-denied envelope'],
]) requireText(ownership, value, label);

for (const [value, label] of [
  ['super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;', 'channel guard'],
  ['let customer_id = ensure_customer_owns_order(', 'verified customer binding'],
  ['"list_order_refunds_access"', 'ownership operation'],
  ['let payment_context = storefront_order_read_port_context(', 'Payment context construction'],
  ['tenant.id,', 'tenant forwarding'],
  ['&auth,', 'actor forwarding'],
  ['&request_context,', 'request context forwarding'],
  ['.payment_order_read_port()', 'host-selected Payment port'],
  ['.list_refunds_by_order(', 'refund owner operation'],
  ['ListRefundsByOrderRequest {', 'typed refund request'],
  ['order_id: id,', 'order forwarding'],
  ['page: params.pagination.page,', 'page forwarding'],
  ['per_page: params.pagination.per_page,', 'per-page forwarding'],
  ['status: params.status,', 'status forwarding'],
  ['PaginationMeta::new(params.pagination.page, params.pagination.limit(), page.total)', 'pagination response contract'],
]) requireText(refundRoute, value, label);

for (const [value, label] of [
  ['pub trait PaymentOrderReadPort: Send + Sync', 'Payment owner trait'],
  ['async fn list_refunds_by_order(', 'Payment refund capability'],
  ['context.require_policy(PortCallPolicy::read())?', 'read admission'],
  ['pub struct ListRefundsByOrderRequest', 'typed request'],
  ['pub struct PaymentOrderRefundPage', 'typed page'],
  ['.list_refunds(', 'owner-local service call'],
  ['payment_collection_id: None,', 'collection filter contract'],
  ['order_id: Some(request.order_id),', 'order filter contract'],
  ['status: request.status,', 'status filter contract'],
]) requireText(paymentOwner, value, label);

for (const value of [
  'PaymentService::new(runtime.db_clone())',
  'StorefrontOrderPaymentErrorContext',
  'fn storefront_order_payment_error_policy(',
  'PaymentError::',
  'map_storefront_payment_error(',
  'error.to_string()',
  'err.to_string()',
  'HttpError::bad_request(error',
  'HttpError::internal(error',
]) forbidText(controller, value, 'stale or unsafe storefront refund boundary');

if (failures.length > 0) {
  console.error('Commerce storefront order-refund error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Storefront order refunds preserve ownership context through the host-selected Payment owner port',
);
