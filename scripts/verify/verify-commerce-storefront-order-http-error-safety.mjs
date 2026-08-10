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
const orderCommand = read('crates/rustok-order/src/post_order_command.rs');
const paymentOrderRead = read('crates/rustok-payment/src/order_read.rs');
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

for (const [value, label] of [
  ['PortActor, PortContext, PortError, PortErrorKind, RequestContext, TenantContext', 'typed Order/Payment context imports'],
  ['CreateOrderReturnRequest', 'typed Order return command request'],
  ['use rustok_payment::ListRefundsByOrderRequest;', 'typed Payment refund request'],
  ['port_error_to_http_error', 'safe shared port HTTP mapper'],
  ['fn map_storefront_customer_port_error(', 'customer port mapper'],
  ['fn map_storefront_order_port_error(', 'Order read port mapper'],
  ['fn map_storefront_order_command_port_error(', 'Order command port mapper'],
  ['fn map_storefront_payment_port_error(', 'Payment owner port mapper'],
  ['async fn current_storefront_customer_id(', 'safe customer lookup'],
  ['async fn ensure_customer_owns_order(', 'safe ownership helper'],
  ['boundary = "commerce_storefront_order_http"', 'structured storefront boundary'],
]) requireText(controller, value, label);

for (const [value, label] of [
  ['PortErrorKind::Validation', 'Order validation mapping'],
  ['PortErrorKind::NotFound', 'Order not-found mapping'],
  ['PortErrorKind::Conflict', 'Order conflict mapping'],
  ['PortErrorKind::Forbidden', 'Order forbidden mapping'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'Order unavailable mapping'],
  ['PortErrorKind::InvariantViolation', 'Order invariant mapping'],
  ['"commerce_store_order_invalid"', 'Order invalid public code'],
  ['"commerce_store_order_not_found"', 'Order not-found public code'],
  ['"commerce_store_order_state_conflict"', 'Order state public code'],
  ['"commerce_store_order_access_denied"', 'Order access public code'],
  ['"commerce_store_order_unavailable"', 'Order unavailable public code'],
  ['"commerce_store_order_failed"', 'Order fail-closed public code'],
]) requireText(controller, value, label);

const paymentMapper = between(
  controller,
  'fn map_storefront_payment_port_error(',
  'async fn current_storefront_customer_id(',
  'Payment refund port mapper',
);
for (const [value, label] of [
  ['"payment.order_refund_provider_unavailable"', 'provider unavailable owner identity'],
  ['"payment.order_refund_provider_invalid_response"', 'provider invalid-response owner identity'],
  ['"payment.order_refund_reconciliation_required"', 'reconciliation owner identity'],
  ['"payment.order_refund_provider_not_configured"', 'provider configuration owner identity'],
  ['"commerce_store_payment_invalid"', 'payment invalid code'],
  ['"commerce_store_payment_not_found"', 'payment not-found code'],
  ['"commerce_store_payment_state_conflict"', 'payment state code'],
  ['"commerce_store_payment_provider_unavailable"', 'payment provider unavailable code'],
  ['"commerce_store_payment_provider_invalid_response"', 'payment invalid response code'],
  ['"commerce_store_payment_reconciliation_required"', 'payment reconciliation code'],
  ['"commerce_store_payment_provider_not_configured"', 'payment configuration code'],
  ['"commerce_store_payment_unavailable"', 'payment unavailable code'],
  ['owner_error_kind = ?error.kind', 'bounded Payment owner kind'],
  ['owner_code_length = error.code.chars().count()', 'bounded Payment owner code'],
  ['retryable = error.retryable', 'bounded Payment retryability'],
]) requireText(paymentMapper, value, label);
for (const value of ['error = ?error', 'error.message', 'internal_message', 'error.to_string()']) {
  forbidText(paymentMapper, value, 'Payment owner mapper raw diagnostic');
}

