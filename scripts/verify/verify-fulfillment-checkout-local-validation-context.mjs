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

const ensure = between(
  source,
  'async fn ensure(',
  'async fn read(',
  'checkout fulfillment ensure helper',
);
const read = between(
  source,
  'async fn read(',
  'async fn find_by_key(',
  'checkout fulfillment read helper',
);
const findByKey = between(
  source,
  'async fn find_by_key(',
  'fn map_checkout_fulfillment_local_port_error(',
  'checkout fulfillment identity lookup helper',
);
const mapper = between(
  source,
  'fn map_checkout_fulfillment_local_port_error(',
  'pub fn in_process_checkout_fulfillment_execution_port(',
  'checkout fulfillment local error mapper',
);
const requestValidator = between(
  source,
  'fn validate_request(',
  'fn build_input(',
  'checkout fulfillment request validator',
);
const fulfillmentValidator = between(
  source,
  'fn validate_fulfillment(',
  'fn fulfillment_index(',
  'checkout fulfillment immutable-plan validator',
);

for (const [value, label] of [
  ['const CHECKOUT_FULFILLMENT_OWNER: &str = "rustok_fulfillment";', 'truthful owner constant'],
  [
    'const CHECKOUT_FULFILLMENT_BOUNDARY: &str = "checkout_fulfillment_execution_port";',
    'fulfillment execution boundary',
  ],
  ['const ENSURE_OPERATION: &str = "ensure_checkout_fulfillments";', 'ensure owner operation'],
  ['const READ_OPERATION: &str = "read_checkout_fulfillments";', 'read owner operation'],
]) requireText(source, value, label);

for (const [block, values, label] of [
  [
    ensure,
    [
      '.map_err(|error| {\n            map_checkout_fulfillment_local_port_error(',
      'ENSURE_OPERATION,\n                "validate_request",',
      'ENSURE_OPERATION,\n                    "validate_fulfillment",',
      'context,\n                    ENSURE_OPERATION,\n                    "find_checkout_fulfillment_before_create",',
      'context,\n                                ENSURE_OPERATION,\n                                "adopt_checkout_fulfillment_after_create_error",',
    ],
    'ensure local validation context',
  ],
  [
    read,
    [
      'READ_OPERATION,\n                "validate_request",',
      'READ_OPERATION,\n                    "collect_checkout_fulfillment_set",',
      'READ_OPERATION,\n                "require_complete_checkout_fulfillment_set",',
      'READ_OPERATION,\n                    "require_complete_checkout_fulfillment_set",',
      'READ_OPERATION,\n                    "validate_fulfillment",',
    ],
    'read local validation context',
  ],
  [
    findByKey,
    [
      "owner_operation: &'static str",
      "service_operation: &'static str",
      'fulfillment_error_to_port_error(context, service_operation, error)',
      'owner_operation,\n                "find_checkout_fulfillment_by_key",',
    ],
    'lookup owner/service separation',
  ],
]) {
  for (const value of values) requireText(block, value, label);
}

