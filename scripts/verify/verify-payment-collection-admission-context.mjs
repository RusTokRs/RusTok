#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const source = read('crates/rustok-payment/src/ports.rs');
const evidence = JSON.parse(
  read(
    'crates/rustok-payment/contracts/evidence/payment-collection-admission-diagnostic-safety-source.json',
  ),
);
const doc = read('crates/rustok-payment/docs/payment-collection-admission-context.md');
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

const createBlock = between(
  source,
  '    async fn create_or_reuse_collection(',
  '    async fn read_collection_status(',
  'create or reuse implementation',
);
const readBlock = between(
  source,
  '    async fn read_collection_status(',
  '\n}\n\nfn require_payment_collection_read_admission(',
  'status read implementation',
);
const readAdmission = between(
  source,
  'fn require_payment_collection_read_admission(',
  'fn require_payment_collection_write_admission(',
  'read admission helper',
);
const writeAdmission = between(
  source,
  'fn require_payment_collection_write_admission(',
  'fn log_payment_collection_admission_rejection(',
  'write admission helper',
);
const admissionLogger = between(
  source,
  'fn log_payment_collection_admission_rejection(',
  'fn parse_port_tenant_id(',
  'admission diagnostic helper',
);
const tenantParser = between(
  source,
  'fn parse_port_tenant_id(',
  'fn payment_error_to_port_error(',
  'tenant parser',
);
const ownerMapper = source.slice(source.indexOf('fn payment_error_to_port_error('));

for (const [value, label] of [
  ['const PAYMENT_COLLECTION_PORT_BOUNDARY: &str = "payment_collection_port";', 'stable boundary identity'],
  ['const PAYMENT_COLLECTION_OWNER: &str = "rustok_payment";', 'truthful payment owner'],
  ['const CREATE_OR_REUSE_COLLECTION_OPERATION: &str = "create_or_reuse_collection";', 'write owner operation'],
  ['const READ_COLLECTION_STATUS_OPERATION: &str = "read_collection_status";', 'read owner operation'],
]) requireText(source, value, label);

for (const [content, values, label] of [
  [
    createBlock,
    [
      'let owner_operation = CREATE_OR_REUSE_COLLECTION_OPERATION;',
      'require_payment_collection_write_admission(&context, owner_operation)?;',
      'let tenant_id = parse_port_tenant_id(&context, owner_operation)?;',
    ],
    'write admission ordering',
  ],
  [
    readBlock,
    [
      'let owner_operation = READ_COLLECTION_STATUS_OPERATION;',
      'require_payment_collection_read_admission(&context, owner_operation)?;',
      'let tenant_id = parse_port_tenant_id(&context, owner_operation)?;',
    ],
    'read admission ordering',
  ],
]) {
  let previous = -1;
  for (const value of values) {
    const index = content.indexOf(value);
    if (index < 0) failures.push(`${label}: missing ${value}`);
    if (index >= 0 && index <= previous) failures.push(`${label}: ${value} is out of order`);
    previous = index;
  }
}

for (const [content, values, label] of [
  [
    readAdmission,
    [
      'context.require_policy(PortCallPolicy::read())',
      '.inspect_err(|error| {',
      'log_payment_collection_admission_rejection(context, owner_operation, "policy", error);',
    ],
    'read admission pass-through',
  ],
  [
    writeAdmission,
    [
      'context.require_policy(PortCallPolicy::write())',
      '.inspect_err(|error| {',
      'log_payment_collection_admission_rejection(context, owner_operation, "policy", error);',
      'context.require_write_semantics().inspect_err(|error| {',
      '"write_semantics",',
    ],
    'write admission pass-through',
  ],
]) {
  for (const value of values) requireText(content, value, label);
}
forbidText(readAdmission, 'require_write_semantics', 'read admission must remain non-write');
forbidText(readAdmission + writeAdmission, 'PortError::', 'admission helper error reconstruction');
forbidText(readAdmission + writeAdmission, '.map_err(', 'admission helper error replacement');

