#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL(
    'crates/rustok-commerce/src/services/checkout_fulfillment_stages.rs',
    root,
  ),
  'utf8',
);
const failures = [];

const requireText = (value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  [
    'const CHECKOUT_FULFILLMENT_STAGE_BOUNDARY: &str = "commerce_checkout_fulfillment_stage";',
    'stable checkout fulfillment stage boundary',
  ],
  ['PortErrorKind,', 'typed port error classification import'],
  ['let fulfillment_context = fulfillment_write_context(', 'retained fulfillment write context'],
  ['let fulfillment_context = fulfillment_read_context(', 'retained fulfillment read context'],
  ['let order_context = order_payment_context(', 'retained order payment context'],
  ['fulfillment_context.clone()', 'fulfillment owner context delegation'],
  ['order_context.clone()', 'order owner context delegation'],
  [
    '"rustok_fulfillment",\n                    "ensure_checkout_fulfillments",\n                    "ensure_fulfillments"',
    'fulfillment ensure owner and operation',
  ],
  [
    '"rustok_fulfillment",\n                    "read_checkout_fulfillments",\n                    "read_fulfillments"',
    'fulfillment read owner and operation',
  ],
  [
    '"rustok_order",\n                    "settle_checkout_payment",\n                    "settle_order_payment"',
    'order settlement owner and operation',
  ],
  ['fn fulfillment_stage_boundary_error(', 'context-aware fulfillment stage mapper'],
  [
    'fn log_checkout_fulfillment_stage_boundary_failure(',
    'shared structured diagnostic helper',
  ],
  ['owner = owner', 'truthful dynamic owner field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['operation = owner_operation', 'exact owner operation'],
  ['stage = stage', 'commerce stage context'],
  ['code = %boundary_error.code', 'stable owner code'],
  ['error_kind = ?boundary_error.kind', 'typed owner kind'],
  ['retryable = boundary_error.retryable', 'owner retryability'],
  ['boundary = CHECKOUT_FULFILLMENT_STAGE_BOUNDARY', 'boundary identity'],
  ['"checkout fulfillment stage owner boundary failed"', 'error diagnostic event'],
  [
    '"checkout fulfillment stage owner boundary was rejected"',
    'warning diagnostic event',
  ],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'error severity classification',
  ],
]) {
  requireText(value, label);
}

const retainedFulfillmentContexts =
  source.match(/let fulfillment_context = fulfillment_(write|read)_context\(/g) ?? [];
if (retainedFulfillmentContexts.length !== 2) {
  failures.push(
    `expected two retained fulfillment contexts, found ${retainedFulfillmentContexts.length}`,
  );
}

const retainedOrderContexts = source.match(/let order_context = order_payment_context\(/g) ?? [];
if (retainedOrderContexts.length !== 1) {
  failures.push(`expected one retained order context, found ${retainedOrderContexts.length}`);
}

const delegatedFulfillmentContexts = source.match(/fulfillment_context\.clone\(\)/g) ?? [];
if (delegatedFulfillmentContexts.length !== 2) {
  failures.push(
    `expected two fulfillment context clones, found ${delegatedFulfillmentContexts.length}`,
  );
}

const delegatedOrderContexts = source.match(/order_context\.clone\(\)/g) ?? [];
if (delegatedOrderContexts.length !== 1) {
  failures.push(`expected one order context clone, found ${delegatedOrderContexts.length}`);
}

const mapperCalls = source.match(/fulfillment_stage_boundary_error\(/g) ?? [];
if (mapperCalls.length !== 4) {
  failures.push(`expected three mapper calls plus definition, found ${mapperCalls.length}`);
}

for (const [value, label] of [
  ['CheckoutFulfillmentStageError::Boundary {', 'existing boundary variant'],
  ['stage,\n        code: error.code,', 'stage and code preservation'],
  ['message: error.message,', 'message preservation'],
  ['retryable: error.retryable,', 'retryability preservation'],
  [
    'expected_stage: CheckoutOperationStage::PaymentCaptured',
    'payment captured checkpoint admission',
  ],
  [
    'next_stage: CheckoutOperationStage::FulfillmentCreated',
    'fulfillment created checkpoint',
  ],
  [
    'state.payment_collection.status_kind() != PaymentCollectionStatusKind::Captured',
    'typed captured payment admission',
  ],
  ['.with_causation_id(operation_id.to_string())', 'causation construction'],
  [
    '.with_idempotency_key(format!("checkout:{operation_id}:fulfillment-set"))',
    'fulfillment idempotency construction',
  ],
  [
    '.with_idempotency_key(format!("checkout:{operation_id}:order:payment-settlement"))',
    'order settlement idempotency construction',
  ],
  ['.with_deadline(deadline)', 'deadline construction'],
]) {
  requireText(value, label);
}

forbidText(
  "fn boundary_error(stage: &'static str, error: PortError)",
  'context-dropping fulfillment stage mapper',
);
forbidText(
  '.ensure_checkout_fulfillments(\n                fulfillment_write_context(',
  'inline fulfillment write context',
);
forbidText(
  '.read_checkout_fulfillments(\n                fulfillment_read_context(',
  'inline fulfillment read context',
);
forbidText(
  '.settle_checkout_payment(\n                order_payment_context(',
  'inline order payment context',
);

if (failures.length > 0) {
  console.error('Checkout fulfillment stage context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout fulfillment ensure/read and order payment settlement failures retain the complete owner PortContext',
);