for (const [value, label] of [
  ['fn map_checkout_fulfillment_local_port_error(', 'local mapper definition'],
  ['context: &PortContext', 'retained context input'],
  ["owner_operation: &'static str", 'exact owner operation input'],
  ["local_operation: &'static str", 'exact local operation input'],
  ['error: PortError', 'mapped error input'],
  ['let error_kind = match &error.kind', 'closed error-kind classification'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'technical severity classification',
  ],
  ['tracing::error!(', 'technical severity'],
  ['tracing::warn!(', 'ordinary severity'],
  ['owner = CHECKOUT_FULFILLMENT_OWNER', 'truthful owner field'],
  ['owner_operation,', 'exact owner operation field'],
  ['local_operation,', 'exact local operation field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id_length', 'tenant context shape'],
  ['actor_kind', 'actor kind shape'],
  ['actor_id_length', 'actor identity shape'],
  ['claim_count', 'claim count'],
  ['role_count', 'role count'],
  ['channel_present', 'channel presence'],
  ['channel_length = ?channel_length', 'channel length'],
  ['locale_length', 'locale length'],
  ['causation_id_present', 'causation presence'],
  ['causation_id_length = ?causation_id_length', 'causation length'],
  ['traceparent_present', 'trace presence'],
  ['traceparent_length = ?traceparent_length', 'trace length'],
  ['idempotency_key_present', 'idempotency presence'],
  ['idempotency_key_length = ?idempotency_key_length', 'idempotency length'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['internal_code = %error.code', 'stable code evidence'],
  ['internal_message_present', 'bounded message presence'],
  ['internal_message_length', 'bounded message length'],
  ['error_kind', 'closed kind evidence'],
  ['retryable = error.retryable', 'retryability evidence'],
  ['boundary = CHECKOUT_FULFILLMENT_BOUNDARY', 'fulfillment boundary'],
  ['"checkout fulfillment local owner operation failed"', 'technical local event'],
  ['"checkout fulfillment local owner operation was rejected"', 'ordinary local event'],
  ['error\n}', 'same mapped error returned'],
]) requireText(mapper, value, label);

for (const value of [
  'error = ?error',
  'error = %error',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'internal_message = %error.message',
  'error_kind = ?error.kind',
]) forbidText(mapper, value, 'unsafe local mapper diagnostic payload');

const technicalLogIndex = mapper.indexOf('tracing::error!(');
const ordinaryLogIndex = mapper.indexOf('tracing::warn!(');
const returnIndex = mapper.lastIndexOf('error\n}');
if (!(technicalLogIndex >= 0 && ordinaryLogIndex >= 0 && technicalLogIndex < returnIndex && ordinaryLogIndex < returnIndex)) {
  failures.push('local mapper diagnostics must precede returning the same PortError');
}

for (const [block, values, label] of [
  [
    requestValidator,
    [
      '"fulfillment.checkout_identity_invalid"',
      '"checkout operation and order identity must be non-nil UUIDs"',
      '"fulfillment.checkout_plan_hash_invalid"',
      '"checkout fulfillment plan hash must be a 64-character hexadecimal value"',
      '"fulfillment.checkout_plan_invalid"',
      '"checkout fulfillment plans require unique indexes and non-empty items"',
      '"fulfillment.checkout_item_invalid"',
      '"checkout fulfillment items require unique order lines and positive quantities"',
    ],
    'stable request validation envelopes',
  ],
  [
    fulfillmentValidator,
    [
      '"fulfillment.checkout_plan_conflict"',
      '"fulfillment does not match the immutable checkout plan"',
      '"fulfillment.checkout_items_conflict"',
      '"fulfillment items do not match the immutable checkout plan"',
      '"fulfillment.checkout_identity_missing"',
      '"fulfillment has no checkout identity"',
      '"fulfillment.checkout_identity_conflict"',
      '"fulfillment has a mismatched checkout identity"',
    ],
    'stable fulfillment validation envelopes',
  ],
  [
    source,
    [
      '"fulfillment.checkout_identity_duplicate"',
      '"multiple fulfillments share one checkout fulfillment identity"',
      '"fulfillment.checkout_set_incomplete"',
      '"checkout fulfillment set is incomplete"',
    ],
    'stable set validation envelopes',
  ],
]) {
  for (const value of values) requireText(block, value, label);
}

for (const value of [
  '        )?;\n        let mut result = Vec::with_capacity(request.plans.len());',
  '        )?;\n        let rows = self',
  'return Err(PortError::conflict(\n                    "fulfillment.checkout_identity_duplicate"',
  'return Err(PortError::conflict(\n                "fulfillment.checkout_set_incomplete"',
  'fulfillment_error_to_port_error(context, owner_operation, error)',
]) forbidText(source, value, 'context-dropping local validation path');

for (const [pattern, expected, label] of [
  [/map_checkout_fulfillment_local_port_error\(/g, 9, 'local mapper definition/use count'],
  [/"validate_request"/g, 2, 'request validation operation count'],
  [/"validate_fulfillment"/g, 2, 'fulfillment validation operation count'],
  [/"collect_checkout_fulfillment_set"/g, 1, 'set collection operation count'],
  [/"require_complete_checkout_fulfillment_set"/g, 2, 'set completeness operation count'],
  [/"find_checkout_fulfillment_by_key"/g, 1, 'identity lookup operation count'],
  [/owner = CHECKOUT_FULFILLMENT_OWNER/g, 11, 'owner diagnostic count'],
  [/boundary = CHECKOUT_FULFILLMENT_BOUNDARY/g, 11, 'boundary diagnostic count'],
]) {
  const count = source.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [value, label] of [
  ['fn require_checkout_fulfillment_read_admission(', 'read admission helper'],
  ['fn require_checkout_fulfillment_write_admission(', 'write admission helper'],
  ['fn log_checkout_fulfillment_admission_rejection(', 'admission diagnostics'],
  ['fn require_operation_context(', 'causation validation'],
  ['fn parse_tenant_id(', 'tenant validation'],
  ['fn fulfillment_error_to_port_error(', 'stable owner mapper'],
  ['fn build_input(', 'fulfillment input builder'],
  ['fn fulfillment_metadata(', 'fulfillment metadata builder'],
  ['fn fulfillment_item_metadata(', 'item metadata builder'],
  ['"create_checkout_fulfillment"', 'create service operation'],
  ['"list_checkout_fulfillments_for_read"', 'read service operation'],
]) requireText(source, value, label);

if (failures.length > 0) {
  console.error('Fulfillment checkout local validation context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Fulfillment checkout request, set, identity, and immutable-plan failures retain bounded PortContext shape, exact owner/local operations, stable PortError envelopes, and unchanged owner behavior',
);
