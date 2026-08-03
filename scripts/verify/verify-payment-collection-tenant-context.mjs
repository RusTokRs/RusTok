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
    'crates/rustok-payment/contracts/evidence/payment-collection-tenant-diagnostic-safety-source.json',
  ),
);
const doc = read('crates/rustok-payment/docs/payment-collection-tenant-context.md');
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
const admissionBlock = between(
  source,
  'fn require_payment_collection_read_admission(',
  'fn parse_port_tenant_id(',
  'payment collection admission block',
);
const tenantParser = between(
  source,
  'fn parse_port_tenant_id(',
  'fn payment_error_to_port_error(',
  'tenant parser and diagnostics',
);
const ownerMapperStart = source.indexOf('fn payment_error_to_port_error(');
if (ownerMapperStart < 0) failures.push('payment owner mapper: unable to isolate source block');
const ownerMapper = ownerMapperStart >= 0 ? source.slice(ownerMapperStart) : '';

for (const [value, label] of [
  ['const PAYMENT_COLLECTION_PORT_BOUNDARY: &str = "payment_collection_port";', 'stable boundary identity'],
  ['const PAYMENT_COLLECTION_OWNER: &str = "rustok_payment";', 'truthful owner identity'],
  ['const CREATE_OR_REUSE_COLLECTION_OPERATION: &str = "create_or_reuse_collection";', 'create owner operation'],
  ['const READ_COLLECTION_STATUS_OPERATION: &str = "read_collection_status";', 'status owner operation'],
]) requireText(source, value, label);

