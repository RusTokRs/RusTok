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
  for (let index = openBrace; index >= 0 && index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    if (source[index] === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated function ${functionName}`);
  return '';
}

for (const [value, label] of [
  ['const CUSTOMER_READ_PORT_BOUNDARY: &str = "customer_read_port";', 'stable owner boundary'],
  ['fn require_customer_read_policy(', 'shared read policy helper'],
  ['.require_policy(PortCallPolicy::read())', 'canonical read policy'],
  ['log_customer_read_admission_rejection(context, owner_operation, &error);', 'admission diagnostics'],
  ['fn customer_read_context_facts(', 'bounded context helper'],
  ['fn customer_port_error_kind(', 'closed kind helper'],
  ['fn log_customer_read_admission_rejection(', 'bounded admission logger'],
]) requireText(customer, value, label);

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
    failures.push(`${operation}: owner operation must be assigned before policy admission`);
  }
}
forbidText(
  customer,
  '        context.require_policy(PortCallPolicy::read())?;',
  'direct unlogged customer read policy admission',
);

const contextFacts = functionBody(customer, 'customer_read_context_facts');
const kindHelper = functionBody(customer, 'customer_port_error_kind');
const admissionLogger = functionBody(customer, 'log_customer_read_admission_rejection');
const admissionScope = [contextFacts, kindHelper, admissionLogger].join('\n');

for (const [value, label] of [
  ['tenant_id_length: context.tenant_id.chars().count()', 'tenant length'],
  ['actor_id_length: context.actor.id.chars().count()', 'actor-id length'],
  ['claim_count: context.claims.len()', 'claim count'],
  ['role_count: context.roles.len()', 'role count'],
  ['channel_present: context.channel.is_some()', 'channel presence'],
  ['locale_length: context.locale.chars().count()', 'locale length'],
  ['causation_id_present: context.causation_id.is_some()', 'causation presence'],
  ['traceparent_present: context.traceparent.is_some()', 'trace presence'],
  ['idempotency_key_present: context.idempotency_key.is_some()', 'idempotency presence'],
  ['deadline_ms: context.deadline_ms', 'deadline shape'],
]) requireText(contextFacts, value, label);

for (const [value, label] of [
  ['PortErrorKind::Validation => "validation"', 'validation kind label'],
  ['PortErrorKind::NotFound => "not_found"', 'not-found kind label'],
  ['PortErrorKind::Conflict => "conflict"', 'conflict kind label'],
  ['PortErrorKind::Forbidden => "forbidden"', 'forbidden kind label'],
  ['PortErrorKind::Unavailable => "unavailable"', 'unavailable kind label'],
  ['PortErrorKind::Timeout => "timeout"', 'timeout kind label'],
  ['PortErrorKind::InvariantViolation => "invariant_violation"', 'invariant kind label'],
]) requireText(kindHelper, value, label);

for (const [value, label] of [
  ['owner = "rustok_customer"', 'truthful owner identity'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id_length = context_facts.tenant_id_length', 'tenant shape'],
  ['actor_kind = context_facts.actor_kind', 'actor kind'],
  ['actor_id_length = context_facts.actor_id_length', 'actor-id shape'],
  ['claim_count = context_facts.claim_count', 'claim shape'],
  ['role_count = context_facts.role_count', 'role shape'],
  ['channel_present = context_facts.channel_present', 'channel shape'],
  ['channel_length = ?context_facts.channel_length', 'channel length'],
  ['locale_length = context_facts.locale_length', 'locale shape'],
  ['causation_id_present = context_facts.causation_id_present', 'causation shape'],
  ['traceparent_present = context_facts.traceparent_present', 'trace shape'],
  ['idempotency_key_present = context_facts.idempotency_key_present', 'idempotency shape'],
  ['deadline_ms = ?context_facts.deadline_ms', 'deadline context'],
  ['operation = owner_operation', 'exact owner operation'],
  ['code = %error.code', 'stable public code'],
  ['error_kind = customer_port_error_kind(&error.kind)', 'closed error kind'],
  ['error_message_present = !error.message.is_empty()', 'message presence'],
  ['error_message_length = error.message.chars().count()', 'message length'],
  ['retryable = error.retryable', 'owner retryability'],
  ['boundary = CUSTOMER_READ_PORT_BOUNDARY', 'boundary context'],
  ['"customer read port admission was rejected with bounded diagnostics"', 'bounded event'],
]) requireText(admissionLogger, value, label);

for (const [value, label] of [
  ['error = ?error', 'complete PortError debug payload'],
  ['error = %error', 'complete PortError display payload'],
  ['internal_message = %error.message', 'raw error message'],
  ['message = %error.message', 'raw error message alias'],
  ['error_kind = ?error.kind', 'debug-formatted kind'],
  ['tenant_id = %context.tenant_id', 'raw tenant context'],
  ['actor = ?context.actor', 'raw actor context'],
  ['channel = ?context.channel', 'raw channel context'],
  ['locale = %context.locale', 'raw locale context'],
  ['causation_id = ?context.causation_id', 'raw causation context'],
  ['traceparent = ?context.traceparent', 'raw trace context'],
  ['idempotency_key = ?context.idempotency_key', 'raw idempotency context'],
]) forbidText(admissionScope, value, label);

for (const [value, label] of [
  ['"customer.database_unavailable"', 'database mapping'],
  ['"customer.customer_not_found"', 'customer not-found mapping'],
  ['"customer.customer_by_user_not_found"', 'user not-found mapping'],
  ['"customer.duplicate_email"', 'duplicate email mapping'],
  ['"customer.duplicate_user_link"', 'duplicate user-link mapping'],
  ['"customer.validation"', 'validation mapping'],
  ['"customer.profile_unavailable"', 'profile mapping'],
  ['"customer storage is temporarily unavailable"', 'stable storage message'],
  ['"customer request is invalid"', 'stable validation message'],
]) requireText(customer, value, label);

const graphqlCustomerConstructors =
  commerceQuery.match(/in_process_customer_read_port\(db\.clone\(\)\)/g) ?? [];
if (graphqlCustomerConstructors.length !== 3) {
  failures.push(
    `expected three unchanged commerce GraphQL customer read constructors, found ${graphqlCustomerConstructors.length}`,
  );
}
for (const [value, label] of [
  ['"customer.customer_by_user_not_found" => {', 'storefront unauthenticated not-found branch'],
  [
    'Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None)',
    'optional customer not-found branch',
  ],
  ['async_graphql::Error::new(error.message)', 'existing GraphQL fallback mapping'],
]) requireText(commerceQuery, value, label);

for (const marker of [
  '# Customer read port policy context',
  'Status: `source_ready_unvalidated`',
  'bounded context shape',
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
  '✔ Customer read policy admission retains bounded context/error shape and returns the original PortError unchanged',
);