for (const [ownerSource, value, label] of [
  [orderCommand, 'async fn create_return(', 'owner Order return command'],
  [orderCommand, 'context.require_policy(PortCallPolicy::write())', 'owner Order write admission'],
  [paymentOrderRead, 'async fn list_refunds_by_order(', 'owner Payment refund read'],
  [paymentOrderRead, 'context.require_policy(PortCallPolicy::read())?', 'owner Payment read admission'],
  [paymentOrderRead, '"payment.order_refund_read_unavailable"', 'external Payment fail-closed default'],
]) requireText(ownerSource, value, label);

for (const [value, label] of [
  ['pub async fn get_me(', 'customer resolver'],
  ['pub async fn get_order(', 'order resolver'],
  ['pub async fn create_order_return(', 'create return resolver'],
  ['pub async fn list_order_returns(', 'list returns resolver'],
  ['pub async fn list_order_refunds(', 'list refunds resolver'],
  ['pub async fn list_order_changes(', 'list changes resolver'],
  ['.read_order_projection(', 'localized Order owner read'],
  ['.order_post_order_command_port()', 'host-selected Order return command'],
  ['CreateOrderReturnRequest {', 'typed return request'],
  ['.payment_order_read_port()', 'host-selected Payment order read'],
  ['.list_refunds_by_order(', 'typed Payment refund read'],
  ['ListRefundsByOrderRequest {', 'typed Payment refund request construction'],
  ['page: params.pagination.page', 'page forwarding'],
  ['per_page: params.pagination.per_page', 'per-page forwarding'],
  ['PaginationMeta::new(params.pagination.page, params.pagination.limit(), page.total)', 'owner pagination metadata'],
]) requireText(controller, value, label);

for (const operation of [
  '"get_me"',
  '"get_order"',
  '"create_order_return_access"',
  '"create_order_return"',
  '"list_order_returns_access"',
  '"list_order_returns"',
  '"list_order_refunds_access"',
  '"list_order_refunds"',
  '"list_order_changes_access"',
  '"list_order_changes"',
]) requireText(controller, operation, 'diagnostic operation label');

for (const value of [
  'OrderService::new(',
  'error::OrderError',
  'PaymentService::new(',
  'error::PaymentError',
  'StorefrontOrderPaymentErrorContext',
  'fn storefront_order_payment_error_policy(',
  'err.to_string()',
  'error.to_string()',
  'HttpError::bad_request("commerce_operation_failed"',
  'super::current_customer_id_for_db',
  'super::ensure_customer_owns_order_for_db',
]) forbidText(controller, value, 'stale or unsafe storefront owner path');

const commandMapper = between(
  controller,
  'fn map_storefront_order_command_port_error(',
  'fn map_storefront_payment_port_error(',
  'Order return command mapper',
);
for (const [value, label] of [
  ['owner_error_kind = ?error.kind', 'bounded Order owner kind'],
  ['owner_code_length = error.code.chars().count()', 'bounded Order owner code'],
  ['retryable = error.retryable', 'bounded Order retryability'],
  ['public_code = code', 'Order public code'],
  ['status = %status', 'Order public status'],
]) requireText(commandMapper, value, label);
for (const value of ['error = ?error', 'error.message', 'internal_message', 'error.to_string()']) {
  forbidText(commandMapper, value, 'Order command mapper raw diagnostic');
}

for (const value of [
  'PortErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE',
  'PortErrorKind::Timeout => StatusCode::GATEWAY_TIMEOUT',
  'PortErrorKind::InvariantViolation => StatusCode::INTERNAL_SERVER_ERROR',
  '"The requested service is temporarily unavailable"',
  '"The requested operation could not be completed"',
]) requireText(webErrors, value, 'shared port HTTP safety contract');

if (failures.length > 0) {
  console.error('Commerce storefront order HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce storefront Order and Payment owner boundaries use stable public envelopes',
);
