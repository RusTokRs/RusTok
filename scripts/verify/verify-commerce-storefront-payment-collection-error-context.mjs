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
const httpRuntime = read('crates/rustok-commerce/src/controllers/mod.rs');
const serverRuntime = read('apps/server/src/services/commerce_provider_runtime.rs');
const paymentPort = read('crates/rustok-payment/src/ports.rs');
const paymentCartRead = read('crates/rustok-payment/src/cart_read.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-commerce/docs/rest-storefront-payment-collection-owner-port-cutover-2026-08-09.md',
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

const route = between(
  checkout,
  'pub async fn create_payment_collection(',
  '/// Complete storefront cart checkout',
  'payment collection route',
);
const mapper = checkout.slice(checkout.indexOf('fn payment_collection_error_policy('));

for (const [value, label] of [
  ['PaymentCartReadRuntime', 'HTTP Payment cart read runtime field'],
  ['PaymentCollectionRuntime', 'HTTP Payment collection runtime field'],
  ['fn payment_cart_read_port(', 'HTTP Payment cart read accessor'],
  ['fn payment_collection_port(', 'HTTP Payment collection accessor'],
  ['shared_get::<rustok_payment::PaymentCartReadRuntime>()', 'HTTP host-selected cart read runtime'],
  ['shared_get::<rustok_payment::PaymentCollectionRuntime>()', 'HTTP host-selected collection runtime'],
]) requireText(httpRuntime, value, label);

for (const [value, label] of [
  ['shared_get::<rustok_payment::PaymentCartReadRuntime>()', 'server cart read host selection'],
  ['server.shared_get::<rustok_payment::PaymentCartReadRuntime>()', 'server cart read reuse'],
  ['rustok_payment::PaymentCartReadRuntime::in_process(server.db_clone())', 'server cart read owner fallback'],
  ['shared_get::<rustok_payment::PaymentCollectionRuntime>()', 'server collection host selection'],
  ['server.shared_get::<rustok_payment::PaymentCollectionRuntime>()', 'server collection reuse'],
  ['rustok_payment::PaymentCollectionRuntime::in_process(server.db_clone())', 'server collection owner fallback'],
]) requireText(serverRuntime, value, label);

for (const [value, label] of [
  ['super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;', 'storefront channel guard'],
  ['super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;', 'customer resolution'],
  ['in_process_cart_storefront_port(runtime.db_clone())', 'cart storefront port'],
  ['super::ensure_store_cart_access(&cart, customer_id)?;', 'cart access guard'],
  ['super::ensure_cart_allows_payment_collection(&cart)?;', 'payment collection lifecycle guard'],
  ['super::reprice_storefront_cart_line_items_for_db(', 'cart repricing'],
  ['super::resolve_context_from_cart_for_db(runtime.db(), tenant.id, &request_context, &cart)', 'store context resolution'],
  ['ReusablePaymentCollectionByCartRequest { cart_id: cart.id }', 'owner reusable request'],
  ['runtime\n        .payment_cart_read_port()', 'host-selected reusable read port'],
  ['.find_reusable_collection_by_cart(', 'owner reusable lookup'],
  ['return Ok((StatusCode::OK, Json(existing)));', 'pre-existing reusable response'],
  ['PaymentCollectionCreateOrReuseRequest {', 'owner create/reuse request'],
  ['runtime\n        .payment_collection_port()', 'host-selected collection port'],
  ['.create_or_reuse_collection(', 'owner create/reuse call'],
  ['cart_id: Some(cart.id)', 'created collection cart id'],
  ['order_id: None', 'created collection order contract'],
  ['customer_id: cart.customer_id', 'created collection customer id'],
  ['currency_code: cart.currency_code.clone()', 'currency forwarding'],
  ['amount: cart.total_amount', 'amount forwarding'],
  ['super::cart_context_metadata(&cart, &store_context)', 'metadata forwarding'],
  ['Ok((StatusCode::CREATED, Json(collection)))', 'post-miss response parity'],
]) requireText(route, value, label);

for (const [value, label] of [
  ['fn storefront_payment_collection_actor(', 'owner actor helper'],
  ['PortActor::user(auth.user_id.to_string())', 'authenticated owner actor'],
  ['PortActor::service("rustok-commerce.storefront-payment-collection")', 'guest owner actor'],
  ['fn storefront_payment_collection_port_context(', 'owner context helper'],
  ['.with_deadline(std::time::Duration::from_secs(2))', 'bounded owner deadline'],
  ['request_context.channel_slug.as_deref()', 'request channel propagation'],
  ['.with_idempotency_key(format!("storefront-payment-collection:{cart_id}"))', 'cart-bound write admission identity'],
]) requireText(checkout, value, label);

for (const [value, label] of [
  ['fn payment_collection_error_policy(error: &PortError)', 'typed owner error policy'],
  ['PortErrorKind::Validation', 'validation mapping'],
  ['PortErrorKind::NotFound', 'not-found mapping'],
  ['error.code == "payment.provider_rejected"', 'provider rejection mapping'],
  ['error.code == "payment.provider_outcome_unknown"', 'provider unknown mapping'],
  ['error.code == "payment.provider_invalid_response"', 'invalid provider response mapping'],
  ['error.code == "payment.provider_not_configured"', 'provider configuration mapping'],
  ['error.code == "payment.database_unavailable"', 'collection storage mapping'],
  ['error.code == "payment.cart_read_unavailable"', 'reusable read storage mapping'],
  ['"payment_request_invalid"', 'validation public code'],
  ['"payment_resource_not_found"', 'not-found public code'],
  ['"payment_state_conflict"', 'state conflict public code'],
  ['"payment_provider_rejected"', 'provider rejection public code'],
  ['"payment_reconciliation_required"', 'reconciliation public code'],
  ['"payment_temporarily_unavailable"', 'temporary public code'],
  ['"payment_storage_unavailable"', 'storage public code'],
  ['"payment_operation_failed"', 'unexpected owner failure code'],
  ['owner_error_kind = ?error.kind', 'bounded owner error kind diagnostic'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner code diagnostic'],
  ['retryable = error.retryable', 'owner retryability diagnostic'],
  ['HttpError::new(status, code, message)', 'stable public HTTP envelope'],
]) requireText(mapper, value, label);

for (const value of [
  'PaymentService::new(',
  'use rustok_payment::{PaymentError, PaymentService}',
  '.create_collection(',
  'error = ?error',
  'owner_message = %error.message',
  'message = %error.message',
  'error.to_string()',
  'err.to_string()',
]) forbidText(route, value, 'mounted storefront Payment concrete/raw owner boundary');

for (const [value, label] of [
  ['pub trait PaymentCollectionPort', 'Payment collection owner port'],
  ['async fn create_or_reuse_collection(', 'Payment owner create/reuse operation'],
  ['"create_or_reuse_collection.adopt_race"', 'Payment owner race adoption'],
]) requireText(paymentPort, value, label);
for (const [value, label] of [
  ['pub trait PaymentCartReadPort', 'Payment cart owner read port'],
  ['async fn find_reusable_collection_by_cart(', 'Payment owner reusable read'],
]) requireText(paymentCartRead, value, label);

requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'broad ecommerce topology item remains open',
);

for (const [value, label] of [
  ['# Commerce REST storefront payment-collection owner-port cutover', 'record title'],
  ['Status: `source_complete_unvalidated`', 'record validation status'],
  ['`PaymentCartReadPort::find_reusable_collection_by_cart`', 'record reusable owner read'],
  ['`PaymentCollectionPort::create_or_reuse_collection`', 'record create/reuse owner call'],
  ['`200 OK`', 'record reusable response parity'],
  ['`201 Created`', 'record post-miss response parity'],
  ['does **not** claim', 'record replay non-claim'],
  ['The canonical ecommerce topology item remains open.', 'record broad item remains open'],
  ['No tests, Cargo commands, Node verifiers, formatter', 'record validation non-execution'],
]) requireText(record, value, label);

if (failures.length > 0) {
  console.error('Commerce storefront payment collection owner-port verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ mounted REST storefront payment collection preserves 200/201 parity and routes Payment reuse/create through host-composed owner ports',
);
