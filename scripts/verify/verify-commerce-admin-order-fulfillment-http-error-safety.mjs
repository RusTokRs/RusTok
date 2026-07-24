#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const admin = read('crates/rustok-commerce/src/controllers/admin/mod.rs');
const orders = read('crates/rustok-commerce/src/controllers/admin/orders.rs');
const returns = read('crates/rustok-commerce/src/controllers/admin/returns.rs');
const fulfillments = read('crates/rustok-commerce/src/controllers/admin/fulfillments.rs');
const orderErrors = read('crates/rustok-order/src/error.rs');
const fulfillmentErrors = read('crates/rustok-fulfillment/src/error.rs');
const fulfillmentOrchestration = read(
  'crates/rustok-commerce/src/services/fulfillment_orchestration.rs',
);
const postOrder = read('crates/rustok-commerce/src/services/post_order.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['use rustok_fulfillment::error::FulfillmentError;', 'typed fulfillment error import'],
  ['use rustok_order::error::OrderError;', 'typed order error import'],
  ['fn admin_public_error<E>(', 'shared safe HTTP constructor'],
  ['E: std::fmt::Debug', 'raw error logging bound'],
  ['error = ?error', 'raw internal error logging'],
  ['owner,', 'owner logging'],
  ['error_kind,', 'error-kind logging'],
  ['public_code = code', 'public-code logging'],
  ['status = %status', 'status logging'],
  ['boundary = "commerce_admin_http"', 'admin HTTP boundary logging'],
  ['HttpError::new(status, code, message)', 'static HTTP envelope construction'],
  ['pub(crate) fn map_order_error(error: OrderError)', 'order mapper'],
  ['pub(crate) fn map_fulfillment_error(error: FulfillmentError)', 'fulfillment mapper'],
  ['pub(crate) fn map_fulfillment_orchestration_error(', 'fulfillment orchestration mapper'],
  ['pub(crate) fn map_post_order_orchestration_error(', 'post-order mapper'],
]) {
  requireText(admin, value, label);
}

for (const [value, label] of [
  ['OrderError::Validation(_)', 'order validation mapping'],
  ['OrderError::OrderNotFound(_)', 'order not-found mapping'],
  ['OrderError::OrderReturnNotFound(_)', 'return not-found mapping'],
  ['OrderError::OrderChangeNotFound(_)', 'change not-found mapping'],
  ['OrderError::InvalidTransition { .. }', 'order transition mapping'],
  ['OrderError::Database(_)', 'order database mapping'],
  ['OrderError::Core(_)', 'order core mapping'],
  ['"commerce_admin_order_invalid"', 'order invalid code'],
  ['"commerce_admin_not_found"', 'shared not-found code'],
  ['"commerce_admin_order_state_conflict"', 'order conflict code'],
  ['"commerce_admin_order_storage_unavailable"', 'order storage code'],
  ['"commerce_admin_order_failed"', 'order fail-closed code'],
  ['axum::http::StatusCode::BAD_REQUEST', 'bad-request status'],
  ['axum::http::StatusCode::NOT_FOUND', 'not-found status'],
  ['axum::http::StatusCode::CONFLICT', 'conflict status'],
  ['axum::http::StatusCode::SERVICE_UNAVAILABLE', 'unavailable status'],
  ['axum::http::StatusCode::INTERNAL_SERVER_ERROR', 'internal status'],
]) {
  requireText(admin, value, label);
}

for (const [value, label] of [
  ['FulfillmentError::Validation(_)', 'fulfillment validation mapping'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping-option not-found mapping'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment not-found mapping'],
  ['FulfillmentError::InvalidTransition { .. }', 'fulfillment transition mapping'],
  ['FulfillmentError::Database(_)', 'fulfillment database mapping'],
  ['"commerce_admin_fulfillment_invalid"', 'fulfillment invalid code'],
  ['"commerce_admin_fulfillment_state_conflict"', 'fulfillment conflict code'],
  ['"commerce_admin_fulfillment_storage_unavailable"', 'fulfillment storage code'],
  ['FulfillmentOrchestrationError::OrderNotFound(_)', 'orchestration order-not-found mapping'],
  ['FulfillmentOrchestrationError::Fulfillment(error)', 'orchestration owner delegation'],
  ['FulfillmentOrchestrationError::Validation(_)', 'orchestration validation mapping'],
  ['FulfillmentOrchestrationError::ProviderAfterPersistence { .. }', 'provider-after-persistence mapping'],
  ['FulfillmentOrchestrationError::PersistenceAfterProvider { .. }', 'persistence-after-provider mapping'],
  ['"commerce_admin_fulfillment_reconciliation_required"', 'fulfillment reconciliation code'],
  ['"Fulfillment operation requires reconciliation"', 'fulfillment reconciliation message'],
]) {
  requireText(admin, value, label);
}

