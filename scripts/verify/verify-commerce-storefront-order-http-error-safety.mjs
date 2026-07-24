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
const orderErrors = read('crates/rustok-order/src/error.rs');
const paymentErrors = read('crates/rustok-payment/src/error.rs');
const webErrors = read('crates/rustok-web/src/lib.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['use rustok_api::{PortError, RequestContext, TenantContext};', 'typed customer port error import'],
  ['use rustok_order::{OrderService, error::OrderError};', 'typed order error import'],
  ['use rustok_payment::{PaymentService, error::PaymentError};', 'typed payment error import'],
  ['port_error_to_http_error', 'safe port HTTP mapper'],
  ['fn map_storefront_customer_port_error(', 'customer port mapper'],
  ['fn map_storefront_order_error(', 'order mapper'],
  ['fn map_storefront_payment_error(', 'payment mapper'],
  ['async fn current_storefront_customer_id(', 'safe customer lookup'],
  ['async fn ensure_customer_owns_order(', 'safe ownership helper'],
  ['boundary = "commerce_storefront_order_http"', 'structured boundary name'],
  ['operation,', 'operation logging'],
  ['tenant_id = %tenant_id', 'tenant logging'],
  ['order_id = %order_id', 'order logging'],
  ['public_code = code', 'public code logging'],
  ['status = %status', 'HTTP status logging'],
]) {
  requireText(controller, value, label);
}

for (const [value, label] of [
  ['OrderError::Validation(_)', 'order validation mapping'],
  ['OrderError::OrderNotFound(_)', 'order not-found mapping'],
  ['OrderError::OrderReturnNotFound(_)', 'return not-found mapping'],
  ['OrderError::OrderChangeNotFound(_)', 'change not-found mapping'],
  ['OrderError::InvalidTransition { .. }', 'order transition mapping'],
  ['OrderError::Database(_)', 'order database mapping'],
  ['OrderError::Core(_)', 'order core mapping'],
  ['StatusCode::BAD_REQUEST', 'bad request status'],
  ['StatusCode::NOT_FOUND', 'not found status'],
  ['StatusCode::CONFLICT', 'conflict status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'unavailable status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'internal status'],
  ['"commerce_store_order_invalid"', 'order invalid code'],
  ['"commerce_store_order_not_found"', 'order not-found code'],
  ['"commerce_store_order_state_conflict"', 'order state code'],
  ['"commerce_store_order_unavailable"', 'order unavailable code'],
  ['"commerce_store_order_failed"', 'order fail-closed code'],
]) {
  requireText(controller, value, label);
}

for (const [value, label] of [
  ['PaymentError::Validation(_)', 'payment validation mapping'],
  ['PaymentError::PaymentCollectionNotFound(_)', 'collection not-found mapping'],
  ['PaymentError::PaymentNotFound(_)', 'payment not-found mapping'],
  ['PaymentError::RefundNotFound(_)', 'refund not-found mapping'],
  ['PaymentError::InvalidTransition { .. }', 'payment transition mapping'],
  ['PaymentError::ProviderUnavailable { .. }', 'provider unavailable mapping'],
  ['PaymentError::ProviderRejected { .. }', 'provider rejected mapping'],
  ['PaymentError::ProviderInvalidResponse { .. }', 'provider invalid response mapping'],
  ['PaymentError::ProviderOutcomeUnknown { .. }', 'provider unknown outcome mapping'],
  ['PaymentError::ProviderConfiguration { .. }', 'provider configuration mapping'],
  ['PaymentError::Database(_)', 'payment database mapping'],
  ['StatusCode::BAD_GATEWAY', 'bad gateway status'],
  ['"commerce_store_payment_invalid"', 'payment invalid code'],
  ['"commerce_store_payment_not_found"', 'payment not-found code'],
  ['"commerce_store_payment_state_conflict"', 'payment state code'],
  ['"commerce_store_payment_provider_unavailable"', 'payment provider unavailable code'],
  ['"commerce_store_payment_provider_invalid_response"', 'payment invalid response code'],
  ['"commerce_store_payment_reconciliation_required"', 'payment reconciliation code'],
  ['"commerce_store_payment_provider_not_configured"', 'payment configuration code'],
  ['"commerce_store_payment_unavailable"', 'payment unavailable code'],
]) {
  requireText(controller, value, label);
}

