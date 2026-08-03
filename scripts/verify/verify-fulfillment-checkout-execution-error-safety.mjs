#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/rustok-fulfillment/src/checkout_execution.rs');
const portContract = read('crates/rustok-api/src/ports.rs');
const evidence = JSON.parse(
  read(
    'crates/rustok-fulfillment/contracts/evidence/checkout-owner-mapper-diagnostic-safety-source.json',
  ),
);
const doc = read('crates/rustok-fulfillment/docs/checkout-owner-mapper-diagnostic-safety.md');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (content, value) => content.split(value).length - 1;
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};
const from = (content, start, label) => {
  const startIndex = content.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex);
};

const ensure = between(
  source,
  'async fn ensure(',
  'async fn read(',
  'checkout fulfillment ensure helper',
);
const readHelper = between(
  source,
  'async fn read(',
  'async fn find_by_key(',
  'checkout fulfillment read helper',
);
const findHelper = between(
  source,
  'async fn find_by_key(',
  'pub fn in_process_checkout_fulfillment_execution_port(',
  'checkout fulfillment lookup helper',
);
const portImpl = between(
  source,
  'impl CheckoutFulfillmentExecutionPort for InProcessCheckoutFulfillmentExecutionPort {',
  'fn validate_request(',
  'checkout fulfillment port implementation',
);
const operationContext = between(
  source,
  'fn require_operation_context(',
  'fn parse_tenant_id(',
  'checkout operation context validator',
);
const tenantParser = between(
  source,
  'fn parse_tenant_id(',
  'fn fulfillment_error_to_port_error(',
  'checkout tenant context parser',
);
const mapper = from(
  source,
  'fn fulfillment_error_to_port_error(',
  'checkout fulfillment owner mapper',
);

for (const [value, label] of [
  ['const CHECKOUT_FULFILLMENT_OWNER: &str = "rustok_fulfillment";', 'truthful owner constant'],
  [
    'const CHECKOUT_FULFILLMENT_BOUNDARY: &str = "checkout_fulfillment_execution_port";',
    'fulfillment execution boundary',
  ],
  ['const ENSURE_OPERATION: &str = "ensure_checkout_fulfillments";', 'ensure operation constant'],
  ['const READ_OPERATION: &str = "read_checkout_fulfillments";', 'read operation constant'],
  ['use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};', 'typed port imports'],
]) requireText(source, value, label);

for (const [content, value, label] of [
  [ensure, 'context: &PortContext', 'ensure context input'],
  [ensure, '"find_checkout_fulfillment_before_create"', 'pre-create lookup operation'],
  [ensure, '"adopt_checkout_fulfillment_after_create_error"', 'post-error adoption operation'],
  [ensure, '"create_checkout_fulfillment"', 'create operation'],
  [ensure, 'fulfillment_error_to_port_error(', 'create mapper handoff'],
  [readHelper, 'context: &PortContext', 'read context input'],
  [readHelper, '"list_checkout_fulfillments_for_read"', 'read owner operation'],
  [readHelper, 'fulfillment_error_to_port_error(', 'read mapper handoff'],
  [findHelper, 'context: &PortContext', 'lookup context input'],
  [findHelper, "owner_operation: &'static str", 'lookup operation input'],
  [findHelper, "service_operation: &'static str", 'lookup service operation input'],
  [findHelper, 'fulfillment_error_to_port_error(context, service_operation, error)', 'lookup mapper handoff'],
  [portImpl, 'parse_tenant_id(&context, ENSURE_OPERATION)', 'ensure tenant validation'],
  [portImpl, 'require_operation_context(&context, ENSURE_OPERATION', 'ensure causation validation'],
  [portImpl, 'self.ensure(&context, tenant_id, request).await', 'ensure context propagation'],
  [portImpl, 'parse_tenant_id(&context, READ_OPERATION)', 'read tenant validation'],
  [portImpl, 'require_operation_context(&context, READ_OPERATION', 'read causation validation'],
  [portImpl, 'self.read(&context, tenant_id, request).await', 'read context propagation'],
]) requireText(content, value, label);

