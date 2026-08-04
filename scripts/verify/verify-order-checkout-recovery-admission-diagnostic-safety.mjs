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
const between = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
};

const sourcePath = 'crates/rustok-order/src/checkout_order_recovery.rs';
const evidencePath =
  'crates/rustok-order/contracts/evidence/checkout-order-recovery-admission-diagnostic-safety-source-review.json';
const documentationPath =
  'crates/rustok-order/docs/checkout-order-recovery-admission-diagnostic-safety.md';
const source = read(sourcePath);
const evidence = JSON.parse(read(evidencePath));
const documentation = read(documentationPath);
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');

const operationAdmission = between(
  source,
  'fn require_operation_context(',
  'fn parse_tenant_id(',
  'operation admission',
);
const tenantAdmission = between(
  source,
  'fn parse_tenant_id(',
  'fn parse_actor_id(',
  'tenant admission',
);
const actorAdmission = between(
  source,
  'fn parse_actor_id(',
  'fn checkout_request_hashes(',
  'actor admission',
);
const admissionLogger = between(
  source,
  'fn log_checkout_order_recovery_admission_rejection(',
  'fn checkout_order_recovery_owner_error_facts(',
  'admission logger',
);
const admissionHelpers = `${operationAdmission}\n${tenantAdmission}\n${actorAdmission}`;

for (const marker of [
  'const CHECKOUT_ORDER_RECOVERY_OWNER: &str = "rustok_order.checkout_order_recovery";',
  'const CHECKOUT_ORDER_RECOVERY_BOUNDARY: &str = "checkout_order_recovery_adapter";',
  'fn checkout_order_recovery_context_facts(',
  'fn log_checkout_order_recovery_admission_rejection(',
]) requireText(source, marker, 'shared admission contract');

for (const marker of [
  'log_checkout_order_recovery_admission_rejection(',
  '"causation_id"',
  'context.causation_id.is_some()',
  '.map(|value| value.chars().count())',
  'context_operation.is_some()',
  'context_operation.map(|value| !value.is_nil())',
  'Some(!checkout_operation_id.is_nil())',
  'Some(false)',
  '"order.checkout_operation_id_invalid"',
  'PortError::validation(',
  '"checkout operation context is invalid"',
]) requireText(operationAdmission, marker, 'causation admission');

for (const [block, field, code] of [
  [tenantAdmission, 'tenant_id', 'order.tenant_id_invalid'],
  [actorAdmission, 'actor_id', 'order.actor_id_invalid'],
]) {
  for (const marker of [
    'Uuid::parse_str(',
    '.map_err(|_| {',
    'log_checkout_order_recovery_admission_rejection(',
    `"${field}"`,
    'true,',
    'false,',
    `"${code}"`,
    'PortError::validation(',
    '"order request context is invalid"',
  ]) requireText(block, marker, `${field} admission`);
}

for (const marker of [
  'owner = CHECKOUT_ORDER_RECOVERY_OWNER',
  'operation,',
  'correlation_id = %context.correlation_id',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'actor_id_length = context_facts.actor_id_length',
  'claim_count = context_facts.claim_count',
  'role_count = context_facts.role_count',
  'channel_present = context_facts.channel_present',
  'channel_length = ?context_facts.channel_length',
  'locale_length = context_facts.locale_length',
  'causation_id_present = context_facts.causation_id_present',
  'causation_id_length = ?context_facts.causation_id_length',
  'traceparent_present = context_facts.traceparent_present',
  'traceparent_length = ?context_facts.traceparent_length',
  'idempotency_key_present = context_facts.idempotency_key_present',
  'idempotency_key_length = ?context_facts.idempotency_key_length',
  'deadline_ms = ?context_facts.deadline_ms',
  'field,',
  'field_value_present,',
  'field_value_length = ?field_value_length',
  'uuid_parseable,',
  'uuid_non_nil = ?uuid_non_nil',
  'expected_uuid_non_nil = ?expected_uuid_non_nil',
  'matches_expected = ?matches_expected',
  'code,',
  'boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY',
  '"order checkout recovery admission was rejected with bounded diagnostics"',
]) requireText(admissionLogger, marker, 'bounded admission logger');

for (const raw of [
  'error = ?error',
  'error = %error',
  'tenant_id = %context.tenant_id',
  'actor_id = %context.actor.id',
  'channel = ?context.channel',
  'expected_checkout_operation_id = %checkout_operation_id',
  'actual_causation_id = ?context.causation_id',
  'causation_id = ?context.causation_id',
]) forbidText(admissionHelpers, raw, 'raw admission diagnostic');

for (const raw of [
  'tenant_id = %context.tenant_id',
  'actor_id = %context.actor.id',
  'channel = ?context.channel',
  'causation_id = ?context.causation_id',
  'expected_checkout_operation_id = %checkout_operation_id',
  'actual_causation_id = ?context.causation_id',
  'error = ?error',
  'error = %error',
]) forbidText(admissionLogger, raw, 'raw shared admission logger');

requireText(
  plan,
  'Finish correlation-safe mapper cleanup for order, payment execution/compensation,',
  'open master cleanup item',
);
requireText(
  documentation,
  'Status: `source_reviewed_unvalidated`',
  'documentation status',
);
requireText(
  documentation,
  'The broad ecommerce correlation-safe mapper cleanup remains open.',
  'documentation open disclosure',
);
requireText(
  documentation,
  'No tests, Node verifiers, Cargo commands, formatting, workflows, or CI were',
  'documentation validation disclosure',
);

if (
  evidence.status !==
  'order_checkout_recovery_admission_diagnostic_safety_source_reviewed_unvalidated'
) failures.push(`evidence status: unexpected ${evidence.status}`);

for (const [key, expected] of Object.entries({
  admission_helper_count: 3,
  raw_uuid_parse_errors_logged: false,
  raw_tenant_id_logged_by_admission_helpers: false,
  raw_actor_id_logged_by_admission_helpers: false,
  raw_channel_logged_by_admission_helpers: false,
  raw_causation_id_logged_by_admission_helpers: false,
  expected_checkout_operation_uuid_logged: false,
  correlation_preserved: true,
  owner_operation_preserved: true,
  context_shape_logged: true,
  field_presence_and_length_logged: true,
  uuid_parseability_logged: true,
  uuid_non_nil_shape_logged: true,
  expected_uuid_non_nil_shape_logged: true,
  identity_match_result_logged: true,
  warning_severity_preserved: true,
  public_codes_preserved: true,
  public_kinds_preserved: true,
  public_messages_preserved: true,
  recovery_flow_changed: false,
  identity_validation_changed: false,
  hash_validation_changed: false,
  serde_diagnostics_changed: false,
  lifecycle_diagnostics_changed: false,
  read_not_found_diagnostics_changed: false,
  non_admission_recovery_diagnostics_remaining_open: true,
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
  'scripts/verify/verify-order-checkout-recovery-admission-diagnostic-safety.mjs',
]) {
  if (!evidence.reviewed_scope?.includes(reviewedPath)) {
    failures.push(`evidence reviewed_scope: missing ${reviewedPath}`);
  }
}

if (failures.length > 0) {
  console.error('Order checkout recovery admission diagnostic safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Order checkout recovery admission diagnostics retain bounded shape and stable public contracts',
);