for (const [ownerSource, value, label] of [
  [orderErrors, 'Validation(String)', 'owner order validation variant'],
  [orderErrors, 'OrderNotFound(Uuid)', 'owner order-not-found variant'],
  [orderErrors, 'OrderReturnNotFound(Uuid)', 'owner return-not-found variant'],
  [orderErrors, 'OrderChangeNotFound(Uuid)', 'owner change-not-found variant'],
  [orderErrors, 'InvalidTransition { from: String, to: String }', 'owner order transition variant'],
  [orderErrors, 'Database(#[from] DbErr)', 'owner order database variant'],
  [orderErrors, 'Core(#[from] rustok_core::Error)', 'owner order core variant'],
  [paymentErrors, 'Validation(String)', 'owner payment validation variant'],
  [paymentErrors, 'PaymentCollectionNotFound(Uuid)', 'owner collection variant'],
  [paymentErrors, 'PaymentNotFound(Uuid)', 'owner payment variant'],
  [paymentErrors, 'RefundNotFound(Uuid)', 'owner refund variant'],
  [paymentErrors, 'ProviderUnavailable {', 'owner provider unavailable variant'],
  [paymentErrors, 'ProviderRejected {', 'owner provider rejected variant'],
  [paymentErrors, 'ProviderInvalidResponse {', 'owner invalid response variant'],
  [paymentErrors, 'ProviderOutcomeUnknown {', 'owner unknown outcome variant'],
  [paymentErrors, 'ProviderConfiguration { provider_id: String }', 'owner provider configuration variant'],
  [paymentErrors, 'Database(#[from] DbErr)', 'owner payment database variant'],
]) {
  requireText(ownerSource, value, label);
}

for (const [value, label] of [
  ['pub async fn get_me(', 'customer resolver'],
  ['pub async fn get_order(', 'order resolver'],
  ['pub async fn create_order_return(', 'create return resolver'],
  ['pub async fn list_order_returns(', 'list returns resolver'],
  ['pub async fn list_order_refunds(', 'list refunds resolver'],
  ['pub async fn list_order_changes(', 'list changes resolver'],
  ['get_order_with_locale_fallback(', 'localized order read'],
  ['create_return(tenant.id, id, input)', 'return creation call'],
  ['ListOrderReturnsInput {', 'return pagination input'],
  ['ListRefundsInput {', 'refund pagination input'],
  ['ListOrderChangesInput {', 'change pagination input'],
  ['page: params.pagination.page', 'page forwarding'],
  ['per_page: params.pagination.per_page', 'per-page forwarding'],
  ['PaginationMeta::new(params.pagination.page, params.pagination.limit(), total)', 'pagination metadata'],
]) {
  requireText(controller, value, label);
}

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
]) {
  requireText(controller, operation, 'diagnostic operation label');
}

for (const value of [
  'err.to_string()',
  'error.to_string()',
  'error.message',
  'HttpError::bad_request("commerce_operation_failed"',
  'super::current_customer_id_for_db',
  'super::ensure_customer_owns_order_for_db',
]) {
  forbidText(controller, value, 'unsafe storefront order public conversion');
}

const orderMapperUses = controller.match(/map_storefront_order_error\(/g) ?? [];
if (orderMapperUses.length !== 6) {
  failures.push(`expected order mapper definition plus five uses, found ${orderMapperUses.length}`);
}
const paymentMapperUses = controller.match(/map_storefront_payment_error\(/g) ?? [];
if (paymentMapperUses.length !== 2) {
  failures.push(`expected payment mapper definition plus one use, found ${paymentMapperUses.length}`);
}
const customerMapperUses = controller.match(/map_storefront_customer_port_error\(/g) ?? [];
if (customerMapperUses.length !== 3) {
  failures.push(`expected customer mapper definition plus two uses, found ${customerMapperUses.length}`);
}

for (const value of [
  'PortErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE',
  'PortErrorKind::Timeout => StatusCode::GATEWAY_TIMEOUT',
  'PortErrorKind::InvariantViolation => StatusCode::INTERNAL_SERVER_ERROR',
  '"The requested service is temporarily unavailable"',
  '"The requested operation could not be completed"',
]) {
  requireText(webErrors, value, 'shared port HTTP safety contract');
}

if (failures.length > 0) {
  console.error('Commerce storefront order HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce storefront order and refund HTTP errors use stable typed public envelopes',
);
