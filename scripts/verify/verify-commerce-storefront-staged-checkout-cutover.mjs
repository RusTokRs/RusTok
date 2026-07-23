#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const lib = read('crates/rustok-commerce/src/lib.rs');
const mountedRuntime = read(
  'crates/rustok-commerce/src/storefront_checkout_runtime_mounted.rs',
);
const legacyRuntime = read('crates/rustok-commerce/src/storefront_checkout_runtime.rs');
const stagedRuntime = read(
  'crates/rustok-commerce/src/storefront_staged_checkout_runtime.rs',
);
const graphqlCheckout = read(
  'crates/rustok-commerce/src/graphql/mutations/checkout.rs',
);
const restCheckout = read(
  'crates/rustok-commerce/src/controllers/store/checkout.rs',
);
const nativeCheckout = read(
  'crates/rustok-order/storefront/src/transport/native_server_adapter/server_functions.rs',
);
const journaledCheckout = read(
  'crates/rustok-commerce/src/services/journaled_checkout.rs',
);
const orderPorts = read('crates/rustok-order/src/ports.rs');

for (const [source, value, label] of [
  [lib, '#[path = "storefront_checkout_runtime_mounted.rs"]', 'commerce root mounted storefront runtime'],
  [mountedRuntime, 'include!("storefront_checkout_runtime.rs")', 'private legacy compatibility include'],
  [mountedRuntime, 'pub async fn complete_storefront_checkout(', 'mounted checkout completion facade'],
  [mountedRuntime, 'payment_provider_registry: rustok_payment::providers::PaymentProviderRegistry', 'mounted provider registry input'],
  [mountedRuntime, 'idempotency_key: impl Into<String>', 'mounted idempotency input'],
  [mountedRuntime, 'storefront_staged_checkout_runtime::complete_storefront_checkout(', 'mounted staged delegation'],
  [stagedRuntime, 'pub async fn complete_storefront_checkout_input(', 'full staged storefront entrypoint'],
  [stagedRuntime, 'RecoveringStagedCheckoutService::new(staged, compensation)', 'staged recovery composition'],
  [stagedRuntime, 'checkout_input.shipping_selections.clone()', 'full shipping selection preservation'],
  [stagedRuntime, 'with_payment_provider_registry(payment_provider_registry.clone())', 'staged provider registry composition'],
  [graphqlCheckout, 'complete_storefront_checkout_input(', 'GraphQL staged checkout entrypoint'],
  [graphqlCheckout, 'payment_provider_registry_from_context(ctx)', 'GraphQL host provider registry'],
  [restCheckout, 'complete_storefront_checkout_input(', 'REST staged checkout entrypoint'],
  [restCheckout, 'runtime.payment_provider_registry()', 'REST host provider registry'],
  [restCheckout, 'let idempotency_key = required_idempotency_key(&headers)?;', 'REST explicit idempotency identity'],
  [nativeCheckout, 'services::storefront_staged_checkout_runtime', 'native staged runtime import'],
  [nativeCheckout, 'payment_provider_registry', 'native host provider registry'],
  [journaledCheckout, 'Execution is fully delegated to', 'journaled compatibility-only contract'],
  [journaledCheckout, 'RecoveringStagedCheckoutService::new(staged, compensation)', 'journaled staged delegation'],
  [legacyRuntime, 'pub async fn complete_storefront_checkout(', 'retained private legacy completion source'],
  [orderPorts, 'match order.status_kind()', 'typed checkout order lifecycle recovery'],
  [orderPorts, 'OrderStatusKind::Unknown', 'unknown checkout order lifecycle fail-close'],
]) {
  requireText(source, value, label);
}

for (const [source, label] of [
  [mountedRuntime, 'mounted storefront facade'],
  [graphqlCheckout, 'GraphQL storefront checkout'],
  [restCheckout, 'REST storefront checkout'],
  [nativeCheckout, 'native storefront checkout'],
]) {
  for (const value of [
    'CheckoutService::new',
    'JournaledCheckoutService::new',
    'CheckoutPlanBuilder::new',
    'CheckoutStagePipeline::new',
    'RecoveringStagedCheckoutService::new',
    'bind_in_process_atomic_cart_checkout_with_pricing',
    'PrepareCartCheckoutSnapshotRequest',
  ]) {
    forbidText(source, value, `${label} duplicate or legacy checkout construction`);
  }
}

forbidText(mountedRuntime, 'pub use legacy::*;', 'mounted wildcard legacy export');
forbidText(mountedRuntime, 'legacy::complete_storefront_checkout', 'mounted legacy completion export');
forbidText(restCheckout, 'recovering_checkout_http_error', 'REST private staged error mapper drift');
forbidText(restCheckout, 'staged_checkout_http_error', 'REST private checkout error mapper drift');
forbidText(orderPorts, 'match order.status.as_str()', 'raw checkout order lifecycle matching');
forbidText(
  orderPorts,
  '"confirmed" | "paid" | "shipped" | "delivered"',
  'raw checkout order lifecycle alternatives',
);

if (failures.length > 0) {
  console.error('Commerce storefront staged checkout cutover verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ REST, GraphQL, native, and mounted storefront checkout delegate to the shared staged runtime; journaled compatibility and order recovery use the same typed recovery policy',
);