for (const [content, values, label] of [
  [
    createBlock,
    [
      'let owner_operation = CREATE_OR_REUSE_COLLECTION_OPERATION;',
      'require_payment_collection_write_admission(&context, owner_operation)?;',
      'let tenant_id = parse_port_tenant_id(&context, owner_operation)?;',
      'find_reusable_collection_by_cart(tenant_id, cart_id)',
    ],
    'create tenant parsing order',
  ],
  [
    readBlock,
    [
      'let owner_operation = READ_COLLECTION_STATUS_OPERATION;',
      'require_payment_collection_read_admission(&context, owner_operation)?;',
      'let tenant_id = parse_port_tenant_id(&context, owner_operation)?;',
      'get_collection(tenant_id, request.collection_id)',
    ],
    'read tenant parsing order',
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

const parserCalls = source.match(/parse_port_tenant_id\(&context, owner_operation\)\?/g) ?? [];
if (parserCalls.length !== 2) {
  failures.push(`expected two operation-aware tenant parser calls, found ${parserCalls.length}`);
}

for (const [value, label] of [
  ['context: &PortContext', 'retained context input'],
  ["owner_operation: &'static str", 'exact owner operation input'],
  ['Uuid::parse_str(&context.tenant_id).map_err(|parse_error| {', 'tenant UUID parsing and mapping'],
  ['let error = PortError::validation(', 'stable validation construction'],
  ['"payment.tenant_id_invalid"', 'stable validation code'],
  ['"PortContext.tenant_id must be a UUID for payment ports"', 'stable validation message'],
  ['let parse_error_type = std::any::type_name_of_val(&parse_error);', 'type-only parse cause'],
  ['tenant_id_parse_failed = true', 'explicit parse-failure fact'],
  ['let actor_kind = match &context.actor.kind', 'closed actor kind'],
  ['let tenant_id_length = context.tenant_id.chars().count();', 'tenant identity shape'],
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
  ['let error_kind = "validation";', 'closed validation kind'],
  ['owner = PAYMENT_COLLECTION_OWNER', 'truthful owner field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['operation = owner_operation', 'exact operation field'],
  ['validation = "tenant_id"', 'validation phase'],
  ['code = %error.code', 'mapped code'],
  ['internal_message_present', 'message presence diagnostic'],
  ['internal_message_length', 'message length diagnostic'],
  ['error_kind', 'closed kind diagnostic'],
  ['retryable = error.retryable', 'retryability'],
  ['boundary = PAYMENT_COLLECTION_PORT_BOUNDARY', 'boundary field'],
  ['tracing::warn!(', 'validation rejection severity'],
  ['"payment collection tenant context was rejected"', 'tenant rejection event'],
]) requireText(tenantParser, value, label);

for (const value of [
  'parse_error = ?parse_error',
  'parse_error = %parse_error',
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
]) forbidText(tenantParser, value, 'unsafe tenant diagnostic payload');

if (countText(tenantParser, 'tracing::warn!(') !== 1) {
  failures.push('tenant parser must retain exactly one warning diagnostic path');
}

const validationIndex = tenantParser.indexOf('let error = PortError::validation(');
const factsIndex = tenantParser.indexOf('let parse_error_type =');
const diagnosticsIndex = tenantParser.indexOf('tracing::warn!(');
const returnIndex = tenantParser.lastIndexOf('\n        error');
if (
  !(
    validationIndex >= 0 &&
    validationIndex < factsIndex &&
    factsIndex < diagnosticsIndex &&
    diagnosticsIndex < returnIndex
  )
) {
  failures.push('tenant validation must be constructed, reduced to bounded facts, diagnosed, and returned in order');
}

for (const [value, label] of [
  ['log_payment_collection_admission_rejection(', 'admission diagnostics remain mounted'],
  ['context.require_policy(PortCallPolicy::read())', 'read policy remains unchanged'],
  ['context.require_policy(PortCallPolicy::write())', 'write policy remains unchanged'],
  ['context.require_write_semantics()', 'write semantics remain unchanged'],
  ['let error_kind = match &error.kind', 'bounded admission kind remains unchanged'],
]) requireText(admissionBlock, value, label);

for (const [value, label] of [
  ['"create_or_reuse_collection.read_existing"', 'existing lookup operation'],
  ['"create_or_reuse_collection.adopt_race"', 'race adoption operation'],
  ['"create_or_reuse_collection.create"', 'create operation'],
  ['cause = %message', 'canonical validation payload remains separate'],
  ['provider_id = %provider_id', 'canonical provider payload remains separate'],
  ['error = ?error', 'canonical database payload remains separate'],
  ['"payment provider outcome requires reconciliation"', 'stable reconciliation envelope'],
  ['"payment storage is temporarily unavailable"', 'stable storage envelope'],
]) requireText(ownerMapper, value, label);

forbidText(source, 'parse_port_tenant_id(&context)?', 'operation-free tenant parser call');
forbidText(
  tenantParser,
  'Uuid::parse_str(&context.tenant_id).map_err(|_|',
  'silent tenant parse rejection',
);

if (evidence.status !== 'payment_collection_tenant_diagnostic_safety_source_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  tenant_parser_bounded: true,
  complete_parse_error_logged: false,
  constructed_port_error_logged: false,
  tenant_internal_message_text_logged: false,
  tenant_context_shape_only: true,
  tenant_correlation_preserved: true,
  tenant_owner_operations_preserved: true,
  tenant_validation_phase_preserved: true,
  tenant_error_kind_closed: true,
  tenant_warning_severity_preserved: true,
  same_validation_port_error_returned: true,
  tenant_parser_call_sites_preserved: true,
  admission_mapper_changed: false,
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
  ['Tenant diagnostics retain only the parse-error type and bounded context/message-shape facts', 'documentation bounded policy'],
  ['The same constructed validation `PortError` is returned after diagnostics', 'documentation error pass-through'],
  ['Canonical `payment_error_to_port_error` remains the next separate cleanup slice', 'documentation residual boundary'],
]) requireText(doc, value, label);

if (failures.length > 0) {
  console.error('Payment collection tenant diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Payment collection tenant UUID rejection uses type-only parse cause and bounded context/message facts while preserving the exact validation envelope and operation-aware routing',
);
