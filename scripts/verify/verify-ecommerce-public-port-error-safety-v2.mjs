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
const requireAny = (source, values, label) => {
  if (!values.some((value) => source.includes(value))) {
    failures.push(`${label}: missing one of ${values.join(' | ')}`);
  }
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireAll = (source, values, label) => {
  for (const value of values) requireText(source, value, label);
};
const forbidAll = (source, values, label) => {
  for (const value of values) forbidText(source, value, label);
};

const channel = read('crates/rustok-channel/src/ports.rs');
const region = read('crates/rustok-region/src/ports.rs');
const cart = read('crates/rustok-cart/src/checkout_snapshot.rs');
const pricing = read('crates/rustok-pricing/src/ports.rs');
const payment = read('crates/rustok-payment/src/ports.rs');
const paymentCompensation = read('crates/rustok-payment/src/checkout_compensation.rs');
const fulfillment = read('crates/rustok-fulfillment/src/ports.rs');
const customer = read('crates/rustok-customer/src/ports.rs');
const inventory = read('crates/rustok-inventory/src/ports.rs');
const order = read('crates/rustok-order/src/ports.rs');
const orderCompensation = read('crates/rustok-order/src/checkout_compensation.rs');
const orderPaymentSettlement = read('crates/rustok-order/src/checkout_payment_settlement.rs');
const orderRecovery = read('crates/rustok-order/src/checkout_order_recovery.rs');
const orderCheckoutAdapters = orderCompensation + orderPaymentSettlement + orderRecovery;

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
]) requireText(source, 'tracing::error!', label);

forbidAll(channel + region, [
  'error.to_string(),\n            true',
  'error.to_string(),\n            false',
  'format!("channel port serialization failed: {error}")',
  'format!("region port failed: {error}")',
], 'channel/region public error mapping');

forbidAll(cart, [
  'CartError::Validation(error.to_string())',
  'format!("failed to serialize cart projection: {error}")',
  'format!("failed to serialize cart snapshot: {error}")',
  'PortError::validation("cart.checkout_validation", message)',
], 'cart checkout public error mapping');

forbidAll(pricing, [
  'format!("pricing storage unavailable: {error}")',
  '"pricing.rich_error",\n            error.to_string()',
  '"pricing.core_error",\n            error.to_string()',
  'PortError::validation("pricing.validation", message)',
  'format!("variant {variant_id} does not belong to product {product_id}")',
  'format!("price for variant {variant_id} was not found")',
  'format!("price list {} was not found", request.price_list_id)',
  'format!("product {id} not found")',
  'format!("variant {id} not found")',
  'format!("duplicate handle `{handle}` for locale `{locale}`")',
  'format!("duplicate sku `{sku}`")',
  'format!("insufficient inventory: requested {requested}, available {available}")',
  'format!("shipping profile {id} not found")',
  'format!("duplicate shipping profile slug `{slug}`")',
  '"PortContext.tenant_id must be a UUID for pricing ports"',
  '"pricing write actor must be a UUID"',
  '.map_err(pricing_error_to_port_error)',
], 'pricing public error mapping');

forbidAll(pricing, [
  'error = ?error',
  'cause = %message',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'product_id = %',
  'variant_id = %',
  'price_list_id = %',
  'handle = %',
  'sku = %',
  'slug = %',
  'code = "pricing.insufficient_inventory",\n                requested,\n                available,',
], 'pricing payload diagnostics');

forbidAll(payment, [
  'PortError::validation("payment.validation", message)',
  'format!("invalid payment transition from `{from}` to `{to}`")',
  'format!("payment provider `{provider_id}` is unavailable for `{operation}`")',
  'format!("payment provider `{provider_id}` rejected `{operation}`")',
  'format!("payment provider `{provider_id}` outcome is unknown for `{operation}`")',
  'format!("payment collection {id} not found")',
  'format!("payment for collection {id} not found")',
  'format!("refund {id} not found")',
  '.map_err(payment_error_to_port_error)',
], 'payment collection public error mapping');
forbidAll(payment, [
  'cause = %message',
  'error = ?error',
  'error = %error',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'provider_id = %provider_id',
  'provider_operation = %operation',
  'from = %from',
  'to = %to',
], 'payment collection payload diagnostics');

forbidAll(paymentCompensation, [
  'fn manual_reconciliation(message: impl Into<String>)',
  '.map_err(payment_error_to_port_error)',
  'fn payment_error_to_port_error(error: PaymentError)',
  '"PortContext.tenant_id must be a UUID for payment ports"',
], 'payment checkout compensation public error mapping');

