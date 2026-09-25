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

const product = read('crates/modules/rustok-product/src/fulfillment.rs');
const productDto = read('crates/modules/rustok-product/src/dto/product.rs');
const cart = read('crates/modules/rustok-cart/src/dto/cart.rs');
const cartEntity = read('crates/modules/rustok-cart/src/entities/cart_line_item.rs');
const cartHelpers = read('crates/modules/rustok-cart/src/services/cart/helpers.rs');
const cartService = read('crates/modules/rustok-cart/src/services/cart.rs');
const cartMarketplace = read('crates/modules/rustok-cart/src/services/marketplace_snapshot.rs');
const order = read('crates/modules/rustok-order/src/dto/order.rs');
const orderEntity = read('crates/modules/rustok-order/src/entities/order_line_item.rs');
const orderService = read('crates/modules/rustok-order/src/services/order.rs');
const plan = read('crates/modules/rustok-commerce/src/services/checkout_plan_builder.rs');
const planJournal = read('crates/modules/rustok-commerce/src/services/checkout_order_plan.rs');
const fulfillmentStage = read('crates/modules/rustok-commerce/src/services/checkout_fulfillment_stages_legacy.rs');
const storefrontRest = read('crates/modules/rustok-commerce/src/controllers/store/line_item_resolution.rs');
const storefrontGraphql = read('crates/modules/rustok-commerce/src/graphql/mutations/helpers.rs');
const graphqlShipping = read('crates/modules/rustok-commerce/src/graphql/mutations/typed_shipping_option_helper.rs');
const graphqlRuntime = read('crates/modules/rustok-commerce/src/graphql_runtime.rs');

for (const marker of [
  'pub enum ProductFulfillmentRequirement',
  'from_product_type',
  'pub fulfillment_requirement: ProductFulfillmentRequirement',
]) requireText(product, marker, 'Product fulfillment authority');

for (const marker of [
  'pub enum CartLineFulfillmentRequirement',
  'pub fulfillment_requirement: CartLineFulfillmentRequirement',
]) requireText(cart, marker, 'Cart line fulfillment contract');
requireText(cartEntity, 'pub fulfillment_requirement: String', 'Cart fulfillment persistence');
requireText(cartHelpers, 'normalize_line_item_shipping_profile(', 'Cart fulfillment normalization');
requireText(cartHelpers, 'if requirement == CartLineFulfillmentRequirement::Digital', 'Digital delivery-group exclusion');
requireText(cartHelpers, 'if collect_delivery_group_snapshots(line_items)?.is_empty()', 'Digital shipping-total exclusion');
forbidText(cartHelpers, 'rustok_fulfillment::entities::', 'Cart must not read Fulfillment entities directly');
requireText(cartService, 'Arc<dyn ShippingOptionReadPort>', 'Cart shipping owner port');
requireText(cartService, 'in_process_shipping_option_read_port(', 'Cart shipping port composition');
requireText(cartMarketplace, 'Arc<dyn ShippingOptionReadPort>', 'Marketplace cart shipping owner port');

for (const marker of [
  'pub enum OrderLineFulfillmentRequirement',
  'pub fulfillment_requirement: OrderLineFulfillmentRequirement',
]) requireText(order, marker, 'Order line fulfillment contract');
requireText(orderEntity, 'pub fulfillment_requirement: String', 'Order fulfillment persistence');
requireText(orderService, 'normalize_order_line_item_shipping_profile(', 'Order fulfillment normalization');
requireText(orderService, 'order_line_item_fulfillment_requirement(', 'Order fulfillment response validation');

for (const marker of [
  'ProductFulfillmentRequirement',
  'line_item.fulfillment_requirement',
  'create_fulfillment: input.create_fulfillment && !fulfillment_plans.is_empty()',
]) requireText(plan, marker, 'Checkout fulfillment requirement propagation');
for (const marker of [
  'OrderLineFulfillmentRequirement::Physical',
  'digital order line',
  'physical order line',
]) requireText(planJournal, marker, 'Immutable checkout fulfillment-plan validation');
requireText(fulfillmentStage, 'if plans.is_empty() {', 'Digital checkout must not invoke Fulfillment owner');
requireText(storefrontRest, 'ProductFulfillmentRequirement::Digital', 'REST product-to-cart fulfillment mapping');
requireText(storefrontGraphql, 'ProductFulfillmentRequirement::Digital', 'GraphQL product-to-cart fulfillment mapping');
requireText(graphqlShipping, 'NoDeliveryGroups', 'GraphQL shipping fail-closed state');

forbidText(storefrontRest, 'shipping_profile_slug: Some(effective_shipping_profile_slug(', 'REST digital lines must not get synthetic shipping profiles');
forbidText(storefrontGraphql, 'shipping_profile_slug: Some(effective_shipping_profile_slug(', 'GraphQL digital lines must not get synthetic shipping profiles');
forbidText(plan, 'shipping_profile_slug: item.shipping_profile_slug.unwrap_or', 'Checkout plan must not synthesize a shipping profile');
forbidText(storefrontGraphql, 'FulfillmentService::new(', 'GraphQL cart shipping must use owner port');

for (const marker of [
  'Payment is mandatory for the Commerce module and is therefore required from host composition.',
  'requires PaymentProviderRegistry in host composition',
  'requires CommercePaymentReadRuntime in host composition',
  'requires CommercePaymentCommandRuntime in host composition',
]) requireText(graphqlRuntime, marker, 'Mounted Commerce GraphQL must fail closed when mandatory Payment composition is missing');
forbidText(graphqlRuntime, 'shared_get::<CommercePaymentReadRuntime>()\\n        .unwrap_or_else', 'Mounted GraphQL must not synthesize Payment read runtime');
forbidText(graphqlRuntime, 'shared_get::<CommercePaymentCommandRuntime>()\\n        .unwrap_or_else', 'Mounted GraphQL must not synthesize Payment command runtime');

const serverCommerceRuntime = read('apps/server/src/services/commerce_provider_runtime.rs');
for (const marker of [
  'shared_get::<rustok_commerce::graphql_runtime::CommercePaymentReadRuntime>()',
  'shared_get::<rustok_commerce::graphql_runtime::CommercePaymentCommandRuntime>()',
]) requireText(serverCommerceRuntime, marker, 'Host must compose Commerce Payment GraphQL runtimes');


if (failures.length) {
  for (const failure of failures) console.error(failure);
  process.exit(1);
}
console.log('commerce fulfillment requirement boundary: ok');