for (const [content, values, label] of [
  [
    operationContext,
    [
      'causation_id_parse_succeeded',
      'causation_id_matches_expected',
      'expected_checkout_operation_id_non_nil',
      'code = "fulfillment.checkout_operation_id_invalid"',
      'return Err(error);',
    ],
    'bounded causation validator',
  ],
  [
    tenantParser,
    [
      'Uuid::parse_str(&context.tenant_id).map_err(|cause| {',
      'let parse_cause_type = std::any::type_name_of_val(&cause);',
      'let tenant_id_parse_failed = true;',
      'code = "fulfillment.tenant_id_invalid"',
      'error\n    })',
    ],
    'bounded tenant parser',
  ],
]) {
  for (const value of values) requireText(content, value, label);
}

for (const [value, label] of [
  ['context: &PortContext', 'mapper context input'],
  ["owner_operation: &'static str", 'mapper operation input'],
  ['error: FulfillmentError', 'typed owner error input'],
  ['let actor_kind = match &context.actor.kind', 'bounded actor kind'],
  ['let tenant_id_length = context.tenant_id.chars().count();', 'tenant shape'],
  ['let actor_id_length = context.actor.id.chars().count();', 'actor identity shape'],
  ['let claim_count = context.claims.len();', 'claim count'],
  ['let role_count = context.roles.len();', 'role count'],
  ['let channel_present = context.channel.is_some();', 'channel presence'],
  ['let channel_length = context.channel.as_ref()', 'channel length'],
  ['let locale_length = context.locale.chars().count();', 'locale length'],
  ['let causation_id_present = context.causation_id.is_some();', 'causation presence'],
  ['let causation_id_length = context', 'causation length'],
  ['let traceparent_present = context.traceparent.is_some();', 'traceparent presence'],
  ['let traceparent_length = context', 'traceparent length'],
  ['let idempotency_key_present = context.idempotency_key.is_some();', 'idempotency presence'],
  ['let idempotency_key_length = context', 'idempotency length'],
  ['FulfillmentError::Validation(cause)', 'validation branch'],
  ['let validation_cause_present = !cause.trim().is_empty();', 'validation cause presence'],
  ['let validation_cause_length = cause.chars().count();', 'validation cause length'],
  ['owner_error_kind = "validation"', 'validation closed kind'],
  ['FulfillmentError::ShippingOptionNotFound(id)', 'shipping option branch'],
  ['let shipping_option_id_non_nil = !id.is_nil();', 'shipping identity shape'],
  ['owner_error_kind = "shipping_option_not_found"', 'shipping closed kind'],
  ['FulfillmentError::FulfillmentNotFound(id)', 'fulfillment branch'],
  ['let fulfillment_id_non_nil = !id.is_nil();', 'fulfillment identity shape'],
  ['owner_error_kind = "fulfillment_not_found"', 'fulfillment closed kind'],
  ['FulfillmentError::InvalidTransition { from, to }', 'transition branch'],
  ['let transition_from_present = !from.trim().is_empty();', 'transition from presence'],
  ['let transition_from_length = from.chars().count();', 'transition from length'],
  ['let transition_to_present = !to.trim().is_empty();', 'transition to presence'],
  ['let transition_to_length = to.chars().count();', 'transition to length'],
  ['let transition_changes_state = from != to;', 'transition relation fact'],
  ['owner_error_kind = "invalid_transition"', 'transition closed kind'],
  ['FulfillmentError::Database(error)', 'database branch'],
  ['let database_error_type = std::any::type_name_of_val(&error);', 'database type-only cause'],
  ['owner_error_kind = "database"', 'database closed kind'],
  ['owner = CHECKOUT_FULFILLMENT_OWNER', 'truthful owner diagnostic'],
  ['operation = owner_operation', 'exact owner operation diagnostic'],
  ['correlation_id = %context.correlation_id', 'correlation diagnostic'],
  ['deadline_ms = ?context.deadline_ms', 'deadline diagnostic'],
  ['boundary = CHECKOUT_FULFILLMENT_BOUNDARY', 'owner mapper boundary'],
  ['code = "fulfillment.checkout_execution_validation"', 'validation stable code'],
  ['code = "fulfillment.shipping_option_not_found"', 'shipping stable code'],
  ['code = "fulfillment.fulfillment_not_found"', 'fulfillment stable code'],
  ['code = "fulfillment.checkout_execution_state_conflict"', 'transition stable code'],
  ['code = "fulfillment.database_unavailable"', 'database stable code'],
  ['"checkout fulfillment request is invalid"', 'static validation public message'],
  ['"shipping option was not found"', 'static shipping public message'],
  ['"fulfillment was not found"', 'static fulfillment public message'],
  ['"fulfillment lifecycle conflicts with checkout execution"', 'static transition public message'],
  ['"fulfillment storage is temporarily unavailable"', 'static database public message'],
]) requireText(mapper, value, label);