forbidAll(fulfillment, [
  'PortError::validation("fulfillment.validation", message)',
  'format!("shipping option {id} not found")',
  'format!("fulfillment {id} not found")',
  'format!("invalid fulfillment transition from `{from}` to `{to}`")',
  'format!("fulfillment storage unavailable: {error}")',
  '.map_err(fulfillment_error_to_port_error)',
  '"PortContext.tenant_id must be a UUID for fulfillment ports"',
], 'fulfillment public error mapping');
forbidAll(fulfillment, [
  'error = ?error',
  'error = %message',
  'resource_id = %id',
  'from = %from',
  'to = %to',
  'tenant_id = %context.tenant_id',
], 'fulfillment payload diagnostics');

forbidAll(customer, [
  'format!("customer storage unavailable: {error}")',
  'format!("customer {id} not found")',
  'format!("customer for user {id} not found")',
  'format!("duplicate customer email `{email}`")',
  'format!("customer already linked to user {user_id}")',
  'PortError::validation("customer.validation", message)',
  'format!("customer profile projection unavailable: {error}")',
  '.map_err(customer_error_to_port_error)',
  '"PortContext.tenant_id must be a UUID for customer ports"',
], 'customer public error mapping');
forbidAll(customer, [
  'error = ?error',
  'error = %error',
  'error = %message',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'customer_id = %',
  'user_id = %',
  'email = %',
  'search = %',
  'search = ?',
], 'customer payload diagnostics');

forbidAll(inventory, [
  'PortError::validation("inventory.validation", message)',
  'format!("variant {id} not found")',
  'format!("variant {variant_id} was not found")',
  'format!("insufficient inventory: requested {requested}, available {available}")',
  '.map_err(inventory_error_to_port_error)',
  '.map_err(storage_unavailable)',
  'return Err(storage_unavailable(error));',
  'fn storage_unavailable(_error: sea_orm::DbErr)',
  '"PortContext.tenant_id must be a UUID for inventory ports"',
], 'inventory public error mapping');
forbidAll(inventory, [
  'error = ?error',
  'error = ?other',
  'error = %error',
  'internal_message = %message',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'variant_id = %variant_id',
  'code = "inventory.insufficient_inventory",\n                requested,\n                available,',
], 'inventory payload diagnostics');

forbidAll(order, [
  'PortError::validation("order.checkout_identity_validation", message)',
  'PortError::validation("order.validation", message)',
  '.map_err(order_checkout_identity_error_to_port_error)',
  '.map_err(order_error_to_port_error)',
  'fn order_checkout_identity_error_to_port_error(error: OrderCheckoutIdentityError)',
  'fn order_error_to_port_error(error: OrderError)',
  '"PortContext.tenant_id must be a UUID for order ports"',
  '"PortContext.actor.id must be a UUID for order write ports"',
], 'order generic port public error mapping');
forbidAll(orderCheckoutAdapters, [
  'fn manual_reconciliation(message: impl Into<String>)',
  'PortError::validation("order.validation", message)',
  '.map_err(order_error_to_port_error)',
  '"PortContext.tenant_id must be a UUID for order ports"',
  '"PortContext.actor.id must be a UUID for order write ports"',
  'format!(\n                "{field} must be a lowercase hexadecimal value',
], 'order checkout adapter public error mapping');

requireText(pricing, 'correlation_id = %context.correlation_id', 'pricing correlation logging');
requireText(payment, 'operation = owner_operation', 'payment owner operation logging');
requireText(fulfillment, 'correlation_id = %context.correlation_id', 'fulfillment correlation logging');
requireAny(customer, [
  'correlation_id_length = context_facts.correlation_id_length',
  'correlation_id = %context.correlation_id',
], 'customer correlation logging');
requireText(inventory, 'correlation_id = %context.correlation_id', 'inventory correlation logging');
requireText(inventory, 'storage_unavailable_with_context(&context, owner_operation, error)', 'inventory identity storage mapping');
requireText(inventory, 'storage_unavailable_with_context(context, owner_operation, error)', 'inventory helper storage mapping');
requireText(order, 'correlation_id = %context.correlation_id', 'order generic correlation logging');
requireText(orderCompensation, 'correlation_id = %context.correlation_id', 'order compensation correlation logging');
requireText(orderRecovery, 'correlation_id = %context.correlation_id', 'order recovery correlation logging');

