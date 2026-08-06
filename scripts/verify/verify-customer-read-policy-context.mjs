#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const customer = read('crates/rustok-customer/src/ports.rs');
const commerceQuery = read('crates/rustok-commerce/src/graphql/query.rs');
const document = read('crates/rustok-customer/docs/read-port-policy-context.md');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
function functionBody(source, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(source);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return '';
  }
  const openBrace = source.indexOf('{', match.index);
  let depth = 0;
  for (let index = openBrace; index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    if (source[index] === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated function ${functionName}`);
  return '';
}

for (const marker of [
  'const CUSTOMER_READ_PORT_BOUNDARY: &str = "customer_read_port";',
  'fn require_customer_read_policy(',
  '.require_policy(PortCallPolicy::read())',
  'log_customer_read_admission_rejection(context, owner_operation, &error);',
  'fn customer_read_context_facts(',
  'fn customer_port_error_kind(',
  'fn log_customer_read_admission_rejection(',
]) requireText(customer, marker, `customer policy: ${marker}`);

const policyCalls = customer.match(/require_customer_read_policy\(&context, owner_operation\)\?;/g) ?? [];
if (policyCalls.length !== 4) {
  failures.push(`expected four customer read policy calls, found ${policyCalls.length}`);
}
for (const operation of [
  'read_customer_projection',
  'read_customer_projection_by_user',
  'list_customer_projections',
  'list_profile_enrichment',
]) {
  const assignment = `let owner_operation = "${operation}";`;
  const assignmentIndex = customer.indexOf(assignment);
  const policyIndex = customer.indexOf(
    'require_customer_read_policy(&context, owner_operation)?;',
    assignmentIndex,
  );
  if (assignmentIndex < 0 || policyIndex < assignmentIndex) {
    failures.push(`${operation}: operation must be assigned before policy admission`);
  }
}

const contextFacts = functionBody(customer, 'customer_read_context_facts');
const kindHelper = functionBody(customer, 'customer_port_error_kind');
const admissionLogger = functionBody(customer, 'log_customer_read_admission_rejection');
const admissionScope = [contextFacts, kindHelper, admissionLogger].join('\n');

for (const marker of [
  'correlation_id_length: context.correlation_id.chars().count()',
  'tenant_id_length: context.tenant_id.chars().count()',
  'actor_id_length: context.actor.id.chars().count()',
  'claim_count: context.claims.len()',
  'role_count: context.roles.len()',
  'channel_present: context.channel.is_some()',
  'locale_length: context.locale.chars().count()',
  'causation_id_present: context.causation_id.is_some()',
  'traceparent_present: context.traceparent.is_some()',
  'idempotency_key_present: context.idempotency_key.is_some()',
  'deadline_ms: context.deadline_ms',
]) requireText(contextFacts, marker, `bounded context: ${marker}`);
for (const marker of [
  'PortErrorKind::Validation => "validation"',
  'PortErrorKind::NotFound => "not_found"',
  'PortErrorKind::Conflict => "conflict"',
  'PortErrorKind::Forbidden => "forbidden"',
  'PortErrorKind::Unavailable => "unavailable"',
  'PortErrorKind::Timeout => "timeout"',
  'PortErrorKind::InvariantViolation => "invariant_violation"',
]) requireText(kindHelper, marker, `closed kind: ${marker}`);
for (const marker of [
  'owner = "rustok_customer"',
  'correlation_id_length = context_facts.correlation_id_length',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'operation = owner_operation',
  'code = %error.code',
  'error_kind = customer_port_error_kind(&error.kind)',
  'error_message_present = !error.message.is_empty()',
  'error_message_length = error.message.chars().count()',
  'retryable = error.retryable',
  'boundary = CUSTOMER_READ_PORT_BOUNDARY',
  '"customer read port admission was rejected with bounded diagnostics"',
]) requireText(admissionLogger, marker, `admission logger: ${marker}`);

for (const forbidden of [
  'correlation_id = %context.correlation_id',
  'error = ?error',
  'error = %error',
  'message = %error.message',
  'error_kind = ?error.kind',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
]) forbidText(admissionScope, forbidden, `admission payload: ${forbidden}`);

const constructors = commerceQuery.match(/in_process_customer_read_port\(db\.clone\(\)\)/g) ?? [];
if (constructors.length !== 3) {
  failures.push(`expected three unchanged Commerce customer constructors, found ${constructors.length}`);
}
for (const marker of [
  '"customer.customer_by_user_not_found" => {',
  'Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None)',
  'async_graphql::Error::new(error.message)',
]) requireText(commerceQuery, marker, `unchanged compatibility source: ${marker}`);

for (const marker of [
  '# Customer read port policy context',
  'Status: `source_ready_unvalidated`',
  'correlation-ID character length',
  'raw correlation ID',
  'closed seven-value error-kind label',
  'complete `PortError` is not copied into the event',
  'The original admission `PortError` is returned unchanged.',
]) requireText(document, marker, 'policy documentation');

if (failures.length > 0) {
  console.error('Customer read policy diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}
console.log(
  '✔ Customer read policy admission retains correlation length and bounded error shape while returning the original PortError unchanged',
);
