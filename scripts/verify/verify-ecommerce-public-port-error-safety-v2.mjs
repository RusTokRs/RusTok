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

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const channel = read('crates/rustok-channel/src/ports.rs');
const region = read('crates/rustok-region/src/ports.rs');
const cart = read('crates/rustok-cart/src/checkout_snapshot.rs');
const pricing = read('crates/rustok-pricing/src/ports.rs');
const payment = read('crates/rustok-payment/src/ports.rs');
const paymentCompensation = read(
  'crates/rustok-payment/src/checkout_compensation.rs',
);
const fulfillment = read('crates/rustok-fulfillment/src/ports.rs');
const customer = read('crates/rustok-customer/src/ports.rs');
const inventory = read('crates/rustok-inventory/src/ports.rs');
const order = read('crates/rustok-order/src/ports.rs');
const orderCompensation = read('crates/rustok-order/src/checkout_compensation.rs');
const orderPaymentSettlement = read(
  'crates/rustok-order/src/checkout_payment_settlement.rs',
);
const orderRecovery = read('crates/rustok-order/src/checkout_order_recovery.rs');
const orderCheckoutAdapters =
  orderCompensation + orderPaymentSettlement + orderRecovery;

for (const [source, label] of [
  [channel, 'channel port'],
  [region, 'region port'],
  [cart, 'cart checkout port'],
  [pricing, 'pricing port'],
  [payment, 'payment collection port'],
  [paymentCompensation, 'payment checkout compensation port'],
  [fulfillment, 'fulfillment shipping selection port'],
  [customer, 'customer read port'],
  [inventory, 'inventory reservation port'],
  [order, 'order generic checkout port'],
  [orderCompensation, 'order checkout compensation port'],
  [orderPaymentSettlement, 'order checkout payment settlement port'],
  [orderRecovery, 'order checkout recovery adapter'],
]) {
  requireText(source, 'tracing::error!', label);
}

for (const value of [
  'error.to_string(),\n            true',
  'error.to_string(),\n            false',
  'format!("channel port serialization failed: {error}")',
  'format!("region port failed: {error}")',
]) {
  forbidText(channel + region, value, 'channel/region public error mapping');
}

for (const value of [
  'CartError::Validation(error.to_string())',
  'format!("failed to serialize cart projection: {error}")',
  'format!("failed to serialize cart snapshot: {error}")',
  'PortError::validation("cart.checkout_validation", message)',
]) {
  forbidText(cart, value, 'cart checkout public error mapping');
}

for (const value of [
  'format!("pricing storage unavailable: {error}")',
  '"pricing.rich_error",\n            error.to_string()',
  '"pricing.core_error",\n            error.to_string()',
  'PortError::validation("pricing.validation", message)',
  '.map_err(pricing_error_to_port_error)',
]) {
  forbidText(pricing, value, 'pricing public error mapping');
}

for (const value of [
  'PortError::validation("payment.validation", message)',
  'format!("invalid payment transition from `{from}` to `{to}`")',
  'format!("payment provider `{provider_id}` is unavailable for `{operation}`")',
  'format!("payment provider `{provider_id}` rejected `{operation}`")',
  'format!("payment provider `{provider_id}` outcome is unknown for `{operation}`")',
  '.map_err(payment_error_to_port_error)',
]) {
  forbidText(payment, value, 'payment collection public error mapping');
}

for (const value of [
  'fn manual_reconciliation(message: impl Into<String>)',
  '.map_err(payment_error_to_port_error)',
  'fn payment_error_to_port_error(error: PaymentError)',
  '"PortContext.tenant_id must be a UUID for payment ports"',
]) {
  forbidText(
    paymentCompensation,
    value,
    'payment checkout compensation public error mapping',
  );
}

for (const value of [
  'PortError::validation("fulfillment.validation", message)',
  'format!("shipping option {id} not found")',
  'format!("fulfillment {id} not found")',
  'format!("invalid fulfillment transition from `{from}` to `{to}`")',
  'format!("fulfillment storage unavailable: {error}")',
  '.map_err(fulfillment_error_to_port_error)',
  '"PortContext.tenant_id must be a UUID for fulfillment ports"',
]) {
  forbidText(fulfillment, value, 'fulfillment public error mapping');
}

