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

function functionBody(source, functionName) {
  const match = new RegExp(`fn\\s+${functionName}\\s*\\(`).exec(source);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return '';
  }
  const openBrace = source.indexOf('{', match.index);
  if (openBrace < 0) {
    failures.push(`missing body for ${functionName}`);
    return '';
  }
  let depth = 0;
  for (let index = openBrace; index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    if (source[index] === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated body for ${functionName}`);
  return '';
}

const paths = {
  error: 'crates/rustok-order/src/error.rs',
  source: 'crates/rustok-order/src/checkout_order_recovery.rs',
  evidence:
    'crates/rustok-order/contracts/evidence/checkout-order-recovery-owner-error-diagnostic-safety-source-review.json',
  doc: 'crates/rustok-order/docs/checkout-order-recovery-owner-error-diagnostic-safety.md',
  plan: 'crates/rustok-commerce/docs/implementation-plan.md',
};

const errorSource = read(paths.error);
const source = read(paths.source);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

for (const marker of [
  'Validation(String)',
  'OrderNotFound(Uuid)',
  'OrderReturnNotFound(Uuid)',
  'OrderChangeNotFound(Uuid)',
  'InvalidTransition { from: String, to: String }',
  'Database(#[from] DbErr)',
  'Core(#[from] rustok_core::Error)',
]) requireText(errorSource, marker, `${paths.error}: retained OrderError variant`);

requireText(
  source,
  'const CHECKOUT_ORDER_RECOVERY_BOUNDARY: &str = "checkout_order_recovery_adapter";',
  `${paths.source}: stable boundary`,
);

const facts = functionBody(source, 'checkout_order_recovery_owner_error_facts');
for (const marker of [
  'CheckoutOrderRecoveryOwnerErrorFacts',
  'OrderError::Database(_) => ("database", 0, 0, 0, 0, true)',
  '"order_not_found"',
  '"validation"',
  '"invalid_transition"',
  '"order_return_not_found"',
  '"order_change_not_found"',
  'OrderError::Core(_) => ("core", 0, 0, 0, 0, true)',
  'value.chars().count()',
  'from.chars().count() + to.chars().count()',
  'if id.is_nil() { 0 } else { 1 }',
]) requireText(facts, marker, `${paths.source}: owner error shape policy`);
requireCount(
  facts,
  'if id.is_nil() { 0 } else { 1 }',
  3,
  `${paths.source}: three UUID-bearing variants`,
);
for (const forbidden of [
  'format!(',
  '.to_string()',
  'error.to_string()',
  'database_error =',
  'core_error =',
  'resource_id =',
]) forbidText(facts, forbidden, `${paths.source}: owner payload values`);

const code = functionBody(source, 'checkout_order_recovery_owner_error_code');
for (const marker of [
  'OrderError::Database(_) => "order.database_unavailable"',
  'OrderError::OrderNotFound(_) => "order.order_not_found"',
  'OrderError::Validation(_) => "order.checkout_recovery_validation"',
  'OrderError::InvalidTransition { .. } => "order.checkout_recovery_state_conflict"',
  'OrderError::OrderReturnNotFound(_) | OrderError::OrderChangeNotFound(_)',
  '"order.related_resource_not_found"',
  'OrderError::Core(_) => "order.invariant_violation"',
]) requireText(code, marker, `${paths.source}: stable mapper code`);

const severity = functionBody(
  source,
  'checkout_order_recovery_owner_error_is_technical',
);
requireText(
  severity,
  'matches!(error, OrderError::Database(_) | OrderError::Core(_))',
  `${paths.source}: technical severity policy`,
);

const logger = functionBody(source, 'log_checkout_order_recovery_owner_error');
for (const marker of [
  'let context_facts = checkout_order_recovery_context_facts(context);',
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
  'traceparent_present = context_facts.traceparent_present',
  'idempotency_key_present = context_facts.idempotency_key_present',
  'deadline_ms = ?context_facts.deadline_ms',
  'owner_error_variant = error_facts.error_variant',
  'owner_error_text_field_count = error_facts.text_field_count',
  'owner_error_text_total_length = error_facts.text_total_length',
  'owner_error_uuid_field_count = error_facts.uuid_field_count',
  'owner_error_uuid_non_nil_count = error_facts.uuid_non_nil_count',
  'owner_error_opaque_payload_present = error_facts.opaque_payload_present',
  'code,',
  'boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY',
  '"order checkout recovery owner operation failed"',
  '"order checkout recovery owner operation was rejected"',
]) requireText(logger, marker, `${paths.source}: bounded owner diagnostics`);
requireCount(logger, 'tracing::error!(', 1, `${paths.source}: technical event count`);
requireCount(logger, 'tracing::warn!(', 1, `${paths.source}: ordinary event count`);
for (const forbidden of [
  'error = ?error',
  'error = %error',
  'cause = %',
  'from = %',
  'to = %',
  'resource_id = %',
  'order_id = %',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
]) forbidText(logger, forbidden, `${paths.source}: raw owner/context payload`);

const mapper = functionBody(source, 'order_error_to_port_error');
for (const marker of [
  'let code = checkout_order_recovery_owner_error_code(&error);',
  'let technical_failure = checkout_order_recovery_owner_error_is_technical(&error);',
  'let error_facts = checkout_order_recovery_owner_error_facts(&error);',
  'log_checkout_order_recovery_owner_error(',
  'OrderError::Database(_) => PortError::unavailable(',
  '"order.database_unavailable"',
  '"order storage is temporarily unavailable"',
  'OrderError::OrderNotFound(_)',
  'PortError::not_found("order.order_not_found", "order was not found")',
  'OrderError::Validation(_) => PortError::validation(',
  '"order.checkout_recovery_validation"',
  '"checkout order recovery request is invalid"',
  'OrderError::InvalidTransition { .. } => PortError::conflict(',
  '"order.checkout_recovery_state_conflict"',
  '"order lifecycle transition conflicts with checkout recovery"',
  'OrderError::OrderReturnNotFound(_) | OrderError::OrderChangeNotFound(_)',
  '"order.related_resource_not_found"',
  '"related order resource was not found"',
  'OrderError::Core(_) => PortError::invariant_violation(',
  '"order.invariant_violation"',
  '"order operation failed an internal invariant"',
]) requireText(mapper, marker, `${paths.source}: preserved public mapping`);
for (const forbidden of [
  'tracing::error!(',
  'tracing::warn!(',
  'error = ?error',
  'cause = %cause',
  'from = %from',
  'to = %to',
  'resource_id = %',
  'tenant_id = %context.tenant_id',
  'channel = ?context.channel',
]) forbidText(mapper, forbidden, `${paths.source}: inline raw mapper diagnostics`);

if (
  evidence.status !==
  'order_checkout_recovery_owner_error_diagnostic_safety_source_reviewed_unvalidated'
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  order_error_variant_count: 7,
  complete_order_error_logged: false,
  database_error_text_logged: false,
  core_error_text_logged: false,
  validation_text_logged: false,
  transition_text_logged: false,
  resource_uuid_values_logged: false,
  raw_context_values_logged_by_mapper: false,
  static_error_variant_logged: true,
  text_field_shape_logged: true,
  uuid_field_shape_logged: true,
  opaque_payload_presence_logged: true,
  correlation_preserved: true,
  owner_operations_preserved: true,
  technical_warning_severity_split_preserved: true,
  public_codes_preserved: true,
  public_kinds_preserved: true,
  public_messages_preserved: true,
  public_retryability_preserved: true,
  recovery_flow_changed: false,
  identity_validation_changed: false,
  causation_validation_changed: false,
  hash_validation_changed: false,
  serde_diagnostics_changed: false,
  lifecycle_diagnostics_changed: false,
  non_mapper_recovery_diagnostics_remaining_open: true,
  commerce_orchestration_changed: false,
  order_status_promoted: false,
  broad_cleanup_remains_open: true,
  runtime_evidence_claimed: false,
})) {
  if (evidence.review_findings?.[key] !== expected) {
    failures.push(`${paths.evidence}: review_findings.${key} must be ${expected}`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push(`${paths.evidence}: execution must remain empty`);
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
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

requireText(doc, 'Status: **source-ready / unvalidated**', `${paths.doc}: status`);
requireText(
  doc,
  'This source slice hardens only the `order_error_to_port_error` mapper',
  `${paths.doc}: bounded scope`,
);
requireText(
  doc,
  'Those non-mapper recovery diagnostics still contain separate raw payload boundaries',
  `${paths.doc}: remaining recovery work`,
);
requireText(
  plan,
  'Finish correlation-safe mapper cleanup for order, payment execution/compensation,',
  `${paths.plan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error('Order checkout recovery owner-error diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Order checkout recovery owner mapping uses closed OrderError variants, aggregate payload shape, bounded context facts, and preserved public PortError contracts',
);