for (const value of [
  'cause = %cause',
  'cause = ?cause',
  'shipping_option_id = %id',
  'fulfillment_id = %id',
  'from = %from',
  'to = %to',
  'error = ?error',
  'error = %error',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
]) forbidText(mapper, value, 'unsafe canonical owner diagnostic payload');

if (countText(mapper, 'tracing::warn!(') !== 4) {
  failures.push('expected exactly four ordinary owner warning branches');
}
if (countText(mapper, 'tracing::error!(') !== 1) {
  failures.push('expected exactly one database owner error branch');
}
for (const marker of [
  'owner = CHECKOUT_FULFILLMENT_OWNER',
  'operation = owner_operation',
  'correlation_id = %context.correlation_id',
  'boundary = CHECKOUT_FULFILLMENT_BOUNDARY',
]) {
  if (countText(mapper, marker) !== 5) {
    failures.push(`all five owner branches must retain ${marker}`);
  }
}

const mapperUses = source.match(/fulfillment_error_to_port_error\(/g) ?? [];
if (mapperUses.length !== 4) {
  failures.push(`expected mapper definition plus three service mappings, found ${mapperUses.length}`);
}
for (const [value, label] of [
  ['"create_checkout_fulfillment"', 'create service operation'],
  ['"list_checkout_fulfillments_for_read"', 'read service operation'],
  ['"find_checkout_fulfillment_before_create"', 'pre-create lookup service operation'],
  ['"adopt_checkout_fulfillment_after_create_error"', 'post-error lookup service operation'],
]) requireText(source, value, label);

for (const value of [
  '.map_err(fulfillment_error_to_port_error)',
  'FulfillmentError::Validation(_) =>',
]) forbidText(source, value, 'context-free checkout fulfillment mapping');

if (evidence.status !== 'fulfillment_checkout_owner_mapper_diagnostic_safety_source_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  canonical_owner_mapper_bounded: true,
  validation_cause_shape_only: true,
  shipping_option_identity_shape_only: true,
  fulfillment_identity_shape_only: true,
  transition_shape_only: true,
  database_cause_type_only: true,
  owner_context_shape_only: true,
  raw_owner_causes_logged: false,
  raw_owner_identifiers_logged: false,
  raw_owner_transition_values_logged: false,
  raw_owner_context_logged: false,
  owner_correlation_preserved: true,
  owner_operations_preserved: true,
  owner_error_kind_closed: true,
  owner_severity_preserved: true,
  stable_public_envelopes_preserved: true,
  service_mapper_call_sites_preserved: true,
  checkout_execution_mapper_cleanup_source_complete: true,
  execution_behavior_changed: false,
  public_port_error_changed: false,
  ffa_promoted: false,
  fba_promoted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push('evidence execution must remain empty');
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
    failures.push(`evidence validation.${key} must remain false`);
  }
}

for (const [value, label] of [
  ['pub struct PortContext {', 'shared port context'],
  ['pub correlation_id: String', 'shared correlation field'],
  ['pub channel: Option<String>', 'shared channel field'],
  ['pub struct PortError {', 'shared port error'],
  ['pub fn validation(', 'typed validation constructor'],
  ['pub fn unavailable(', 'typed unavailable constructor'],
]) requireText(portContract, value, label);

for (const [value, label] of [
  ['Status: **source-ready / unvalidated**', 'documentation status'],
  ['Variant-specific evidence is bounded as follows', 'documentation variant policy'],
  ['The validation, shipping-option not-found, fulfillment not-found, and invalid-transition', 'documentation warning severity'],
  ['The database branch remains a `tracing::error!` event', 'documentation database severity'],
  ['makes the mounted Fulfillment checkout execution diagnostic mapper surface source-complete', 'documentation source completion boundary'],
  ['The broad ecommerce correlation-safe mapper item remains open', 'documentation broad residual'],
]) requireText(doc, value, label);

if (failures.length > 0) {
  console.error('Fulfillment checkout execution owner mapper diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Fulfillment checkout owner mapper diagnostics use bounded variant and context facts while preserving all public envelopes and service mappings',
);
