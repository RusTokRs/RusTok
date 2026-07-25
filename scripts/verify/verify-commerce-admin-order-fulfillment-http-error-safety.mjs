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
const changes = read('crates/rustok-commerce/src/controllers/admin/changes.rs');
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
  ['pub(crate) fn map_order_error(error: OrderError)', 'legacy shared order mapper'],
  ['pub(crate) fn map_post_order_orchestration_error(', 'legacy shared post-order mapper'],
]) requireText(admin, value, label);

for (const value of [
  'pub(crate) fn map_fulfillment_error(',
  'pub(crate) fn map_fulfillment_orchestration_error(',
]) forbidText(admin, value, 'removed shared fulfillment mapper definition');

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
]) requireText(admin, value, label);

for (const [value, label] of [
  ['PostOrderOrchestrationError::Order(error) => map_order_error(error)', 'post-order order delegation'],
  ['PostOrderOrchestrationError::Payment(error) => map_payment_error(error)', 'post-order payment delegation'],
  ['PostOrderOrchestrationError::PaymentOrchestration(error)', 'post-order payment orchestration delegation'],
  ['PostOrderOrchestrationError::Validation(_)', 'post-order validation mapping'],
  ['"commerce_admin_post_order_invalid"', 'post-order validation code'],
]) requireText(admin, value, label);

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
]) requireText(ownerSource, value, label);

for (const [value, label] of [
  ['pub async fn list_orders(', 'admin list-orders handler'],
  ['pub async fn show_order(', 'admin show-order handler'],
  ['pub async fn mark_order_paid(', 'admin mark-paid handler'],
  ['pub async fn ship_order(', 'admin ship-order handler'],
  ['pub async fn deliver_order(', 'admin deliver-order handler'],
  ['pub async fn cancel_order(', 'admin cancel-order handler'],
  ['struct AdminOrderErrorContext {', 'order route context'],
  ['fn map_admin_order_error(', 'context-aware order route mapper'],
  ['let customer_id = params.customer_id;', 'customer filter capture'],
  ['list_orders_with_locale_fallback(', 'localized order list'],
  ['get_order_with_locale_fallback(', 'localized order detail'],
  ['find_latest_collection_by_order(tenant.id, id)', 'payment collection detail read'],
  ['find_by_order(tenant.id, id)', 'fulfillment detail read'],
  ['fn map_order_detail_payment_error(', 'order-detail payment mapper'],
  ['fn map_order_detail_fulfillment_error(', 'order-detail fulfillment mapper'],
  ['page: pagination.page', 'order page forwarding'],
  ['per_page: pagination.limit()', 'order page-size forwarding'],
]) requireText(orders, value, label);

for (const value of [
  '.map_err(super::map_order_error)?;',
  '.map_err(super::map_payment_error)?;',
  '.map_err(super::map_fulfillment_error)?;',
]) forbidText(orders, value, 'stale admin order shared mapper callsite');

for (const [value, label] of [
  ['pub async fn create_order_change(', 'admin order-change create'],
  ['pub async fn list_order_changes(', 'admin order-change list'],
  ['pub async fn show_order_change(', 'admin order-change detail'],
  ['pub async fn apply_order_change(', 'admin order-change apply'],
  ['pub async fn cancel_order_change(', 'admin order-change cancel'],
  ['struct AdminOrderChangeErrorContext {', 'order-change owner context'],
  ['fn map_admin_order_change_error(', 'context-aware order-change owner mapper'],
  ['struct AdminOrderChangeOrchestrationErrorContext {', 'order-change orchestration context'],
  ['fn map_admin_order_change_orchestration_error(', 'context-aware order-change orchestration mapper'],
  ['let order_id = params.order_id;', 'order-change list identity capture'],
]) requireText(changes, value, label);

for (const value of [
  '.map_err(super::map_order_error)?;',
  '.map_err(super::map_post_order_orchestration_error)?;',
]) forbidText(changes, value, 'stale admin order-change shared mapper callsite');

for (const [value, label] of [
  ['pub async fn list_order_returns(', 'admin return list'],
  ['pub async fn show_order_return(', 'admin return detail'],
  ['pub async fn create_order_return(', 'admin return create'],
  ['pub async fn create_order_return_decision(', 'admin return decision'],
  ['pub async fn complete_order_return(', 'admin return complete'],
  ['pub async fn cancel_order_return(', 'admin return cancel'],
  ['struct AdminOrderReturnErrorContext {', 'return owner context'],
  ['fn map_admin_order_return_error(', 'context-aware return owner mapper'],
  ['struct AdminOrderReturnOrchestrationErrorContext {', 'return orchestration context'],
  ['fn map_admin_order_return_orchestration_error(', 'context-aware return orchestration mapper'],
  ['ListOrderReturnsInput {', 'return list input'],
]) requireText(returns, value, label);

for (const value of [
  '.map_err(super::map_order_error)?;',
  '.map_err(super::map_post_order_orchestration_error)?;',
]) forbidText(returns, value, 'stale admin return shared mapper callsite');

