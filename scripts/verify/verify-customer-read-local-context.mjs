#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const wrapper = read('crates/rustok-customer/src/read_context.rs');
const ports = read('crates/rustok-customer/src/ports.rs');
const lib = read('crates/rustok-customer/src/lib.rs');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [source, value, label] of [
  [lib, 'pub mod ports;', 'legacy customer ports module'],
  [lib, 'mod read_context;', 'private customer context wrapper module'],
  [lib, 'pub use ports::{', 'selective root contract exports'],
  [lib, 'CustomerReadPort,', 'root customer read trait export'],
  [lib, 'pub use read_context::{InProcessCustomerReadPort, in_process_customer_read_port};', 'canonical root wrapper exports'],
  [ports, 'pub fn in_process_customer_read_port(db: DatabaseConnection)', 'legacy module-path factory'],
  [ports, 'Arc::new(crate::CustomerService::new(db))', 'unchanged persistent factory behavior'],
  [wrapper, 'pub struct InProcessCustomerReadPort', 'canonical wrapper type'],
  [wrapper, 'inner: CustomerService', 'unchanged owner implementation delegation'],
  [wrapper, 'pub fn new(db: DatabaseConnection) -> Self', 'default wrapper constructor'],
  [wrapper, 'pub fn from_service(inner: CustomerService) -> Self', 'host-composed service constructor'],
  [wrapper, 'pub fn in_process_customer_read_port(db: DatabaseConnection)', 'canonical wrapper factory'],
  [wrapper, 'Arc::new(InProcessCustomerReadPort::new(db))', 'canonical wrapper construction'],
  [wrapper, 'impl CustomerReadPort for InProcessCustomerReadPort', 'wrapper trait implementation'],
]) {
  requireText(source, value, label);
}

for (const [operation, request, response] of [
  ['read_customer_projection', 'CustomerProjectionRequest', 'CustomerResponse'],
  ['read_customer_projection_by_user', 'CustomerUserProjectionRequest', 'CustomerResponse'],
  ['list_customer_projections', 'CustomerListProjectionRequest', 'CustomerListProjectionResponse'],
  ['list_profile_enrichment', 'CustomerProfileEnrichmentRequest', 'Vec<CustomerProfileEnrichment>'],
]) {
  requireText(wrapper, `async fn ${operation}(`, `${operation} wrapper operation`);
  requireText(wrapper, `request: ${request}`, `${operation} request contract`);
  requireText(wrapper, `Result<${response}, PortError>`, `${operation} response contract`);
  requireText(wrapper, `CustomerReadPort::${operation}(&self.inner, context, request).await`, `${operation} unchanged delegation`);
}

for (const [value, label] of [
  ['let diagnostic_context = context.clone();', 'retained delegated context'],
  ['CustomerReadDiagnosticFacts', 'safe request facts'],
  ['customer_id: Some(request.customer_id)', 'typed customer identity'],
  ['user_id: Some(request.user_id)', 'typed user identity'],
  ['page: Some(request.page)', 'list page fact'],
  ['per_page: Some(request.per_page)', 'list page-size fact'],
  ['.map(|value| value.chars().count())', 'search length only'],
  ['requested_user_count: Some(requested_user_count)', 'profile request count'],
  ['unique_user_count: Some(unique_user_count)', 'profile unique count'],
  ['map_customer_read_local_port_error(', 'post-delegation local mapper'],
  ['owner = CUSTOMER_OWNER', 'truthful customer owner'],
  ['operation = owner_operation', 'exact owner operation'],
  ['local_operation,', 'local operation label'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['customer_id = ?facts.customer_id', 'customer fact logging'],
  ['user_id = ?facts.user_id', 'user fact logging'],
  ['search_length = ?facts.search_length', 'search length logging'],
  ['requested_user_count = ?facts.requested_user_count', 'profile count logging'],
  ['internal_code = %error.code', 'stable internal code'],
  ['internal_message = %error.message', 'public-safe internal message'],
  ['error_kind = ?error.kind', 'typed error kind'],
  ['retryable = error.retryable', 'retryability'],
  ['boundary = CUSTOMER_READ_BOUNDARY', 'customer read boundary'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation', 'technical severity classification'],
  ['"customer read local technical outcome retained delegated context"', 'technical diagnostic event'],
  ['"customer read local outcome retained delegated context"', 'ordinary diagnostic event'],
  ['_ => return error', 'unknown envelope pass-through'],
]) {
  requireText(wrapper, value, label);
}

for (const [code, message, operation] of [
  ['customer.context_invalid', 'customer request context is invalid', 'validate_tenant_context'],
  ['customer.page_invalid', 'customer projection page is invalid', 'validate_page'],
  ['customer.per_page_invalid', 'customer projection page size is invalid', 'validate_page_size'],
  ['customer.database_unavailable', 'customer storage is temporarily unavailable', 'owner_storage'],
  ['customer.customer_not_found', 'customer was not found', 'load_customer'],
  ['customer.customer_by_user_not_found', 'customer was not found for the requested user', 'load_customer_by_user'],
  ['customer.validation', 'customer request is invalid', 'validate_owner_request'],
  ['customer.profile_unavailable', 'customer profile projection is temporarily unavailable', 'load_profile_projection'],
]) {
  requireText(wrapper, `"${code}"`, `${code} exact code`);
  requireText(wrapper, `"${message}"`, `${code} exact message`);
  requireText(wrapper, `"${operation}"`, `${code} local operation`);
}

for (const [value, label] of [
  ['search = ?', 'raw search text'],
  ['search = %', 'raw search text'],
  ['email = ?', 'raw email'],
  ['email = %', 'raw email'],
  ['first_name =', 'raw first name'],
  ['last_name =', 'raw last name'],
  ['preferred_locale =', 'raw preferred locale'],
  ['request = ?', 'raw request payload'],
  ['result = ?', 'raw result payload'],
  ['items = ?', 'raw customer rows'],
]) {
  forbidText(wrapper, value, label);
}

requireText(ports, 'require_customer_read_policy(&context, owner_operation)?;', 'unchanged policy admission');
requireText(ports, 'parse_port_tenant_id(&context, owner_operation)?;', 'unchanged tenant parsing');
requireText(ports, 'validate_customer_list_projection_request(&context, owner_operation, &request)?;', 'unchanged list validation');
requireText(ports, 'customer_error_to_port_error(&context, owner_operation, error)', 'unchanged public error mapping');

if (!/\n\s*error\n\}/.test(wrapper)) {
  failures.push('same delegated PortError return: missing terminal error return');
}

if (failures.length > 0) {
  console.error('Customer read local context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Canonical customer reads retain complete local context and safe request facts without changing owner behavior',
);
