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
    'crates/rustok-fulfillment/contracts/evidence/checkout-causation-diagnostic-safety-source.json',
  ),
);
const doc = read('crates/rustok-fulfillment/docs/checkout-context-validation.md');
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
  ['let causation_id_present = context.causation_id.is_some();', 'causation presence'],
  ['let causation_id_length = context', 'causation length'],
  ['.and_then(|value| Uuid::parse_str(value).ok());', 'optional UUID causation parsing'],
  ['let causation_id_parse_succeeded = context_operation.is_some();', 'causation parse fact'],
  [
    'let causation_id_matches_expected = context_operation == Some(checkout_operation_id);',
    'causation match fact',
  ],
  ['if !causation_id_matches_expected {', 'causation mismatch condition'],
  ['let error = PortError::validation(', 'causation stable error construction'],
  ['"fulfillment.checkout_operation_id_invalid"', 'causation stable code'],
  [
    '"checkout fulfillment causation_id must match the checkout operation"',
    'causation stable message',
  ],
  ['let actor_kind = match &context.actor.kind', 'bounded actor kind'],
  ['let tenant_id_length = context.tenant_id.chars().count();', 'tenant shape'],
  ['let actor_id_length = context.actor.id.chars().count();', 'actor identity shape'],
  ['let claim_count = context.claims.len();', 'claim count'],
  ['let role_count = context.roles.len();', 'role count'],
  ['let channel_present = context.channel.is_some();', 'channel presence'],
  ['let channel_length = context.channel.as_ref()', 'channel length'],
  ['let locale_length = context.locale.chars().count();', 'locale length'],
  ['let traceparent_present = context.traceparent.is_some();', 'traceparent presence'],
  ['let traceparent_length = context', 'traceparent length'],
  ['let idempotency_key_present = context.idempotency_key.is_some();', 'idempotency presence'],
  ['let idempotency_key_length = context', 'idempotency length'],
  [
    'let expected_checkout_operation_id_non_nil = !checkout_operation_id.is_nil();',
    'expected operation identity shape',
  ],
  ['let internal_message_present = !error.message.trim().is_empty();', 'message presence'],
  ['let internal_message_length = error.message.chars().count();', 'message length'],
  ['let error_kind = "validation";', 'closed validation kind'],
  ['tracing::warn!(', 'causation warning severity'],
  ['owner = CHECKOUT_FULFILLMENT_OWNER', 'causation truthful owner'],
  ['operation = owner_operation', 'causation exact operation'],
  ['validation_phase = "causation_id"', 'causation validation phase'],
  ['correlation_id = %context.correlation_id', 'causation correlation context'],
  ['causation_id_parse_succeeded', 'causation parse diagnostic'],
  ['causation_id_matches_expected', 'causation match diagnostic'],
  ['expected_checkout_operation_id_non_nil', 'expected identity shape diagnostic'],
  ['code = "fulfillment.checkout_operation_id_invalid"', 'causation code diagnostic'],
  ['internal_code = %error.code', 'causation internal code'],
  ['internal_message_present', 'bounded message presence diagnostic'],
  ['internal_message_length', 'bounded message length diagnostic'],
  ['error_kind', 'closed kind diagnostic'],
  ['retryable = error.retryable', 'causation retryability'],
  ['boundary = CHECKOUT_FULFILLMENT_BOUNDARY', 'causation boundary'],
  ['return Err(error);', 'same causation error returned'],
]) requireText(operationContext, value, label);

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
  'expected_checkout_operation_id = %checkout_operation_id',
  'internal_message = %error.message',
  'error_kind = ?error.kind',
]) forbidText(operationContext, value, 'unsafe causation diagnostic payload');

if (countText(operationContext, 'tracing::warn!(') !== 1) {
  failures.push('expected exactly one causation warning path');
}

for (const [value, label] of [
  ['fn parse_tenant_id(', 'tenant parser'],
  ['Uuid::parse_str(&context.tenant_id).map_err(|cause| {', 'tenant parse cause capture'],
  ['"fulfillment.tenant_id_invalid"', 'tenant stable code'],
  ['"PortContext.tenant_id must be a UUID for fulfillment ports"', 'tenant stable message'],
  ['cause = ?cause', 'open tenant parse cause diagnostic'],
  ['error = ?error', 'open tenant complete error diagnostic'],
  ['tenant_id = %context.tenant_id', 'open tenant identity diagnostic'],
  ['actor = ?context.actor', 'open tenant actor diagnostic'],
  ['channel = ?context.channel', 'open tenant channel diagnostic'],
  ['locale = %context.locale', 'open tenant locale diagnostic'],
  ['causation_id = ?context.causation_id', 'open tenant causation diagnostic'],
  ['traceparent = ?context.traceparent', 'open tenant trace diagnostic'],
  ['idempotency_key = ?context.idempotency_key', 'open tenant idempotency diagnostic'],
  ['internal_message = %error.message', 'open tenant message diagnostic'],
  ['error_kind = ?error.kind', 'open tenant kind diagnostic'],
  ['error\n    })', 'same tenant error returned'],
]) requireText(tenantContext, value, label);

const operationErrorIndex = operationContext.indexOf('let error = PortError::validation(');
const operationLogIndex = operationContext.indexOf('tracing::warn!(');
const operationReturnIndex = operationContext.indexOf('return Err(error);');
if (!(operationErrorIndex >= 0 && operationErrorIndex < operationLogIndex && operationLogIndex < operationReturnIndex)) {
  failures.push('causation validation must construct the stable error, log bounded facts, then return the same error');
}

for (const [block, values, label] of [
  [
    portImpl,
    [
      'require_checkout_fulfillment_write_admission(&context, ENSURE_OPERATION)?;',
      'let tenant_id = parse_tenant_id(&context, ENSURE_OPERATION)?;',
      'require_operation_context(&context, ENSURE_OPERATION, request.checkout_operation_id)?;',
      'self.ensure(&context, tenant_id, request).await',
    ],
    'ensure routing',
  ],
  [
    portImpl,
    [
      'require_checkout_fulfillment_read_admission(&context, READ_OPERATION)?;',
      'let tenant_id = parse_tenant_id(&context, READ_OPERATION)?;',
      'require_operation_context(&context, READ_OPERATION, request.checkout_operation_id)?;',
      'self.read(&context, tenant_id, request).await',
    ],
    'read routing',
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

if (evidence.status !== 'fulfillment_checkout_causation_diagnostic_safety_source_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  causation_validation_bounded: true,
  complete_causation_port_error_logged: false,
  causation_internal_message_text_logged: false,
  causation_context_shape_only: true,
  causation_correlation_preserved: true,
  causation_owner_operations_preserved: true,
  causation_validation_phase_preserved: true,
  causation_error_kind_closed: true,
  causation_warning_severity_preserved: true,
  causation_parse_and_match_facts_preserved: true,
  expected_operation_identity_shape_only: true,
  original_causation_port_error_returned: true,
  tenant_parser_cleanup_out_of_scope: true,
  admission_mapper_unchanged: true,
  local_porterror_mapper_unchanged: true,
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
  ['Causation diagnostics retain only bounded context and identity-shape facts', 'documentation causation policy'],
  ['The exact constructed causation `PortError` is returned unchanged', 'documentation pass-through policy'],
  ['Tenant parsing remains the next separate diagnostic cleanup slice', 'documentation remaining boundary'],
]) requireText(doc, value, label);

if (failures.length > 0) {
  console.error('Fulfillment checkout causation diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Fulfillment checkout causation diagnostics use bounded context and identity-shape facts while tenant parser cleanup remains open',
);
