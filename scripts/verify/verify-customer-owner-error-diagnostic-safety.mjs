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
  ports: 'crates/rustok-customer/src/ports.rs',
  error: 'crates/rustok-customer/src/error.rs',
  evidence:
    'crates/rustok-customer/contracts/evidence/customer-owner-error-diagnostic-safety-source.json',
  review:
    'crates/rustok-customer/contracts/evidence/customer-owner-error-diagnostic-safety-source-review.json',
  document: 'crates/rustok-customer/docs/customer-owner-error-diagnostic-safety.md',
  broad: 'scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs',
};

const ports = read(paths.ports);
const errorSource = read(paths.error);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const document = read(paths.document);
const broad = read(paths.broad);

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

for (const marker of [
  'pub trait CustomerReadPort: Send + Sync',
  'async fn read_customer_projection(',
  'async fn read_customer_projection_by_user(',
  'async fn list_customer_projections(',
  'async fn list_profile_enrichment(',
  'pub fn in_process_customer_read_port(',
  'self.get_customer(tenant_id, request.customer_id)',
  'self.get_customer_by_user(tenant_id, request.user_id)',
  'self.list_customers(',
  'crate::CustomerService::list_profile_enrichment(self, tenant_id, &request.user_ids)',
  'require_customer_read_policy(&context, owner_operation)?;',
  'validate_customer_list_projection_request(&context, owner_operation, &request)?;',
  'let tenant_id = parse_port_tenant_id(&context, owner_operation)?;',
  'customer_error_to_port_error(&context, owner_operation, error)',
]) requireText(ports, marker, `${paths.ports}: preserved read contract`);

for (const marker of [
  'Validation(String)',
  'CustomerNotFound(Uuid)',
  'CustomerByUserNotFound(Uuid)',
  'DuplicateEmail(String)',
  'DuplicateUserLink(Uuid)',
  'Profile(#[from] rustok_profiles::ProfileError)',
  'Database(#[from] DbErr)',
]) requireText(errorSource, marker, `${paths.error}: owner error shape`);

const contextFacts = functionBody(ports, 'customer_read_context_facts');
const listFacts = functionBody(ports, 'customer_list_request_facts');
const kindMapper = functionBody(ports, 'customer_port_error_kind');
const admission = functionBody(ports, 'require_customer_read_policy');
const admissionLogger = functionBody(ports, 'log_customer_read_admission_rejection');
const listValidation = functionBody(ports, 'validate_customer_list_projection_request');
const listLogger = functionBody(ports, 'log_customer_list_validation_rejection');
const tenantParser = functionBody(ports, 'parse_port_tenant_id');
const tenantLogger = functionBody(ports, 'log_customer_tenant_parse_rejection');
const errorFacts = functionBody(ports, 'customer_owner_error_facts');
const ownerLogger = functionBody(ports, 'log_customer_owner_failure');
const mapper = functionBody(ports, 'customer_error_to_port_error');
const diagnosticScope = [
  contextFacts,
  listFacts,
  kindMapper,
  admission,
  admissionLogger,
  listValidation,
  listLogger,
  tenantParser,
  tenantLogger,
  errorFacts,
  ownerLogger,
  mapper,
].join('\n');

for (const marker of [
  'struct CustomerReadContextFacts',
  'tenant_id_length: context.tenant_id.chars().count()',
  'actor_kind',
  'actor_id_length: context.actor.id.chars().count()',
  'claim_count: context.claims.len()',
  'role_count: context.roles.len()',
  'channel_present: context.channel.is_some()',
  'locale_length: context.locale.chars().count()',
  'causation_id_present: context.causation_id.is_some()',
  'traceparent_present: context.traceparent.is_some()',
  'idempotency_key_present: context.idempotency_key.is_some()',
  'deadline_ms: context.deadline_ms',
  'struct CustomerListRequestFacts',
  'search_present: request.search.is_some()',
  'search_length: request.search.as_ref().map(|value| value.chars().count())',
  'page: request.page',
  'per_page: request.per_page',
  'struct CustomerOwnerErrorFacts',
  'CustomerError::Validation(message)',
  '("validation", 1, message.chars().count(), 0, 0, false)',
  'CustomerError::CustomerNotFound(customer_id)',
  '"customer_not_found"',
  'CustomerError::CustomerByUserNotFound(user_id)',
  '"customer_by_user_not_found"',
  'CustomerError::DuplicateEmail(email)',
  '("duplicate_email", 1, email.chars().count(), 0, 0, false)',
  'CustomerError::DuplicateUserLink(user_id)',
  '"duplicate_user_link"',
  'CustomerError::Profile(_) => ("profile", 0, 0, 0, 0, true)',
  'CustomerError::Database(_) => ("database", 0, 0, 0, 0, true)',
]) requireText(ports, marker, `${paths.ports}: bounded diagnostic facts`);

