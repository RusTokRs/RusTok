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
const httpRuntime = read('crates/rustok-commerce/src/controllers/mod.rs');
const paymentOwner = read('crates/rustok-payment/src/order_read.rs');
const paymentLib = read('crates/rustok-payment/src/lib.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-commerce/docs/rest-storefront-order-refund-read-owner-port-cutover-2026-08-10.md',
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

const refundRoute = between(
  controller,
  'pub async fn list_order_refunds(',
  '/// List order changes for the current customer',
  'mounted storefront refund route',
);
const paymentMapper = between(
  controller,
  'fn map_storefront_payment_port_error(',
  'async fn current_storefront_customer_id(',
  'storefront Payment port mapper',
);

requireText(
  controller,
  'use rustok_payment::ListRefundsByOrderRequest;',
  'typed Payment request import',
);
for (const [value, label] of [
  ['let customer_id = ensure_customer_owns_order(', 'ownership admission'],
  ['"list_order_refunds_access"', 'ownership operation label'],
  ['let payment_context = storefront_order_read_port_context(', 'bounded read context'],
  ['.payment_order_read_port()', 'host-selected Payment read port'],
  ['.list_refunds_by_order(', 'Payment refund owner call'],
  ['ListRefundsByOrderRequest {', 'typed refund request'],
  ['order_id: id,', 'order filter'],
  ['page: params.pagination.page,', 'page forwarding'],
  ['per_page: params.pagination.per_page,', 'per-page forwarding'],
  ['status: params.status,', 'status forwarding'],
  ['data: page.items,', 'page item projection'],
  ['PaginationMeta::new(params.pagination.page, params.pagination.limit(), page.total)', 'pagination metadata'],
]) requireText(refundRoute, value, label);

for (const value of [
  'PaymentService::new(',
  'PaymentError',
  'ListRefundsInput',
  '.list_refunds(',
]) forbidText(refundRoute, value, 'mounted refund route concrete Payment dependency');

for (const [value, label] of [
  ['fn payment_order_read_port(', 'Commerce runtime accessor'],
  ['std::sync::Arc<dyn rustok_payment::PaymentOrderReadPort>', 'Commerce trait object'],
  ['self.payment_order_read_runtime.read_port()', 'host-selected Payment runtime'],
]) requireText(httpRuntime, value, label);

for (const [value, label] of [
  ['pub trait PaymentOrderReadPort: Send + Sync', 'Payment order-read trait'],
  ['async fn list_refunds_by_order(', 'refund-list owner capability'],
  ['request: ListRefundsByOrderRequest', 'in-process typed request'],
  ['pub struct ListRefundsByOrderRequest', 'request type'],
  ['pub struct PaymentOrderRefundPage', 'page type'],
  ['context.require_policy(PortCallPolicy::read())?', 'read admission'],
  ['"payment.order_refund_read_unavailable"', 'default fail-closed capability'],
  ['.list_refunds(', 'owner-local concrete execution'],
  ['payment_collection_id: None,', 'collection filter parity'],
  ['order_id: Some(request.order_id),', 'order filter parity'],
  ['status: request.status,', 'status parity'],
  ['Ok(PaymentOrderRefundPage { items, total })', 'owner page projection'],
]) requireText(paymentOwner, value, label);

for (const [value, label] of [
  ['ListRefundsByOrderRequest', 'request export'],
  ['PaymentOrderRefundPage', 'page export'],
]) requireText(paymentLib, value, label);

for (const [value, label] of [
  ['"payment.order_refund_provider_unavailable"', 'provider unavailable identity'],
  ['"payment.order_refund_provider_invalid_response"', 'provider invalid-response identity'],
  ['"payment.order_refund_reconciliation_required"', 'reconciliation identity'],
  ['"payment.order_refund_provider_not_configured"', 'provider configuration identity'],
  ['"commerce_store_payment_provider_unavailable"', 'provider unavailable envelope'],
  ['"commerce_store_payment_provider_invalid_response"', 'provider invalid-response envelope'],
  ['"commerce_store_payment_reconciliation_required"', 'reconciliation envelope'],
  ['"commerce_store_payment_provider_not_configured"', 'provider configuration envelope'],
  ['"commerce_store_payment_unavailable"', 'storage unavailable envelope'],
  ['owner_error_kind = ?error.kind', 'bounded owner kind diagnostic'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner code diagnostic'],
  ['retryable = error.retryable', 'retryability diagnostic'],
]) requireText(paymentMapper, value, label);
for (const value of ['error = ?error', 'error.message', 'error.to_string()', 'internal_message']) {
  forbidText(paymentMapper, value, 'raw Payment owner diagnostic');
}

for (const value of [
  'use rustok_payment::{PaymentService',
  'let payment_service = PaymentService::new(runtime.db_clone());',
  'StorefrontOrderPaymentErrorContext',
  'fn storefront_order_payment_error_policy(',
]) forbidText(controller, value, 'stale mounted concrete Payment boundary');

requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'broad ecommerce topology P0 remains open',
);

for (const [value, label] of [
  ['# Commerce REST storefront order-refund owner-read cutover', 'record title'],
  ['Status: `source_complete_unvalidated`', 'record status'],
  ['PaymentOrderReadPort::list_refunds_by_order', 'record owner capability'],
  ['default fail-closed', 'record external adapter behavior'],
  ['no tests, Cargo commands, Node verifiers, formatter', 'record validation status'],
]) requireText(record, value, label);

if (failures.length > 0) {
  console.error('Commerce REST storefront order-refund owner-read cutover verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ mounted storefront order refunds use the host-selected Payment owner read port');
