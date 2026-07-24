#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const payments = read('crates/rustok-commerce/src/controllers/admin/payments.rs');
const admin = read('crates/rustok-commerce/src/controllers/admin/mod.rs');
const paymentErrors = read('crates/rustok-payment/src/error.rs');
const paymentService = read('crates/rustok-payment/src/services/payment.rs');
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

const listHandler = between(
  payments,
  'pub async fn list_payment_collections(',
  '#[utoipa::path(\n    get,\n    path = "/admin/payment-collections/{id}"',
  'payment collection list handler',
);
const mapper = between(
  admin,
  'pub(crate) fn map_payment_error(error: PaymentError)',
  'fn map_reserved_refund_provider_error(error: PaymentError)',
  'admin payment mapper',
);
const serviceList = between(
  paymentService,
  'pub async fn list_collections(',
  'pub async fn attach_order_to_collection(',
  'payment service list operation',
);

for (const [value, label] of [
  ['Permission::PAYMENTS_READ', 'payment read permission'],
  ['PaymentService::new(runtime.db_clone())', 'payment service construction'],
  ['.list_collections(', 'payment collection list call'],
  ['tenant.id,', 'tenant argument'],
  ['page: pagination.page', 'page argument'],
  ['per_page: pagination.limit()', 'per-page argument'],
  ['status: params.status', 'status filter argument'],
  ['order_id: params.order_id', 'order filter argument'],
  ['cart_id: params.cart_id', 'cart filter argument'],
  ['customer_id: params.customer_id', 'customer filter argument'],
  ['.map_err(super::map_payment_error)?;', 'typed payment error mapper'],
  ['PaginationMeta::new(pagination.page, pagination.limit(), total)', 'pagination response metadata'],
]) {
  requireText(listHandler, value, label);
}

for (const value of [
  'commerce_operation_failed',
  'err.to_string()',
  'error.to_string()',
  'other.to_string()',
]) {
  forbidText(payments, value, 'unsafe admin payment public conversion');
}

const paymentMapperUses = payments.match(/super::map_payment_error/g) ?? [];
if (paymentMapperUses.length !== 4) {
  failures.push(
    `expected four typed PaymentService callsites (collection list/detail and refund list/detail), found ${paymentMapperUses.length}`,
  );
}

for (const [value, label] of [
  ['PaymentError::PaymentCollectionNotFound(_)', 'collection not-found variant'],
  ['PaymentError::PaymentNotFound(_)', 'payment not-found variant'],
  ['PaymentError::RefundNotFound(_)', 'refund not-found variant'],
  ['PaymentError::Validation(_)', 'validation variant'],
  ['PaymentError::InvalidTransition { .. }', 'transition variant'],
  ['PaymentError::ProviderRejected { .. }', 'provider rejection variant'],
  ['PaymentError::ProviderUnavailable { .. }', 'provider unavailable variant'],
  ['PaymentError::ProviderInvalidResponse { .. }', 'provider invalid-response variant'],
  ['PaymentError::ProviderOutcomeUnknown { .. }', 'provider unknown-outcome variant'],
  ['PaymentError::ProviderConfiguration { .. }', 'provider configuration variant'],
  ['PaymentError::Database(_)', 'database variant'],
  ['"commerce_admin_not_found"', 'shared not-found code'],
  ['"commerce_admin_payment_invalid"', 'payment invalid code'],
  ['"commerce_admin_payment_state_conflict"', 'payment state code'],
  ['"commerce_admin_payment_provider_unavailable"', 'provider unavailable code'],
  ['"commerce_admin_payment_provider_invalid_response"', 'provider invalid-response code'],
  ['"commerce_admin_payment_reconciliation_required"', 'reconciliation code'],
  ['"commerce_admin_payment_provider_not_configured"', 'provider configuration code'],
  ['"commerce_admin_payment_storage_unavailable"', 'storage unavailable code'],
]) {
  requireText(mapper, value, label);
}

for (const [value, label] of [
  ['Validation(String)', 'owner validation variant'],
  ['PaymentCollectionNotFound(Uuid)', 'owner collection variant'],
  ['PaymentNotFound(Uuid)', 'owner payment variant'],
  ['RefundNotFound(Uuid)', 'owner refund variant'],
  ['InvalidTransition { from: String, to: String }', 'owner transition variant'],
  ['ProviderUnavailable {', 'owner unavailable variant'],
  ['ProviderRejected {', 'owner rejection variant'],
  ['ProviderInvalidResponse {', 'owner invalid-response variant'],
  ['ProviderOutcomeUnknown {', 'owner unknown-outcome variant'],
  ['ProviderConfiguration { provider_id: String }', 'owner configuration variant'],
  ['Database(#[from] DbErr)', 'owner database variant'],
]) {
  requireText(paymentErrors, value, label);
}

for (const [value, label] of [
  ['-> PaymentResult<(Vec<PaymentCollectionResponse>, u64)>', 'typed list result'],
  ['input.per_page.clamp(1, 100)', 'service page-size bound'],
  ['payment_collection::Column::TenantId.eq(tenant_id)', 'service tenant filter'],
  ['Self::normalize_collection_status_filter(&status)?', 'typed status validation'],
  ['payment_collection::Column::OrderId.eq(order_id)', 'service order filter'],
  ['payment_collection::Column::CartId.eq(cart_id)', 'service cart filter'],
  ['payment_collection::Column::CustomerId.eq(customer_id)', 'service customer filter'],
  ['query.clone().count(&self.db).await?', 'service count propagation'],
  ['.all(&self.db)', 'service page query'],
  ['items.push(self.build_response(row).await?);', 'response build propagation'],
]) {
  requireText(serviceList, value, label);
}

if (failures.length > 0) {
  console.error('Commerce admin payment collection list HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Admin payment collection list uses the typed payment HTTP mapper');
