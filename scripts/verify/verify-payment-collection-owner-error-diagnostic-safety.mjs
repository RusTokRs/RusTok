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
  error: 'crates/rustok-payment/src/error.rs',
  mapper: 'crates/rustok-payment/src/ports.rs',
  evidence:
    'crates/rustok-payment/contracts/evidence/payment-collection-owner-error-diagnostic-safety-source.json',
  doc: 'crates/rustok-payment/docs/payment-collection-owner-error-diagnostic-safety.md',
  plan: 'crates/rustok-commerce/docs/implementation-plan.md',
};

const errorSource = read(paths.error);
const mapperSource = read(paths.mapper);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

for (const marker of [
  'Validation(String)',
  'PaymentCollectionNotFound(Uuid)',
  'PaymentNotFound(Uuid)',
  'RefundNotFound(Uuid)',
  'InvalidTransition { from: String, to: String }',
  'ProviderUnavailable {',
  'ProviderRejected {',
  'ProviderInvalidResponse {',
  'ProviderOutcomeUnknown {',
  'ProviderConfiguration { provider_id: String }',
  'Database(#[from] DbErr)',
]) requireText(errorSource, marker, `${paths.error}: retained PaymentError variant`);

const facts = functionBody(mapperSource, 'payment_collection_owner_error_facts');
for (const marker of [
  'PaymentCollectionOwnerErrorFacts',
  '"validation"',
  '"payment_collection_not_found"',
  '"payment_not_found"',
  '"refund_not_found"',
  '"invalid_transition"',
  '"provider_unavailable"',
  '"provider_rejected"',
  '"provider_invalid_response"',
  '"provider_outcome_unknown"',
  '"provider_configuration"',
  'crate::PaymentError::Database(_) => ("database", 0, 0, 0, 0, true)',
  'value.chars().count()',
  'from.chars().count() + to.chars().count()',
  'provider_id.chars().count() + operation.chars().count()',
  'if id.is_nil() { 0 } else { 1 }',
]) requireText(facts, marker, `${paths.mapper}: owner error shape policy`);
for (const forbidden of [
  'format!(',
  '.to_string()',
  'error.to_string()',
  'provider_id =',
  'provider_operation =',
  'database_error =',
]) forbidText(facts, forbidden, `${paths.mapper}: owner payload values`);
requireCount(
  facts,
  'if id.is_nil() { 0 } else { 1 }',
  3,
  `${paths.mapper}: three UUID-bearing variants`,
);

const stableCode = functionBody(mapperSource, 'payment_collection_owner_error_code');
for (const marker of [
  'crate::PaymentError::Validation(_) => "payment.validation"',
  'crate::PaymentError::PaymentCollectionNotFound(_) => "payment.collection_not_found"',
  'crate::PaymentError::PaymentNotFound(_) => "payment.payment_not_found"',
  'crate::PaymentError::RefundNotFound(_) => "payment.refund_not_found"',
  'crate::PaymentError::InvalidTransition { .. } => "payment.invalid_transition"',
  'crate::PaymentError::ProviderUnavailable { .. } => "payment.provider_unavailable"',
  'crate::PaymentError::ProviderRejected { .. } => "payment.provider_rejected"',
  '"payment.provider_invalid_response"',
  '"payment.provider_outcome_unknown"',
  'crate::PaymentError::ProviderConfiguration { .. } => "payment.provider_not_configured"',
  'crate::PaymentError::Database(_) => "payment.database_unavailable"',
]) requireText(stableCode, marker, `${paths.mapper}: stable owner code`);

const severity = functionBody(
  mapperSource,
  'payment_collection_owner_error_is_technical',
);
for (const marker of [
  'crate::PaymentError::ProviderUnavailable { .. }',
  'crate::PaymentError::ProviderInvalidResponse { .. }',
  'crate::PaymentError::ProviderOutcomeUnknown { .. }',
  'crate::PaymentError::ProviderConfiguration { .. }',
  'crate::PaymentError::Database(_)',
]) requireText(severity, marker, `${paths.mapper}: technical severity policy`);

