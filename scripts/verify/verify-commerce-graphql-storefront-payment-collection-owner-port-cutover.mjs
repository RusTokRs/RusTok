#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const ownerRuntime = read('crates/rustok-payment/src/collection_runtime.rs');
const paymentLib = read('crates/rustok-payment/src/lib.rs');
const paymentPort = read('crates/rustok-payment/src/ports.rs');
const paymentCartRead = read('crates/rustok-payment/src/cart_read.rs');
const paymentCommands = read('crates/rustok-commerce/src/graphql_runtime/payment_commands.rs');
const server = read('apps/server/src/services/commerce_provider_runtime.rs');
const checkout = read('crates/rustok-commerce/src/graphql/mutations/checkout.rs');
const routing = read('crates/rustok-commerce/src/graphql/mutations/mod.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-commerce/docs/graphql-storefront-payment-collection-owner-port-cutover-2026-08-09.md',
);

const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const sliceBetween = (source, start, end) => {
  const from = source.indexOf(start);
  const to = source.indexOf(end, from + start.length);
  if (from < 0 || to < 0) return '';
  return source.slice(from, to);
};

for (const marker of [
  'pub struct PaymentCollectionRuntime',
  'port: Arc<dyn PaymentCollectionPort>',
  'pub fn new(port: Arc<dyn PaymentCollectionPort>)',
  'pub fn in_process(db: DatabaseConnection)',
  'PaymentService::new(db)',
  'pub fn port(&self) -> Arc<dyn PaymentCollectionPort>',
]) need(ownerRuntime, marker, 'Payment collection owner runtime');
need(paymentLib, 'pub use collection_runtime::PaymentCollectionRuntime;', 'Payment public runtime export');

for (const marker of [
  'pub trait PaymentCollectionPort',
  'async fn create_or_reuse_collection(',
  '.require_policy(PortCallPolicy::write())',
  'context.require_write_semantics()',
  'find_reusable_collection_by_cart(tenant_id, cart_id)',
  '"create_or_reuse_collection.adopt_race"',
]) need(paymentPort, marker, 'Payment create/reuse owner contract');
for (const marker of [
  'pub trait PaymentCartReadPort',
  'async fn find_reusable_collection_by_cart(',
  '.require_policy(PortCallPolicy::read())',
]) need(paymentCartRead, marker, 'Payment reusable read owner contract');

for (const marker of [
  'PaymentCollectionRuntime',
  'collection_create_or_reuse: PaymentCollectionRuntime',
  '.shared_get::<PaymentCollectionRuntime>()',
  'PaymentCollectionRuntime::in_process(inputs.db_clone())',
  'pub fn collection_create_or_reuse_port(&self) -> Arc<dyn PaymentCollectionPort>',
]) need(paymentCommands, marker, 'Commerce Payment command composition');

for (const marker of [
  'shared_get::<rustok_payment::PaymentCollectionRuntime>()',
  'server.shared_get::<rustok_payment::PaymentCollectionRuntime>()',
  'rustok_payment::PaymentCollectionRuntime::in_process(server.db_clone())',
  'server.shared_insert(runtime.clone());',
  'host.with_shared_value(runtime)',
]) need(server, marker, 'server Payment collection host composition');

need(
  routing,
  '#[path = "safe_checkout.rs"]\npub mod checkout;',
  'mounted checkout routing',
);

const resolver = sliceBetween(
  checkout,
  'async fn create_storefront_payment_collection(',
  'async fn complete_storefront_checkout(',
);
if (!resolver) failures.push('unable to isolate mounted storefront payment collection resolver');

for (const marker of [
  'ReusablePaymentCollectionByCartRequest',
  'PaymentCollectionCreateOrReuseRequest',
  'storefront_payment_collection_read_context(',
  'payment_read_runtime_for_current_graphql_scope(',
  '.cart_read_port()',
  '.find_reusable_collection_by_cart(',
  'return Ok(existing.into());',
  'storefront_payment_collection_command_context(',
  'payment_command_runtime_from_context(',
  '.collection_create_or_reuse_port()',
  '.create_or_reuse_collection(',
  'cart_id: Some(cart.id)',
  'order_id: None',
  'customer_id: cart.customer_id',
  'currency_code: cart.currency_code.clone()',
  'amount: cart.total_amount',
  'cart_context_metadata(&cart, &store_context)',
]) need(resolver, marker, 'mounted Payment collection owner cutover');