for (const [value, label] of [
  ['context: &PortContext', 'retained context input'],
  ["owner_operation: &'static str", 'exact owner operation input'],
  ["admission: &'static str", 'admission phase input'],
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
  ['let traceparent_present = context.traceparent.is_some();', 'trace presence'],
  ['let traceparent_length = context', 'trace length'],
  ['let idempotency_key_present = context.idempotency_key.is_some();', 'idempotency presence'],
  ['let idempotency_key_length = context', 'idempotency length'],
  ['let internal_message_present = !error.message.trim().is_empty();', 'message presence'],
  ['let internal_message_length = error.message.chars().count();', 'message length'],
  ['owner = PAYMENT_COLLECTION_OWNER', 'truthful owner field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['operation = owner_operation', 'owner operation field'],
  ['admission,', 'admission phase field'],
  ['code = %error.code', 'stable error code'],
  ['internal_message_present', 'message presence diagnostic'],
  ['internal_message_length', 'message length diagnostic'],
  ['error_kind', 'closed kind diagnostic'],
  ['retryable = error.retryable', 'retryability'],
  ['boundary = PAYMENT_COLLECTION_PORT_BOUNDARY', 'boundary field'],
  ['tracing::error!(', 'technical severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['"payment collection admission failed"', 'technical event'],
  ['"payment collection admission was rejected"', 'rejection event'],
]) requireText(admissionLogger, value, label);

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
]) forbidText(admissionLogger, value, 'unsafe admission diagnostic payload');

if (countText(admissionLogger, 'tracing::error!(') !== 1) {
  failures.push('expected exactly one admission technical diagnostic path');
}
if (countText(admissionLogger, 'tracing::warn!(') !== 1) {
  failures.push('expected exactly one admission rejection diagnostic path');
}
for (const marker of [
  'owner = PAYMENT_COLLECTION_OWNER',
  'correlation_id = %context.correlation_id',
  'operation = owner_operation',
  'admission,',
  'code = %error.code',
  'internal_message_present',
  'internal_message_length',
  'error_kind',
  'retryable = error.retryable',
  'boundary = PAYMENT_COLLECTION_PORT_BOUNDARY',
]) {
  if (countText(admissionLogger, marker) < 2) {
    failures.push(`both admission severity paths must retain ${marker}`);
  }
}

for (const [pattern, expected, label] of [
  [/log_payment_collection_admission_rejection\(/g, 4, 'diagnostic helper definition/use count'],
  [/"policy"/g, 2, 'policy phase count'],
  [/"write_semantics"/g, 1, 'write semantics phase count'],
]) {
  const count = source.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [content, values, label] of [
  [
    tenantParser,
    [
      'Uuid::parse_str(&context.tenant_id).map_err(|parse_error| {',
      'parse_error = ?parse_error',
      'error = ?error',
      'validation = "tenant_id"',
      '"payment.tenant_id_invalid"',
    ],
    'tenant parser remains separate',
  ],
  [
    ownerMapper,
    [
      'fn payment_error_to_port_error(',
      'cause = %message',
      'provider_id = %provider_id',
      'error = ?error',
      '"payment request is invalid"',
      '"payment storage is temporarily unavailable"',
    ],
    'canonical payment mapper remains separate',
  ],
]) {
  for (const value of values) requireText(content, value, label);
}

for (const [value, label] of [
  ['"create_or_reuse_collection.read_existing"', 'existing collection read mapping'],
  ['"create_or_reuse_collection.adopt_race"', 'race adoption mapping'],
  ['"create_or_reuse_collection.create"', 'collection create mapping'],
  ['find_reusable_collection_by_cart(tenant_id, cart_id)', 'reusable collection lookup'],
  ['create_collection(', 'collection creation'],
]) requireText(createBlock, value, label);
for (const [value, label] of [
  ['get_collection(tenant_id, request.collection_id)', 'status collection identity'],
  ['payment_error_to_port_error(&context, owner_operation, error)', 'status owner mapping'],
  ['PaymentCollectionStatusSnapshot::from_response(&response)', 'status snapshot mapping'],
]) requireText(readBlock, value, label);

if (evidence.status !== 'payment_collection_admission_diagnostic_safety_source_unvalidated') {
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
  tenant_parser_cleanup_out_of_scope: true,
  canonical_payment_error_mapper_cleanup_out_of_scope: true,
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
  ['Admission diagnostics retain only bounded context and message-shape facts', 'documentation bounded policy'],
  ['The exact original admission `PortError` continues through `inspect_err` unchanged', 'documentation pass-through policy'],
  ['Tenant UUID parsing and canonical `PaymentError` mapping remain separate cleanup slices', 'documentation residual boundary'],
]) requireText(doc, value, label);

if (failures.length > 0) {
  console.error('Payment collection admission diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Payment collection admission diagnostics use bounded kind, message-shape, and context-shape facts while preserving admission ordering and original-error pass-through',
);