for (const marker of [
  'PortErrorKind::Validation => "validation"',
  'PortErrorKind::NotFound => "not_found"',
  'PortErrorKind::Conflict => "conflict"',
  'PortErrorKind::Forbidden => "forbidden"',
  'PortErrorKind::Unavailable => "unavailable"',
  'PortErrorKind::Timeout => "timeout"',
  'PortErrorKind::InvariantViolation => "invariant_violation"',
]) requireText(kindMapper, marker, `${paths.ports}: closed PortError kind`);

for (const marker of [
  '.require_policy(PortCallPolicy::read())',
  'log_customer_read_admission_rejection(context, owner_operation, &error);',
  'error',
]) requireText(admission, marker, `${paths.ports}: preserved admission return`);

for (const marker of [
  'tracing::warn!(',
  'owner = "rustok_customer"',
  'correlation_id = %context.correlation_id',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'actor_id_length = context_facts.actor_id_length',
  'claim_count = context_facts.claim_count',
  'role_count = context_facts.role_count',
  'channel_present = context_facts.channel_present',
  'locale_length = context_facts.locale_length',
  'causation_id_present = context_facts.causation_id_present',
  'traceparent_present = context_facts.traceparent_present',
  'idempotency_key_present = context_facts.idempotency_key_present',
  'deadline_ms = ?context_facts.deadline_ms',
  'operation = owner_operation',
  'code = %error.code',
  'error_kind = customer_port_error_kind(&error.kind)',
  'error_message_present = !error.message.is_empty()',
  'error_message_length = error.message.chars().count()',
  'retryable = error.retryable',
  'boundary = CUSTOMER_READ_PORT_BOUNDARY',
]) requireText(admissionLogger, marker, `${paths.ports}: bounded admission logger`);

for (const marker of [
  'if request.page == 0',
  '"page"',
  '"customer.page_invalid"',
  '"customer projection page is invalid"',
  'if !(1..=MAX_CUSTOMERS_PER_PAGE).contains(&request.per_page)',
  '"per_page"',
  '"customer.per_page_invalid"',
  '"customer projection page size is invalid"',
]) requireText(listValidation, marker, `${paths.ports}: preserved list validation`);

for (const marker of [
  'tracing::warn!(',
  'validation_field,',
  'code,',
  'search_present = request_facts.search_present',
  'search_length = ?request_facts.search_length',
  'page = request_facts.page',
  'per_page = request_facts.per_page',
  'max_per_page = MAX_CUSTOMERS_PER_PAGE',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'boundary = CUSTOMER_READ_PORT_BOUNDARY',
]) requireText(listLogger, marker, `${paths.ports}: bounded list logger`);

for (const marker of [
  'Uuid::parse_str(&context.tenant_id).map_err(|_|',
  'log_customer_tenant_parse_rejection(context, owner_operation);',
  '"customer.context_invalid"',
  '"customer request context is invalid"',
]) requireText(tenantParser, marker, `${paths.ports}: stable tenant parser`);

for (const marker of [
  'tracing::warn!(',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'operation = owner_operation',
  'code = "customer.context_invalid"',
  'tenant_id_parse_failed = true',
  'boundary = CUSTOMER_READ_PORT_BOUNDARY',
]) requireText(tenantLogger, marker, `${paths.ports}: bounded tenant parser logger`);

for (const marker of [
  'tracing::error!(',
  'tracing::warn!(',
  'owner = "rustok_customer"',
  'correlation_id = %context.correlation_id',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'claim_count = context_facts.claim_count',
  'role_count = context_facts.role_count',
  'operation = owner_operation',
  'error_variant = error_facts.error_variant',
  'text_field_count = error_facts.text_field_count',
  'text_total_length = error_facts.text_total_length',
  'uuid_field_count = error_facts.uuid_field_count',
  'uuid_non_nil_count = error_facts.uuid_non_nil_count',
  'opaque_payload_present = error_facts.opaque_payload_present',
  'boundary = CUSTOMER_READ_PORT_BOUNDARY',
]) requireText(ownerLogger, marker, `${paths.ports}: bounded owner logger`);

for (const [variant, code, message, constructor, technical] of [
  [
    'CustomerError::Database(_)',
    'customer.database_unavailable',
    'customer storage is temporarily unavailable',
    'PortError::unavailable',
    'true',
  ],
  [
    'CustomerError::CustomerNotFound(_)',
    'customer.customer_not_found',
    'customer was not found',
    'PortError::not_found',
    'false',
  ],
  [
    'CustomerError::CustomerByUserNotFound(_)',
    'customer.customer_by_user_not_found',
    'customer was not found for the requested user',
    'PortError::not_found',
    'false',
  ],
  [
    'CustomerError::DuplicateEmail(_)',
    'customer.duplicate_email',
    'customer email is already in use',
    'PortError::conflict',
    'false',
  ],
  [
    'CustomerError::DuplicateUserLink(_)',
    'customer.duplicate_user_link',
    'customer user link already exists',
    'PortError::conflict',
    'false',
  ],
  [
    'CustomerError::Validation(_)',
    'customer.validation',
    'customer request is invalid',
    'PortError::validation',
    'false',
  ],
  [
    'CustomerError::Profile(_)',
    'customer.profile_unavailable',
    'customer profile projection is temporarily unavailable',
    'PortError::unavailable',
    'true',
  ],
]) {
  for (const marker of [variant, `"${code}"`, `"${message}"`, constructor]) {
    requireText(mapper, marker, `${paths.ports}: stable ${code} mapping`);
  }
  const call = new RegExp(
    `"${code.replaceAll('.', '\\.')}",[\\s\\S]*?&error_facts,[\\s\\S]*?${technical},`,
  );
  if (!call.test(mapper)) {
    failures.push(`${paths.ports}: ${code} severity classification drift`);
  }
}

