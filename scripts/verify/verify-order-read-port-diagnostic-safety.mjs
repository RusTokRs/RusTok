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

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (source, value, expected, label) => {
  const count = source.split(value).length - 1;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
};
const between = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
};

const sourcePath = 'crates/rustok-order/src/order_read.rs';
const evidencePath =
  'crates/rustok-order/contracts/evidence/order-read-port-diagnostic-safety-source-review.json';
const documentationPath = 'crates/rustok-order/docs/order-read-port-diagnostic-safety.md';
const source = read(sourcePath);
const evidence = JSON.parse(read(evidencePath));
const documentation = read(documentationPath);
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');

const contextFacts = between(
  source,
  'struct OrderReadContextFacts {',
  '#[derive(Clone, Copy, Debug)]\nstruct OrderReadRequestFacts {',
  'context facts',
);
const requestFacts = between(
  source,
  'struct OrderReadRequestFacts {',
  '#[derive(Clone, Copy, Debug)]\nstruct OrderReadOwnerErrorFacts {',
  'request facts',
);
const errorFacts = between(
  source,
  'struct OrderReadOwnerErrorFacts {',
  'fn order_read_context_facts(',
  'owner error facts',
);
const contextBuilder = between(
  source,
  'fn order_read_context_facts(',
  'fn order_read_request_facts(',
  'context builder',
);
const requestBuilder = between(
  source,
  'fn order_read_request_facts(',
  'fn order_read_owner_error_facts(',
  'request builder',
);
const ownerFactsBuilder = between(
  source,
  'fn order_read_owner_error_facts(',
  'fn parse_tenant_id(',
  'owner facts builder',
);
const tenantParser = between(
  source,
  'fn parse_tenant_id(',
  'fn order_read_owner_error_policy(',
  'tenant parser',
);
const policy = between(
  source,
  'fn order_read_owner_error_policy(',
  'fn log_order_read_owner_error(',
  'owner policy',
);
const logger = between(
  source,
  'fn log_order_read_owner_error(',
  '#[allow(clippy::too_many_arguments)]\nfn map_owner_error(',
  'bounded owner logger',
);
const mapper = source.slice(source.indexOf('#[allow(clippy::too_many_arguments)]\nfn map_owner_error('));

for (const marker of [
  'const ORDER_READ_OWNER: &str = "rustok_order";',
  'const ORDER_READ_BOUNDARY: &str = "order_read_port";',
  'struct OrderReadContextFacts {',
  'struct OrderReadRequestFacts {',
  'struct OrderReadOwnerErrorFacts {',
  'fn order_read_context_facts(',
  'fn order_read_request_facts(',
  'fn order_read_owner_error_facts(',
  'fn order_read_owner_error_policy(',
  'fn log_order_read_owner_error(',
]) requireText(source, marker, 'order read bounded contract');

for (const marker of [
  'tenant_id_length: usize,',
  'actor_kind: &\'static str,',
  'actor_id_length: usize,',
  'claim_count: usize,',
  'role_count: usize,',
  'channel_present: bool,',
  'channel_length: Option<usize>,',
  'locale_length: usize,',
  'causation_id_present: bool,',
  'causation_id_length: Option<usize>,',
  'traceparent_present: bool,',
  'traceparent_length: Option<usize>,',
  'idempotency_key_present: bool,',
  'idempotency_key_length: Option<usize>,',
  'deadline_ms: Option<u64>,',
]) requireText(contextFacts, marker, 'context fact field');

for (const marker of [
  'context.tenant_id.chars().count()',
  'rustok_api::PortActorKind::User => "user"',
  'rustok_api::PortActorKind::Service => "service"',
  'rustok_api::PortActorKind::System => "system"',
  'context.actor.id.chars().count()',
  'context.claims.len()',
  'context.roles.len()',
  'context.channel.is_some()',
  'context.locale.chars().count()',
  'context.causation_id.is_some()',
  'context.traceparent.is_some()',
  'context.idempotency_key.is_some()',
  'deadline_ms: context.deadline_ms',
]) requireText(contextBuilder, marker, 'context shape builder');

for (const marker of [
  'order_id_present: bool,',
  'order_id_non_nil: Option<bool>,',
  'return_id_present: bool,',
  'return_id_non_nil: Option<bool>,',
  'change_id_present: bool,',
  'change_id_non_nil: Option<bool>,',
  'customer_id_present: bool,',
  'customer_id_non_nil: Option<bool>,',
  'status_length: Option<usize>,',
  'change_type_length: Option<usize>,',
  'fallback_locale_length: Option<usize>,',
]) requireText(requestFacts, marker, 'request fact field');

