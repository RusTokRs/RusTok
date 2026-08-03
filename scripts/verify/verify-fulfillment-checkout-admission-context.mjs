#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const source = read('crates/rustok-fulfillment/src/checkout_execution.rs');
const evidence = JSON.parse(
  read(
    'crates/rustok-fulfillment/contracts/evidence/checkout-admission-diagnostic-safety-source.json',
  ),
);
const doc = read('crates/rustok-fulfillment/docs/checkout-admission-context.md');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (content, value) => content.split(value).length - 1;
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};

const admissionHelpers = between(
  source,
  'fn require_checkout_fulfillment_read_admission(',
  '#[async_trait]\nimpl CheckoutFulfillmentExecutionPort for InProcessCheckoutFulfillmentExecutionPort',
  'checkout fulfillment admission helpers',
);
const admissionMapper = between(
  source,
  'fn log_checkout_fulfillment_admission_rejection(',
  '#[async_trait]\nimpl CheckoutFulfillmentExecutionPort for InProcessCheckoutFulfillmentExecutionPort',
  'checkout fulfillment admission mapper',
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
const readOperation = between(
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
  ['.require_policy(PortCallPolicy::read())', 'read policy selection'],
  ['.inspect_err(|error| {', 'read original-error inspection'],
  ['log_checkout_fulfillment_admission_rejection(context, owner_operation, "policy", error);', 'read policy diagnostics'],
  ['fn require_checkout_fulfillment_write_admission(', 'write admission helper'],
  ['.require_policy(PortCallPolicy::write())', 'write policy selection'],
  ['context.require_write_semantics().inspect_err(|error| {', 'write semantics inspection'],
  ['"write_semantics",', 'write semantics phase'],
]) requireText(admissionHelpers, value, label);

for (const [value, label] of [
  ['fn log_checkout_fulfillment_admission_rejection(', 'shared admission mapper'],
  ['error: &PortError', 'borrowed original port error'],
  ['let error_kind = match &error.kind', 'closed error-kind classification'],
  ['PortErrorKind::Validation => "validation"', 'validation kind label'],
  ['PortErrorKind::NotFound => "not_found"', 'not-found kind label'],
  ['PortErrorKind::Conflict => "conflict"', 'conflict kind label'],
  ['PortErrorKind::Forbidden => "forbidden"', 'forbidden kind label'],
  ['PortErrorKind::Unavailable => "unavailable"', 'unavailable kind label'],
  ['PortErrorKind::Timeout => "timeout"', 'timeout kind label'],
  ['PortErrorKind::InvariantViolation => "invariant_violation"', 'invariant kind label'],
  ['let technical_failure = matches!(', 'technical severity classification'],
  ['let actor_kind = match &context.actor.kind', 'bounded actor kind'],
  ['let tenant_id_length = context.tenant_id.chars().count();', 'tenant shape'],
  ['let actor_id_length = context.actor.id.chars().count();', 'actor identity shape'],
  ['let claim_count = context.claims.len();', 'claim count'],
  ['let role_count = context.roles.len();', 'role count'],
  ['let channel_present = context.channel.is_some();', 'channel presence'],
  ['let channel_length = context.channel.as_ref()', 'channel length'],
  ['let locale_length = context.locale.chars().count();', 'locale length'],
  ['let causation_id_present = context.causation_id.is_some();', 'causation presence'],
  ['let causation_id_length = context', 'causation length'],
  ['let traceparent_present = context.traceparent.is_some();', 'traceparent presence'],
  ['let traceparent_length = context', 'traceparent length'],
  ['let idempotency_key_present = context.idempotency_key.is_some();', 'idempotency presence'],
  ['let idempotency_key_length = context', 'idempotency length'],
  ['let internal_message_present = !error.message.trim().is_empty();', 'message presence'],
  ['let internal_message_length = error.message.chars().count();', 'message length'],
  ['owner = CHECKOUT_FULFILLMENT_OWNER', 'truthful owner field'],
  ['owner_operation,', 'exact owner operation field'],
  ['admission_phase,', 'exact admission phase field'],
  ['correlation_id = %context.correlation_id', 'correlation diagnostic'],
  ['internal_code = %error.code', 'stable internal code'],
  ['internal_message_present', 'bounded message presence'],
  ['internal_message_length', 'bounded message length'],
  ['error_kind', 'closed kind diagnostic'],
  ['retryable = error.retryable', 'retryability diagnostic'],
  ['boundary = CHECKOUT_FULFILLMENT_BOUNDARY', 'execution boundary'],
  ['"checkout fulfillment owner admission failed"', 'technical event'],
  ['"checkout fulfillment owner admission was rejected"', 'ordinary event'],
]) requireText(admissionMapper, value, label);

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
]) forbidText(admissionMapper, value, 'unsafe admission diagnostic payload');

