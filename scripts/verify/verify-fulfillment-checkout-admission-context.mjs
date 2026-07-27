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

const admission = between(
  source,
  'fn require_checkout_fulfillment_read_admission(',
  '#[async_trait]\nimpl CheckoutFulfillmentExecutionPort for InProcessCheckoutFulfillmentExecutionPort',
  'checkout fulfillment admission helpers',
);
const portImpl = between(
  source,
  '#[async_trait]\nimpl CheckoutFulfillmentExecutionPort for InProcessCheckoutFulfillmentExecutionPort',
  'fn validate_request(',
  'checkout fulfillment port implementation',
);
const ensure = between(
  portImpl,
  'async fn ensure_checkout_fulfillments(',
  'async fn read_checkout_fulfillments(',
  'ensure fulfillment operation',
);
const read = between(
  portImpl,
  'async fn read_checkout_fulfillments(',
  '\n}',
  'read fulfillment operation',
);

for (const [value, label] of [
  ['const CHECKOUT_FULFILLMENT_OWNER: &str = "rustok_fulfillment";', 'truthful fulfillment owner'],
  [
    'const CHECKOUT_FULFILLMENT_BOUNDARY: &str = "checkout_fulfillment_execution_port";',
    'stable fulfillment execution boundary',
  ],
  ['const ENSURE_OPERATION: &str = "ensure_checkout_fulfillments";', 'ensure operation'],
  ['const READ_OPERATION: &str = "read_checkout_fulfillments";', 'read operation'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['fn require_checkout_fulfillment_read_admission(', 'read admission helper'],
  ['context.require_policy(PortCallPolicy::read()).map_err(|error| {', 'read policy interception'],
  [
    'log_checkout_fulfillment_admission_rejection(\n            context,\n            owner_operation,\n            "policy",\n            &error,',
    'read policy diagnostics',
  ],
  ['fn require_checkout_fulfillment_write_admission(', 'write admission helper'],
  ['context.require_policy(PortCallPolicy::write()).map_err(|error| {', 'write policy interception'],
  ['context.require_write_semantics().map_err(|error| {', 'write semantics interception'],
  ['"write_semantics",', 'write semantics phase'],
  ['fn log_checkout_fulfillment_admission_rejection(', 'shared rejection diagnostics'],
  ['context: &PortContext', 'retained context input'],
  ["owner_operation: &'static str", 'exact owner operation input'],
  ["admission_phase: &'static str", 'admission phase input'],
  ['error: &PortError', 'original port error input'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'technical severity classification',
  ],
  ['tracing::error!(', 'technical rejection severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['error = ?error', 'original error evidence'],
  ['owner = CHECKOUT_FULFILLMENT_OWNER', 'truthful owner field'],
  ['owner_operation,', 'exact operation field'],
  ['admission_phase,', 'admission phase field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['internal_code = %error.code', 'original internal code'],
  ['internal_message = %error.message', 'original internal message'],
  ['error_kind = ?error.kind', 'typed error kind'],
  ['retryable = error.retryable', 'original retryability'],
  ['boundary = CHECKOUT_FULFILLMENT_BOUNDARY', 'fulfillment boundary field'],
  ['"checkout fulfillment owner admission failed"', 'technical event'],
  ['"checkout fulfillment owner admission was rejected"', 'ordinary event'],
]) requireText(admission, value, label);

for (const [block, values, label] of [
  [
    ensure,
    [
      'require_checkout_fulfillment_write_admission(&context, ENSURE_OPERATION)?;',
      'let tenant_id = parse_tenant_id(&context, ENSURE_OPERATION)?;',
      'require_operation_context(&context, ENSURE_OPERATION, request.checkout_operation_id)?;',
      'self.ensure(&context, tenant_id, request).await',
    ],
    'ensure behavior',
  ],
  [
    read,
    [
      'require_checkout_fulfillment_read_admission(&context, READ_OPERATION)?;',
      'let tenant_id = parse_tenant_id(&context, READ_OPERATION)?;',
      'require_operation_context(&context, READ_OPERATION, request.checkout_operation_id)?;',
      'self.read(&context, tenant_id, request).await',
    ],
    'read behavior',
  ],
]) {
  for (const value of values) requireText(block, value, label);
  const admissionIndex = block.indexOf('require_checkout_fulfillment_');
  const tenantIndex = block.indexOf('let tenant_id = parse_tenant_id(');
  if (!(admissionIndex >= 0 && admissionIndex < tenantIndex)) {
    failures.push(`${label}: admission must precede tenant parsing`);
  }
}

for (const value of [
  'context.require_policy(PortCallPolicy::write())?;',
  'context.require_write_semantics()?;',
  'context.require_policy(PortCallPolicy::read())?;',
]) forbidText(portImpl, value, 'context-dropping direct admission');

for (const [pattern, expected, label] of [
  [/require_checkout_fulfillment_read_admission\(/g, 2, 'read helper definition/use count'],
  [/require_checkout_fulfillment_write_admission\(/g, 2, 'write helper definition/use count'],
  [/log_checkout_fulfillment_admission_rejection\(/g, 4, 'diagnostic helper definition/use count'],
  [/owner = CHECKOUT_FULFILLMENT_OWNER/g, 6, 'owner diagnostic count'],
  [/boundary = CHECKOUT_FULFILLMENT_BOUNDARY/g, 6, 'boundary diagnostic count'],
  [/"policy"/g, 2, 'policy phase count'],
  [/"write_semantics"/g, 1, 'write semantics phase count'],
]) {
  const count = source.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [value, label] of [
  ['fn validate_request(', 'request validation'],
  ['fn validate_fulfillment(', 'fulfillment validation'],
  ['fn require_operation_context(', 'causation validation'],
  ['fn parse_tenant_id(', 'tenant parser'],
  ['fn fulfillment_error_to_port_error(', 'stable fulfillment mapper'],
  ['"find_checkout_fulfillment_before_create"', 'pre-create lookup'],
  ['"adopt_checkout_fulfillment_after_create_error"', 'post-error adoption'],
  ['"create_checkout_fulfillment"', 'create operation'],
  ['"list_checkout_fulfillments_for_read"', 'read list operation'],
]) requireText(source, value, label);

if (failures.length > 0) {
  console.error('Fulfillment checkout admission context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Fulfillment checkout owner admission retains full PortContext, exact operation and phase, original PortError, stable behavior, and technical-versus-ordinary severity',
);
