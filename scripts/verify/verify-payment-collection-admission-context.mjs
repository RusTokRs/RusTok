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
const ownerMapper = between(
  source,
  'fn payment_error_to_port_error(',
  '\n}',
  'payment owner mapper',
);

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

for (const [value, label] of [
  ['context.require_policy(PortCallPolicy::read())', 'read policy'],
  ['log_payment_collection_admission_rejection(context, owner_operation, "policy", &error);', 'read policy diagnostics'],
]) requireText(readAdmission, value, label);
forbidText(readAdmission, 'require_write_semantics', 'read admission must remain non-write');

for (const [value, label] of [
  ['context.require_policy(PortCallPolicy::write())', 'write policy'],
  ['log_payment_collection_admission_rejection(context, owner_operation, "policy", &error);', 'write policy diagnostics'],
  ['context.require_write_semantics()', 'write semantics'],
  ['"write_semantics"', 'write semantics diagnostic classification'],
]) requireText(writeAdmission, value, label);

for (const [value, label] of [
  ['context: &PortContext', 'retained context input'],
  ["owner_operation: &'static str", 'exact owner operation input'],
  ["admission: &'static str", 'admission phase input'],
  ['error: &PortError', 'original port error input'],
  ['error = ?error', 'original error'],
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
  ['operation = owner_operation', 'owner operation field'],
  ['admission,', 'admission phase field'],
  ['code = %error.code', 'error code'],
  ['internal_message = %error.message', 'error message'],
  ['error_kind = ?error.kind', 'typed error kind'],
  ['retryable = error.retryable', 'retryability'],
  ['boundary = PAYMENT_COLLECTION_PORT_BOUNDARY', 'boundary field'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation', 'technical severity classification'],
  ['tracing::error!(', 'technical severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['"payment collection admission failed"', 'technical event'],
  ['"payment collection admission was rejected"', 'rejection event'],
]) requireText(admissionLogger, value, label);

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

for (const [value, label] of [
  ['PortError::validation("payment.validation", "payment request is invalid")', 'stable validation envelope'],
  ['PortError::unavailable(', 'stable unavailable envelope'],
  ['"payment provider outcome requires reconciliation"', 'stable reconciliation envelope'],
  ['"payment storage is temporarily unavailable"', 'stable storage envelope'],
]) requireText(source, value, label);

forbidText(createBlock, 'context.require_policy(', 'direct write policy admission');
forbidText(createBlock, 'context.require_write_semantics()', 'direct write semantics admission');
forbidText(readBlock, 'context.require_policy(', 'direct read policy admission');
forbidText(source, 'context.require_policy(PortCallPolicy::write())?;', 'context-dropping write policy rejection');
forbidText(source, 'context.require_write_semantics()?;', 'context-dropping write semantics rejection');
forbidText(source, 'context.require_policy(PortCallPolicy::read())?;', 'context-dropping read policy rejection');
forbidText(source, 'parse_port_tenant_id(&context)?', 'operation-free tenant parsing');

if (failures.length > 0) {
  console.error('Payment collection admission context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Payment collection read/write admission rejections retain truthful owner context without changing collection lifecycle or public PortError behavior',
);
