#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-payment/src/ports.rs', root),
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
  ['Uuid::parse_str(&context.tenant_id)', 'tenant UUID parsing'],
  ['map_err(|parse_error|', 'parse failure mapping'],
  ['let error = PortError::validation(', 'stable validation construction'],
  ['"payment.tenant_id_invalid"', 'stable validation code'],
  ['"PortContext.tenant_id must be a UUID for payment ports"', 'stable validation message'],
  ['parse_error = ?parse_error', 'internal parse cause'],
  ['error = ?error', 'mapped public error'],
  ['owner = PAYMENT_COLLECTION_OWNER', 'truthful owner field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['operation = owner_operation', 'exact operation field'],
  ['validation = "tenant_id"', 'validation phase'],
  ['code = %error.code', 'mapped code'],
  ['internal_message = %error.message', 'mapped message'],
  ['error_kind = ?error.kind', 'typed error kind'],
  ['retryable = error.retryable', 'retryability'],
  ['boundary = PAYMENT_COLLECTION_PORT_BOUNDARY', 'boundary field'],
  ['tracing::warn!(', 'validation rejection severity'],
  ['"payment collection tenant context was rejected"', 'tenant rejection event'],
]) requireText(tenantParser, value, label);

const validationIndex = tenantParser.indexOf('let error = PortError::validation(');
const diagnosticsIndex = tenantParser.indexOf('tracing::warn!(');
const returnIndex = tenantParser.lastIndexOf('\n        error');
if (!(validationIndex >= 0 && validationIndex < diagnosticsIndex && diagnosticsIndex < returnIndex)) {
  failures.push('tenant validation must be constructed, diagnosed, and then returned in order');
}

for (const [value, label] of [
  ['log_payment_collection_admission_rejection(', 'admission diagnostics remain mounted'],
  ['context.require_policy(PortCallPolicy::read())', 'read policy remains unchanged'],
  ['context.require_policy(PortCallPolicy::write())', 'write policy remains unchanged'],
  ['context.require_write_semantics()', 'write semantics remain unchanged'],
]) requireText(admissionBlock, value, label);

for (const [value, label] of [
  ['"create_or_reuse_collection.read_existing"', 'existing lookup operation'],
  ['"create_or_reuse_collection.adopt_race"', 'race adoption operation'],
  ['"create_or_reuse_collection.create"', 'create operation'],
  ['"payment provider outcome requires reconciliation"', 'stable reconciliation envelope'],
  ['"payment storage is temporarily unavailable"', 'stable storage envelope'],
]) requireText(ownerMapper, value, label);

forbidText(source, 'parse_port_tenant_id(&context)?', 'operation-free tenant parser call');
forbidText(
  tenantParser,
  'Uuid::parse_str(&context.tenant_id).map_err(|_|',
  'silent tenant parse rejection',
);
forbidText(
  tenantParser,
  `PortError::validation(
            "payment.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for payment ports",
        )
    })`,
  'undiagnosed tenant validation return',
);

if (failures.length > 0) {
  console.error('Payment collection tenant context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Payment collection tenant UUID rejection retains exact owner context without changing admission, lifecycle, or public PortError behavior',
);
