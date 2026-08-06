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
  ports: 'crates/rustok-region/src/ports.rs',
  error: 'crates/rustok-region/src/error.rs',
  evidence:
    'crates/rustok-region/contracts/evidence/region-owner-port-error-safety-source.json',
  review:
    'crates/rustok-region/contracts/evidence/region-owner-port-error-safety-source-review.json',
  document: 'crates/rustok-region/docs/region-owner-port-error-safety.md',
  plan: 'crates/rustok-region/docs/implementation-plan.md',
};

const ports = read(paths.ports);
const errorSource = read(paths.error);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const document = read(paths.document);
const plan = read(paths.plan);

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
  'pub trait RegionReadPort: Send + Sync',
  'async fn read_region(',
  'async fn list_regions_for_tenant(',
  'impl RegionReadPort for crate::RegionService',
  'self.get_region(',
  'self.resolve_region_for_country(',
  'self.list_regions(',
  'require_region_read_policy(&context, owner_operation)?;',
  'parse_tenant_id(&context, owner_operation)?;',
  'map_region_error(&context, owner_operation, error)',
]) requireText(ports, marker, `${paths.ports}: preserved owner boundary`);

for (const marker of [
  'Validation(String)',
  'RegionNotFound(Uuid)',
  'InvalidCountryCode(String)',
  'Database(#[from] DbErr)',
]) requireText(errorSource, marker, `${paths.error}: owner error shape`);

const contextFacts = functionBody(ports, 'region_read_context_facts');
const requestFacts = functionBody(ports, 'region_read_request_facts');
const kindMapper = functionBody(ports, 'region_port_error_kind');
const admission = functionBody(ports, 'require_region_read_policy');
const admissionLogger = functionBody(ports, 'log_region_read_admission_rejection');
const tenantParser = functionBody(ports, 'parse_tenant_id');
const tenantLogger = functionBody(ports, 'log_region_tenant_parse_rejection');
const validation = functionBody(ports, 'validate_region_read_request');
const validationLogger = functionBody(ports, 'log_region_request_validation_rejection');
const ownerFacts = functionBody(ports, 'region_owner_error_facts');
const ownerLogger = functionBody(ports, 'log_region_owner_failure');
const mapper = functionBody(ports, 'map_region_error');
const diagnosticScope = [
  contextFacts,
  requestFacts,
  kindMapper,
  admission,
  admissionLogger,
  tenantParser,
  tenantLogger,
  validation,
  validationLogger,
  ownerFacts,
  ownerLogger,
  mapper,
].join('\n');

for (const marker of [
  'tenant_id_length: context.tenant_id.chars().count()',
  'actor_kind',
  'actor_id_length: context.actor.id.chars().count()',
  'claim_count: context.claims.len()',
  'role_count: context.roles.len()',
  'correlation_id_length: context.correlation_id.chars().count()',
  'channel_present: context.channel.is_some()',
  'locale_length: context.locale.chars().count()',
  'causation_id_present: context.causation_id.is_some()',
  'traceparent_present: context.traceparent.is_some()',
  'idempotency_key_present: context.idempotency_key.is_some()',
  'deadline_ms: context.deadline_ms',
]) requireText(contextFacts, marker, `${paths.ports}: bounded context facts`);

for (const marker of [
  'selector_kind',
  'selector_uuid_non_nil',
  'country_code_length',
  'requested_locale_length',
  'tenant_default_locale_length',
]) requireText(requestFacts, marker, `${paths.ports}: bounded request facts`);

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
  'log_region_read_admission_rejection(context, owner_operation, &error);',
  'error',
]) requireText(admission, marker, `${paths.ports}: preserved admission return`);

for (const marker of [
  'correlation_id_length = context_facts.correlation_id_length',
  'tenant_id_length = context_facts.tenant_id_length',
  'operation = owner_operation',
  'code = %error.code',
  'error_kind = region_port_error_kind(&error.kind)',
  'error_message_present = !error.message.is_empty()',
  'error_message_length = error.message.chars().count()',
  'retryable = error.retryable',
  'boundary = REGION_READ_PORT_BOUNDARY',
]) requireText(admissionLogger, marker, `${paths.ports}: bounded admission logger`);

for (const marker of [
  'context.tenant_id.parse::<Uuid>().map_err(|_|',
  'log_region_tenant_parse_rejection(context, owner_operation);',
  '"region.tenant_id_invalid"',
  '"region request context is invalid"',
]) requireText(tenantParser, marker, `${paths.ports}: stable tenant parser`);

for (const marker of [
  'correlation_id_length = context_facts.correlation_id_length',
  'tenant_id_parse_failed = true',
  'tenant_id_length = context_facts.tenant_id_length',
  'boundary = REGION_READ_PORT_BOUNDARY',
]) requireText(tenantLogger, marker, `${paths.ports}: bounded tenant parser logger`);

for (const marker of [
  'country_code.trim().is_empty()',
  'log_region_request_validation_rejection(context, owner_operation, request);',
  '"region.country_code_empty"',
  '"region read port requires a non-empty country code selector"',
]) requireText(validation, marker, `${paths.ports}: preserved direct validation`);