requireAny(paymentCompensation, [
  'tenant_id_length = context_facts.tenant_id_length',
  'tenant_id = %context.tenant_id',
], 'payment compensation tenant diagnostics');
requireAny(paymentCompensation, [
  '"payment.checkout_compensation_encoding_failed"',
  'code = "payment.checkout_compensation_encoding_failed"',
], 'payment compensation encoding code');
requireAny(orderCompensation, [
  'tenant_id_length = context_facts.tenant_id_length',
  'tenant_id = %context.tenant_id',
], 'order compensation tenant diagnostics');
requireAny(orderPaymentSettlement, [
  'tenant_id_length = context_facts.tenant_id_length',
  'tenant_id = %context.tenant_id',
], 'order payment tenant diagnostics');
requireAny(orderPaymentSettlement, [
  '"order.checkout_payment_validation"',
  'code = "order.checkout_payment_validation"',
], 'order payment validation code');
requireAny(orderRecovery, [
  'tenant_id_length = context_facts.tenant_id_length',
  'tenant_id = %context.tenant_id',
], 'order recovery tenant diagnostics');
requireAny(orderRecovery, [
  '"order.checkout_recovery_validation"',
  'code = "order.checkout_recovery_validation"',
], 'order recovery validation code');

const required = [
  [pricing, [
    'correlation_id = %context.correlation_id',
    'tenant_id_length = context_facts.tenant_id_length',
    'actor_kind = context_facts.actor_kind',
    'claim_count = context_facts.claim_count',
    'role_count = context_facts.role_count',
    'parse_failed = true',
    'error_variant = error_facts.error_variant',
    'text_field_count = error_facts.text_field_count',
    'uuid_field_count = error_facts.uuid_field_count',
    'numeric_field_count = error_facts.numeric_field_count',
    'opaque_payload_present = error_facts.opaque_payload_present',
    'boundary = PRICING_PORT_BOUNDARY',
    'operation,',
    '"pricing.tenant_id_invalid"',
    '"pricing.actor_id_invalid"',
    '"pricing.database_unavailable"',
    '"pricing.validation"',
    '"pricing.rich_error"',
    '"pricing.core_error"',
    '"pricing request context is invalid"',
    '"pricing write actor is invalid"',
    '"variant does not belong to the requested product"',
    '"price was not found"',
    '"price list was not found"',
    '"product was not found"',
    '"variant was not found"',
    '"pricing handle is already in use"',
    '"pricing SKU is already in use"',
    '"inventory is insufficient for the pricing operation"',
    '"shipping profile was not found"',
    '"shipping profile slug is already in use"',
    '"pricing storage is temporarily unavailable"',
    '"pricing operation failed an internal invariant"',
    '"pricing request is invalid"',
    'parse_port_tenant_id(&context, owner_operation)',
    'parse_port_actor_id(&context, owner_operation)',
    'pricing_error_to_port_error(&context, owner_operation, error)',
    'let owner_operation = "resolve_product_price"',
    'let owner_operation = "upsert_variant_price"',
  ], 'pricing'],
  [payment, [
    'struct PaymentCollectionOwnerErrorFacts',
    'fn payment_collection_owner_error_facts(',
    'fn payment_collection_owner_error_code(',
    'fn payment_collection_owner_error_is_technical(',
    'owner_error_variant = error_facts.error_variant',
    'owner_error_text_field_count = error_facts.text_field_count',
    'owner_error_text_total_length = error_facts.text_total_length',
    'owner_error_uuid_field_count = error_facts.uuid_field_count',
    'owner_error_uuid_non_nil_count = error_facts.uuid_non_nil_count',
    'owner_error_opaque_payload_present = error_facts.opaque_payload_present',
    'correlation_id = %context.correlation_id',
    'tenant_id_length',
    'actor_kind',
    'claim_count',
    'role_count',
    'operation = owner_operation',
    'boundary = PAYMENT_COLLECTION_PORT_BOUNDARY',
    '"payment.validation"',
    '"payment.collection_not_found"',
    '"payment.payment_not_found"',
    '"payment.refund_not_found"',
    '"payment.invalid_transition"',
    '"payment.provider_unavailable"',
    '"payment.provider_rejected"',
    '"payment.provider_invalid_response"',
    '"payment.provider_outcome_unknown"',
    '"payment.provider_not_configured"',
    '"payment.database_unavailable"',
    '"payment collection was not found"',
    '"payment was not found"',
    '"refund was not found"',
    '"payment storage is temporarily unavailable"',
    '"payment provider outcome requires reconciliation"',
    '"payment provider response could not be applied safely"',
    '"payment provider rejected the requested operation"',
    '"payment request is invalid"',
    'payment_error_to_port_error(&context, owner_operation, error)',
  ], 'payment'],
  [paymentCompensation, [
    'correlation_id = %context.correlation_id',
    'operation = owner_operation',
    'code = "payment.checkout_compensation_manual_reconciliation"',
    '"payment storage is temporarily unavailable"',
    '"payment provider rejected the requested operation"',
    '"payment provider response could not be applied safely"',
    '"payment request context is invalid"',
    '"payment checkout compensation requires manual reconciliation"',
    'let owner_operation = COMPENSATE_CHECKOUT_PAYMENT_OPERATION;',
    'parse_tenant_id(&context, owner_operation)',
    'require_operation_context(&context, owner_operation',
    'payment_error_to_port_error(&context, owner_operation, error)',
    'payment_error_to_port_error(context, owner_operation, error)',
    'persisted_cancel_result(context, owner_operation',
    'fn manual_reconciliation(\n    context: &PortContext,',
  ], 'payment compensation'],
  [fulfillment, [
    'correlation_id = %context.correlation_id',
    'tenant_id_length = context_facts.tenant_id_length',
    'actor_kind = context_facts.actor_kind',
    'claim_count = context_facts.claim_count',
    'role_count = context_facts.role_count',
    'tenant_id_parse_failed = true',
    'error_variant = error_facts.error_variant',
    'text_field_count = error_facts.text_field_count',
    'uuid_field_count = error_facts.uuid_field_count',
    'opaque_payload_present = error_facts.opaque_payload_present',
    'boundary = SHIPPING_SELECTION_BOUNDARY',
    'operation = owner_operation',
    '"fulfillment.context_invalid"',
    '"fulfillment.validation"',
    '"fulfillment.shipping_option_not_found"',
    '"fulfillment.fulfillment_not_found"',
    '"fulfillment.invalid_transition"',
    '"fulfillment.database_unavailable"',
    '"fulfillment request context is invalid"',
    '"fulfillment request is invalid"',
    '"shipping option was not found"',
    '"fulfillment was not found"',
    '"fulfillment lifecycle transition conflicts with the current state"',
    '"fulfillment storage is temporarily unavailable"',
    'parse_port_tenant_id(&context, "list_seller_shipping_options")',
    'parse_port_tenant_id(&context, "select_shipping_option")',
  ], 'fulfillment'],
  [customer, [
    'tenant_id_length = context_facts.tenant_id_length',
    'actor_kind = context_facts.actor_kind',
    'claim_count = context_facts.claim_count',
    'role_count = context_facts.role_count',
    'tenant_id_parse_failed = true',
    'error_kind = customer_port_error_kind(&error.kind)',
    'error_message_present = !error.message.is_empty()',
    'search_present = request_facts.search_present',
    'search_length = ?request_facts.search_length',
    'boundary = CUSTOMER_READ_PORT_BOUNDARY',
    'operation = owner_operation',
    '"customer.context_invalid"',
    '"customer.page_invalid"',
    '"customer.per_page_invalid"',
    '"customer.database_unavailable"',
    '"customer.validation"',
    '"customer.profile_unavailable"',
    '"customer request context is invalid"',
    '"customer storage is temporarily unavailable"',
    '"customer request is invalid"',
    '"customer profile projection is temporarily unavailable"',
    'customer_error_to_port_error(&context, owner_operation, error)',
    'let owner_operation = "read_customer_projection"',
    'let owner_operation = "read_customer_projection_by_user"',
    'let owner_operation = "list_customer_projections"',
    'let owner_operation = "list_profile_enrichment"',
  ], 'customer'],
  [inventory, [
    'correlation_id = %context.correlation_id',
    'tenant_id_length = context_facts.tenant_id_length',
    'actor_kind = context_facts.actor_kind',
    'claim_count = context_facts.claim_count',
    'role_count = context_facts.role_count',
    'tenant_id_parse_failed = true',
    'error_variant = error_facts.error_variant',
    'text_field_count = error_facts.text_field_count',
    'uuid_field_count = error_facts.uuid_field_count',
    'numeric_field_count = error_facts.numeric_field_count',
    'opaque_payload_present = error_facts.opaque_payload_present',
    'boundary = INVENTORY_PORT_BOUNDARY',
    'operation = owner_operation',
    '"inventory.context_invalid"',
    '"inventory.database_unavailable"',
    '"inventory.variant_not_found"',
    '"inventory.insufficient_inventory"',
    '"inventory.validation"',
    '"inventory.invariant_violation"',
    '"inventory request context is invalid"',
    '"inventory storage is temporarily unavailable"',
    '"inventory variant was not found"',
    '"inventory reservation conflicts with available stock"',
    '"inventory request is invalid"',
    'parse_port_tenant_id(&context, owner_operation)',
    'inventory_error_to_port_error(&context, owner_operation, error)',
    'let owner_operation = "check_availability"',
    'let owner_operation = "reserve_inventory"',
    'let owner_operation = "release_inventory_reservation"',
    'let owner_operation = "reserve_inventory_by_identity"',
    'let owner_operation = "release_inventory_by_identity"',
    'storage_unavailable_with_context(&context, owner_operation, error)',
    'storage_unavailable_with_context(context, owner_operation, error)',
    'async fn load_inventory_item_for_update<C>(\n    context: &PortContext,',
    'async fn load_inventory_item_by_id_for_update<C>(\n    context: &PortContext,',
    'async fn find_reservation_by_external_id<C>(\n    context: &PortContext,',
    'async fn existing_reservation_snapshot<C>(\n    context: &PortContext,',
    'async fn available_quantity<C>(\n    context: &PortContext,',
  ], 'inventory'],
  [order, [
    'correlation_id = %context.correlation_id',
    'tenant_id = %context.tenant_id',
    'operation = owner_operation',
    'code = "order.checkout_identity_validation"',
    'code = "order.checkout_identity_storage_unavailable"',
    'code = "order.database_unavailable"',
    'code = "order.validation"',
    'code = "order.invalid_transition"',
    'code = "order.invariant_violation"',
    '"checkout order identity request is invalid"',
    '"order request is invalid"',
    '"order request context is invalid"',
    'order_checkout_identity_error_to_port_error(',
    'order_error_to_port_error(context, owner_operation, error)',
    'order_error_to_port_error(&context, owner_operation, error)',
    'let owner_operation = "read_checkout_identity_by_operation"',
    'let owner_operation = "read_checkout_identity_by_cart"',
    'let owner_operation = "bind_checkout_identity"',
    'let owner_operation = "adopt_legacy_checkout_identity"',
    'let owner_operation = "complete_checkout"',
    'let owner_operation = "read_checkout_result"',
    'let owner_operation = "read_checkout_result_by_operation"',
    'let owner_operation = "read_order_status"',
  ], 'order'],
  [orderCompensation, [
    'correlation_id = %context.correlation_id',
    'operation,',
    'code = "order.checkout_compensation_manual_reconciliation"',
    '"checkout requires manual reconciliation"',
    '"order request context is invalid"',
    '"read_checkout_order_for_compensation"',
  ], 'order compensation'],
  [orderPaymentSettlement, [
    'correlation_id = %context.correlation_id',
    'operation,',
    'code = "order.checkout_payment_state_conflict"',
    '"checkout requires manual reconciliation"',
    '"order request context is invalid"',
    '"mark_checkout_order_paid"',
  ], 'order payment'],
  [orderRecovery, [
    'correlation_id = %context.correlation_id',
    'operation,',
    'code = "order.checkout_request_encoding_failed"',
    'code = "order.checkout_hash_invalid"',
    '"checkout hash evidence is invalid"',
    '"order request context is invalid"',
    '"confirm_recovered_checkout_order"',
    'hash_json(context, "encode_checkout_snapshot_hash"',
  ], 'order recovery'],
];

for (const [source, values, label] of required) requireAll(source, values, label);

requireText(channel, '"channel storage is temporarily unavailable"', 'channel stable storage message');
requireText(region, '"region storage is temporarily unavailable"', 'region stable storage message');
requireText(cart, '"cart checkout request or projection is invalid"', 'cart stable validation message');
requireText(cart, '"cart checkout snapshot could not be encoded"', 'cart stable encoding message');

if (failures.length > 0) {
  console.error('Scoped ecommerce public port error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Channel, region, cart, pricing, payment collection/compensation, fulfillment, customer, inventory, and order checkout adapters keep raw owner errors out of public PortError messages and retain correlation-safe bounded technical logs',
);