for (const marker of [
  'order_id_present: order_id.is_some()',
  'order_id_non_nil: order_id.map(|value| !value.is_nil())',
  'return_id_present: return_id.is_some()',
  'return_id_non_nil: return_id.map(|value| !value.is_nil())',
  'change_id_present: change_id.is_some()',
  'change_id_non_nil: change_id.map(|value| !value.is_nil())',
  'customer_id_present: customer_id.is_some()',
  'customer_id_non_nil: customer_id.map(|value| !value.is_nil())',
]) requireText(requestBuilder, marker, 'request shape builder');

for (const marker of [
  'error_variant: &\'static str,',
  'text_field_count: usize,',
  'text_total_length: usize,',
  'uuid_field_count: usize,',
  'uuid_non_nil_count: usize,',
  'opaque_payload_present: bool,',
]) requireText(errorFacts, marker, 'owner error fact field');

for (const marker of [
  'OrderError::Validation(value)',
  '"validation"',
  'value.chars().count()',
  'OrderError::OrderNotFound(id)',
  '"order_not_found"',
  'OrderError::OrderReturnNotFound(id)',
  '"return_not_found"',
  'OrderError::OrderChangeNotFound(id)',
  '"change_not_found"',
  'OrderError::InvalidTransition { from, to }',
  '"invalid_transition"',
  'from.chars().count() + to.chars().count()',
  'OrderError::Database(_) => ("database", 0, 0, 0, 0, true)',
  'OrderError::Core(_) => ("core", 0, 0, 0, 0, true)',
]) requireText(ownerFactsBuilder, marker, 'closed owner error shape');

for (const marker of [
  'Uuid::parse_str(&context.tenant_id).map_err(|_| {',
  'let facts = order_read_context_facts(context);',
  'owner = ORDER_READ_OWNER',
  'correlation_id = %context.correlation_id',
  'tenant_id_length = facts.tenant_id_length',
  'tenant_id_parseable = false',
  'actor_kind = facts.actor_kind',
  'actor_id_length = facts.actor_id_length',
  'claim_count = facts.claim_count',
  'role_count = facts.role_count',
  'channel_present = facts.channel_present',
  'channel_length = ?facts.channel_length',
  'locale_length = facts.locale_length',
  'causation_id_length = ?facts.causation_id_length',
  'traceparent_length = ?facts.traceparent_length',
  'idempotency_key_length = ?facts.idempotency_key_length',
  'code = "order.context_invalid"',
  'boundary = ORDER_READ_BOUNDARY',
  'PortError::validation(',
  '"order request context is invalid"',
]) requireText(tenantParser, marker, 'bounded tenant parser');

for (const marker of [
  'OrderError::Validation(_)',
  'PortErrorKind::Validation',
  '"order.validation"',
  '"order request is invalid"',
  'OrderError::OrderNotFound(_)',
  'PortErrorKind::NotFound',
  '"order.order_not_found"',
  '"order was not found"',
  'OrderError::OrderReturnNotFound(_)',
  '"order.return_not_found"',
  '"order return was not found"',
  'OrderError::OrderChangeNotFound(_)',
  '"order.change_not_found"',
  '"order change was not found"',
  'OrderError::InvalidTransition { .. }',
  'PortErrorKind::Conflict',
  '"order.invalid_transition"',
  '"order lifecycle transition conflicts with the current state"',
  'OrderError::Database(_)',
  'PortErrorKind::Unavailable',
  '"order.database_unavailable"',
  '"order storage is temporarily unavailable"',
  'OrderError::Core(_)',
  'PortErrorKind::InvariantViolation',
  '"order.operation_failed"',
  '"order operation could not be completed safely"',
]) requireText(policy, marker, 'stable public policy');

for (const marker of [
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'actor_id_length = context_facts.actor_id_length',
  'claim_count = context_facts.claim_count',
  'role_count = context_facts.role_count',
  'channel_present = context_facts.channel_present',
  'channel_length = ?context_facts.channel_length',
  'locale_length = context_facts.locale_length',
  'fallback_locale_length = ?request_facts.fallback_locale_length',
  'causation_id_length = ?context_facts.causation_id_length',
  'traceparent_length = ?context_facts.traceparent_length',
  'idempotency_key_length = ?context_facts.idempotency_key_length',
  'order_id_present = request_facts.order_id_present',
  'order_id_non_nil = ?request_facts.order_id_non_nil',
  'return_id_present = request_facts.return_id_present',
  'return_id_non_nil = ?request_facts.return_id_non_nil',
  'change_id_present = request_facts.change_id_present',
  'change_id_non_nil = ?request_facts.change_id_non_nil',
  'customer_id_present = request_facts.customer_id_present',
  'customer_id_non_nil = ?request_facts.customer_id_non_nil',
  'status_length = ?request_facts.status_length',
  'change_type_length = ?request_facts.change_type_length',
  'owner_error_variant = error_facts.error_variant',
  'owner_error_text_field_count = error_facts.text_field_count',
  'owner_error_text_total_length = error_facts.text_total_length',
  'owner_error_uuid_field_count = error_facts.uuid_field_count',
  'owner_error_uuid_non_nil_count = error_facts.uuid_non_nil_count',
  'owner_error_opaque_payload_present = error_facts.opaque_payload_present',
  'boundary = ORDER_READ_BOUNDARY',
]) requireText(logger, marker, 'bounded owner logger');

