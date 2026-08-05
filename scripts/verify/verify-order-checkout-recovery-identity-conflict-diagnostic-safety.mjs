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
    'crates/rustok-order/contracts/evidence/checkout-order-recovery-identity-conflict-diagnostic-safety-source-review.json',
  doc: 'crates/rustok-order/docs/checkout-order-recovery-identity-conflict-diagnostic-safety.md',
  plan: 'crates/rustok-commerce/docs/implementation-plan.md',
};

const source = read(paths.source);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

const validator = functionBody(source, 'validate_identity');
for (const marker of [
  'let tenant_matches = identity.tenant_id == tenant_id;',
  'let checkout_operation_matches =',
  'let source_cart_matches = identity',
  'let payment_collection_matches = identity',
  'let shipping_option_matches = identity',
  'let base_matches = tenant_matches',
  'let owner_hashes_match =',
  'let legacy_hashes_match =',
  'if !base_matches || !(owner_hashes_match || legacy_hashes_match)',
  'log_checkout_order_recovery_identity_conflict(',
  'PortError::conflict(',
  '"order.checkout_request_conflict"',
  '"checkout operation is already bound to a different completion request"',
]) requireText(validator, marker, `${paths.source}: preserved identity policy`);
for (const forbidden of [
  'tracing::error!(',
  'tenant_id = %context.tenant_id',
  'channel = ?context.channel',
  'request_checkout_operation_id = %',
  'request_cart_id = %',
  'identity_order_id = %',
]) forbidText(validator, forbidden, `${paths.source}: inline raw conflict diagnostic`);

const logger = functionBody(source, 'log_checkout_order_recovery_identity_conflict');
for (const marker of [
  'let context_facts = checkout_order_recovery_context_facts(context);',
  'tracing::error!(',
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
  'traceparent_present = context_facts.traceparent_present',
  'idempotency_key_present = context_facts.idempotency_key_present',
  'deadline_ms = ?context_facts.deadline_ms',
  'request_checkout_operation_id_non_nil = !request.checkout_operation_id.is_nil()',
  'request_cart_id_non_nil = !request.completion.cart_id.is_nil()',
  'request_payment_collection_id_present = request.completion.payment_collection_id.is_some()',
  'request_shipping_option_id_present = request.completion.shipping_option_id.is_some()',
  'identity_tenant_id_non_nil = !identity.tenant_id.is_nil()',
  'identity_checkout_operation_id_non_nil = !identity.checkout_operation_id.is_nil()',
  'identity_order_id_non_nil = !identity.order_id.is_nil()',
  'identity_source_cart_id_present = identity.source_cart_id.is_some()',
  'identity_payment_collection_id_present = identity.payment_collection_id.is_some()',
  'identity_shipping_option_id_present = identity.shipping_option_id.is_some()',
  'identity_snapshot_hash_present = identity.snapshot_hash.is_some()',
  'identity_snapshot_hash_length = ?identity.snapshot_hash.as_ref().map(String::len)',
  'identity_request_hash_present = identity.request_hash.is_some()',
  'identity_request_hash_length = ?identity.request_hash.as_ref().map(String::len)',
  'tenant_matches,',
  'checkout_operation_matches,',
  'source_cart_matches,',
  'payment_collection_matches,',
  'shipping_option_matches,',
  'base_matches,',
  'owner_hashes_match,',
  'legacy_hashes_match,',
  'code = "order.checkout_request_conflict"',
  'boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY',
  '"checkout recovery identity conflicts with the completion request"',
]) requireText(logger, marker, `${paths.source}: bounded identity-conflict diagnostic`);
requireCount(logger, 'tracing::error!(', 1, `${paths.source}: conflict event count`);
for (const forbidden of [
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
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
  'snapshot_hash = %',
  'request_hash = %',
  'error = ?error',
  'error = %error',
]) forbidText(logger, forbidden, `${paths.source}: raw conflict payload`);

if (
  evidence.status !==
  'order_checkout_recovery_identity_conflict_diagnostic_safety_source_reviewed_unvalidated'
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  identity_conflict_event_count: 1,
  raw_context_values_logged: false,
  raw_request_uuid_values_logged: false,
  raw_identity_uuid_values_logged: false,
  raw_hash_values_logged: false,
  bounded_context_shape_logged: true,
  request_uuid_presence_non_nil_shape_logged: true,
  identity_uuid_presence_non_nil_shape_logged: true,
  identity_hash_presence_length_shape_logged: true,
  five_identity_match_facts_logged: true,
  base_match_fact_logged: true,
  owner_hash_match_fact_logged: true,
  legacy_hash_match_fact_logged: true,
  correlation_preserved: true,
  owner_operation_preserved: true,
  error_severity_preserved: true,
  public_code_preserved: true,
  public_kind_preserved: true,
  public_message_preserved: true,
  public_retryability_preserved: true,
  identity_acceptance_policy_changed: false,
  identity_lookup_or_adoption_changed: false,
  recovery_order_changed: false,
  admission_diagnostics_changed: false,
  read_identity_diagnostics_changed: false,
  owner_error_mapper_changed: false,
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
  '`validate_identity` now keeps the five component equality checks explicit',
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
  console.error('Order checkout recovery identity-conflict diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Order checkout recovery identity conflicts retain bounded context, UUID/hash shape, and exact match facts without raw identity payloads',
);