for (const value of [
  'format!("customer storage unavailable: {error}")',
  'format!("customer {id} not found")',
  'format!("customer for user {id} not found")',
  'format!("duplicate customer email `{email}`")',
  'format!("customer already linked to user {user_id}")',
  'PortError::validation("customer.validation", message)',
  'format!("customer profile projection unavailable: {error}")',
  '.map_err(customer_error_to_port_error)',
  '"PortContext.tenant_id must be a UUID for customer ports"',
]) {
  forbidText(customer, value, 'customer public error mapping');
}

for (const value of [
  'PortError::validation("inventory.validation", message)',
  'format!("variant {id} not found")',
  'format!("variant {variant_id} was not found")',
  'format!("insufficient inventory: requested {requested}, available {available}")',
  '.map_err(inventory_error_to_port_error)',
  '.map_err(storage_unavailable)',
  'return Err(storage_unavailable(error));',
  'fn storage_unavailable(_error: sea_orm::DbErr)',
  '"PortContext.tenant_id must be a UUID for inventory ports"',
]) {
  forbidText(inventory, value, 'inventory public error mapping');
}

for (const value of [
  'PortError::validation("order.checkout_identity_validation", message)',
  'PortError::validation("order.validation", message)',
  '.map_err(order_checkout_identity_error_to_port_error)',
  '.map_err(order_error_to_port_error)',
  'fn order_checkout_identity_error_to_port_error(error: OrderCheckoutIdentityError)',
  'fn order_error_to_port_error(error: OrderError)',
  '"PortContext.tenant_id must be a UUID for order ports"',
  '"PortContext.actor.id must be a UUID for order write ports"',
]) {
  forbidText(order, value, 'order generic port public error mapping');
}

for (const value of [
  'fn manual_reconciliation(message: impl Into<String>)',
  'PortError::validation("order.validation", message)',
  '.map_err(order_error_to_port_error)',
  '"PortContext.tenant_id must be a UUID for order ports"',
  '"PortContext.actor.id must be a UUID for order write ports"',
  'format!(\n                "{field} must be a lowercase hexadecimal value',
]) {
  forbidText(
    orderCheckoutAdapters,
    value,
    'order checkout adapter public error mapping',
  );
}

