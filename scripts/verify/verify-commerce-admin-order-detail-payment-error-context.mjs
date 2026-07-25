#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/rustok-commerce/src/controllers/admin/orders.rs');
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

const showOrder = between(
  source,
  'pub async fn show_order(',
  'fn map_order_detail_payment_error(',
  'admin order detail handler',
);
const mapper = between(
  source,
  'fn map_order_detail_payment_error(',
  'fn map_order_detail_fulfillment_error(',
  'admin order detail payment mapper',
);

for (const [value, label] of [
  ['use rustok_payment::{PaymentError, PaymentService};', 'typed payment error import'],
  [
    'const ADMIN_ORDER_DETAIL_PAYMENT_OWNER: &str = "rustok_payment.admin_order_detail";',
    'payment owner constant',
  ],
  [
    'const ADMIN_ORDER_DETAIL_PAYMENT_OPERATION: &str = "find_latest_payment_collection_by_order";',
    'payment operation constant',
  ],
]) requireText(source, value, label);

for (const [value, label] of [
  [
    '.map_err(|error| map_order_detail_payment_error(tenant.id, id, error))?',
    'order-detail payment mapper handoff',
  ],
  ['[Permission::ORDERS_READ]', 'order read permission'],
  ['Path(id): Path<Uuid>', 'typed order path'],
  ['HttpResult<Json<AdminOrderDetailResponse>>', 'order detail result contract'],
  ['find_latest_collection_by_order(tenant.id, id)', 'payment lookup contract'],
]) requireText(showOrder, value, label);

for (const [value, label] of [
  ['PaymentError::Validation(_)', 'validation variant'],
  ['PaymentError::PaymentCollectionNotFound(_)', 'collection not-found variant'],
  ['PaymentError::PaymentNotFound(_)', 'payment not-found variant'],
  ['PaymentError::RefundNotFound(_)', 'refund not-found variant'],
  ['PaymentError::InvalidTransition { .. }', 'transition variant'],
  ['PaymentError::ProviderUnavailable { .. }', 'provider unavailable variant'],
  ['PaymentError::ProviderRejected { .. }', 'provider rejected variant'],
  ['PaymentError::ProviderInvalidResponse { .. }', 'provider invalid-response variant'],
  ['PaymentError::ProviderOutcomeUnknown { .. }', 'provider unknown-outcome variant'],
  ['PaymentError::ProviderConfiguration { .. }', 'provider configuration variant'],
  ['PaymentError::Database(_)', 'database variant'],
  ['error = ?error', 'internal typed cause'],
  ['owner = ADMIN_ORDER_DETAIL_PAYMENT_OWNER', 'owner log'],
  ['tenant_id = %tenant_id', 'tenant log'],
  ['order_id = %order_id', 'order identity log'],
  ['operation = ADMIN_ORDER_DETAIL_PAYMENT_OPERATION', 'operation log'],
  ['error_kind,', 'error kind log'],
  ['public_code = code', 'stable code log'],
  ['status = %status', 'status log'],
  ['boundary = "commerce_admin_order_detail_http"', 'HTTP boundary log'],
]) requireText(mapper, value, label);

for (const [value, label] of [
  ['"Commerce resource not found"', 'static not-found envelope'],
  ['"Payment request is invalid"', 'static validation envelope'],
  ['"Payment operation conflicts with the current state"', 'static conflict envelope'],
  ['"Payment provider is temporarily unavailable"', 'static provider-unavailable envelope'],
  [
    '"Payment provider returned an invalid response; reconciliation may be required"',
    'static provider-invalid-response envelope',
  ],
  [
    '"Payment provider outcome is unknown and requires reconciliation"',
    'static reconciliation envelope',
  ],
  [
    '"Payment provider is not configured for this tenant"',
    'static provider-configuration envelope',
  ],
  ['"Payment storage is temporarily unavailable"', 'static storage envelope'],
  ['HttpError::new(status, code, message)', 'single public envelope constructor'],
]) requireText(mapper, value, label);

for (const [value, label] of [
  ['Validation(String)', 'owner validation variant'],
  ['PaymentCollectionNotFound(Uuid)', 'owner collection not-found variant'],
  ['PaymentNotFound(Uuid)', 'owner payment not-found variant'],
  ['RefundNotFound(Uuid)', 'owner refund not-found variant'],
  ['InvalidTransition { from: String, to: String }', 'owner transition variant'],
  ['ProviderUnavailable {', 'owner provider unavailable variant'],
  ['ProviderRejected {', 'owner provider rejected variant'],
  ['ProviderInvalidResponse {', 'owner provider invalid-response variant'],
  ['ProviderOutcomeUnknown {', 'owner provider unknown-outcome variant'],
  ['ProviderConfiguration { provider_id: String }', 'owner provider configuration variant'],
  ['Database(#[from] DbErr)', 'owner database variant'],
]) requireText(paymentErrors, value, label);

for (const value of [
  '.map_err(super::map_payment_error)',
  'error.to_string()',
  'format!("Payment request is invalid:',
  'HttpError::bad_request("commerce_admin_payment_invalid", error',
]) forbidText(showOrder + mapper, value, 'unsafe admin order detail payment mapping');

if (failures.length > 0) {
  console.error('Commerce admin order-detail payment error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin order detail retains payment causes internally and returns static public envelopes',
);
