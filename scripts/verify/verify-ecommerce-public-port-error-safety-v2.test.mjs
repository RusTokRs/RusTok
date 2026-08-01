#!/usr/bin/env node

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const scriptPath = path.resolve(
  'scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs',
);

function put(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function canonicalPricing() {
  return `
struct PricingPortContextFacts {}
struct PricingOwnerErrorFacts {}
fn pricing_owner_error_facts() {}
tracing::error!(
  correlation_id = %context.correlation_id,
  tenant_id_length = context_facts.tenant_id_length,
  actor_kind = context_facts.actor_kind,
  claim_count = context_facts.claim_count,
  role_count = context_facts.role_count,
  parse_failed = true,
  error_variant = error_facts.error_variant,
  text_field_count = error_facts.text_field_count,
  uuid_field_count = error_facts.uuid_field_count,
  numeric_field_count = error_facts.numeric_field_count,
  opaque_payload_present = error_facts.opaque_payload_present,
  boundary = PRICING_PORT_BOUNDARY,
  operation,
);
"pricing.tenant_id_invalid";
"pricing.actor_id_invalid";
"pricing.database_unavailable";
"pricing.validation";
"pricing.rich_error";
"pricing.core_error";
"pricing request context is invalid";
"pricing write actor is invalid";
"variant does not belong to the requested product";
"price was not found";
"price list was not found";
"product was not found";
"variant was not found";
"pricing handle is already in use";
"pricing SKU is already in use";
"inventory is insufficient for the pricing operation";
"shipping profile was not found";
"shipping profile slug is already in use";
"pricing storage is temporarily unavailable";
"pricing operation failed an internal invariant";
"pricing request is invalid";
parse_port_tenant_id(&context, owner_operation);
parse_port_actor_id(&context, owner_operation);
pricing_error_to_port_error(&context, owner_operation, error);
let owner_operation = "resolve_product_price";
let owner_operation = "upsert_variant_price";
`;
}

function canonicalPayment() {
  return `
tracing::error!(
  correlation_id = %context.correlation_id,
  tenant_id = %context.tenant_id,
  operation = owner_operation,
  code = "payment.database_unavailable",
);
tracing::warn!(code = "payment.validation");
tracing::warn!(code = "payment.invalid_transition");
tracing::error!(code = "payment.provider_unavailable");
tracing::warn!(code = "payment.provider_rejected");
tracing::error!(code = "payment.provider_invalid_response");
tracing::error!(code = "payment.provider_outcome_unknown");
tracing::error!(code = "payment.provider_not_configured");
PortError::unavailable("payment.database_unavailable", "payment storage is temporarily unavailable");
PortError::conflict("payment.provider_outcome_unknown", "payment provider outcome requires reconciliation");
PortError::invariant_violation("payment.provider_invalid_response", "payment provider response could not be applied safely");
PortError::conflict("payment.provider_rejected", "payment provider rejected the requested operation");
PortError::validation("payment.validation", "payment request is invalid");
.map_err(|error| payment_error_to_port_error(&context, "read_collection_status", error));
`;
}

function canonicalPaymentCompensation() {
  return `
tracing::error!(
  correlation_id = %context.correlation_id,
  tenant_id = %context.tenant_id,
  operation = owner_operation,
  code = "payment.checkout_compensation_manual_reconciliation",
);
tracing::error!(code = "payment.checkout_compensation_encoding_failed");
"payment storage is temporarily unavailable";
"payment provider rejected the requested operation";
"payment provider response could not be applied safely";
"payment request context is invalid";
"payment checkout compensation requires manual reconciliation";
let owner_operation = COMPENSATE_CHECKOUT_PAYMENT_OPERATION;
parse_tenant_id(&context, owner_operation);
require_operation_context(&context, owner_operation, operation_context);
payment_error_to_port_error(&context, owner_operation, error);
payment_error_to_port_error(context, owner_operation, error);
persisted_cancel_result(context, owner_operation, payment);
fn manual_reconciliation(
    context: &PortContext,
) {}
`;
}

function canonicalFulfillment() {
  return `
struct FulfillmentPortContextFacts {}
struct FulfillmentOwnerErrorFacts {}
fn fulfillment_owner_error_facts() {}
tracing::error!(
  correlation_id = %context.correlation_id,
  tenant_id_length = context_facts.tenant_id_length,
  actor_kind = context_facts.actor_kind,
  claim_count = context_facts.claim_count,
  role_count = context_facts.role_count,
  tenant_id_parse_failed = true,
  error_variant = error_facts.error_variant,
  text_field_count = error_facts.text_field_count,
  uuid_field_count = error_facts.uuid_field_count,
  opaque_payload_present = error_facts.opaque_payload_present,
  boundary = SHIPPING_SELECTION_BOUNDARY,
  operation = owner_operation,
  code = "fulfillment.database_unavailable",
);
tracing::warn!(code = "fulfillment.context_invalid");
tracing::warn!(code = "fulfillment.validation");
tracing::warn!(code = "fulfillment.shipping_option_not_found");
tracing::warn!(code = "fulfillment.fulfillment_not_found");
tracing::warn!(code = "fulfillment.invalid_transition");
PortError::validation("fulfillment.context_invalid", "fulfillment request context is invalid");
PortError::validation("fulfillment.validation", "fulfillment request is invalid");
PortError::new(NotFound, "fulfillment.shipping_option_not_found", "shipping option was not found", false);
PortError::new(NotFound, "fulfillment.fulfillment_not_found", "fulfillment was not found", false);
PortError::conflict("fulfillment.invalid_transition", "fulfillment lifecycle transition conflicts with the current state");
PortError::unavailable("fulfillment.database_unavailable", "fulfillment storage is temporarily unavailable");
parse_port_tenant_id(&context, "list_seller_shipping_options");
parse_port_tenant_id(&context, "select_shipping_option");
`;
}

function canonicalCustomer() {
  return `
struct CustomerReadContextFacts {}
struct CustomerOwnerErrorFacts {}
struct CustomerListRequestFacts {}
fn customer_port_error_kind() {}
tracing::error!(
  correlation_id = %context.correlation_id,
  tenant_id_length = context_facts.tenant_id_length,
  actor_kind = context_facts.actor_kind,
  claim_count = context_facts.claim_count,
  role_count = context_facts.role_count,
  tenant_id_parse_failed = true,
  error_kind = customer_port_error_kind(&error.kind),
  error_message_present = !error.message.is_empty(),
  search_present = request_facts.search_present,
  search_length = ?request_facts.search_length,
  boundary = CUSTOMER_READ_PORT_BOUNDARY,
  operation = owner_operation,
);
"customer.context_invalid";
"customer.page_invalid";
"customer.per_page_invalid";
"customer.database_unavailable";
"customer.validation";
"customer.profile_unavailable";
PortError::validation("customer.context_invalid", "customer request context is invalid");
PortError::unavailable("customer.database_unavailable", "customer storage is temporarily unavailable");
PortError::validation("customer.validation", "customer request is invalid");
PortError::unavailable("customer.profile_unavailable", "customer profile projection is temporarily unavailable");
.map_err(|error| customer_error_to_port_error(&context, owner_operation, error));
let owner_operation = "read_customer_projection";
let owner_operation = "read_customer_projection_by_user";
let owner_operation = "list_customer_projections";
let owner_operation = "list_profile_enrichment";
`;
}

function canonicalInventory() {
  return `
struct InventoryPortContextFacts {}
struct InventoryOwnerErrorFacts {}
fn inventory_owner_error_facts() {}
tracing::error!(
  correlation_id = %context.correlation_id,
  tenant_id_length = context_facts.tenant_id_length,
  actor_kind = context_facts.actor_kind,
  claim_count = context_facts.claim_count,
  role_count = context_facts.role_count,
  tenant_id_parse_failed = true,
  error_variant = error_facts.error_variant,
  text_field_count = error_facts.text_field_count,
  uuid_field_count = error_facts.uuid_field_count,
  numeric_field_count = error_facts.numeric_field_count,
  opaque_payload_present = error_facts.opaque_payload_present,
  boundary = INVENTORY_PORT_BOUNDARY,
  operation = owner_operation,
);
"inventory.context_invalid";
"inventory.database_unavailable";
"inventory.variant_not_found";
"inventory.insufficient_inventory";
"inventory.validation";
"inventory.invariant_violation";
PortError::validation("inventory.context_invalid", "inventory request context is invalid");
PortError::unavailable("inventory.database_unavailable", "inventory storage is temporarily unavailable");
PortError::new(NotFound, "inventory.variant_not_found", "inventory variant was not found", false);
PortError::new(Conflict, "inventory.insufficient_inventory", "inventory reservation conflicts with available stock", false);
PortError::validation("inventory.validation", "inventory request is invalid");
parse_port_tenant_id(&context, owner_operation);
inventory_error_to_port_error(&context, owner_operation, error);
let owner_operation = "check_availability";
let owner_operation = "reserve_inventory";
let owner_operation = "release_inventory_reservation";
let owner_operation = "reserve_inventory_by_identity";
let owner_operation = "release_inventory_by_identity";
.map_err(|error| storage_unavailable_with_context(&context, owner_operation, error));
.map_err(|error| storage_unavailable_with_context(context, owner_operation, error));
async fn load_inventory_item_for_update<C>(
    context: &PortContext,
async fn load_inventory_item_by_id_for_update<C>(
    context: &PortContext,
async fn find_reservation_by_external_id<C>(
    context: &PortContext,
async fn existing_reservation_snapshot<C>(
    context: &PortContext,
async fn available_quantity<C>(
    context: &PortContext,
`;
}

function canonicalOrder() {
  return `
tracing::error!(
  correlation_id = %context.correlation_id,
  tenant_id = %context.tenant_id,
  operation = owner_operation,
  code = "order.checkout_identity_storage_unavailable",
);
tracing::warn!(code = "order.checkout_identity_validation");
tracing::error!(code = "order.database_unavailable");
tracing::warn!(code = "order.validation");
tracing::warn!(code = "order.invalid_transition");
tracing::error!(code = "order.invariant_violation");
"checkout order identity request is invalid";
"order request is invalid";
"order request context is invalid";
order_checkout_identity_error_to_port_error(
                    &context,
                    owner_operation,
                    error,
                );
order_error_to_port_error(context, owner_operation, error);
order_error_to_port_error(&context, owner_operation, error);
let owner_operation = "read_checkout_identity_by_operation";
let owner_operation = "read_checkout_identity_by_cart";
let owner_operation = "bind_checkout_identity";
let owner_operation = "adopt_legacy_checkout_identity";
let owner_operation = "complete_checkout";
let owner_operation = "read_checkout_result";
let owner_operation = "read_checkout_result_by_operation";
let owner_operation = "read_order_status";
`;
}

function canonicalOrderCompensation() {
  return `
tracing::error!(
  correlation_id = %context.correlation_id,
  tenant_id = %context.tenant_id,
  operation,
  code = "order.checkout_compensation_manual_reconciliation",
);
"checkout requires manual reconciliation";
"order request context is invalid";
"read_checkout_order_for_compensation";
`;
}

function canonicalOrderPaymentSettlement() {
  return `
tracing::error!(
  correlation_id = %context.correlation_id,
  tenant_id = %context.tenant_id,
  operation,
  code = "order.checkout_payment_validation",
);
tracing::warn!(code = "order.checkout_payment_state_conflict");
"checkout requires manual reconciliation";
"order request context is invalid";
"mark_checkout_order_paid";
`;
}

function canonicalOrderRecovery() {
  return `
tracing::error!(
  correlation_id = %context.correlation_id,
  tenant_id = %context.tenant_id,
  operation,
  code = "order.checkout_request_encoding_failed",
);
tracing::warn!(code = "order.checkout_recovery_validation");
tracing::warn!(code = "order.checkout_hash_invalid");
"checkout hash evidence is invalid";
"order request context is invalid";
"confirm_recovered_checkout_order";
hash_json(context, "encode_checkout_snapshot_hash", snapshot);
`;
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-public-port-errors-'));
  put(
    root,
    'crates/rustok-channel/src/ports.rs',
    `tracing::error!();\n"channel storage is temporarily unavailable";\n${options.channelAppend ?? ''}`,
  );
  put(
    root,
    'crates/rustok-region/src/ports.rs',
    `tracing::error!();\n"region storage is temporarily unavailable";\n${options.regionAppend ?? ''}`,
  );
  put(
    root,
    'crates/rustok-cart/src/checkout_snapshot.rs',
    `tracing::error!();\n"cart checkout request or projection is invalid";\n"cart checkout snapshot could not be encoded";\n${options.cartAppend ?? ''}`,
  );

  let pricing = `${canonicalPricing()}${options.pricingAppend ?? ''}`;
  if (options.removePricingCorrelation) {
    pricing = pricing.replace(
      'correlation_id = %context.correlation_id',
      'correlation_id = omitted',
    );
  }
  put(root, 'crates/rustok-pricing/src/ports.rs', pricing);

  let payment = `${canonicalPayment()}${options.paymentAppend ?? ''}`;
  if (options.removePaymentOperation) {
    payment = payment.replace('operation = owner_operation', 'operation = omitted');
  }
  put(root, 'crates/rustok-payment/src/ports.rs', payment);

  const paymentCompensation = `${canonicalPaymentCompensation()}${options.paymentCompensationAppend ?? ''}`;
  put(
    root,
    'crates/rustok-payment/src/checkout_compensation.rs',
    paymentCompensation,
  );

  let fulfillment = `${canonicalFulfillment()}${options.fulfillmentAppend ?? ''}`;
  if (options.removeFulfillmentCorrelation) {
    fulfillment = fulfillment.replace(
      'correlation_id = %context.correlation_id',
      'correlation_id = omitted',
    );
  }
  put(root, 'crates/rustok-fulfillment/src/ports.rs', fulfillment);

  let customer = `${canonicalCustomer()}${options.customerAppend ?? ''}`;
  if (options.removeCustomerCorrelation) {
    customer = customer.replace(
      'correlation_id = %context.correlation_id',
      'correlation_id = omitted',
    );
  }
  put(root, 'crates/rustok-customer/src/ports.rs', customer);

  let inventory = `${canonicalInventory()}${options.inventoryAppend ?? ''}`;
  if (options.removeInventoryCorrelation) {
    inventory = inventory.replace(
      'correlation_id = %context.correlation_id',
      'correlation_id = omitted',
    );
  }
  if (options.removeInventoryIdentityStorageContext) {
    inventory = inventory.replace(
      'storage_unavailable_with_context(&context, owner_operation, error)',
      'storage_context_omitted(error)',
    );
  }
  if (options.removeInventoryHelperStorageContext) {
    inventory = inventory.replace(
      'storage_unavailable_with_context(context, owner_operation, error)',
      'storage_context_omitted(error)',
    );
  }
  put(root, 'crates/rustok-inventory/src/ports.rs', inventory);

  let order = `${canonicalOrder()}${options.orderAppend ?? ''}`;
  if (options.removeOrderCorrelation) {
    order = order.replace(
      'correlation_id = %context.correlation_id',
      'correlation_id = omitted',
    );
  }
  put(root, 'crates/rustok-order/src/ports.rs', order);

  let orderCompensation = `${canonicalOrderCompensation()}${options.orderCompensationAppend ?? ''}`;
  if (options.removeOrderCompensationCorrelation) {
    orderCompensation = orderCompensation.replace(
      'correlation_id = %context.correlation_id',
      'correlation_id = omitted',
    );
  }
  put(
    root,
    'crates/rustok-order/src/checkout_compensation.rs',
    orderCompensation,
  );

  const orderPaymentSettlement = `${canonicalOrderPaymentSettlement()}${options.orderPaymentSettlementAppend ?? ''}`;
  put(
    root,
    'crates/rustok-order/src/checkout_payment_settlement.rs',
    orderPaymentSettlement,
  );

  let orderRecovery = `${canonicalOrderRecovery()}${options.orderRecoveryAppend ?? ''}`;
  if (options.removeOrderRecoveryCorrelation) {
    orderRecovery = orderRecovery.replace(
      'correlation_id = %context.correlation_id',
      'correlation_id = omitted',
    );
  }
  put(root, 'crates/rustok-order/src/checkout_order_recovery.rs', orderRecovery);
  return root;
}

function run(root) {
  return spawnSync('node', [scriptPath], {
    cwd: path.resolve('.'),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: 'utf8',
  });
}

function expectFailure(options, pattern) {
  const root = fixture(options);
  try {
    const result = run(root);
    assert.notEqual(result.status, 0, result.stdout);
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test('public port error verifier passes canonical fixture', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /keep raw owner errors out of public PortError messages/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

const failureCases = [
  ["provider id in unavailable message", { paymentAppend: 'format!("payment provider `{provider_id}` is unavailable for `{operation}`");' }, /payment collection public error mapping: forbidden/],
  ["provider id in rejection message", { paymentAppend: 'format!("payment provider `{provider_id}` rejected `{operation}`");' }, /payment collection public error mapping: forbidden/],
  ["provider id in unknown-outcome message", { paymentAppend: 'format!("payment provider `{provider_id}` outcome is unknown for `{operation}`");' }, /payment collection public error mapping: forbidden/],
  ["raw payment validation cause", { paymentAppend: 'PortError::validation("payment.validation", message);' }, /payment collection public error mapping: forbidden/],
  ["raw pricing validation cause", { pricingAppend: 'PortError::validation("pricing.validation", message);' }, /pricing public error mapping: forbidden/],
  ["dynamic pricing product message", { pricingAppend: 'format!("product {id} not found");' }, /pricing public error mapping: forbidden/],
  ["dynamic pricing mismatch message", { pricingAppend: 'format!("variant {variant_id} does not belong to product {product_id}");' }, /pricing public error mapping: forbidden/],
  ["complete pricing error diagnostics", { pricingAppend: 'tracing::error!(error = ?error);' }, /pricing payload diagnostics: forbidden/],
  ["raw pricing validation diagnostics", { pricingAppend: 'cause = %message;' }, /pricing payload diagnostics: forbidden/],
  ["raw pricing tenant diagnostics", { pricingAppend: 'tenant_id = %context.tenant_id;' }, /pricing payload diagnostics: forbidden/],
  ["pricing resource diagnostic identity", { pricingAppend: 'variant_id = %variant_id;' }, /pricing payload diagnostics: forbidden/],
  ["exact pricing stock diagnostics", { pricingAppend: 'code = "pricing.insufficient_inventory",\n                requested,\n                available,' }, /pricing payload diagnostics: forbidden/],
  ["raw fulfillment validation cause", { fulfillmentAppend: 'PortError::validation("fulfillment.validation", message);' }, /fulfillment public error mapping: forbidden/],
  ["raw fulfillment storage cause", { fulfillmentAppend: 'format!("fulfillment storage unavailable: {error}");' }, /fulfillment public error mapping: forbidden/],
  ["complete fulfillment error diagnostics", { fulfillmentAppend: 'tracing::error!(error = ?error);' }, /fulfillment payload diagnostics: forbidden/],
  ["raw fulfillment tenant diagnostics", { fulfillmentAppend: 'tenant_id = %context.tenant_id;' }, /fulfillment payload diagnostics: forbidden/],
  ["fulfillment resource identity diagnostics", { fulfillmentAppend: 'resource_id = %id;' }, /fulfillment payload diagnostics: forbidden/],
  ["fulfillment transition text diagnostics", { fulfillmentAppend: 'from = %from;' }, /fulfillment payload diagnostics: forbidden/],
  ["raw customer validation cause", { customerAppend: 'PortError::validation("customer.validation", message);' }, /customer public error mapping: forbidden/],
  ["raw customer storage cause", { customerAppend: 'format!("customer storage unavailable: {error}");' }, /customer public error mapping: forbidden/],
  ["customer email disclosure", { customerAppend: 'format!("duplicate customer email `{email}`");' }, /customer public error mapping: forbidden/],
  ["complete customer error diagnostics", { customerAppend: 'tracing::error!(error = ?error);' }, /customer payload diagnostics: forbidden/],
  ["raw customer tenant diagnostics", { customerAppend: 'tenant_id = %context.tenant_id;' }, /customer payload diagnostics: forbidden/],
  ["raw inventory validation cause", { inventoryAppend: 'PortError::validation("inventory.validation", message);' }, /inventory public error mapping: forbidden/],
  ["inventory stock disclosure", { inventoryAppend: 'format!("insufficient inventory: requested {requested}, available {available}");' }, /inventory public error mapping: forbidden/],
  ["inventory variant id disclosure", { inventoryAppend: 'format!("variant {id} not found");' }, /inventory public error mapping: forbidden/],
  ["complete inventory error diagnostics", { inventoryAppend: 'tracing::error!(error = ?error);' }, /inventory payload diagnostics: forbidden/],
  ["fallback inventory error diagnostics", { inventoryAppend: 'tracing::error!(error = ?other);' }, /inventory payload diagnostics: forbidden/],
  ["raw inventory validation diagnostics", { inventoryAppend: 'internal_message = %message;' }, /inventory payload diagnostics: forbidden/],
  ["raw inventory tenant diagnostics", { inventoryAppend: 'tenant_id = %context.tenant_id;' }, /inventory payload diagnostics: forbidden/],
  ["inventory variant diagnostic identity", { inventoryAppend: 'variant_id = %variant_id;' }, /inventory payload diagnostics: forbidden/],
  ["exact inventory stock diagnostics", { inventoryAppend: 'code = "inventory.insufficient_inventory",\n                requested,\n                available,' }, /inventory payload diagnostics: forbidden/],
  ["inventory correlation logging", { removeInventoryCorrelation: true }, /inventory correlation logging: missing/],
  ["contextless inventory storage mapper", { inventoryAppend: '.map_err(storage_unavailable);' }, /inventory public error mapping: forbidden/],
  ["contextless inventory storage constructor", { inventoryAppend: 'fn storage_unavailable(_error: sea_orm::DbErr) -> PortError {}' }, /inventory public error mapping: forbidden/],
  ["identity storage context", { removeInventoryIdentityStorageContext: true }, /inventory identity storage mapping: missing/],
  ["helper storage context", { removeInventoryHelperStorageContext: true }, /inventory helper storage mapping: missing/],
  ["raw generic order validation cause", { orderAppend: 'PortError::validation("order.validation", message);' }, /order generic port public error mapping: forbidden/],
  ["raw checkout identity validation cause", { orderAppend: 'PortError::validation("order.checkout_identity_validation", message);' }, /order generic port public error mapping: forbidden/],
  ["contextless generic order mapper", { orderAppend: '.map_err(order_error_to_port_error);' }, /order generic port public error mapping: forbidden/],
  ["generic order context disclosure", { orderAppend: '"PortContext.tenant_id must be a UUID for order ports";' }, /order generic port public error mapping: forbidden/],
  ["generic order correlation logging", { removeOrderCorrelation: true }, /order generic correlation logging: missing/],
  ["legacy order reconciliation message passthrough", { orderCompensationAppend: 'fn manual_reconciliation(message: impl Into<String>) {}' }, /order checkout adapter public error mapping: forbidden/],
  ["raw order validation cause", { orderPaymentSettlementAppend: 'PortError::validation("order.validation", message);' }, /order checkout adapter public error mapping: forbidden/],
  ["dynamic checkout hash detail", { orderRecoveryAppend: 'format!(\n                "{field} must be a lowercase hexadecimal value with {min_len} to {max_len} bytes"\n            );' }, /order checkout adapter public error mapping: forbidden/],
  ["pricing correlation logging", { removePricingCorrelation: true }, /pricing correlation logging: missing/],
  ["payment owner operation logging", { removePaymentOperation: true }, /payment owner operation logging: missing/],
  ["fulfillment correlation logging", { removeFulfillmentCorrelation: true }, /fulfillment correlation logging: missing/],
  ["customer correlation logging", { removeCustomerCorrelation: true }, /customer correlation logging: missing/],
  ["order compensation correlation logging", { removeOrderCompensationCorrelation: true }, /order compensation correlation logging: missing/],
  ["order recovery correlation logging", { removeOrderRecoveryCorrelation: true }, /order recovery correlation logging: missing/],
];

for (const [name, options, pattern] of failureCases) {
  test(`public port error verifier rejects ${name}`, () => {
    expectFailure(options, pattern);
  });
}
