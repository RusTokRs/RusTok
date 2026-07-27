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
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['const CUSTOMER_READ_PORT_BOUNDARY: &str = "customer_read_port";', 'stable owner boundary'],
  ['fn require_customer_read_policy(', 'shared read policy helper'],
  ['.require_policy(PortCallPolicy::read())', 'canonical read policy'],
  ['owner = "rustok_customer"', 'truthful owner identity'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['operation = owner_operation', 'exact owner operation'],
  ['code = %error.code', 'stable public code'],
  ['error_kind = ?error.kind', 'typed error kind'],
  ['retryable = error.retryable', 'owner retryability'],
  ['boundary = CUSTOMER_READ_PORT_BOUNDARY', 'boundary context'],
  ['"customer read port admission was rejected"', 'stable diagnostic event'],
]) {
  requireText(customer, value, label);
}

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
]) {
  requireText(customer, value, label);
}

const graphqlCustomerConstructors =
  commerceQuery.match(/in_process_customer_read_port\(db\.clone\(\)\)/g) ?? [];
if (graphqlCustomerConstructors.length !== 3) {
  failures.push(
    `expected three unchanged commerce GraphQL customer read constructors, found ${graphqlCustomerConstructors.length}`,
  );
}
for (const [value, label] of [
  ['"customer.customer_by_user_not_found" => {', 'storefront unauthenticated not-found branch'],
  ['Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None)', 'optional customer not-found branch'],
  ['async_graphql::Error::new(error.message)', 'existing GraphQL fallback mapping'],
]) {
  requireText(commerceQuery, value, label);
}

if (failures.length > 0) {
  console.error('Customer read policy context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Customer read owner operations retain complete PortContext when read policy admission is rejected',
);