if (countText(admissionMapper, 'tracing::error!(') !== 1) {
  failures.push('expected exactly one admission technical diagnostic path');
}
if (countText(admissionMapper, 'tracing::warn!(') !== 1) {
  failures.push('expected exactly one admission rejection diagnostic path');
}
for (const marker of [
  'owner_operation,',
  'admission_phase,',
  'correlation_id = %context.correlation_id',
  'internal_code = %error.code',
  'internal_message_present',
  'internal_message_length',
  'error_kind',
  'retryable = error.retryable',
]) {
  if (countText(admissionMapper, marker) < 2) {
    failures.push(`both admission severity paths must retain ${marker}`);
  }
}

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
    readOperation,
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
  [/"policy"/g, 2, 'policy phase count'],
  [/"write_semantics"/g, 1, 'write semantics phase count'],
]) {
  const count = source.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [value, label] of [
  ['fn map_checkout_fulfillment_local_port_error(', 'bounded local PortError mapper'],
  ['fn require_operation_context(', 'causation validation'],
  ['fn parse_tenant_id(', 'tenant parser'],
  ['fn fulfillment_error_to_port_error(', 'canonical fulfillment mapper'],
  ['"find_checkout_fulfillment_before_create"', 'pre-create lookup'],
  ['"adopt_checkout_fulfillment_after_create_error"', 'post-error adoption'],
  ['"create_checkout_fulfillment"', 'create operation'],
  ['"list_checkout_fulfillments_for_read"', 'read list operation'],
]) requireText(source, value, label);

if (evidence.status !== 'fulfillment_checkout_admission_diagnostic_safety_source_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  admission_mapper_bounded: true,
  complete_admission_port_error_logged: false,
  admission_internal_message_text_logged: false,
  admission_context_shape_only: true,
  admission_correlation_preserved: true,
  admission_owner_operations_preserved: true,
  admission_phases_preserved: true,
  admission_error_kind_closed: true,
  admission_severity_split_preserved: true,
  original_admission_port_error_returned: true,
  read_write_admission_order_preserved: true,
  local_porterror_mapper_unchanged: true,
  causation_tenant_parser_cleanup_out_of_scope: true,
  canonical_fulfillment_error_mapper_cleanup_out_of_scope: true,
  execution_behavior_changed: false,
  public_port_error_changed: false,
  ffa_promoted: false,
  fba_promoted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push('evidence execution must remain empty');
}
for (const key of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
  'runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

for (const [value, label] of [
  ['Status: **source-ready / unvalidated**', 'documentation status'],
  ['Admission diagnostics record only a closed error-kind label', 'documentation error policy'],
  ['The original admission `PortError` continues through `inspect_err` unchanged', 'documentation pass-through policy'],
  ['Causation validation, tenant parsing, and canonical `FulfillmentError` diagnostics remain separate', 'documentation remaining boundary'],
]) requireText(doc, value, label);

if (failures.length > 0) {
  console.error('Fulfillment checkout admission diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Fulfillment checkout admission diagnostics use bounded kind, message-shape, and context-shape facts while preserving admission order and original-error pass-through',
);
