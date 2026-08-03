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
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
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
  [portImpl, 'parse_tenant_id(&context, ENSURE_OPERATION)', 'ensure context validation'],
  [portImpl, 'require_operation_context(&context, ENSURE_OPERATION', 'ensure causation validation'],
  [portImpl, 'self.ensure(&context, tenant_id, request).await', 'ensure context propagation'],
  [portImpl, 'parse_tenant_id(&context, READ_OPERATION)', 'read context validation'],
  [portImpl, 'require_operation_context(&context, READ_OPERATION', 'read causation validation'],
  [portImpl, 'self.read(&context, tenant_id, request).await', 'read context propagation'],
]) requireText(content, value, label);

for (const [value, label] of [
  ['correlation_id = %context.correlation_id', 'causation correlation log'],
  ['tenant_id_length', 'causation tenant shape'],
  ['actor_kind', 'causation actor shape'],
  ['channel_present', 'causation channel shape'],
  ['causation_id_parse_succeeded', 'causation parse fact'],
  ['causation_id_matches_expected', 'causation match fact'],
  ['expected_checkout_operation_id_non_nil', 'expected operation shape'],
  ['operation = owner_operation', 'causation operation log'],
  ['code = "fulfillment.checkout_operation_id_invalid"', 'causation code log'],
  ['internal_message_present', 'causation message shape'],
  ['error_kind', 'causation closed kind'],
]) requireText(operationContext, value, label);
for (const value of [
  'error = ?error',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'expected_checkout_operation_id = %checkout_operation_id',
  'internal_message = %error.message',
  'error_kind = ?error.kind',
]) forbidText(operationContext, value, 'unsafe causation diagnostic payload');

for (const [value, label] of [
  ['Uuid::parse_str(&context.tenant_id).map_err(|cause| {', 'tenant parsing'],
  ['let parse_cause_type = std::any::type_name_of_val(&cause);', 'tenant parse-cause type'],
  ['let tenant_id_length = context.tenant_id.chars().count();', 'tenant identity shape'],
  ['let tenant_id_parse_failed = true;', 'tenant parse-failure fact'],
  ['let actor_kind = match &context.actor.kind', 'tenant actor shape'],
  ['let actor_id_length = context.actor.id.chars().count();', 'tenant actor identity shape'],
  ['let claim_count = context.claims.len();', 'tenant claim count'],
  ['let role_count = context.roles.len();', 'tenant role count'],
  ['let channel_present = context.channel.is_some();', 'tenant channel shape'],
  ['let locale_length = context.locale.chars().count();', 'tenant locale shape'],
  ['let causation_id_present = context.causation_id.is_some();', 'tenant causation shape'],
  ['let traceparent_present = context.traceparent.is_some();', 'tenant trace shape'],
  ['let idempotency_key_present = context.idempotency_key.is_some();', 'tenant idempotency shape'],
  ['let internal_message_present = !error.message.trim().is_empty();', 'tenant message presence'],
  ['let internal_message_length = error.message.chars().count();', 'tenant message length'],
  ['let error_kind = "validation";', 'tenant closed kind'],
  ['parse_cause_type,', 'tenant type-only cause diagnostic'],
  ['correlation_id = %context.correlation_id', 'tenant correlation log'],
  ['operation = owner_operation', 'tenant operation log'],
  ['validation_phase = "tenant_id"', 'tenant validation phase'],
  ['code = "fulfillment.tenant_id_invalid"', 'tenant code log'],
  ['internal_code = %error.code', 'tenant internal code'],
  ['retryable = error.retryable', 'tenant retryability'],
  ['boundary = CHECKOUT_FULFILLMENT_BOUNDARY', 'tenant boundary'],
  ['error\n    })', 'same tenant error returned'],
]) requireText(tenantParser, value, label);
for (const value of [
  'cause = ?cause',
  'cause = %cause',
  'error = ?error',
  'error = %error',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'internal_message = %error.message',
  'error_kind = ?error.kind',
]) forbidText(tenantParser, value, 'unsafe tenant diagnostic payload');

for (const [value, label] of [
  ['context: &PortContext', 'mapper context input'],
  ["owner_operation: &'static str", 'mapper operation input'],
  ['FulfillmentError::Validation(cause)', 'validation cause capture'],
  ['FulfillmentError::ShippingOptionNotFound(id)', 'shipping option identity capture'],
  ['FulfillmentError::FulfillmentNotFound(id)', 'fulfillment identity capture'],
  ['FulfillmentError::InvalidTransition { from, to }', 'transition cause capture'],
  ['FulfillmentError::Database(error)', 'database cause capture'],
  ['correlation_id = %context.correlation_id', 'mapper correlation log'],
  ['tenant_id = %context.tenant_id', 'mapper tenant log'],
  ['channel = ?context.channel', 'mapper channel log'],
  ['operation = owner_operation', 'mapper operation log'],
  ['code = "fulfillment.checkout_execution_validation"', 'validation stable code log'],
  ['code = "fulfillment.shipping_option_not_found"', 'shipping stable code log'],
  ['code = "fulfillment.fulfillment_not_found"', 'fulfillment stable code log'],
  ['code = "fulfillment.checkout_execution_state_conflict"', 'transition stable code log'],
  ['code = "fulfillment.database_unavailable"', 'database stable code log'],
  ['"checkout fulfillment request is invalid"', 'static validation public message'],
  ['"shipping option was not found"', 'static shipping public message'],
  ['"fulfillment was not found"', 'static fulfillment public message'],
  ['"fulfillment lifecycle conflicts with checkout execution"', 'static transition public message'],
  ['"fulfillment storage is temporarily unavailable"', 'static database public message'],
]) requireText(mapper, value, label);

for (const value of [
  '.map_err(fulfillment_error_to_port_error)',
  'tracing::error!(error = ?error, "checkout fulfillment storage failed")',
  'FulfillmentError::Validation(_) =>',
]) forbidText(source, value, 'context-free checkout fulfillment mapping');

const mapperUses = source.match(/fulfillment_error_to_port_error\(/g) ?? [];
if (mapperUses.length !== 4) {
  failures.push(`expected mapper definition plus three service mappings, found ${mapperUses.length}`);
}

for (const [value, label] of [
  ['pub struct PortContext {', 'shared port context'],
  ['pub correlation_id: String', 'shared correlation field'],
  ['pub channel: Option<String>', 'shared channel field'],
  ['pub struct PortError {', 'shared port error'],
  ['pub fn validation(', 'typed validation constructor'],
  ['pub fn unavailable(', 'typed unavailable constructor'],
]) requireText(portContract, value, label);

if (failures.length > 0) {
  console.error('Fulfillment checkout execution error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Fulfillment checkout execution owner errors retain bounded causation and tenant context with stable public envelopes');
