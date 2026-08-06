#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const paths = {
  wrapper: 'crates/rustok-customer/src/read_context.rs',
  ports: 'crates/rustok-customer/src/ports.rs',
  lib: 'crates/rustok-customer/src/lib.rs',
  evidence:
    'crates/rustok-customer/contracts/evidence/customer-read-diagnostic-safety-source.json',
  review:
    'crates/rustok-customer/contracts/evidence/customer-read-diagnostic-safety-source-review.json',
  document: 'crates/rustok-customer/docs/read-local-context.md',
  policyDocument: 'crates/rustok-customer/docs/read-port-policy-context.md',
};

const wrapper = read(paths.wrapper);
const ports = read(paths.ports);
const lib = read(paths.lib);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const document = read(paths.document);
const policyDocument = read(paths.policyDocument);

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

for (const [source, marker, label] of [
  [lib, 'pub mod ports;', 'legacy customer ports module'],
  [lib, 'mod read_context;', 'private wrapper module'],
  [lib, 'pub use read_context::{InProcessCustomerReadPort, in_process_customer_read_port};', 'canonical exports'],
  [ports, 'pub fn in_process_customer_read_port(db: DatabaseConnection)', 'compatibility factory'],
  [wrapper, 'pub struct InProcessCustomerReadPort', 'canonical wrapper type'],
  [wrapper, 'inner: CustomerService', 'owner implementation delegation'],
  [wrapper, 'Arc::new(InProcessCustomerReadPort::new(db))', 'canonical construction'],
  [wrapper, 'impl CustomerReadPort for InProcessCustomerReadPort', 'wrapper trait implementation'],
]) requireText(source, marker, label);

for (const [operation, request, response] of [
  ['read_customer_projection', 'CustomerProjectionRequest', 'CustomerResponse'],
  ['read_customer_projection_by_user', 'CustomerUserProjectionRequest', 'CustomerResponse'],
  ['list_customer_projections', 'CustomerListProjectionRequest', 'CustomerListProjectionResponse'],
  ['list_profile_enrichment', 'CustomerProfileEnrichmentRequest', 'Vec<CustomerProfileEnrichment>'],
]) {
  requireText(wrapper, `async fn ${operation}(`, `${operation} operation`);
  requireText(wrapper, `request: ${request}`, `${operation} request`);
  requireText(wrapper, `Result<${response}, PortError>`, `${operation} response`);
  requireText(
    wrapper,
    `CustomerReadPort::${operation}(&self.inner, context, request).await`,
    `${operation} unchanged delegation`,
  );
}

const contextFacts = functionBody(wrapper, 'customer_read_context_facts');
const mapper = functionBody(wrapper, 'map_customer_read_local_port_error');
const logger = functionBody(wrapper, 'log_customer_read_local_outcome');
const diagnosticScope = [contextFacts, logger].join('\n');

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
]) requireText(contextFacts, marker, `wrapper context: ${marker}`);

for (const marker of [
  'CustomerReadDiagnosticFacts::customer(request.customer_id)',
  'CustomerReadDiagnosticFacts::user(request.user_id)',
  'CustomerReadDiagnosticFacts::list(&request)',
  'CustomerReadDiagnosticFacts::enrichment(&request)',
  'customer_id_non_nil: !customer_id.is_nil()',
  'user_id_non_nil: !user_id.is_nil()',
  'page_nonzero: request.page != 0',
  'per_page_nonzero: request.per_page != 0',
  'search_present: request.search.is_some()',
  'requested_user_ids_empty: request.user_ids.is_empty()',
  'duplicate_user_ids_present: unique_user_count < requested_user_count',
]) requireText(wrapper, marker, `bounded request facts: ${marker}`);

