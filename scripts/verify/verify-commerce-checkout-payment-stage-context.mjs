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
    'crates/rustok-commerce/src/services/checkout_payment_stages.rs',
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
    'const CHECKOUT_PAYMENT_STAGE_BOUNDARY: &str = "commerce_checkout_payment_stage";',
    'stable checkout payment stage boundary',
  ],
  ['PortErrorKind,', 'typed port error classification import'],
  ['let prepare_context = payment_write_context(', 'retained prepare context'],
  ['let authorize_context = payment_write_context(', 'retained authorize context'],
  ['let capture_context = payment_write_context(', 'retained capture context'],
  ['let read_context = payment_read_context(', 'retained read context'],
  ['prepare_context.clone()', 'prepare owner delegation'],
  ['authorize_context.clone()', 'authorize owner delegation'],
  ['capture_context.clone()', 'capture owner delegation'],
  ['read_context.clone()', 'read owner delegation'],
  ['"prepare_checkout_collection",\n                                "prepare"', 'prepare owner operation'],
  ['"authorize_checkout_collection",\n                                "authorize"', 'authorize owner operation'],
  ['"capture_checkout_collection",\n                                "capture"', 'capture owner operation'],
  ['"read_checkout_collection",\n                    "read"', 'read owner operation'],
  ['fn payment_boundary_error(', 'context-aware payment mapper'],
  ['fn log_checkout_payment_boundary_failure(', 'structured diagnostic helper'],
  ['owner = "rustok_payment"', 'truthful payment owner'],
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
  ['stage,', 'commerce stage context'],
  ['code = %boundary_error.code', 'stable owner code'],
  ['error_kind = ?boundary_error.kind', 'typed owner kind'],
  ['retryable = boundary_error.retryable', 'owner retryability'],
  ['boundary = CHECKOUT_PAYMENT_STAGE_BOUNDARY', 'boundary identity'],
  ['"checkout payment owner boundary failed"', 'error diagnostic event'],
  ['"checkout payment owner boundary was rejected"', 'warning diagnostic event'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'error severity classification',
  ],
]) {
  requireText(value, label);
}

const retainedContexts =
  source.match(/let (prepare|authorize|capture|read)_context = payment_(write|read)_context\(/g) ?? [];
if (retainedContexts.length !== 4) {
  failures.push(`expected four retained payment contexts, found ${retainedContexts.length}`);
}

const delegatedContexts =
  source.match(/(prepare|authorize|capture|read)_context\.clone\(\)/g) ?? [];
if (delegatedContexts.length !== 4) {
  failures.push(`expected four cloned payment contexts, found ${delegatedContexts.length}`);
}

const mapperCalls = source.match(/payment_boundary_error\(/g) ?? [];
if (mapperCalls.length !== 5) {
  failures.push(`expected four mapper calls plus definition, found ${mapperCalls.length}`);
}

for (const [value, label] of [
  ['CheckoutPaymentStageError::Boundary {', 'existing boundary variant'],
  ['stage,\n        code: error.code,', 'stage and code preservation'],
  ['message: error.message,', 'message preservation'],
  ['retryable: error.retryable,', 'retryability preservation'],
  ['expected_stage: CheckoutOperationStage::PaymentReady', 'prepare checkpoint'],
  ['next_stage: CheckoutOperationStage::PaymentAuthorized', 'authorize checkpoint'],
  ['expected_stage: CheckoutOperationStage::PaymentAuthorized', 'capture checkpoint'],
  ['next_stage: CheckoutOperationStage::PaymentCaptured', 'captured checkpoint'],
  ['.with_causation_id(operation_id.to_string())', 'causation construction'],
  ['.with_idempotency_key(idempotency_key)', 'write idempotency construction'],
  ['.with_deadline(deadline)', 'deadline construction'],
]) {
  requireText(value, label);
}

forbidText(
  'fn boundary_error(stage: &\'static str, error: PortError)',
  'context-dropping payment boundary mapper',
);
forbidText(
  '.prepare_checkout_collection(\n                            payment_write_context(',
  'inline prepare context',
);
forbidText(
  '.authorize_checkout_collection(\n                            payment_write_context(',
  'inline authorize context',
);
forbidText(
  '.capture_checkout_collection(\n                            payment_write_context(',
  'inline capture context',
);
forbidText(
  '.read_checkout_collection(\n                payment_read_context(',
  'inline read context',
);

if (failures.length > 0) {
  console.error('Checkout payment stage context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout payment prepare/authorize/capture/read failures retain the complete owner PortContext',
);
