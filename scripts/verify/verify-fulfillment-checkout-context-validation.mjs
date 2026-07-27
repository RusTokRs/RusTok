#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-fulfillment/src/checkout_execution.rs', root),
  'utf8',
);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};

const operationContext = between(
  source,
  'fn require_operation_context(',
  'fn parse_tenant_id(',
  'checkout causation context validation',
);
const tenantContext = between(
  source,
  'fn parse_tenant_id(',
  'fn fulfillment_error_to_port_error(',
  'checkout tenant context validation',
);
const portImpl = between(
  source,
  '#[async_trait]\nimpl CheckoutFulfillmentExecutionPort for InProcessCheckoutFulfillmentExecutionPort',
  'fn validate_request(',
  'checkout fulfillment port implementation',
);

for (const [value, label] of [
  ['const CHECKOUT_FULFILLMENT_OWNER: &str = "rustok_fulfillment";', 'truthful owner constant'],
  [
    'const CHECKOUT_FULFILLMENT_BOUNDARY: &str = "checkout_fulfillment_execution_port";',
    'fulfillment execution boundary',
  ],
  ['const ENSURE_OPERATION: &str = "ensure_checkout_fulfillments";', 'ensure operation'],
  ['const READ_OPERATION: &str = "read_checkout_fulfillments";', 'read operation'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['fn require_operation_context(', 'causation validator'],
  ['context: &PortContext', 'retained causation context'],
  ["owner_operation: &'static str", 'causation owner operation'],
  ['checkout_operation_id: Uuid', 'expected checkout operation identity'],
  ['let context_operation = context', 'causation parsing'],
  ['.and_then(|value| Uuid::parse_str(value).ok());', 'optional UUID causation parsing'],
  ['if context_operation != Some(checkout_operation_id) {', 'causation mismatch condition'],
  ['let error = PortError::validation(', 'causation stable error construction'],
  ['"fulfillment.checkout_operation_id_invalid"', 'causation stable code'],
  [
    '"checkout fulfillment causation_id must match the checkout operation"',
    'causation stable message',
  ],
  ['tracing::warn!(', 'causation warning severity'],
  ['error = ?error', 'causation mapped error evidence'],
  ['owner = CHECKOUT_FULFILLMENT_OWNER', 'causation truthful owner'],
  ['operation = owner_operation', 'causation exact operation'],
  ['validation_phase = "causation_id"', 'causation validation phase'],
  ['correlation_id = %context.correlation_id', 'causation correlation context'],
  ['tenant_id = %context.tenant_id', 'causation tenant context'],
  ['actor = ?context.actor', 'causation actor context'],
  ['channel = ?context.channel', 'causation channel context'],
  ['locale = %context.locale', 'causation locale context'],
  ['causation_id = ?context.causation_id', 'causation raw context'],
  ['traceparent = ?context.traceparent', 'causation trace context'],
  ['idempotency_key = ?context.idempotency_key', 'causation idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'causation deadline context'],
  ['expected_checkout_operation_id = %checkout_operation_id', 'causation expected identity'],
  ['internal_code = %error.code', 'causation mapped code'],
  ['internal_message = %error.message', 'causation mapped message'],
  ['error_kind = ?error.kind', 'causation mapped kind'],
  ['retryable = error.retryable', 'causation retryability'],
  ['boundary = CHECKOUT_FULFILLMENT_BOUNDARY', 'causation boundary'],
  ['return Err(error);', 'same causation error returned'],
]) requireText(operationContext, value, label);

for (const [value, label] of [
  ['fn parse_tenant_id(', 'tenant parser'],
  ['context: &PortContext', 'retained tenant context'],
  ["owner_operation: &'static str", 'tenant owner operation'],
  ['Uuid::parse_str(&context.tenant_id).map_err(|cause| {', 'tenant parse cause capture'],
  ['let error = PortError::validation(', 'tenant stable error construction'],
  ['"fulfillment.tenant_id_invalid"', 'tenant stable code'],
  ['"PortContext.tenant_id must be a UUID for fulfillment ports"', 'tenant stable message'],
  ['tracing::warn!(', 'tenant warning severity'],
  ['cause = ?cause', 'tenant original parse cause'],
  ['error = ?error', 'tenant mapped error evidence'],
  ['owner = CHECKOUT_FULFILLMENT_OWNER', 'tenant truthful owner'],
  ['operation = owner_operation', 'tenant exact operation'],
  ['validation_phase = "tenant_id"', 'tenant validation phase'],
  ['correlation_id = %context.correlation_id', 'tenant correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant identity context'],
  ['actor = ?context.actor', 'tenant actor context'],
  ['channel = ?context.channel', 'tenant channel context'],
  ['locale = %context.locale', 'tenant locale context'],
  ['causation_id = ?context.causation_id', 'tenant causation context'],
  ['traceparent = ?context.traceparent', 'tenant trace context'],
  ['idempotency_key = ?context.idempotency_key', 'tenant idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'tenant deadline context'],
  ['internal_code = %error.code', 'tenant mapped code'],
  ['internal_message = %error.message', 'tenant mapped message'],
  ['error_kind = ?error.kind', 'tenant mapped kind'],
  ['retryable = error.retryable', 'tenant retryability'],
  ['boundary = CHECKOUT_FULFILLMENT_BOUNDARY', 'tenant boundary'],
  ['error\n    })', 'same tenant error returned'],
]) requireText(tenantContext, value, label);

const operationErrorIndex = operationContext.indexOf('let error = PortError::validation(');
const operationLogIndex = operationContext.indexOf('tracing::warn!(');
const operationReturnIndex = operationContext.indexOf('return Err(error);');
if (!(operationErrorIndex >= 0 && operationErrorIndex < operationLogIndex && operationLogIndex < operationReturnIndex)) {
  failures.push('causation validation must construct the stable error, log it, then return the same error');
}

const tenantErrorIndex = tenantContext.indexOf('let error = PortError::validation(');
const tenantLogIndex = tenantContext.indexOf('tracing::warn!(');
const tenantReturnIndex = tenantContext.lastIndexOf('error\n    })');
if (!(tenantErrorIndex >= 0 && tenantErrorIndex < tenantLogIndex && tenantLogIndex < tenantReturnIndex)) {
  failures.push('tenant validation must construct the stable error, log it, then return the same error');
}

for (const value of [
  'return Err(PortError::validation(\n            "fulfillment.checkout_operation_id_invalid"',
  'Uuid::parse_str(&context.tenant_id).map_err(|error| {',
]) forbidText(source, value, 'superseded context-partial validation');

for (const [block, values, label] of [
  [
    portImpl,
    [
      'require_checkout_fulfillment_write_admission(&context, ENSURE_OPERATION)?;',
      'let tenant_id = parse_tenant_id(&context, ENSURE_OPERATION)?;',
      'require_operation_context(&context, ENSURE_OPERATION, request.checkout_operation_id)?;',
      'require_checkout_fulfillment_read_admission(&context, READ_OPERATION)?;',
      'let tenant_id = parse_tenant_id(&context, READ_OPERATION)?;',
      'require_operation_context(&context, READ_OPERATION, request.checkout_operation_id)?;',
    ],
    'public operation routing',
  ],
  [
    source,
    [
      'fn require_checkout_fulfillment_read_admission(',
      'fn require_checkout_fulfillment_write_admission(',
      'fn log_checkout_fulfillment_admission_rejection(',
      'fn validate_request(',
      'fn validate_fulfillment(',
      'fn fulfillment_error_to_port_error(',
      '"find_checkout_fulfillment_before_create"',
      '"adopt_checkout_fulfillment_after_create_error"',
      '"create_checkout_fulfillment"',
      '"list_checkout_fulfillments_for_read"',
    ],
    'preserved fulfillment behavior',
  ],
]) {
  for (const value of values) requireText(block, value, label);
}

for (const [pattern, expected, label] of [
  [/validation_phase = "causation_id"/g, 1, 'causation phase count'],
  [/validation_phase = "tenant_id"/g, 1, 'tenant phase count'],
  [/owner = CHECKOUT_FULFILLMENT_OWNER/g, 6, 'owner diagnostic count'],
  [/boundary = CHECKOUT_FULFILLMENT_BOUNDARY/g, 6, 'boundary diagnostic count'],
]) {
  const count = source.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

if (failures.length > 0) {
  console.error('Fulfillment checkout tenant and causation context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Fulfillment checkout tenant UUID and causation validation retain full PortContext, exact owner operation, original parse evidence, stable PortError envelopes, and unchanged routing',
);
