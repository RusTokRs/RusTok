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
  source: 'crates/rustok-order/src/checkout_order_recovery.rs',
  evidence:
    'crates/rustok-order/contracts/evidence/checkout-order-recovery-lifecycle-diagnostic-safety-source-review.json',
  doc: 'crates/rustok-order/docs/checkout-order-recovery-lifecycle-diagnostic-safety.md',
  plan: 'crates/rustok-commerce/docs/implementation-plan.md',
};

const source = read(paths.source);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

const resumeOrder = functionBody(source, 'resume_order');
for (const marker of [
  'OrderStatusKind::Pending =>',
  '.confirm_order(tenant_id, actor_id, order.id)',
  'get_order_with_locale_fallback(',
  'OrderStatusKind::Confirmed',
  '| OrderStatusKind::Paid',
  '| OrderStatusKind::Shipped',
  '| OrderStatusKind::Delivered => Ok(order)',
  'OrderStatusKind::Cancelled =>',
  'log_checkout_order_recovery_lifecycle_rejection(',
  '"cancelled"',
  '"order.checkout_order_cancelled"',
  'false,',
  'PortError::conflict(',
  '"checkout order is already cancelled"',
  'OrderStatusKind::Unknown =>',
  '"unknown"',
  '"order.checkout_order_status_invalid"',
  'true,',
  'PortError::invariant_violation(',
  '"checkout order has an unsupported lifecycle state"',
]) requireText(resumeOrder, marker, `${paths.source}: preserved lifecycle flow`);
requireCount(
  resumeOrder,
  'log_checkout_order_recovery_lifecycle_rejection(',
  2,
  `${paths.source}: lifecycle logger calls`,
);
for (const forbidden of [
  'tracing::warn!(',
  'tracing::error!(',
  'tenant_id = %context.tenant_id',
  'channel = ?context.channel',
  'order_id = %order.id',
  'order_state = ?OrderStatusKind::Cancelled',
  'order_state = ?OrderStatusKind::Unknown',
]) forbidText(resumeOrder, forbidden, `${paths.source}: inline raw lifecycle diagnostic`);

const lifecycleLogger = functionBody(
  source,
  'log_checkout_order_recovery_lifecycle_rejection',
);
for (const marker of [
  'let context_facts = checkout_order_recovery_context_facts(context);',
  'if technical_failure',
  'tracing::error!(',
  'tracing::warn!(',
  'owner = CHECKOUT_ORDER_RECOVERY_OWNER',
  'operation = RECOVER_OPERATION',
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
  'order_id_non_nil = !order_id.is_nil()',
  'lifecycle_state,',
  'code,',
  'boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY',
  '"checkout recovery found an unsupported order lifecycle state"',
  '"checkout recovery found a terminal order lifecycle state"',
]) requireText(lifecycleLogger, marker, `${paths.source}: bounded lifecycle diagnostic`);
requireCount(
  lifecycleLogger,
  'tracing::error!(',
  1,
  `${paths.source}: unknown lifecycle event count`,
);
requireCount(
  lifecycleLogger,
  'tracing::warn!(',
  1,
  `${paths.source}: cancelled lifecycle event count`,
);
for (const forbidden of [
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'order_id = %order_id',
  'order_id = ?order_id',
  'order_state = ?',
  'OrderStatusKind::Cancelled',
  'OrderStatusKind::Unknown',
]) forbidText(lifecycleLogger, forbidden, `${paths.source}: raw lifecycle payload`);

for (const forbidden of [
  'tenant_id = %context.tenant_id',
  'channel = ?context.channel',
  'error = ?error',
  'error = %error',
  'request_checkout_operation_id = %',
  'request_cart_id = %',
  'request_payment_collection_id = ?',
  'request_shipping_option_id = ?',
  'identity_tenant_id = %',
  'identity_checkout_operation_id = %',
  'identity_order_id = %',
  'identity_source_cart_id = ?',
  'identity_payment_collection_id = ?',
  'identity_shipping_option_id = ?',
  'order_id = %order.id',
  'order_state = ?OrderStatusKind::',
]) forbidText(source, forbidden, `${paths.source}: checkout recovery raw diagnostic residue`);

if (
  evidence.status !==
  'order_checkout_recovery_lifecycle_diagnostic_safety_source_reviewed_unvalidated'
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  cancelled_lifecycle_event_count: 1,
  unknown_lifecycle_event_count: 1,
  raw_context_values_logged: false,
  raw_order_uuid_logged: false,
  debug_lifecycle_enum_logged: false,
  bounded_context_shape_logged: true,
  order_uuid_non_nil_shape_logged: true,
  closed_lifecycle_state_logged: true,
  correlation_preserved: true,
  owner_operation_preserved: true,
  cancelled_warning_severity_preserved: true,
  unknown_error_severity_preserved: true,
  cancelled_public_code_preserved: true,
  cancelled_public_kind_preserved: true,
  cancelled_public_message_preserved: true,
  cancelled_public_retryability_preserved: true,
  unknown_public_code_preserved: true,
  unknown_public_kind_preserved: true,
  unknown_public_message_preserved: true,
  unknown_public_retryability_preserved: true,
  pending_confirmation_flow_changed: false,
  successful_terminal_flow_changed: false,
  order_loading_changed: false,
  identity_validation_changed: false,
  hash_serde_diagnostics_changed: false,
  other_checkout_recovery_raw_diagnostics_remaining: false,
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
  'Both branches now delegate to `log_checkout_order_recovery_lifecycle_rejection`.',
  `${paths.doc}: source change`,
);
requireText(
  doc,
  'This completes the source-level raw diagnostic cleanup identified in `checkout_order_recovery.rs`.',
  `${paths.doc}: local completion boundary`,
);
requireText(
  doc,
  'does not close the master ecommerce correlation-safe mapper-cleanup item',
  `${paths.doc}: broad boundary`,
);
requireText(
  plan,
  'Finish correlation-safe mapper cleanup for order, payment execution/compensation,',
  `${paths.plan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error('Order checkout recovery lifecycle diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Order checkout recovery lifecycle diagnostics retain bounded context, closed state, severity, and public envelopes without raw lifecycle payloads',
);