for (const [value, label] of [
  ['pub async fn list_fulfillments(', 'admin fulfillment list'],
  ['pub async fn create_fulfillment(', 'admin fulfillment create'],
  ['pub async fn show_fulfillment(', 'admin fulfillment detail'],
  ['pub async fn ship_fulfillment(', 'admin fulfillment ship'],
  ['pub async fn deliver_fulfillment(', 'admin fulfillment deliver'],
  ['pub async fn reopen_fulfillment(', 'admin fulfillment reopen'],
  ['pub async fn reship_fulfillment(', 'admin fulfillment reship'],
  ['pub async fn cancel_fulfillment(', 'admin fulfillment cancel'],
  ['struct AdminFulfillmentErrorContext {', 'fulfillment route context'],
  ['fn map_admin_fulfillment_error(', 'context-aware fulfillment owner mapper'],
  ['fn map_admin_fulfillment_orchestration_error(', 'context-aware fulfillment orchestration mapper'],
  ['ListFulfillmentsInput {', 'fulfillment list input'],
]) requireText(fulfillments, value, label);

for (const value of [
  '.map_err(super::map_fulfillment_error)?;',
  '.map_err(super::map_fulfillment_orchestration_error)?;',
]) forbidText(fulfillments, value, 'stale admin fulfillment shared mapper callsite');

for (const [content, label] of [
  [orders, 'admin order endpoints'],
  [changes, 'admin order-change endpoints'],
  [returns, 'admin return endpoints'],
  [fulfillments, 'admin fulfillment endpoints'],
]) {
  for (const value of [
    'err.to_string()',
    'error.to_string()',
    'other.to_string()',
    'HttpError::bad_request("commerce_operation_failed"',
  ]) forbidText(content, value, `${label} unsafe public conversion`);
}

const orderMapperUses =
  orders.match(/map_admin_order_error\(\s+AdminOrderErrorContext::new\(/g) ?? [];
if (orderMapperUses.length !== 6) {
  failures.push(`expected six context-aware admin order mapper callsites, found ${orderMapperUses.length}`);
}
const sharedOrderMapperUses = orders.match(/\.map_err\(super::map_order_error\)\?;/g) ?? [];
if (sharedOrderMapperUses.length !== 0) {
  failures.push(`expected zero shared order mapper callsites, found ${sharedOrderMapperUses.length}`);
}

const orderChangeOwnerMapperUses =
  changes.match(/map_admin_order_change_error\(\s+AdminOrderChangeErrorContext::new\(/g) ?? [];
if (orderChangeOwnerMapperUses.length !== 4) {
  failures.push(`expected four context-aware order-change owner mapper callsites, found ${orderChangeOwnerMapperUses.length}`);
}
const orderChangeOrchestrationUses =
  changes.match(/map_admin_order_change_orchestration_error\(\s+AdminOrderChangeOrchestrationErrorContext::new\(/g) ?? [];
if (orderChangeOrchestrationUses.length !== 1) {
  failures.push(`expected one context-aware order-change orchestration mapper callsite, found ${orderChangeOrchestrationUses.length}`);
}

const returnOwnerMapperUses =
  returns.match(/map_admin_order_return_error\(\s+AdminOrderReturnErrorContext::new\(/g) ?? [];
if (returnOwnerMapperUses.length !== 4) {
  failures.push(`expected four context-aware return owner mapper callsites, found ${returnOwnerMapperUses.length}`);
}
const returnOrchestrationMapperUses =
  returns.match(/map_admin_order_return_orchestration_error\(\s+AdminOrderReturnOrchestrationErrorContext::new\(/g) ?? [];
if (returnOrchestrationMapperUses.length !== 2) {
  failures.push(`expected two context-aware return orchestration mapper callsites, found ${returnOrchestrationMapperUses.length}`);
}

const fulfillmentMapperUses =
  fulfillments.match(/map_admin_fulfillment_error\(\s+AdminFulfillmentErrorContext::new\(/g) ?? [];
if (fulfillmentMapperUses.length !== 4) {
  failures.push(`expected four context-aware fulfillment owner mapper callsites, found ${fulfillmentMapperUses.length}`);
}
const fulfillmentOrchestrationUses =
  fulfillments.match(/map_admin_fulfillment_orchestration_error\(\s+AdminFulfillmentErrorContext::new\(/g) ?? [];
if (fulfillmentOrchestrationUses.length !== 4) {
  failures.push(`expected four context-aware fulfillment orchestration mapper callsites, found ${fulfillmentOrchestrationUses.length}`);
}

forbidText(admin, 'other.to_string()', 'unsafe shared admin dynamic string conversion');
requireText(
  admin,
  'pub(crate) fn map_shipping_profile_error(error: crate::CommerceError)',
  'shared static shipping-profile mapper',
);

if (failures.length > 0) {
  console.error('Commerce admin order/fulfillment HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin order, change, return, and fulfillment HTTP errors use stable typed public envelopes',
);
