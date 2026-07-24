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
tracing::error!(
  correlation_id = %context.correlation_id,
  tenant_id = %context.tenant_id,
  operation,
  code = "pricing.database_unavailable",
);
tracing::warn!(code = "pricing.validation");
tracing::error!(code = "pricing.rich_error");
tracing::error!(code = "pricing.core_error");
PortError::unavailable("pricing.database_unavailable", "pricing storage is temporarily unavailable");
PortError::invariant_violation("pricing.core_error", "pricing operation failed an internal invariant");
PortError::validation("pricing.validation", "pricing request is invalid");
.map_err(|error| pricing_error_to_port_error(&context, "resolve_product_price", error));
.map_err(|error| pricing_error_to_port_error(&context, "upsert_variant_price", error));
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

function canonicalFulfillment() {
  return `
tracing::error!(
  correlation_id = %context.correlation_id,
  tenant_id = %context.tenant_id,
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
tracing::error!(
  correlation_id = %context.correlation_id,
  tenant_id = %context.tenant_id,
  operation = owner_operation,
  code = "customer.database_unavailable",
);
tracing::warn!(code = "customer.context_invalid");
tracing::warn!(code = "customer.validation");
tracing::error!(code = "customer.profile_unavailable");
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
tracing::error!(
  correlation_id = %context.correlation_id,
  tenant_id = %context.tenant_id,
  operation = owner_operation,
  code = "inventory.database_unavailable",
);
tracing::warn!(code = "inventory.context_invalid");
tracing::warn!(code = "inventory.variant_not_found");
tracing::warn!(code = "inventory.insufficient_inventory");
tracing::warn!(code = "inventory.validation");
tracing::error!(code = "inventory.invariant_violation");
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

test('public port error verifier rejects provider id in unavailable message', () => {
  expectFailure(
    {
      paymentAppend:
        'format!("payment provider `{provider_id}` is unavailable for `{operation}`");',
    },
    /payment collection public error mapping: forbidden/,
  );
});

test('public port error verifier rejects provider id in rejection message', () => {
  expectFailure(
    {
      paymentAppend:
        'format!("payment provider `{provider_id}` rejected `{operation}`");',
    },
    /payment collection public error mapping: forbidden/,
  );
});

test('public port error verifier rejects provider id in unknown-outcome message', () => {
  expectFailure(
    {
      paymentAppend:
        'format!("payment provider `{provider_id}` outcome is unknown for `{operation}`");',
    },
    /payment collection public error mapping: forbidden/,
  );
});

test('public port error verifier rejects raw payment validation cause', () => {
  expectFailure(
    {
      paymentAppend: 'PortError::validation("payment.validation", message);',
    },
    /payment collection public error mapping: forbidden/,
  );
});

test('public port error verifier rejects raw pricing validation cause', () => {
  expectFailure(
    {
      pricingAppend: 'PortError::validation("pricing.validation", message);',
    },
    /pricing public error mapping: forbidden/,
  );
});

test('public port error verifier rejects raw fulfillment validation cause', () => {
  expectFailure(
    {
      fulfillmentAppend:
        'PortError::validation("fulfillment.validation", message);',
    },
    /fulfillment public error mapping: forbidden/,
  );
});

test('public port error verifier rejects raw fulfillment storage cause', () => {
  expectFailure(
    {
      fulfillmentAppend:
        'format!("fulfillment storage unavailable: {error}");',
    },
    /fulfillment public error mapping: forbidden/,
  );
});

test('public port error verifier rejects raw customer validation cause', () => {
  expectFailure(
    {
      customerAppend: 'PortError::validation("customer.validation", message);',
    },
    /customer public error mapping: forbidden/,
  );
});

test('public port error verifier rejects raw customer storage cause', () => {
  expectFailure(
    {
      customerAppend: 'format!("customer storage unavailable: {error}");',
    },
    /customer public error mapping: forbidden/,
  );
});

test('public port error verifier rejects customer email disclosure', () => {
  expectFailure(
    {
      customerAppend: 'format!("duplicate customer email `{email}`");',
    },
    /customer public error mapping: forbidden/,
  );
});

test('public port error verifier rejects raw inventory validation cause', () => {
  expectFailure(
    {
      inventoryAppend: 'PortError::validation("inventory.validation", message);',
    },
    /inventory public error mapping: forbidden/,
  );
});

test('public port error verifier rejects inventory stock disclosure', () => {
  expectFailure(
    {
      inventoryAppend:
        'format!("insufficient inventory: requested {requested}, available {available}");',
    },
    /inventory public error mapping: forbidden/,
  );
});

test('public port error verifier rejects inventory variant id disclosure', () => {
  expectFailure(
    {
      inventoryAppend: 'format!("variant {id} not found");',
    },
    /inventory public error mapping: forbidden/,
  );
});

test('public port error verifier requires inventory correlation logging', () => {
  expectFailure(
    { removeInventoryCorrelation: true },
    /inventory correlation logging: missing/,
  );
});

test('public port error verifier rejects contextless inventory storage mapper', () => {
  expectFailure(
    { inventoryAppend: '.map_err(storage_unavailable);' },
    /inventory public error mapping: forbidden/,
  );
});

test('public port error verifier rejects contextless inventory storage constructor', () => {
  expectFailure(
    {
      inventoryAppend:
        'fn storage_unavailable(_error: sea_orm::DbErr) -> PortError {}',
    },
    /inventory public error mapping: forbidden/,
  );
});

test('public port error verifier requires identity storage context', () => {
  expectFailure(
    { removeInventoryIdentityStorageContext: true },
    /inventory identity storage mapping: missing/,
  );
});

test('public port error verifier requires helper storage context', () => {
  expectFailure(
    { removeInventoryHelperStorageContext: true },
    /inventory helper storage mapping: missing/,
  );
});

test('public port error verifier rejects legacy order reconciliation message passthrough', () => {
  expectFailure(
    {
      orderCompensationAppend:
        'fn manual_reconciliation(message: impl Into<String>) {}',
    },
    /order checkout adapter public error mapping: forbidden/,
  );
});

test('public port error verifier rejects raw order validation cause', () => {
  expectFailure(
    {
      orderPaymentSettlementAppend:
        'PortError::validation("order.validation", message);',
    },
    /order checkout adapter public error mapping: forbidden/,
  );
});

test('public port error verifier rejects dynamic checkout hash detail', () => {
  expectFailure(
    {
      orderRecoveryAppend:
        'format!(\n                "{field} must be a lowercase hexadecimal value with {min_len} to {max_len} bytes"\n            );',
    },
    /order checkout adapter public error mapping: forbidden/,
  );
});

test('public port error verifier requires pricing correlation logging', () => {
  expectFailure(
    { removePricingCorrelation: true },
    /pricing correlation logging: missing/,
  );
});

test('public port error verifier requires payment owner operation logging', () => {
  expectFailure(
    { removePaymentOperation: true },
    /payment owner operation logging: missing/,
  );
});

test('public port error verifier requires fulfillment correlation logging', () => {
  expectFailure(
    { removeFulfillmentCorrelation: true },
    /fulfillment correlation logging: missing/,
  );
});

test('public port error verifier requires customer correlation logging', () => {
  expectFailure(
    { removeCustomerCorrelation: true },
    /customer correlation logging: missing/,
  );
});

test('public port error verifier requires order compensation correlation logging', () => {
  expectFailure(
    { removeOrderCompensationCorrelation: true },
    /order compensation correlation logging: missing/,
  );
});

test('public port error verifier requires order recovery correlation logging', () => {
  expectFailure(
    { removeOrderRecoveryCorrelation: true },
    /order recovery correlation logging: missing/,
  );
});
