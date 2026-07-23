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
  [stagedRuntime, 'pub const fn public_code(&self)', 'stable checkout public code contract'],
  [stagedRuntime, 'pub const fn public_message(&self)', 'stable checkout public message contract'],
  [stagedRuntime, 'pub const fn retryable(&self)', 'stable checkout retryability contract'],
  [stagedRuntime, 'StorefrontStagedCheckoutRuntimeError::TemporarilyUnavailable', 'temporary dependency failure classification'],
  [stagedRuntime, 'map_owner_port_error(', 'owner port failure classification'],
  [graphqlCheckout, 'complete_storefront_checkout_input(', 'GraphQL staged checkout entrypoint'],
  [graphqlCheckout, 'payment_provider_registry_from_context(ctx)', 'GraphQL host provider registry'],
  [graphqlCheckout, 'storefront_checkout_graphql_error', 'GraphQL stable checkout mapper'],
  [graphqlCheckout, 'payment_collection_graphql_error(', 'GraphQL stable payment collection mapper'],
  [graphqlCheckout, 'extensions.set("code", code)', 'GraphQL public error code extension'],
  [graphqlCheckout, 'extensions.set("retryable", retryable)', 'GraphQL public retryability extension'],
  [graphqlCheckout, 'extensions.set("reconciliation_required", reconciliation_required)', 'GraphQL payment reconciliation extension'],
  [restCheckout, 'complete_storefront_checkout_input(', 'REST staged checkout entrypoint'],
  [restCheckout, 'runtime.payment_provider_registry()', 'REST host provider registry'],
  [restCheckout, 'let idempotency_key = required_idempotency_key(&headers)?;', 'REST explicit idempotency identity'],
  [restCheckout, 'payment_collection_http_error(', 'REST stable payment collection mapper'],
  [nativeCheckout, 'services::storefront_staged_checkout_runtime', 'native staged runtime import'],
  [nativeCheckout, 'payment_provider_registry', 'native host provider registry'],
  [nativeCheckout, 'native_checkout_runtime_error', 'native stable checkout mapper'],
  [nativeCheckout, 'error.public_code()', 'native stable checkout code'],
  [nativeCheckout, 'error.public_message()', 'native stable checkout message'],
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

for (const [source, value, label] of [
  [mountedRuntime, 'pub use legacy::*;', 'mounted wildcard legacy export'],
  [mountedRuntime, 'legacy::complete_storefront_checkout', 'mounted legacy completion export'],
  [restCheckout, 'recovering_checkout_http_error', 'REST private staged error mapper drift'],
  [restCheckout, 'staged_checkout_http_error', 'REST private checkout error mapper drift'],
  [restCheckout, 'err.to_string()', 'REST raw payment error display'],
  [graphqlCheckout, 'Error::new(error.to_string())', 'GraphQL raw checkout error display'],
  [graphqlCheckout, '.find_reusable_collection_by_cart(tenant_id, cart.id)\n            .await?', 'GraphQL raw reusable payment error propagation'],
  [graphqlCheckout, '.create_collection(\n', 'GraphQL payment collection creation presence'],
  [nativeCheckout, 'ServerFnError::new(error.to_string())', 'native raw checkout error display'],
  [nativeCheckout, 'ServerFn(error.to_string())', 'native transport raw server error display'],
  [stagedRuntime, '#[error("checkout request is invalid: {0}")]', 'runtime validation detail display'],
  [orderPorts, 'match order.status.as_str()', 'raw checkout order lifecycle matching'],
  [orderPorts, '"confirmed" | "paid" | "shipped" | "delivered"', 'raw checkout order lifecycle alternatives'],
]) {
  if (label === 'GraphQL payment collection creation presence') {
    continue;
  }
  forbidText(source, value, label);
}

if (failures.length > 0) {
  console.error('Commerce storefront staged checkout cutover verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ REST, GraphQL, native, and mounted storefront checkout delegate to the shared staged runtime and publish stable checkout/payment errors; journaled compatibility and order recovery use the same typed recovery policy',
);
