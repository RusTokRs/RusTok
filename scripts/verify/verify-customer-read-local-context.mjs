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

for (const [source, value, label] of [
  [lib, 'pub mod ports;', 'legacy customer ports module'],
  [lib, 'mod read_context;', 'private customer context wrapper module'],
  [lib, 'pub use ports::{', 'selective root contract exports'],
  [lib, 'CustomerReadPort,', 'root customer read trait export'],
  [
    lib,
    'pub use read_context::{InProcessCustomerReadPort, in_process_customer_read_port};',
    'canonical root wrapper exports',
  ],
  [ports, 'pub fn in_process_customer_read_port(db: DatabaseConnection)', 'legacy module factory'],
  [ports, 'Arc::new(crate::CustomerService::new(db))', 'legacy owner construction'],
  [wrapper, 'pub struct InProcessCustomerReadPort', 'canonical wrapper type'],
  [wrapper, 'inner: CustomerService', 'owner implementation delegation'],
  [wrapper, 'pub fn from_service(inner: CustomerService) -> Self', 'host-composed constructor'],
  [wrapper, 'Arc::new(InProcessCustomerReadPort::new(db))', 'canonical wrapper construction'],
  [wrapper, 'impl CustomerReadPort for InProcessCustomerReadPort', 'wrapper trait implementation'],
]) requireText(source, value, label);

for (const [operation, request, response] of [
  ['read_customer_projection', 'CustomerProjectionRequest', 'CustomerResponse'],
  ['read_customer_projection_by_user', 'CustomerUserProjectionRequest', 'CustomerResponse'],
  ['list_customer_projections', 'CustomerListProjectionRequest', 'CustomerListProjectionResponse'],
  ['list_profile_enrichment', 'CustomerProfileEnrichmentRequest', 'Vec<CustomerProfileEnrichment>'],
]) {
  requireText(wrapper, `async fn ${operation}(`, `${operation} wrapper operation`);
  requireText(wrapper, `request: ${request}`, `${operation} request contract`);
  requireText(wrapper, `Result<${response}, PortError>`, `${operation} response contract`);
  requireText(
    wrapper,
    `CustomerReadPort::${operation}(&self.inner, context, request).await`,
    `${operation} unchanged delegation`,
  );
}

for (const [value, label] of [
  ['struct CustomerReadContextFacts', 'bounded context fact type'],
  ['struct CustomerReadDiagnosticFacts', 'bounded request fact type'],
  ['fn customer_read_context_facts(', 'bounded context helper'],
  ['fn customer_read_port_error_kind(', 'closed error-kind helper'],
  ['fn log_customer_read_local_outcome(', 'bounded local logger'],
  ['CustomerReadDiagnosticFacts::customer(request.customer_id)', 'customer-id shape capture'],
  ['CustomerReadDiagnosticFacts::user(request.user_id)', 'user-id shape capture'],
  ['CustomerReadDiagnosticFacts::list(&request)', 'list shape capture'],
  ['CustomerReadDiagnosticFacts::enrichment(&request)', 'enrichment shape capture'],
  ['customer_id_non_nil: !customer_id.is_nil()', 'customer UUID shape'],
  ['user_id_non_nil: !user_id.is_nil()', 'user UUID shape'],
  ['page_nonzero: request.page != 0', 'page zero shape'],
  ['per_page_nonzero: request.per_page != 0', 'page-size zero shape'],
  ['search_present: request.search.is_some()', 'search presence shape'],
  ['search_length: request.search.as_ref().map(|value| value.chars().count())', 'search length'],
  ['requested_user_ids_empty: request.user_ids.is_empty()', 'enrichment emptiness shape'],
  ['duplicate_user_ids_present: unique_user_count < requested_user_count', 'duplicate shape'],
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
  ['PortErrorKind::Validation => "validation"', 'validation kind label'],
  ['PortErrorKind::NotFound => "not_found"', 'not-found kind label'],
  ['PortErrorKind::Conflict => "conflict"', 'conflict kind label'],
  ['PortErrorKind::Forbidden => "forbidden"', 'forbidden kind label'],
  ['PortErrorKind::Unavailable => "unavailable"', 'unavailable kind label'],
  ['PortErrorKind::Timeout => "timeout"', 'timeout kind label'],
  ['PortErrorKind::InvariantViolation => "invariant_violation"', 'invariant kind label'],
]) requireText(wrapper, value, label);