const reusableIndex = resolver.indexOf('.find_reusable_collection_by_cart(');
const metadataIndex = resolver.indexOf('parse_optional_metadata(input.metadata.as_deref())?');
if (reusableIndex < 0 || metadataIndex < 0 || reusableIndex >= metadataIndex) {
  failures.push('reusable Payment owner read must happen before GraphQL metadata parsing');
}

for (const marker of [
  'fn storefront_payment_collection_actor(',
  'PortActor::user(auth.user_id.to_string())',
  'PortActor::service("rustok-commerce.graphql-storefront-payment-collection")',
  'fn storefront_payment_collection_locale(',
  '.with_deadline(std::time::Duration::from_secs(2))',
  'request.channel_slug.as_deref()',
  '.with_idempotency_key(format!("graphql-storefront-payment-collection:{cart_id}"))',
]) need(checkout, marker, 'trusted Payment owner context');

for (const marker of [
  'fn payment_collection_port_graphql_policy(',
  'PortErrorKind::Validation',
  'PortErrorKind::NotFound',
  'error.code == "payment.provider_rejected"',
  'error.code == "payment.provider_outcome_unknown"',
  'error.code == "payment.provider_invalid_response"',
  'error.code == "payment.provider_not_configured"',
  'error.code == "payment.database_unavailable"',
  'error.code == "payment.cart_read_unavailable"',
  '"payment_request_invalid"',
  '"payment_resource_not_found"',
  '"payment_state_conflict"',
  '"payment_temporarily_unavailable"',
  '"payment_provider_rejected"',
  '"payment_reconciliation_required"',
  '"payment_storage_unavailable"',
  '"payment_operation_failed"',
  'owner_error_kind = ?error.kind',
  'owner_code_length = error.code.chars().count()',
  'boundary = "commerce_graphql_storefront_payment_collection"',
]) need(checkout, marker, 'bounded Payment GraphQL error compatibility');

for (const marker of [
  'PaymentService::new(',
  'use rustok_payment::{PaymentError, PaymentService}',
  '.create_collection(',
]) forbid(resolver, marker, 'mounted Payment collection concrete owner boundary');
for (const marker of [
  'PaymentService::new(',
  'use rustok_payment::{PaymentError, PaymentService}',
]) forbid(checkout, marker, 'mounted checkout concrete Payment owner construction');

const paymentMapperStart = checkout.indexOf('fn payment_collection_owner_graphql_error(');
const paymentMapper = paymentMapperStart < 0 ? '' : checkout.slice(paymentMapperStart);
if (!paymentMapper) failures.push('unable to isolate Payment collection GraphQL owner mapper');
for (const marker of [
  'error = ?error',
  'owner_message = %error.message',
  'message = %error.message',
]) forbid(paymentMapper, marker, 'bounded Payment owner diagnostics');

need(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'canonical topology item remains open',
);

for (const marker of [
  '# Commerce GraphQL storefront payment-collection owner-port cutover',
  'Status: `source_complete_unvalidated`',
  '`PaymentCartReadPort::find_reusable_collection_by_cart`',
  '`PaymentCollectionPort::create_or_reuse_collection`',
  '`PaymentCollectionRuntime`',
  'without parsing the new request metadata',
  'does **not** claim',
  'The canonical ecommerce topology item',
  'no tests, Cargo commands, Node verifiers, formatter',
]) need(record, marker, 'truthful source record');

if (failures.length > 0) {
  console.error('Commerce GraphQL storefront payment collection owner-port cutover verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ mounted GraphQL storefront payment collection preserves reusable-first parity and routes Payment reads/writes through host-composed owner ports',
);
