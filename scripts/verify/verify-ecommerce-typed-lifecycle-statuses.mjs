#!/usr/bin/env node

import { existsSync, readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const cart = read('crates/modules/rustok-cart/src/dto/status.rs');
const cartLib = read('crates/modules/rustok-cart/src/lib.rs');
const cartAtomic = read('crates/modules/rustok-cart/src/atomic_checkout_port.rs');
const storefrontStore = read('crates/modules/rustok-commerce/src/controllers/store/mod.rs');
const order = read('crates/modules/rustok-order/src/status.rs');
const orderLib = read('crates/modules/rustok-order/src/lib.rs');
const orderCompensation = read('crates/modules/rustok-order/src/checkout_compensation.rs');
const orderRecovery = read('crates/modules/rustok-order/src/checkout_order_recovery.rs');
const orderStageFacade = read('crates/modules/rustok-commerce/src/services/checkout_order_stages.rs');
const orderCreation = read('crates/modules/rustok-commerce/src/services/checkout_order_creation.rs');
const orderConfirmation = read('crates/modules/rustok-commerce/src/services/checkout_order_confirmation.rs');
const orderAdoption = read('crates/modules/rustok-commerce/src/services/checkout_inventory_order_adoption.rs');
// The order-stage compatibility include source was retired: the mounted facade
// is the only order-stage implementation and it must stay that way.
const orderStageLegacy = 'crates/modules/rustok-commerce/src/services/checkout_order_stages_legacy.rs';
if (existsSync(new URL(orderStageLegacy, root))) {
  failures.push(`${orderStageLegacy}: retired source must not return`);
}
const orderStage = orderStageFacade;
const payment = read('crates/modules/rustok-payment/src/dto/payment.rs');
const paymentPorts = read('crates/modules/rustok-payment/src/ports.rs');
const paymentStageFacade = read(
  'crates/modules/rustok-commerce/src/services/checkout_payment_stages.rs',
);
const paymentStageLegacy = read(
  'crates/modules/rustok-commerce/src/services/checkout_payment_stages_legacy.rs',
);
const paymentStage = `${paymentStageFacade}\n${paymentStageLegacy}`;
const fulfillment = read('crates/modules/rustok-fulfillment/src/status.rs');
const fulfillmentTypedExecution = read(
  'crates/modules/rustok-fulfillment/src/checkout_execution_typed.rs',
);
const fulfillmentLib = read('crates/modules/rustok-fulfillment/src/lib.rs');
const fulfillmentStage = read(
  'crates/modules/rustok-commerce/src/services/checkout_fulfillment_stages.rs',
);
const fulfillmentAdminCommand = read(
  'crates/modules/rustok-fulfillment/src/admin_command.rs',
);
const fulfillmentCommerceOrchestration = read(
  'crates/modules/rustok-commerce/src/services/fulfillment_orchestration.rs',
);
const fulfillmentCommerceFacade = read(
  'crates/modules/rustok-commerce/src/services/fulfillment_orchestration_facade.rs',
);
const finalization = read('crates/modules/rustok-commerce/src/services/checkout_finalization.rs');
const compensation = read(
  'crates/modules/rustok-commerce/src/services/checkout_compensation_owner_ports.rs',
);

requireText(
  orderStageFacade,
  'CheckoutOrderRecoveryAdapter',
  'mounted order-stage recovery adapter',
);
requireText(
  orderStageFacade,
  'ReadCheckoutOrderProjectionRequest {',
  'mounted order-stage projection read',
);
requireText(
  paymentStageFacade,
  '#[path = "checkout_payment_stages_legacy.rs"]',
  'mounted payment-stage facade',
);
requireText(
  paymentStageFacade,
  'struct SanitizingCheckoutPaymentExecutionPort',
  'mounted payment-stage typed owner adapter',
);

for (const [source, value, label] of [
  [cart, 'pub enum CartStatus', 'canonical cart status enum'],
  [storefrontStore, 'use rustok_cart::{CartStatus, CartStorefrontPort, CartStorefrontRepriceRequest};', 'storefront typed cart status import'],
  [storefrontStore, 'let status = cart.lifecycle_status().map_err', 'storefront cart lifecycle accessor'],
  [storefrontStore, 'if status == CartStatus::Completed', 'storefront completed status check'],
  [cart, 'pub fn lifecycle_status(&self) -> CartResult<CartStatus>', 'cart typed accessor'],
  [cart, 'pub const fn can_begin_checkout(self) -> bool', 'cart begin predicate'],
  [cart, 'pub const fn can_complete_checkout(self) -> bool', 'cart completion predicate'],
  [cart, 'CartStatus::parse(self.status.as_str()).ok_or_else', 'cart unknown fail-closed mapping'],
  [cartAtomic, 'let status = cart_status(&cart)?;', 'atomic cart typed result'],
  [cartAtomic, 'let current_status = cart_status(&current)?;', 'atomic cart typed admission'],
  [cartAtomic, 'CartStatus::parse(from.as_str())', 'atomic transition race parse'],
  [cartAtomic, 'cart.lifecycle_status().map_err(cart_error_to_port_error)', 'atomic cart typed helper'],
  [order, 'pub enum OrderStatusKind', 'order status enum'],
  [order, 'pub enum OrderChangeStatusKind', 'order change status enum'],
  [order, 'pub enum OrderReturnStatusKind', 'order return status enum'],
  [order, 'pub fn status_kind(&self) -> OrderStatusKind', 'order typed accessor'],
  [order, '_ => Self::Unknown', 'order unknown mapping'],
  [orderLib, 'pub mod status;', 'order status module export'],
  [orderCompensation, 'pub fn status_kind(&self) -> OrderStatusKind', 'order snapshot typed accessor'],
  [orderRecovery, 'match order.status_kind()', 'typed order recovery dispatch'],
  [orderRecovery, 'OrderStatusKind::Pending', 'typed pending recovery'],
  [orderRecovery, 'OrderStatusKind::Cancelled', 'typed cancelled recovery'],
  [orderRecovery, 'OrderStatusKind::Unknown', 'typed unknown recovery'],
  [orderCreation, 'OrderStatusKind', 'order creation typed status import'],
  [orderCreation, 'order.status_kind() != OrderStatusKind::Pending', 'typed order creation admission'],
  [orderConfirmation, 'OrderStatusKind', 'order confirmation typed status import'],
  [orderConfirmation, 'match order.status_kind()', 'typed confirmation dispatch'],
  [orderAdoption, 'order.status_kind(),', 'typed adoption admission'],
  [orderAdoption, 'OrderStatusKind::Pending | OrderStatusKind::Confirmed', 'typed adoption status set'],
  [payment, 'pub enum PaymentCollectionStatusKind', 'payment collection status enum'],
  [payment, 'pub enum PaymentStatusKind', 'payment status enum'],
  [payment, 'pub enum RefundStatusKind', 'refund status enum'],
  [payment, 'pub fn status_kind(&self) -> PaymentCollectionStatusKind', 'payment typed accessor'],
  [payment, '_ => Self::Unknown', 'payment unknown mapping'],
  [paymentPorts, 'pub fn status_kind(&self) -> PaymentCollectionStatusKind', 'payment snapshot typed accessor'],
  [paymentStage, 'authorized.status_kind().is_authorized_or_captured()', 'typed authorization result'],
  [paymentStage, 'captured.status_kind().is_captured()', 'typed capture result'],
  [paymentStage, 'collection.status_kind().is_captured()', 'typed captured replay'],
  [paymentStage, 'order.status_kind()', 'typed order admission'],
  [fulfillment, 'pub enum FulfillmentStatusKind', 'fulfillment status enum'],
  [fulfillment, 'pub fn status_kind(&self) -> FulfillmentStatusKind', 'fulfillment typed accessor'],
  [fulfillment, '_ => Self::Unknown', 'fulfillment unknown mapping'],
  [fulfillmentTypedExecution, 'match fulfillment.status_kind()', 'typed fulfillment dispatch'],
  [fulfillmentTypedExecution, 'FulfillmentStatusKind::Cancelled', 'cancelled reconciliation'],
  [fulfillmentTypedExecution, 'FulfillmentStatusKind::Unknown', 'unknown reconciliation'],
  [fulfillmentLib, 'mod checkout_execution_typed;', 'typed fulfillment factory module'],
  [
    fulfillmentLib,
    'TypedCheckoutFulfillmentExecutionPort, in_process_checkout_fulfillment_execution_port',
    'typed fulfillment root factory export',
  ],
  [fulfillmentLib, 'pub mod status;', 'fulfillment status module export'],
  [
    fulfillmentStage,
    'in_process_checkout_fulfillment_execution_port',
    'mounted fulfillment typed root factory',
  ],
  [
    fulfillmentAdminCommand,
    'FulfillmentStatusKind::Pending | FulfillmentStatusKind::Shipped',
    'fulfillment admin typed ship admission',
  ],
  [
    fulfillmentAdminCommand,
    'current.status_kind() == FulfillmentStatusKind::Shipped',
    'fulfillment admin typed reship replay',
  ],
  [
    fulfillmentAdminCommand,
    'current.status_kind() != FulfillmentStatusKind::Delivered',
    'fulfillment admin typed reship transition',
  ],
  [
    fulfillmentAdminCommand,
    'current.status_kind() == FulfillmentStatusKind::Cancelled',
    'fulfillment admin typed cancel replay',
  ],
  [
    fulfillmentAdminCommand,
    'current.status_kind() == FulfillmentStatusKind::Delivered',
    'fulfillment admin typed cancel transition',
  ],
  [
    fulfillmentCommerceOrchestration,
    'FulfillmentStatusKind::Cancelled',
    'commerce fulfillment orchestration typed cancellation filter',
  ],
  [
    fulfillmentCommerceFacade,
    'current.status_kind() == FulfillmentStatusKind::Shipped',
    'commerce fulfillment facade typed reship replay',
  ],
  [finalization, 'let cart = match cart_status(&current)?', 'typed cart finalization dispatch'],
  [finalization, 'PaymentCollectionStatusKind::Captured', 'typed finalization payment admission'],
  [finalization, 'state.order.status_kind()', 'typed finalization order admission'],
  [finalization, 'cart_status(cart)? != CartStatus::Completed', 'typed completed cart validation'],
  [compensation, 'snapshot.status_kind() != PaymentCollectionStatusKind::Cancelled', 'typed payment compensation result'],
  [compensation, 'snapshot.status_kind() != OrderStatusKind::Cancelled', 'typed order compensation result'],
  [compensation, 'match cart_status(&current)?', 'typed cart compensation dispatch'],
  [compensation, 'cart_status(&released)? != CartStatus::Active', 'typed cart release result'],
]) {
  requireText(source, value, label);
}

for (const value of [
  'authorized.status.as_str()',
  'captured.status != "captured"',
  'collection.status != "captured"',
  'payment_collection.status',
  'collection.status.as_str()',
  'order.status.as_str()',
]) {
  forbidText(paymentStage, value, 'mounted payment stage raw lifecycle status');
}

for (const value of [
  'match order.status.as_str()',
  '"pending" =>',
  '"confirmed" | "paid" | "shipped" | "delivered"',
]) {
  forbidText(orderRecovery, value, 'order checkout recovery raw lifecycle status');
}

for (const [source, label] of [
  [orderStage, 'mounted order stage'],
  [orderCreation, 'order creation executor'],
  [orderConfirmation, 'order confirmation executor'],
  [orderAdoption, 'order adoption service'],
]) {
  for (const value of [
    'allowed_statuses: &[&str]',
    'order.status.as_str()',
    'order.status != "pending"',
    '&["confirmed", "paid", "shipped", "delivered"]',
  ]) {
    forbidText(source, value, `${label} raw lifecycle status`);
  }
}

for (const value of [
  'cart.status.as_str()',
  'current.status.as_str()',
  'current.status == CartStatus::Active.as_str()',
]) {
  forbidText(cartAtomic, value, 'atomic cart raw lifecycle status');
}

for (const value of [
  'current.status == CartStatus::Completed.as_str()',
  'current.status.as_str()',
  'state.payment_collection.status != "captured"',
  'state.order.status.as_str()',
  'cart.status != CartStatus::Completed.as_str()',
  'cart.status == "completed"',
]) {
  forbidText(finalization, value, 'checkout finalization raw lifecycle status');
}

for (const value of [
  'snapshot.status != "cancelled"',
  'current.status.as_str()',
  'released.status != CartStatus::Active.as_str()',
]) {
  forbidText(compensation, value, 'checkout compensation raw owner lifecycle status');
}

for (const value of [
  'matches!(current.status.as_str(), "',
  'current.status == "shipped"',
  'current.status != "delivered"',
  'current.status == "cancelled"',
  'current.status == "delivered"',
]) {
  forbidText(fulfillmentAdminCommand, value, 'fulfillment admin raw lifecycle status');
}

forbidText(
  fulfillmentCommerceOrchestration,
  'fulfillment.status == "cancelled"',
  'commerce fulfillment orchestration raw lifecycle status',
);
forbidText(
  fulfillmentCommerceFacade,
  'current.status == "shipped"',
  'commerce fulfillment facade raw lifecycle status',
);

forbidText(
  fulfillmentLib,
  'pub use checkout_execution::*;',
  'unsafe fulfillment root factory wildcard export',
);
forbidText(
  fulfillmentStage,
  'checkout_execution::in_process_checkout_fulfillment_execution_port',
  'mounted fulfillment legacy factory bypass',
);
forbidText(
  fulfillmentStage,
  'InProcessCheckoutFulfillmentExecutionPort',
  'mounted fulfillment direct adapter construction',
);

forbidText(cartLib, 'pub mod status;', 'duplicate cart status module export');
if (existsSync(new URL('crates/modules/rustok-cart/src/status.rs', root))) {
  failures.push('duplicate cart lifecycle status file must not exist');
}

if (failures.length > 0) {
  console.error('Ecommerce typed lifecycle owner verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Ecommerce owners plus mounted order, payment, fulfillment, cart finalization, and compensation paths use canonical typed lifecycle views',
);