const mapper = functionBody(wrapper, 'map_customer_read_local_port_error');
const logger = functionBody(wrapper, 'log_customer_read_local_outcome');
const diagnosticScope = [
  functionBody(wrapper, 'customer_read_context_facts'),
  functionBody(wrapper, 'customer_read_port_error_kind'),
  logger,
].join('\n');

for (const [value, label] of [
  ['owner = CUSTOMER_OWNER', 'truthful owner'],
  ['operation = owner_operation', 'exact owner operation'],
  ['local_operation,', 'local operation label'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id_length = context_facts.tenant_id_length', 'tenant shape'],
  ['actor_kind = context_facts.actor_kind', 'actor kind'],
  ['actor_id_length = context_facts.actor_id_length', 'actor-id shape'],
  ['claim_count = context_facts.claim_count', 'claim shape'],
  ['role_count = context_facts.role_count', 'role shape'],
  ['channel_present = context_facts.channel_present', 'channel shape'],
  ['locale_length = context_facts.locale_length', 'locale shape'],
  ['customer_id_present = request_facts.customer_id_present', 'customer presence'],
  ['customer_id_non_nil = request_facts.customer_id_non_nil', 'customer non-nil'],
  ['user_id_present = request_facts.user_id_present', 'user presence'],
  ['user_id_non_nil = request_facts.user_id_non_nil', 'user non-nil'],
  ['page_present = request_facts.page_present', 'page presence'],
  ['page_nonzero = request_facts.page_nonzero', 'page shape'],
  ['per_page_present = request_facts.per_page_present', 'page-size presence'],
  ['per_page_nonzero = request_facts.per_page_nonzero', 'page-size shape'],
  ['search_present = request_facts.search_present', 'search presence'],
  ['search_length = ?request_facts.search_length', 'search length logging'],
  ['requested_user_ids_empty = request_facts.requested_user_ids_empty', 'enrichment empty shape'],
  [
    'duplicate_user_ids_present = request_facts.duplicate_user_ids_present',
    'enrichment duplicate shape',
  ],
  ['code = %error.code', 'stable code'],
  ['error_message_present = !error.message.is_empty()', 'message presence'],
  ['error_message_length = error.message.chars().count()', 'message length'],
  ['error_kind = customer_read_port_error_kind(&error.kind)', 'closed error kind'],
  ['retryable = error.retryable', 'retryability'],
  ['boundary = CUSTOMER_READ_BOUNDARY', 'customer read boundary'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'technical severity classification',
  ],
  [
    '"customer read local technical outcome retained bounded delegated context"',
    'technical event',
  ],
  ['"customer read local outcome retained bounded delegated context"', 'ordinary event'],
]) requireText(logger, value, label);

for (const [code, message, operation] of [
  ['customer.context_invalid', 'customer request context is invalid', 'validate_tenant_context'],
  ['customer.page_invalid', 'customer projection page is invalid', 'validate_page'],
  ['customer.per_page_invalid', 'customer projection page size is invalid', 'validate_page_size'],
  ['customer.database_unavailable', 'customer storage is temporarily unavailable', 'owner_storage'],
  ['customer.customer_not_found', 'customer was not found', 'load_customer'],
  [
    'customer.customer_by_user_not_found',
    'customer was not found for the requested user',
    'load_customer_by_user',
  ],
  ['customer.validation', 'customer request is invalid', 'validate_owner_request'],
  [
    'customer.profile_unavailable',
    'customer profile projection is temporarily unavailable',
    'load_profile_projection',
  ],
]) {
  requireText(mapper, `"${code}"`, `${code} exact code`);
  requireText(mapper, `"${message}"`, `${code} exact message`);
  requireText(mapper, `"${operation}"`, `${code} local operation`);
}
requireText(mapper, '_ => return error', 'unknown envelope pass-through');
requireText(mapper, 'log_customer_read_local_outcome(', 'bounded logger call');
if (!/\n\s*error\n\}/.test(mapper)) {
  failures.push('same delegated PortError return: missing terminal error return');
}