for (const forbidden of [
  'error = ?error',
  'error = %error',
  'error = %message',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'customer_id = %',
  'user_id = %',
  'email = %',
  'search = %',
  'search = ?',
]) forbidText(diagnosticScope, forbidden, `${paths.ports}: read-boundary payload diagnostics`);

for (const marker of [
  "[customer, 'tenant_id_length = context_facts.tenant_id_length', 'customer tenant shape logging']",
  "[customer, 'tenant_id_parse_failed = true', 'customer tenant parse failure']",
  "[customer, 'error_kind = customer_port_error_kind(&error.kind)', 'customer admission error kind']",
  "[customer, 'search_present = request_facts.search_present', 'customer list request shape']",
  "[customer, 'customer_error_to_port_error(&context, owner_operation, error)', 'customer context-aware mapping']",
]) requireText(broad, marker, `${paths.broad}: aggregate customer coverage`);

for (const [key, expected] of Object.entries({
  owner_error_variant_count: 7,
  complete_customer_error_logged: false,
  database_error_payload_logged: false,
  profile_error_payload_logged: false,
  validation_email_text_logged: false,
  customer_user_uuid_logged: false,
  raw_context_logged_by_owner_mapper: false,
  static_error_variant_logged: true,
  aggregate_text_shape_logged: true,
  aggregate_uuid_shape_logged: true,
  opaque_payload_presence_logged: true,
  bounded_context_shape_logged: true,
  complete_port_error_logged_by_admission: false,
  port_error_message_text_logged_by_admission: false,
  raw_context_logged_by_admission: false,
  static_port_error_kind_logged_by_admission: true,
  port_error_message_shape_logged_by_admission: true,
  raw_context_logged_by_list_validation: false,
  list_search_text_logged: false,
  bounded_list_request_shape_logged: true,
  uuid_parse_error_payload_logged: false,
  raw_tenant_logged_by_parser: false,
  static_tenant_parse_failure_logged: true,
  database_profile_error_severity_changed: false,
  ordinary_warning_severity_changed: false,
  admission_warning_severity_changed: false,
  list_validation_warning_severity_changed: false,
  tenant_parser_warning_severity_changed: false,
  public_code_changed: false,
  public_message_changed: false,
  public_kind_changed: false,
  public_retryability_changed: false,
  owner_delegation_changed: false,
  read_operations_changed: false,
  admission_diagnostic_cleanup_closed: true,
  tenant_parser_diagnostic_cleanup_closed: true,
  list_validation_diagnostic_cleanup_closed: true,
  customer_read_boundary_diagnostic_cleanup_closed: true,
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
  public_request_response_contracts_preserved: true,
  owner_delegation_preserved: true,
  all_seven_public_mappings_preserved: true,
  database_profile_severity_preserved: true,
  ordinary_warning_severity_preserved: true,
  admission_list_parser_warning_severity_preserved: true,
  complete_customer_error_logging_removed_from_owner_mapper: true,
  database_profile_payload_removed: true,
  validation_email_text_removed: true,
  customer_user_uuid_removed: true,
  raw_context_removed_from_owner_mapper: true,
  bounded_context_shape_retained: true,
  bounded_owner_error_shape_retained: true,
  complete_port_error_logging_removed_from_admission: true,
  port_error_message_text_removed_from_admission: true,
  static_port_error_kind_and_message_shape_retained: true,
  raw_context_removed_from_admission_and_list_validation: true,
  list_search_text_removed: true,
  bounded_list_request_shape_retained: true,
  uuid_parser_payload_removed: true,
  raw_tenant_removed_from_parser: true,
  static_tenant_parse_failure_retained: true,
  admission_parser_list_validation_diagnostics_source_closed: true,
  customer_read_boundary_source_closed: true,
  broad_ecommerce_cleanup_remains_open: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`${paths.review}: review_findings.${key} must be ${expected}`);
  }
}

for (const marker of [
  '# Customer read-boundary diagnostic safety',
  'Status: **source-ready / unvalidated**',
  'read-policy admission',
  'list request validation',
  'tenant UUID parsing',
  '`customer_error_to_port_error`',
  'All seven current `CustomerError` variants',
  'database and profile failures remain error severity',
  'The UUID parser error and raw tenant value are not recorded.',
  'the broader ecommerce cleanup remain open',
]) requireText(document, marker, `${paths.document}: truthful source scope`);

if (failures.length > 0) {
  console.error('Customer read-boundary diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Customer read admission, list validation, tenant parsing, and owner errors retain bounded diagnostics while preserving public behavior; execution evidence remains open',
);
