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
};

const ports = read(paths.ports);
const errorSource = read(paths.error);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const document = read(paths.document);

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
  'pub trait CustomerReadPort: Send + Sync',
  'async fn read_customer_projection(',
  'async fn read_customer_projection_by_user(',
  'async fn list_customer_projections(',
  'async fn list_profile_enrichment(',
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
const admissionLogger = functionBody(ports, 'log_customer_read_admission_rejection');
const listLogger = functionBody(ports, 'log_customer_list_validation_rejection');
const tenantLogger = functionBody(ports, 'log_customer_tenant_parse_rejection');
const ownerLogger = functionBody(ports, 'log_customer_owner_failure');
const mapper = functionBody(ports, 'customer_error_to_port_error');
const diagnosticScope = [contextFacts, admissionLogger, listLogger, tenantLogger, ownerLogger].join('\n');

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
]) requireText(contextFacts, marker, `${paths.ports}: bounded context facts`);

for (const [source, label] of [
  [admissionLogger, 'admission logger'],
  [listLogger, 'list-validation logger'],
  [tenantLogger, 'tenant-parser logger'],
  [ownerLogger, 'owner-error logger'],
]) {
  requireText(
    source,
    'correlation_id_length = context_facts.correlation_id_length',
    `${paths.ports}: ${label}`,
  );
  requireText(source, 'tenant_id_length = context_facts.tenant_id_length', `${label} tenant shape`);
  requireText(source, 'operation = owner_operation', `${label} operation`);
  requireText(source, 'boundary = CUSTOMER_READ_PORT_BOUNDARY', `${label} boundary`);
}

for (const forbidden of [
  'correlation_id = %context.correlation_id',
  'error = ?error',
  'error = %error',
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
]) forbidText(diagnosticScope, forbidden, `${paths.ports}: diagnostic payload`);

for (const [variant, code, message, constructor] of [
  ['CustomerError::Database(_)', 'customer.database_unavailable', 'customer storage is temporarily unavailable', 'PortError::unavailable'],
  ['CustomerError::CustomerNotFound(_)', 'customer.customer_not_found', 'customer was not found', 'PortError::not_found'],
  ['CustomerError::CustomerByUserNotFound(_)', 'customer.customer_by_user_not_found', 'customer was not found for the requested user', 'PortError::not_found'],
  ['CustomerError::DuplicateEmail(_)', 'customer.duplicate_email', 'customer email is already in use', 'PortError::conflict'],
  ['CustomerError::DuplicateUserLink(_)', 'customer.duplicate_user_link', 'customer user link already exists', 'PortError::conflict'],
  ['CustomerError::Validation(_)', 'customer.validation', 'customer request is invalid', 'PortError::validation'],
  ['CustomerError::Profile(_)', 'customer.profile_unavailable', 'customer profile projection is temporarily unavailable', 'PortError::unavailable'],
]) {
  for (const marker of [variant, `"${code}"`, `"${message}"`, constructor]) {
    requireText(mapper, marker, `${paths.ports}: stable ${code} mapping`);
  }
}

for (const [key, expected] of Object.entries({
  complete_customer_error_logged: false,
  raw_context_logged_by_owner_mapper: false,
  bounded_context_shape_logged: true,
  complete_port_error_logged_by_admission: false,
  raw_context_logged_by_admission: false,
  raw_context_logged_by_list_validation: false,
  public_code_changed: false,
  public_message_changed: false,
  public_kind_changed: false,
  public_retryability_changed: false,
  owner_delegation_changed: false,
  read_operations_changed: false,
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
  raw_context_removed_from_owner_mapper: true,
  bounded_context_shape_retained: true,
  raw_context_removed_from_admission_and_list_validation: true,
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
  'correlation-id character length',
  'raw correlation id',
  'all public codes, messages, kinds and retryability are unchanged',
  'broader ecommerce cleanup remain open',
  'No test, verifier, formatter, Cargo, workflow or CI command was executed',
]) requireText(document, marker, `${paths.document}: truthful source scope`);

if (failures.length > 0) {
  console.error('Customer read-boundary diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Customer owner read diagnostics retain correlation length and bounded context without raw correlation payloads; execution evidence remains open',
);