for (const marker of [
  'owner = CUSTOMER_OWNER',
  'operation = owner_operation',
  'local_operation,',
  'correlation_id_length = context_facts.correlation_id_length',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'customer_id_present = request_facts.customer_id_present',
  'user_id_present = request_facts.user_id_present',
  'page_nonzero = request_facts.page_nonzero',
  'per_page_nonzero = request_facts.per_page_nonzero',
  'search_length = ?request_facts.search_length',
  'duplicate_user_ids_present = request_facts.duplicate_user_ids_present',
  'code = %error.code',
  'error_message_present = !error.message.is_empty()',
  'error_message_length = error.message.chars().count()',
  'error_kind = customer_read_port_error_kind(&error.kind)',
  'retryable = error.retryable',
  'boundary = CUSTOMER_READ_BOUNDARY',
  'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
  '"customer read local technical outcome retained bounded delegated context"',
  '"customer read local outcome retained bounded delegated context"',
]) requireText(logger, marker, `bounded local logger: ${marker}`);

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
  requireText(mapper, `"${code}"`, `${code} code`);
  requireText(mapper, `"${message}"`, `${code} message`);
  requireText(mapper, `"${operation}"`, `${code} local operation`);
}
requireText(mapper, '_ => return error', 'unknown envelope pass-through');
requireText(mapper, 'log_customer_read_local_outcome(', 'bounded logger call');
if (!/\n\s*error\n\}/.test(mapper)) {
  failures.push('same delegated PortError return: missing terminal error return');
}

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
  'customer_id = ?',
  'user_id = ?',
  'page = ?request_facts.page',
  'per_page = ?request_facts.per_page',
  'search = ?',
  'search = %',
  'request = ?',
  'result = ?',
  'items = ?',
]) forbidText(diagnosticScope, forbidden, `wrapper payload: ${forbidden}`);

for (const marker of [
  'require_customer_read_policy(&context, owner_operation)?;',
  'parse_port_tenant_id(&context, owner_operation)?;',
  'validate_customer_list_projection_request(&context, owner_operation, &request)?;',
  'customer_error_to_port_error(&context, owner_operation, error)',
]) requireText(ports, marker, `unchanged owner behavior: ${marker}`);

for (const [key, expected] of Object.entries({
  owner_policy_diagnostics_bounded: true,
  canonical_wrapper_diagnostics_bounded: true,
  complete_port_error_logged: false,
  raw_context_logged: false,
  raw_request_uuid_logged: false,
  exact_local_classification_changed: false,
  delegated_error_return_changed: false,
  public_contract_changed: false,
  owner_delegation_changed: false,
  customer_ffa_fba_status_promoted: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  'tests_run',
  'verifiers_run',
  'cargo_run',
  'format_run',
  'workflow_checks_run',
  'ci_run',
  'compile_proven',
  'mounted_runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push(`${paths.evidence}: execution must remain empty`);
}

for (const [key, expected] of Object.entries({
  all_four_read_operations_preserved: true,
  canonical_root_factory_preserved: true,
  compatibility_factory_preserved: true,
  exact_owner_delegation_preserved: true,
  exact_local_classification_preserved: true,
  unknown_error_pass_through_preserved: true,
  complete_wrapper_error_removed: true,
  raw_wrapper_context_removed: true,
  bounded_context_shape_retained: true,
  bounded_request_shape_retained: true,
  same_delegated_error_returned: true,
  customer_status_not_promoted: true,
  broad_ecommerce_cleanup_remains_open: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`${paths.review}: review_findings.${key} must be ${expected}`);
  }
}

for (const marker of [
  '# Customer read local diagnostic safety',
  'Status: **source-ready / unvalidated**',
  'correlation-ID character length',
  'raw correlation IDs',
  'The same delegated `PortError` is returned unchanged.',
  'The broader ecommerce correlation-safe mapper cleanup remains open.',
]) requireText(document, marker, `${paths.document}: truthful source scope`);
for (const marker of [
  '# Customer read port policy context',
  'correlation-ID character length',
  'raw correlation ID',
  'The original admission `PortError` is returned unchanged.',
]) requireText(policyDocument, marker, `${paths.policyDocument}: policy scope`);

if (failures.length > 0) {
  console.error('Customer read local diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}
console.log(
  '✔ Canonical customer reads retain correlation length and bounded context/request/error shape without raw correlation payloads; execution evidence remains open',
);
