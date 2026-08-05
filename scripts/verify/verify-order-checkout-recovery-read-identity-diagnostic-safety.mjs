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
    'crates/rustok-order/contracts/evidence/checkout-order-recovery-read-identity-diagnostic-safety-source-review.json',
  doc: 'crates/rustok-order/docs/checkout-order-recovery-read-identity-diagnostic-safety.md',
  plan: 'crates/rustok-commerce/docs/implementation-plan.md',
};

const source = read(paths.source);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

const helper = functionBody(source, 'log_checkout_order_recovery_identity_not_found');
for (const marker of [
  'let context_facts = checkout_order_recovery_context_facts(context);',
  'tracing::warn!(',
  'owner = CHECKOUT_ORDER_RECOVERY_OWNER',
  'operation = READ_OPERATION',
  'correlation_id = %context.correlation_id',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'actor_id_length = context_facts.actor_id_length',
  'claim_count = context_facts.claim_count',
  'role_count = context_facts.role_count',
  'channel_present = context_facts.channel_present',
  'channel_length = ?context_facts.channel_length',
  'context_locale_length = context_facts.locale_length',
  'causation_id_present = context_facts.causation_id_present',
  'traceparent_present = context_facts.traceparent_present',
  'idempotency_key_present = context_facts.idempotency_key_present',
  'deadline_ms = ?context_facts.deadline_ms',
  'checkout_operation_id_non_nil = !request.checkout_operation_id.is_nil()',
  'request_locale_present = request.locale.is_some()',
  'request_locale_length = ?request.locale.as_ref().map(|value| value.chars().count())',
  'request_fallback_locale_present = request.fallback_locale.is_some()',
  'request_fallback_locale_length = ?request',
  'code = "order.checkout_order_not_found"',
  'boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY',
  '"checkout order identity was not found for the requested operation"',
]) requireText(helper, marker, `${paths.source}: bounded read-identity diagnostic`);
requireCount(helper, 'tracing::warn!(', 1, `${paths.source}: warning event count`);
for (const forbidden of [
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'checkout_operation_id = %request.checkout_operation_id',
  'locale = %',
  'fallback_locale = %',
  'request.locale.as_deref()',
  'request.fallback_locale.as_deref()',
  'error = ?error',
  'error = %error',
]) forbidText(helper, forbidden, `${paths.source}: raw read-identity payload`);

const readCheckoutOrder = functionBody(source, 'read_checkout_order');
for (const marker of [
  'context.require_policy(PortCallPolicy::read())?;',
  'let tenant_id = parse_tenant_id(&context, READ_OPERATION)?;',
  '.read_by_operation(',
  'checkout_operation_id: request.checkout_operation_id',
  'log_checkout_order_recovery_identity_not_found(&context, &request);',
  'PortError::not_found(',
  '"order.checkout_order_not_found"',
  '"checkout order was not found for the requested operation"',
  'self.load_order(',
  'request.locale.as_deref()',
  'request.fallback_locale.as_deref()',
]) requireText(readCheckoutOrder, marker, `${paths.source}: preserved projection read`);
for (const forbidden of [
  'tracing::warn!(',
  'tenant_id = %context.tenant_id',
  'channel = ?context.channel',
  'checkout_operation_id = %request.checkout_operation_id',
]) forbidText(readCheckoutOrder, forbidden, `${paths.source}: inline raw read diagnostic`);

if (
  evidence.status !==
  'order_checkout_recovery_read_identity_diagnostic_safety_source_reviewed_unvalidated'
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  read_identity_not_found_event_count: 1,
  raw_tenant_id_logged: false,
  raw_actor_logged: false,
  raw_channel_logged: false,
  raw_checkout_operation_id_logged: false,
  raw_locale_logged: false,
  raw_fallback_locale_logged: false,
  bounded_context_shape_logged: true,
  checkout_operation_non_nil_shape_logged: true,
  locale_presence_length_shape_logged: true,
  fallback_locale_presence_length_shape_logged: true,
  correlation_preserved: true,
  owner_operation_preserved: true,
  warning_severity_preserved: true,
  public_code_preserved: true,
  public_kind_preserved: true,
  public_message_preserved: true,
  public_retryability_preserved: true,
  identity_lookup_order_changed: false,
  read_projection_flow_changed: false,
  owner_service_calls_changed: false,
  admission_diagnostics_changed: false,
  owner_error_mapper_changed: false,
  identity_conflict_diagnostics_changed: false,
  hash_serde_diagnostics_changed: false,
  lifecycle_diagnostics_changed: false,
  other_recovery_diagnostics_remain_open: true,
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
  'The read path now delegates the warning to `log_checkout_order_recovery_identity_not_found`.',
  `${paths.doc}: source change`,
);
requireText(
  doc,
  'The master ecommerce correlation-safe mapper-cleanup item remains open.',
  `${paths.doc}: broad boundary`,
);
requireText(
  plan,
  'Finish correlation-safe mapper cleanup for order, payment execution/compensation,',
  `${paths.plan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error('Order checkout recovery read-identity diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Order checkout recovery missing-identity diagnostics retain bounded context and request shape without raw tenant, channel, locale, or checkout-operation values',
);
