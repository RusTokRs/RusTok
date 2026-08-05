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
    'crates/rustok-order/contracts/evidence/checkout-order-recovery-hash-serde-diagnostic-safety-source-review.json',
  doc: 'crates/rustok-order/docs/checkout-order-recovery-hash-serde-diagnostic-safety.md',
  plan: 'crates/rustok-commerce/docs/implementation-plan.md',
};

const source = read(paths.source);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

const requestHashes = functionBody(source, 'checkout_request_hashes');
for (const marker of [
  'let snapshot = serde_json::json!({',
  '"cart_id": request.cart_id',
  '"line_items": request.line_items',
  '"adjustments": request.adjustments',
  '"tax_lines": request.tax_lines',
  'serde_json::to_value(request).map_err(|_|',
  'log_checkout_order_recovery_encoding_failure(',
  'RECOVER_OPERATION',
  '"checkout_completion_request"',
  'PortError::invariant_violation(',
  '"order.checkout_request_encoding_failed"',
  '"checkout completion request could not be encoded"',
  'hash_json(context, "encode_checkout_snapshot_hash", snapshot)',
  'hash_json(context, "encode_checkout_request_hash", full_request)',
]) requireText(requestHashes, marker, `${paths.source}: preserved request hashing`);
for (const forbidden of [
  'error = ?error',
  'error = %error',
  'tenant_id = %context.tenant_id',
  'channel = ?context.channel',
  'tracing::error!(',
]) forbidText(requestHashes, forbidden, `${paths.source}: inline request serde payload`);

const hashJson = functionBody(source, 'hash_json');
for (const marker of [
  'let canonical = canonicalize_json(value);',
  'serde_json::to_vec(&canonical).map_err(|_|',
  'log_checkout_order_recovery_encoding_failure(',
  '"canonical_checkout_json"',
  'PortError::invariant_violation(',
  '"order.checkout_request_encoding_failed"',
  '"checkout completion request could not be encoded"',
  'hex::encode(Sha256::digest(bytes))',
]) requireText(hashJson, marker, `${paths.source}: preserved canonical hashing`);
for (const forbidden of [
  'error = ?error',
  'error = %error',
  'tenant_id = %context.tenant_id',
  'channel = ?context.channel',
  'tracing::error!(',
]) forbidText(hashJson, forbidden, `${paths.source}: inline canonical serde payload`);

const canonicalize = functionBody(source, 'canonicalize_json');
for (const marker of [
  'Value::Object(values) => Value::Object(',
  '.map(|(key, value)| (key, canonicalize_json(value)))',
  '.collect::<BTreeMap<_, _>>()',
  'Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect())',
]) requireText(canonicalize, marker, `${paths.source}: canonicalization policy`);

const normalize = functionBody(source, 'normalize_hash');
for (const marker of [
  'let value = value.trim().to_ascii_lowercase();',
  'let value_length = value.len();',
  'let length_within_bounds = (min_len..=max_len).contains(&value_length);',
  'let ascii_hex = value.bytes().all(|byte| byte.is_ascii_hexdigit());',
  'if !length_within_bounds || !ascii_hex',
  'log_checkout_order_recovery_hash_rejection(',
  'PortError::validation(',
  '"order.checkout_hash_invalid"',
  '"checkout hash evidence is invalid"',
  'Ok(value)',
]) requireText(normalize, marker, `${paths.source}: preserved hash normalization`);
for (const forbidden of [
  'tracing::warn!(',
  'tenant_id = %context.tenant_id',
  'channel = ?context.channel',
  'value = %',
  'hash = %',
]) forbidText(normalize, forbidden, `${paths.source}: inline raw hash diagnostic`);

const encodingLogger = functionBody(
  source,
  'log_checkout_order_recovery_encoding_failure',
);
for (const marker of [
  'let context_facts = checkout_order_recovery_context_facts(context);',
  'tracing::error!(',
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
  'serialization_target,',
  'code = "order.checkout_request_encoding_failed"',
  'boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY',
  '"checkout recovery request encoding failed with bounded diagnostics"',
]) requireText(encodingLogger, marker, `${paths.source}: bounded serde diagnostic`);
requireCount(encodingLogger, 'tracing::error!(', 1, `${paths.source}: serde event count`);
for (const forbidden of [
  'error = ?error',
  'error = %error',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
]) forbidText(encodingLogger, forbidden, `${paths.source}: raw serde/context payload`);

const hashLogger = functionBody(source, 'log_checkout_order_recovery_hash_rejection');
for (const marker of [
  'let context_facts = checkout_order_recovery_context_facts(context);',
  'tracing::warn!(',
  'owner = CHECKOUT_ORDER_RECOVERY_OWNER',
  'operation,',
  'correlation_id = %context.correlation_id',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'channel_present = context_facts.channel_present',
  'field,',
  'value_length,',
  'min_len,',
  'max_len,',
  'length_within_bounds,',
  'ascii_hex,',
  'code = "order.checkout_hash_invalid"',
  'boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY',
  '"checkout recovery rejected invalid hash evidence with bounded diagnostics"',
]) requireText(hashLogger, marker, `${paths.source}: bounded hash diagnostic`);
requireCount(hashLogger, 'tracing::warn!(', 1, `${paths.source}: hash event count`);
for (const forbidden of [
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'value = %',
  'hash = %',
  'error = ?error',
]) forbidText(hashLogger, forbidden, `${paths.source}: raw hash/context payload`);

if (
  evidence.status !==
  'order_checkout_recovery_hash_serde_diagnostic_safety_source_reviewed_unvalidated'
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  serde_failure_event_count: 2,
  hash_rejection_event_count: 1,
  serde_error_payload_logged: false,
  raw_context_values_logged: false,
  raw_hash_values_logged: false,
  bounded_context_shape_logged: true,
  serialization_target_logged: true,
  hash_field_logged: true,
  hash_length_bounds_logged: true,
  hash_ascii_hex_shape_logged: true,
  correlation_preserved: true,
  owner_operations_preserved: true,
  serde_error_severity_preserved: true,
  hash_error_severity_preserved: true,
  public_codes_preserved: true,
  public_kinds_preserved: true,
  public_messages_preserved: true,
  public_retryability_preserved: true,
  canonicalization_changed: false,
  sha256_changed: false,
  hash_normalization_changed: false,
  hash_acceptance_policy_changed: false,
  request_hash_shape_changed: false,
  identity_validation_changed: false,
  recovery_flow_changed: false,
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
  'Serialization failures now delegate to `log_checkout_order_recovery_encoding_failure`.',
  `${paths.doc}: serde source change`,
);
requireText(
  doc,
  'Hash rejection now computes the same length and ASCII-hex acceptance facts explicitly',
  `${paths.doc}: hash source change`,
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
  console.error('Order checkout recovery hash/serde diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Order checkout recovery hash/serde diagnostics retain bounded context, closed targets, and exact hash acceptance without raw serializer or identity payloads',
);