for (const [source, value, label] of [
  [pricing, 'correlation_id = %context.correlation_id', 'pricing correlation logging'],
  [pricing, 'tenant_id = %context.tenant_id', 'pricing tenant logging'],
  [pricing, 'operation,', 'pricing owner operation logging'],
  [pricing, 'code = "pricing.database_unavailable"', 'pricing database stable code'],
  [pricing, 'code = "pricing.validation"', 'pricing validation stable code'],
  [pricing, 'code = "pricing.rich_error"', 'pricing rich stable code'],
  [pricing, 'code = "pricing.core_error"', 'pricing core stable code'],
  [pricing, '"pricing storage is temporarily unavailable"', 'pricing stable storage message'],
  [pricing, '"pricing operation failed an internal invariant"', 'pricing stable invariant message'],
  [pricing, '"pricing request is invalid"', 'pricing stable validation message'],
  [pricing, 'pricing_error_to_port_error(&context, "resolve_product_price"', 'pricing operation mapping'],
  [pricing, 'pricing_error_to_port_error(&context, "upsert_variant_price"', 'pricing write operation mapping'],
  [payment, 'correlation_id = %context.correlation_id', 'payment correlation logging'],
  [payment, 'tenant_id = %context.tenant_id', 'payment tenant logging'],
  [payment, 'operation = owner_operation', 'payment owner operation logging'],
  [payment, 'code = "payment.validation"', 'payment validation stable code'],
  [payment, 'code = "payment.invalid_transition"', 'payment transition stable code'],
  [payment, 'code = "payment.provider_unavailable"', 'payment unavailable stable code'],
  [payment, 'code = "payment.provider_rejected"', 'payment rejection stable code'],
  [payment, 'code = "payment.provider_invalid_response"', 'payment invalid response stable code'],
  [payment, 'code = "payment.provider_outcome_unknown"', 'payment outcome stable code'],
  [payment, 'code = "payment.provider_not_configured"', 'payment configuration stable code'],
  [payment, 'code = "payment.database_unavailable"', 'payment database stable code'],
  [payment, '"payment storage is temporarily unavailable"', 'payment stable storage message'],
  [payment, '"payment provider outcome requires reconciliation"', 'payment stable reconciliation message'],
  [payment, '"payment provider response could not be applied safely"', 'payment stable invalid response message'],
  [payment, '"payment provider rejected the requested operation"', 'payment stable rejection message'],
  [payment, '"payment request is invalid"', 'payment stable validation message'],
  [payment, 'payment_error_to_port_error(&context, "read_collection_status"', 'payment read operation mapping'],
  [paymentCompensation, 'correlation_id = %context.correlation_id', 'payment compensation correlation logging'],
  [paymentCompensation, 'tenant_id = %context.tenant_id', 'payment compensation tenant logging'],
  [paymentCompensation, 'operation = owner_operation', 'payment compensation owner operation logging'],
  [paymentCompensation, 'code = "payment.checkout_compensation_manual_reconciliation"', 'payment compensation reconciliation stable code'],
  [paymentCompensation, 'code = "payment.checkout_compensation_encoding_failed"', 'payment compensation encoding stable code'],
  [paymentCompensation, '"payment storage is temporarily unavailable"', 'payment compensation stable storage message'],
  [paymentCompensation, '"payment provider rejected the requested operation"', 'payment compensation stable rejection message'],
  [paymentCompensation, '"payment provider response could not be applied safely"', 'payment compensation stable invalid response message'],
  [paymentCompensation, '"payment request context is invalid"', 'payment compensation stable context message'],
  [paymentCompensation, '"payment checkout compensation requires manual reconciliation"', 'payment compensation stable reconciliation message'],
  [paymentCompensation, 'let owner_operation = COMPENSATE_CHECKOUT_PAYMENT_OPERATION;', 'payment compensation operation mapping'],
  [paymentCompensation, 'parse_tenant_id(&context, owner_operation)', 'payment compensation context-aware tenant parsing'],
  [paymentCompensation, 'require_operation_context(&context, owner_operation', 'payment compensation context-aware causation parsing'],
  [paymentCompensation, 'payment_error_to_port_error(&context, owner_operation, error)', 'payment compensation public context-aware mapping'],
  [paymentCompensation, 'payment_error_to_port_error(context, owner_operation, error)', 'payment compensation helper context-aware mapping'],
  [paymentCompensation, 'persisted_cancel_result(context, owner_operation', 'payment compensation checkpoint context mapping'],
  [paymentCompensation, 'fn manual_reconciliation(\n    context: &PortContext,', 'payment compensation reconciliation context'],
  [fulfillment, 'correlation_id = %context.correlation_id', 'fulfillment correlation logging'],
  [fulfillment, 'tenant_id = %context.tenant_id', 'fulfillment tenant logging'],
  [fulfillment, 'operation = owner_operation', 'fulfillment owner operation logging'],
  [fulfillment, 'code = "fulfillment.context_invalid"', 'fulfillment context stable code'],
  [fulfillment, 'code = "fulfillment.validation"', 'fulfillment validation stable code'],
  [fulfillment, 'code = "fulfillment.shipping_option_not_found"', 'fulfillment shipping option stable code'],
  [fulfillment, 'code = "fulfillment.fulfillment_not_found"', 'fulfillment resource stable code'],
  [fulfillment, 'code = "fulfillment.invalid_transition"', 'fulfillment transition stable code'],
  [fulfillment, 'code = "fulfillment.database_unavailable"', 'fulfillment database stable code'],
  [fulfillment, '"fulfillment request context is invalid"', 'fulfillment stable context message'],
  [fulfillment, '"fulfillment request is invalid"', 'fulfillment stable validation message'],
  [fulfillment, '"shipping option was not found"', 'fulfillment stable shipping option message'],
  [fulfillment, '"fulfillment was not found"', 'fulfillment stable resource message'],
  [fulfillment, '"fulfillment lifecycle transition conflicts with the current state"', 'fulfillment stable transition message'],
  [fulfillment, '"fulfillment storage is temporarily unavailable"', 'fulfillment stable storage message'],
  [fulfillment, 'parse_port_tenant_id(&context, "list_seller_shipping_options")', 'fulfillment list operation mapping'],
  [fulfillment, 'parse_port_tenant_id(&context, "select_shipping_option")', 'fulfillment select operation mapping'],
  [customer, 'correlation_id = %context.correlation_id', 'customer correlation logging'],
  [customer, 'tenant_id = %context.tenant_id', 'customer tenant logging'],
  [customer, 'operation = owner_operation', 'customer owner operation logging'],
  [customer, 'code = "customer.context_invalid"', 'customer context stable code'],
  [customer, 'code = "customer.database_unavailable"', 'customer database stable code'],
  [customer, 'code = "customer.validation"', 'customer validation stable code'],
  [customer, 'code = "customer.profile_unavailable"', 'customer profile stable code'],
  [customer, '"customer request context is invalid"', 'customer stable context message'],
  [customer, '"customer storage is temporarily unavailable"', 'customer stable storage message'],
  [customer, '"customer request is invalid"', 'customer stable validation message'],
  [customer, '"customer profile projection is temporarily unavailable"', 'customer stable profile message'],
  [customer, 'customer_error_to_port_error(&context, owner_operation, error)', 'customer context-aware mapping'],
  [customer, 'let owner_operation = "read_customer_projection"', 'customer projection operation mapping'],
  [customer, 'let owner_operation = "read_customer_projection_by_user"', 'customer user operation mapping'],
  [customer, 'let owner_operation = "list_customer_projections"', 'customer list operation mapping'],
  [customer, 'let owner_operation = "list_profile_enrichment"', 'customer enrichment operation mapping'],
  [inventory, 'correlation_id = %context.correlation_id', 'inventory correlation logging'],
  [inventory, 'tenant_id = %context.tenant_id', 'inventory tenant logging'],
  [inventory, 'operation = owner_operation', 'inventory owner operation logging'],
  [inventory, 'code = "inventory.context_invalid"', 'inventory context stable code'],
  [inventory, 'code = "inventory.database_unavailable"', 'inventory database stable code'],
  [inventory, 'code = "inventory.variant_not_found"', 'inventory not-found stable code'],
  [inventory, 'code = "inventory.insufficient_inventory"', 'inventory conflict stable code'],
  [inventory, 'code = "inventory.validation"', 'inventory validation stable code'],
  [inventory, 'code = "inventory.invariant_violation"', 'inventory invariant stable code'],
  [inventory, '"inventory request context is invalid"', 'inventory stable context message'],
  [inventory, '"inventory storage is temporarily unavailable"', 'inventory stable storage message'],
  [inventory, '"inventory variant was not found"', 'inventory stable not-found message'],
  [inventory, '"inventory reservation conflicts with available stock"', 'inventory stable conflict message'],
  [inventory, '"inventory request is invalid"', 'inventory stable validation message'],
  [inventory, 'parse_port_tenant_id(&context, owner_operation)', 'inventory context-aware parsing'],
  [inventory, 'inventory_error_to_port_error(&context, owner_operation, error)', 'inventory context-aware mapping'],
  [inventory, 'let owner_operation = "check_availability"', 'inventory availability operation mapping'],
  [inventory, 'let owner_operation = "reserve_inventory"', 'inventory reserve operation mapping'],
  [inventory, 'let owner_operation = "release_inventory_reservation"', 'inventory release operation mapping'],
  [inventory, 'let owner_operation = "reserve_inventory_by_identity"', 'inventory identity reserve operation mapping'],
  [inventory, 'let owner_operation = "release_inventory_by_identity"', 'inventory identity release operation mapping'],
  [inventory, 'storage_unavailable_with_context(&context, owner_operation, error)', 'inventory identity storage mapping'],
  [inventory, 'storage_unavailable_with_context(context, owner_operation, error)', 'inventory helper storage mapping'],
  [inventory, 'async fn load_inventory_item_for_update<C>(\n    context: &PortContext,', 'inventory item helper context'],
  [inventory, 'async fn load_inventory_item_by_id_for_update<C>(\n    context: &PortContext,', 'inventory item by id helper context'],
  [inventory, 'async fn find_reservation_by_external_id<C>(\n    context: &PortContext,', 'inventory reservation lookup helper context'],
  [inventory, 'async fn existing_reservation_snapshot<C>(\n    context: &PortContext,', 'inventory reservation snapshot helper context'],
  [inventory, 'async fn available_quantity<C>(\n    context: &PortContext,', 'inventory available quantity helper context'],
  [order, 'correlation_id = %context.correlation_id', 'order generic correlation logging'],
  [order, 'tenant_id = %context.tenant_id', 'order generic tenant logging'],
  [order, 'operation = owner_operation', 'order generic owner operation logging'],
  [order, 'code = "order.checkout_identity_validation"', 'order identity validation stable code'],
  [order, 'code = "order.checkout_identity_storage_unavailable"', 'order identity storage stable code'],
  [order, 'code = "order.database_unavailable"', 'order database stable code'],
  [order, 'code = "order.validation"', 'order validation stable code'],
  [order, 'code = "order.invalid_transition"', 'order transition stable code'],
  [order, 'code = "order.invariant_violation"', 'order invariant stable code'],
  [order, '"checkout order identity request is invalid"', 'order identity stable validation message'],
  [order, '"order request is invalid"', 'order stable validation message'],
  [order, '"order request context is invalid"', 'order stable context message'],
  [order, 'order_checkout_identity_error_to_port_error(', 'order identity context-aware mapping'],
  [order, 'order_error_to_port_error(context, owner_operation, error)', 'order helper context-aware mapping'],
  [order, 'order_error_to_port_error(&context, owner_operation, error)', 'order public context-aware mapping'],
  [order, 'let owner_operation = "read_checkout_identity_by_operation"', 'order identity operation read mapping'],
  [order, 'let owner_operation = "read_checkout_identity_by_cart"', 'order identity cart read mapping'],
  [order, 'let owner_operation = "bind_checkout_identity"', 'order identity bind mapping'],
  [order, 'let owner_operation = "adopt_legacy_checkout_identity"', 'order identity adoption mapping'],
  [order, 'let owner_operation = "complete_checkout"', 'order completion operation mapping'],
  [order, 'let owner_operation = "read_checkout_result"', 'order result read mapping'],
  [order, 'let owner_operation = "read_checkout_result_by_operation"', 'order operation result read mapping'],
  [order, 'let owner_operation = "read_order_status"', 'order status read mapping'],
  [orderCompensation, 'correlation_id = %context.correlation_id', 'order compensation correlation logging'],
  [orderCompensation, 'tenant_id = %context.tenant_id', 'order compensation tenant logging'],
  [orderCompensation, 'operation,', 'order compensation owner operation logging'],
  [orderCompensation, 'code = "order.checkout_compensation_manual_reconciliation"', 'order compensation reconciliation stable code'],
  [orderCompensation, '"checkout requires manual reconciliation"', 'order compensation stable reconciliation message'],
  [orderCompensation, '"order request context is invalid"', 'order compensation stable context message'],
  [orderCompensation, '"read_checkout_order_for_compensation"', 'order compensation operation mapping'],
  [orderPaymentSettlement, 'correlation_id = %context.correlation_id', 'order payment correlation logging'],
  [orderPaymentSettlement, 'tenant_id = %context.tenant_id', 'order payment tenant logging'],
  [orderPaymentSettlement, 'operation,', 'order payment owner operation logging'],
  [orderPaymentSettlement, 'code = "order.checkout_payment_validation"', 'order payment validation stable code'],
  [orderPaymentSettlement, 'code = "order.checkout_payment_state_conflict"', 'order payment transition stable code'],
  [orderPaymentSettlement, '"checkout requires manual reconciliation"', 'order payment stable reconciliation message'],
  [orderPaymentSettlement, '"order request context is invalid"', 'order payment stable context message'],
  [orderPaymentSettlement, '"mark_checkout_order_paid"', 'order payment operation mapping'],
  [orderRecovery, 'correlation_id = %context.correlation_id', 'order recovery correlation logging'],
  [orderRecovery, 'tenant_id = %context.tenant_id', 'order recovery tenant logging'],
  [orderRecovery, 'operation,', 'order recovery owner operation logging'],
  [orderRecovery, 'code = "order.checkout_request_encoding_failed"', 'order recovery encoding stable code'],
  [orderRecovery, 'code = "order.checkout_recovery_validation"', 'order recovery validation stable code'],
  [orderRecovery, 'code = "order.checkout_hash_invalid"', 'order recovery hash stable code'],
  [orderRecovery, '"checkout hash evidence is invalid"', 'order recovery stable hash message'],
  [orderRecovery, '"order request context is invalid"', 'order recovery stable context message'],
  [orderRecovery, '"confirm_recovered_checkout_order"', 'order recovery confirm operation mapping'],
  [orderRecovery, 'hash_json(context, "encode_checkout_snapshot_hash"', 'order recovery snapshot hash mapping'],
]) {
  requireText(source, value, label);
}

requireText(
  channel,
  '"channel storage is temporarily unavailable"',
  'channel stable storage message',
);
requireText(
  region,
  '"region storage is temporarily unavailable"',
  'region stable storage message',
);
requireText(
  cart,
  '"cart checkout request or projection is invalid"',
  'cart stable validation message',
);
requireText(
  cart,
  '"cart checkout snapshot could not be encoded"',
  'cart stable encoding message',
);

if (failures.length > 0) {
  console.error('Scoped ecommerce public port error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Channel, region, cart, pricing, payment collection/compensation, fulfillment, customer, inventory, and order checkout adapters keep raw owner errors out of public PortError messages and retain correlation-safe technical logs',
);