for (const [value, label] of [
  ['PostOrderOrchestrationError::Order(error) => map_order_error(error)', 'post-order order delegation'],
  ['PostOrderOrchestrationError::Payment(error) => map_payment_error(error)', 'post-order payment delegation'],
  ['PostOrderOrchestrationError::PaymentOrchestration(error)', 'post-order payment orchestration delegation'],
  ['PostOrderOrchestrationError::Validation(_)', 'post-order validation mapping'],
  ['"commerce_admin_post_order_invalid"', 'post-order validation code'],
]) {
  requireText(admin, value, label);
}

for (const [ownerSource, value, label] of [
  [orderErrors, 'Validation(String)', 'owner order validation variant'],
  [orderErrors, 'OrderNotFound(Uuid)', 'owner order not-found variant'],
  [orderErrors, 'OrderReturnNotFound(Uuid)', 'owner return not-found variant'],
  [orderErrors, 'OrderChangeNotFound(Uuid)', 'owner change not-found variant'],
  [orderErrors, 'InvalidTransition { from: String, to: String }', 'owner order transition variant'],
  [orderErrors, 'Database(#[from] DbErr)', 'owner order database variant'],
  [orderErrors, 'Core(#[from] rustok_core::Error)', 'owner order core variant'],
  [fulfillmentErrors, 'Validation(String)', 'owner fulfillment validation variant'],
  [fulfillmentErrors, 'ShippingOptionNotFound(Uuid)', 'owner shipping-option variant'],
  [fulfillmentErrors, 'FulfillmentNotFound(Uuid)', 'owner fulfillment variant'],
  [fulfillmentErrors, 'InvalidTransition { from: String, to: String }', 'owner fulfillment transition variant'],
  [fulfillmentErrors, 'Database(#[from] DbErr)', 'owner fulfillment database variant'],
  [fulfillmentOrchestration, 'OrderNotFound(Uuid)', 'orchestration order-not-found variant'],
  [fulfillmentOrchestration, 'Database(#[from] sea_orm::DbErr)', 'orchestration database variant'],
  [fulfillmentOrchestration, 'Fulfillment(#[from] rustok_fulfillment::error::FulfillmentError)', 'orchestration fulfillment variant'],
  [fulfillmentOrchestration, 'Validation(String)', 'orchestration validation variant'],
  [fulfillmentOrchestration, 'ProviderAfterPersistence {', 'orchestration provider-after-persistence variant'],
  [fulfillmentOrchestration, 'PersistenceAfterProvider {', 'orchestration persistence-after-provider variant'],
  [postOrder, 'Order(#[from] rustok_order::error::OrderError)', 'post-order order variant'],
  [postOrder, 'Payment(#[from] rustok_payment::error::PaymentError)', 'post-order payment variant'],
  [postOrder, 'PaymentOrchestration(#[from] PaymentOrchestrationError)', 'post-order payment orchestration variant'],
  [postOrder, 'Validation(String)', 'post-order validation variant'],
]) {
  requireText(ownerSource, value, label);
}

for (const [value, label] of [
  ['pub async fn list_orders(', 'admin list-orders handler'],
  ['pub async fn show_order(', 'admin show-order handler'],
  ['list_orders_with_locale_fallback(', 'localized order list'],
  ['get_order_with_locale_fallback(', 'localized order detail'],
  ['find_latest_collection_by_order(tenant.id, id)', 'payment collection detail read'],
  ['find_by_order(tenant.id, id)', 'fulfillment detail read'],
  ['.map_err(super::map_payment_error)?;', 'typed payment detail mapping'],
  ['.map_err(super::map_fulfillment_error)?;', 'typed fulfillment detail mapping'],
  ['page: pagination.page', 'order page forwarding'],
  ['per_page: pagination.limit()', 'order page-size forwarding'],
]) {
  requireText(orders, value, label);
}