for (const marker of [
  'correlation_id_length = context_facts.correlation_id_length',
  'selector_kind = request_facts.selector_kind',
  'selector_uuid_non_nil = ?request_facts.selector_uuid_non_nil',
  'country_code_length = ?request_facts.country_code_length',
  'requested_locale_length = ?request_facts.requested_locale_length',
  'tenant_default_locale_length = ?request_facts.tenant_default_locale_length',
]) requireText(validationLogger, marker, `${paths.ports}: bounded validation logger`);

for (const marker of [
  'crate::RegionError::Validation(message)',
  'error_variant: "validation"',
  'text_total_length: message.chars().count()',
  'crate::RegionError::RegionNotFound(region_id)',
  'error_variant: "region_not_found"',
  'uuid_field_count: 1',
  'crate::RegionError::InvalidCountryCode(country_code)',
  'error_variant: "invalid_country_code"',
  'text_total_length: country_code.chars().count()',
  'crate::RegionError::Database(_)',
  'error_variant: "database"',
  'opaque_payload_present: true',
]) requireText(ownerFacts, marker, `${paths.ports}: bounded owner error facts`);

for (const marker of [
  'tracing::error!(',
  'tracing::warn!(',
  'owner = REGION_OWNER',
  'correlation_id_length = context_facts.correlation_id_length',
  'operation = owner_operation',
  'error_variant = error_facts.error_variant',
  'text_field_count = error_facts.text_field_count',
  'text_total_length = error_facts.text_total_length',
  'uuid_field_count = error_facts.uuid_field_count',
  'uuid_non_nil_count = error_facts.uuid_non_nil_count',
  'opaque_payload_present = error_facts.opaque_payload_present',
  'boundary = REGION_READ_PORT_BOUNDARY',
]) requireText(ownerLogger, marker, `${paths.ports}: bounded owner logger`);

for (const [variant, code, message, constructor, technical] of [
  [
    'crate::RegionError::RegionNotFound(_)',
    'region.not_found',
    'region read projection was not found',
    'PortError::not_found',
    'false',
  ],
  [
    'crate::RegionError::Validation(_) | crate::RegionError::InvalidCountryCode(_)',
    'region.validation',
    'region request is invalid',
    'PortError::validation',
    'false',
  ],
  [
    'crate::RegionError::Database(_)',
    'region.read_failed',
    'region storage is temporarily unavailable',
    'PortError::unavailable',
    'true',
  ],
]) {
  for (const marker of [variant, `"${code}"`, `"${message}"`, constructor]) {
    requireText(mapper, marker, `${paths.ports}: stable ${code} mapping`);
  }
  const severity = new RegExp(
    `"${code.replaceAll('.', '\\.')}\\",[\\s\\S]*?&error_facts,[\\s\\S]*?${technical},`,
  );
  if (!severity.test(mapper)) {
    failures.push(`${paths.ports}: ${code} severity classification drift`);
  }
}

for (const forbidden of [
  'error = ?error',
  'error = %error',
  'internal_message = %error.message',
  'correlation_id = %context.correlation_id',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'region_id = %',
  'country_code = %',
  'requested_locale = %',
  'tenant_default_locale = %',
]) forbidText(diagnosticScope, forbidden, `${paths.ports}: payload diagnostics`);

for (const [key, expected] of Object.entries({
  owner_operation_count: 2,
  dynamic_owner_validation_message_public: false,
  database_error_payload_logged: false,
  complete_port_error_logged_by_admission: false,
  port_error_message_text_logged_by_admission: false,
  raw_context_logged: false,
  raw_correlation_id_logged: false,
  raw_region_uuid_logged: false,
  raw_country_code_logged: false,
  static_owner_messages: true,
  closed_error_variant_logged: true,
  aggregate_text_shape_logged: true,
  aggregate_uuid_shape_logged: true,
  opaque_payload_presence_logged: true,
  bounded_context_shape_logged: true,
  bounded_request_shape_logged: true,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract[key] !== expected) {
    failures.push(`${paths.evidence}: ${key} expected ${expected}`);
  }
}

for (const [key, expected] of Object.entries({
  both_read_operations_preserved: true,
  owner_delegation_preserved: true,
  public_error_kind_code_retryability_preserved: true,
  dynamic_validation_payload_removed_from_public_message: true,
  database_payload_removed_from_diagnostics: true,
  admission_payload_removed_from_diagnostics: true,
  context_payload_removed_from_diagnostics: true,
  raw_correlation_id_removed_from_diagnostics: true,
  request_identity_payload_removed_from_diagnostics: true,
  broad_ecommerce_cleanup_remains_open: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings[key] !== expected) {
    failures.push(`${paths.review}: ${key} expected ${expected}`);
  }
}

for (const marker of [
  'Status: **source-ready / unvalidated**',
  '`RegionReadPort`',
  '`region request is invalid`',
  'raw correlation id is no longer emitted',
  'bounded context',
  'No tests, Node verifiers, Cargo commands, formatting, workflows, CI, or mounted runtime validation',
]) requireText(document, marker, `${paths.document}: documentation contract`);

for (const marker of [
  'Owner read-port error safety: `source_closed_unvalidated`',
  'verify-region-owner-port-error-safety.mjs',
]) requireText(plan, marker, `${paths.plan}: plan registration`);

if (failures.length > 0) {
  console.error('region owner port error safety verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('region owner port error safety verification passed');
