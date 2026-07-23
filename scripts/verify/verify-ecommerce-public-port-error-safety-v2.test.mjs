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
    pricing = pricing.replace('correlation_id = %context.correlation_id', 'correlation_id = omitted');
  }
  put(root, 'crates/rustok-pricing/src/ports.rs', pricing);

  let payment = `${canonicalPayment()}${options.paymentAppend ?? ''}`;
  if (options.removePaymentOperation) {
    payment = payment.replace('operation = owner_operation', 'operation = omitted');
  }
  put(root, 'crates/rustok-payment/src/ports.rs', payment);
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