for (const [value, label] of [
  ['pub async fn list_order_returns(', 'admin return list'],
  ['pub async fn show_order_return(', 'admin return detail'],
  ['pub async fn create_order_return(', 'admin return create'],
  ['pub async fn cancel_order_return(', 'admin return cancel'],
  ['.map_err(super::map_order_error)?;', 'typed return owner mapping'],
  ['.map_err(super::map_post_order_orchestration_error)?;', 'typed return orchestration mapping'],
  ['ListOrderReturnsInput {', 'return list input'],
  ['page: pagination.page', 'return page forwarding'],
  ['per_page: pagination.limit()', 'return page-size forwarding'],
]) {
  requireText(returns, value, label);
}

for (const [value, label] of [
  ['pub async fn list_fulfillments(', 'admin fulfillment list'],
  ['pub async fn show_fulfillment(', 'admin fulfillment detail'],
  ['ListFulfillmentsInput {', 'fulfillment list input'],
  ['page: pagination.page', 'fulfillment page forwarding'],
  ['per_page: pagination.limit()', 'fulfillment page-size forwarding'],
  ['.map_err(super::map_fulfillment_error)?;', 'typed fulfillment owner mapping'],
  ['.map_err(super::map_fulfillment_orchestration_error)?;', 'typed fulfillment orchestration mapping'],
]) {
  requireText(fulfillments, value, label);
}

for (const [content, label] of [
  [orders, 'admin order reads'],
  [returns, 'admin return endpoints'],
  [fulfillments, 'admin fulfillment endpoints'],
]) {
  for (const value of [
    'err.to_string()',
    'error.to_string()',
    'other.to_string()',
    'HttpError::bad_request("commerce_operation_failed"',
  ]) {
    forbidText(content, value, `${label} unsafe public conversion`);
  }
}

const orderMapperUses = (orders.match(/\.map_err\(super::map_order_error\)\?;/g) ?? []).length
  + (returns.match(/\.map_err\(super::map_order_error\)\?;/g) ?? []).length;
if (orderMapperUses !== 10) {
  failures.push(`expected ten order/return owner mapper callsites, found ${orderMapperUses}`);
}

const fulfillmentMapperUses =
  fulfillments.match(/\.map_err\(super::map_fulfillment_error\)\?;/g) ?? [];
if (fulfillmentMapperUses.length !== 4) {
  failures.push(`expected four fulfillment owner mapper callsites, found ${fulfillmentMapperUses.length}`);
}

const fulfillmentOrchestrationUses =
  fulfillments.match(/\.map_err\(super::map_fulfillment_orchestration_error\)\?;/g) ?? [];
if (fulfillmentOrchestrationUses.length !== 4) {
  failures.push(`expected four fulfillment orchestration mapper callsites, found ${fulfillmentOrchestrationUses.length}`);
}

const postOrderUses =
  returns.match(/\.map_err\(super::map_post_order_orchestration_error\)\?;/g) ?? [];
if (postOrderUses.length !== 2) {
  failures.push(`expected two post-order orchestration mapper callsites, found ${postOrderUses.length}`);
}

const remainingAdminDynamicStrings = admin.match(/other\.to_string\(\)/g) ?? [];
if (remainingAdminDynamicStrings.length !== 1) {
  failures.push(
    `expected only the separately scoped shipping-profile mapper to retain other.to_string(), found ${remainingAdminDynamicStrings.length}`,
  );
}
requireText(
  admin,
  'pub(crate) fn map_shipping_profile_error(error: crate::CommerceError)',
  'separately scoped shipping-profile mapper',
);

if (failures.length > 0) {
  console.error('Commerce admin order/fulfillment HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin order, return, and fulfillment HTTP errors use stable typed public envelopes',
);