const mapper = functionBody(mapperSource, 'payment_error_to_port_error');
for (const marker of [
  'let code = payment_collection_owner_error_code(&error);',
  'let technical_failure = payment_collection_owner_error_is_technical(&error);',
  'let error_facts = payment_collection_owner_error_facts(&error);',
  'owner = PAYMENT_COLLECTION_OWNER',
  'owner_error_variant = error_facts.error_variant',
  'owner_error_text_field_count = error_facts.text_field_count',
  'owner_error_text_total_length = error_facts.text_total_length',
  'owner_error_uuid_field_count = error_facts.uuid_field_count',
  'owner_error_uuid_non_nil_count = error_facts.uuid_non_nil_count',
  'owner_error_opaque_payload_present = error_facts.opaque_payload_present',
  'correlation_id = %context.correlation_id',
  'tenant_id_length',
  'actor_kind',
  'actor_id_length',
  'claim_count',
  'role_count',
  'channel_present',
  'locale_length',
  'causation_id_present',
  'traceparent_present',
  'idempotency_key_present',
  'deadline_ms = ?context.deadline_ms',
  'operation = owner_operation',
  'code,',
  'boundary = PAYMENT_COLLECTION_PORT_BOUNDARY',
  '"payment collection owner operation failed"',
  '"payment collection owner operation was rejected"',
]) requireText(mapper, marker, `${paths.mapper}: safe owner mapper diagnostics`);
for (const forbidden of [
  'error = ?error',
  'error = %error',
  'cause = %message',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'provider_id = %provider_id',
  'provider_operation = %operation',
  'from = %from',
  'to = %to',
]) forbidText(mapper, forbidden, `${paths.mapper}: complete owner payload diagnostics`);
requireCount(mapper, 'tracing::error!(', 1, `${paths.mapper}: technical event count`);
requireCount(mapper, 'tracing::warn!(', 1, `${paths.mapper}: ordinary event count`);

for (const marker of [
  'PortError::validation("payment.validation", "payment request is invalid")',
  '"payment.collection_not_found",\n            "payment collection was not found"',
  'PortError::not_found("payment.payment_not_found", "payment was not found")',
  'PortError::not_found("payment.refund_not_found", "refund was not found")',
  '"payment.invalid_transition",\n            "payment lifecycle conflicts with the requested operation"',
  '"payment.provider_unavailable",\n            "payment provider is temporarily unavailable"',
  '"payment.provider_rejected",\n            "payment provider rejected the requested operation"',
  '"payment.provider_invalid_response",\n            "payment provider response could not be applied safely"',
  '"payment.provider_outcome_unknown",\n            "payment provider outcome requires reconciliation"',
  '"payment.provider_not_configured",\n            "payment provider is not configured for the requested operation"',
  '"payment.database_unavailable",\n            "payment storage is temporarily unavailable"',
]) requireText(mapper, marker, `${paths.mapper}: preserved public mapping`);
for (const forbidden of [
  'format!("payment collection {id} not found")',
  'format!("payment for collection {id} not found")',
  'format!("refund {id} not found")',
]) forbidText(mapper, forbidden, `${paths.mapper}: identifier-bearing public message`);

if (
  evidence.status !==
  'payment_collection_owner_error_diagnostic_safety_source_unvalidated'
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  payment_error_variant_count: 11,
  complete_payment_error_logged: false,
  database_error_text_logged: false,
  validation_text_logged: false,
  transition_text_logged: false,
  provider_id_text_logged: false,
  provider_operation_text_logged: false,
  raw_context_values_logged: false,
  static_error_variant_logged: true,
  text_field_shape_logged: true,
  uuid_field_shape_logged: true,
  opaque_database_payload_presence_logged: true,
  correlation_preserved: true,
  owner_operations_preserved: true,
  severity_split_preserved: true,
  static_not_found_public_messages: true,
  public_codes_kinds_retryability_preserved: true,
  collection_flow_changed: false,
  admission_mapper_changed: false,
  tenant_parser_changed: false,
  commerce_orchestration_changed: false,
  ffa_promoted: false,
  fba_promoted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
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
  'runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

requireText(doc, 'Status: **source-ready / unvalidated**', `${paths.doc}: status`);
requireText(
  doc,
  'The mapper records only a closed variant and aggregate field shape',
  `${paths.doc}: bounded policy`,
);
requireText(
  doc,
  'Payment collection not-found envelopes no longer interpolate owner UUIDs',
  `${paths.doc}: static not-found messages`,
);
requireText(
  plan,
  'Finish correlation-safe mapper cleanup',
  `${paths.plan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error('Payment collection owner-error diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Payment collection owner mapping uses bounded PaymentError/context shape, static public not-found messages, and stable correlation-aware severity',
);