requireText(logger, 'tracing::error!(', 'technical error severity');
requireText(logger, 'tracing::warn!(', 'ordinary warning severity');
requireText(
  logger,
  '"order projection read failed with bounded diagnostics"',
  'technical bounded message',
);
requireText(
  logger,
  '"order projection read was rejected with bounded diagnostics"',
  'ordinary bounded message',
);

for (const marker of [
  'let request_facts = order_read_request_facts(',
  'let error_facts = order_read_owner_error_facts(&error);',
  'order_read_owner_error_policy(&error)',
  'matches!(&error, OrderError::Database(_) | OrderError::Core(_))',
  'log_order_read_owner_error(',
  'PortError::new(kind, code, message, retryable)',
]) requireText(mapper, marker, 'bounded owner mapper');

for (const raw of [
  'error = ?error',
  'error = %error',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'actor_id = %context.actor.id',
  'channel = ?context.channel',
  'order_id = ?order_id',
  'return_id = ?return_id',
  'change_id = ?change_id',
  'customer_id = ?customer_id',
]) forbidText(`${tenantParser}\n${logger}\n${mapper}`, raw, 'raw diagnostic payload');

requireCount(source, 'map_owner_error(', 7, 'six callsites plus mapper definition');
requireCount(source, 'parse_tenant_id(&context, OPERATION)?;', 6, 'six tenant parser callsites');

for (const operation of [
  'read_order_projection',
  'list_order_projections',
  'read_order_return_projection',
  'list_order_return_projections',
  'read_order_change_projection',
  'list_order_change_projections',
]) requireText(source, `const OPERATION: &str = "${operation}";`, 'owner operation');

requireText(
  plan,
  'Finish correlation-safe mapper cleanup for order, payment execution/compensation,',
  'open master cleanup item',
);
requireText(documentation, 'Status: `source_reviewed_unvalidated`', 'documentation status');
requireText(
  documentation,
  'The broad ecommerce mapper-cleanup item remains open.',
  'documentation open disclosure',
);
requireText(
  documentation,
  'No tests, Node verifiers, Cargo commands, formatting, workflows, or CI were',
  'documentation validation disclosure',
);

if (evidence.status !== 'order_read_port_diagnostic_safety_source_reviewed_unvalidated') {
  failures.push(`evidence status: unexpected ${evidence.status}`);
}

for (const [key, expected] of Object.entries({
  read_operation_count: 6,
  order_error_variant_count: 7,
  complete_owner_error_logged: false,
  database_error_payload_logged: false,
  core_error_payload_logged: false,
  validation_text_logged: false,
  transition_text_logged: false,
  owner_resource_uuid_values_logged: false,
  raw_tenant_id_logged: false,
  raw_actor_logged: false,
  raw_channel_logged: false,
  raw_request_uuid_values_logged: false,
  uuid_parse_error_logged: false,
  correlation_preserved: true,
  owner_operation_preserved: true,
  context_shape_logged: true,
  request_identity_presence_logged: true,
  request_identity_non_nil_shape_logged: true,
  filter_length_shape_logged: true,
  owner_error_variant_logged: true,
  owner_error_text_shape_logged: true,
  owner_error_uuid_shape_logged: true,
  owner_error_opaque_payload_presence_logged: true,
  technical_warning_severity_split_preserved: true,
  public_codes_preserved: true,
  public_kinds_preserved: true,
  public_messages_preserved: true,
  public_retryability_preserved: true,
  tenant_validation_contract_preserved: true,
  order_read_flow_changed: false,
  pagination_changed: false,
  filter_forwarding_changed: false,
  locale_fallback_changed: false,
  owner_service_calls_changed: false,
  commerce_orchestration_changed: false,
  order_status_promoted: false,
  broad_cleanup_remains_open: true,
  runtime_evidence_claimed: false,
})) {
  if (evidence.review_findings?.[key] !== expected) {
    failures.push(
      `evidence review_findings.${key}: expected ${expected}, found ${evidence.review_findings?.[key]}`,
    );
  }
}

for (const key of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
  'compile_proven',
  'runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key}: expected false`);
  }
}

if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push('evidence execution: expected empty array');
}

for (const reviewedPath of [
  sourcePath,
  documentationPath,
  evidencePath,
  'crates/rustok-commerce/docs/implementation-plan.md',
  'scripts/verify/verify-order-read-port-diagnostic-safety.mjs',
]) {
  if (!evidence.reviewed_scope?.includes(reviewedPath)) {
    failures.push(`evidence reviewed_scope: missing ${reviewedPath}`);
  }
}

if (failures.length > 0) {
  console.error('Order read port diagnostic safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Order read diagnostics retain bounded context, request and owner-error shapes',
);