for (const [value, label] of [
  ['error = ?error', 'complete PortError debug payload'],
  ['error = %error', 'complete PortError display payload'],
  ['internal_message = %error.message', 'raw PortError message'],
  ['message = %error.message', 'raw PortError message alias'],
  ['error_kind = ?error.kind', 'debug-formatted error kind'],
  ['tenant_id = %context.tenant_id', 'raw tenant context'],
  ['actor = ?context.actor', 'raw actor context'],
  ['channel = ?context.channel', 'raw channel context'],
  ['locale = %context.locale', 'raw locale context'],
  ['causation_id = ?context.causation_id', 'raw causation context'],
  ['traceparent = ?context.traceparent', 'raw trace context'],
  ['idempotency_key = ?context.idempotency_key', 'raw idempotency context'],
  ['customer_id = ?request_facts.customer_id', 'raw customer UUID'],
  ['user_id = ?request_facts.user_id', 'raw user UUID'],
  ['page = ?request_facts.page', 'exact page'],
  ['per_page = ?request_facts.per_page', 'exact page size'],
  ['requested_user_count =', 'exact requested user count'],
  ['unique_user_count =', 'exact unique user count'],
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
]) forbidText(diagnosticScope, value, `${paths.wrapper}: ${label}`);

for (const marker of [
  'require_customer_read_policy(&context, owner_operation)?;',
  'parse_port_tenant_id(&context, owner_operation)?;',
  'validate_customer_list_projection_request(&context, owner_operation, &request)?;',
  'customer_error_to_port_error(&context, owner_operation, error)',
]) requireText(ports, marker, `${paths.ports}: unchanged owner behavior`);

for (const [key, expected] of Object.entries({
  owner_policy_diagnostics_bounded: true,
  canonical_wrapper_diagnostics_bounded: true,
  complete_port_error_logged: false,
  raw_context_logged: false,
  raw_request_uuid_logged: false,
  exact_pagination_logged: false,
  exact_profile_counts_logged: false,
  raw_search_logged: false,
  bounded_context_shape_logged: true,
  bounded_request_shape_logged: true,
  stable_code_logged: true,
  error_message_shape_logged: true,
  closed_error_kind_logged: true,
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
  raw_customer_and_user_ids_removed: true,
  exact_pagination_and_counts_removed: true,
  bounded_context_shape_retained: true,
  bounded_request_shape_retained: true,
  same_delegated_error_returned: true,
  owner_policy_source_already_bounded: true,
  stale_policy_verifier_corrected: true,
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
  'bounded context shape',
  'UUID presence and non-nil status',
  'exact stable `operation + code + message`',
  'The same delegated `PortError` is returned unchanged.',
  'The broader ecommerce correlation-safe mapper cleanup remains open.',
]) requireText(document, marker, `${paths.document}: truthful source scope`);
for (const marker of [
  '# Customer read port policy context',
  'bounded context shape',
  'The original admission `PortError` is returned unchanged.',
  'complete `PortError` is not copied into the event',
]) requireText(policyDocument, marker, `${paths.policyDocument}: bounded policy scope`);

if (failures.length > 0) {
  console.error('Customer read local diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Canonical customer reads retain bounded context/request/error shape and unchanged owner behavior; execution evidence remains open',
);
