#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const boundary = read(
  'crates/rustok-commerce/src/graphql/safe_query/source/fulfillment_query_boundary.rs',
);
const service = read(
  'crates/rustok-commerce/src/graphql/safe_query/source/fulfillment_query_service.rs',
);
const shim = read(
  'crates/rustok-commerce/src/graphql/safe_query/source/rustok_fulfillment_shim.rs',
);
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
const requireBefore = (content, first, second, label) => {
  const firstIndex = content.indexOf(first);
  const secondIndex = content.indexOf(second);
  if (firstIndex < 0 || secondIndex < 0 || firstIndex > secondIndex) {
    failures.push(`${label}: ${first} must precede ${second}`);
  }
};

const helperBlock = between(
  boundary,
  'struct FulfillmentQueryDiagnosticError;',
  '#[allow(clippy::too_many_arguments)]\nfn log_shipping_option_port_error(',
  'diagnostic helper block',
);
const shippingLog = between(
  boundary,
  'fn log_shipping_option_port_error(',
  '#[allow(clippy::too_many_arguments)]\nfn log_fulfillment_port_error(',
  'shipping-option logger',
);
const fulfillmentLog = boundary.slice(
  boundary.indexOf('fn log_fulfillment_port_error('),
);
if (!fulfillmentLog) failures.push('fulfillment logger: unable to isolate source block');

for (const [value, label] of [
  ['struct FulfillmentQueryDiagnosticError;', 'diagnostic token'],
  ['impl std::fmt::Debug for FulfillmentQueryDiagnosticError', 'diagnostic Debug'],
  ['formatter.write_str("redacted")', 'redacted Debug output'],
  ['struct FulfillmentQueryContextFacts {', 'bounded context facts'],
  ['tenant_id_length: usize', 'tenant length fact'],
  ['actor_kind: &\'static str', 'actor kind fact'],
  ['actor_id_length: usize', 'actor length fact'],
  ['claim_count: usize', 'claim count fact'],
  ['role_count: usize', 'role count fact'],
  ['correlation_id_length: usize', 'correlation length fact'],
  ['context_locale_length: usize', 'locale length fact'],
  ['channel_present: bool', 'channel presence fact'],
  ['channel_length: Option<usize>', 'channel length fact'],
  ['deadline_ms: Option<u64>', 'deadline fact'],
  ['fn fulfillment_query_context_facts(', 'facts projection'],
  ['::rustok_api::PortActorKind::User => "user"', 'user actor projection'],
  ['::rustok_api::PortActorKind::Service => "service"', 'service actor projection'],
  ['::rustok_api::PortActorKind::System => "system"', 'system actor projection'],
  ['fn optional_uuid_shape(value: Option<Uuid>)', 'UUID shape helper'],
  ['None => "absent"', 'absent UUID shape'],
  ['Some(value) if value.is_nil() => "nil"', 'nil UUID shape'],
  ['Some(_) => "non_nil"', 'non-nil UUID shape'],
  ['fn text_presence_shape(value: &str)', 'text presence helper'],
]) requireText(helperBlock, value, label);

for (const [content, label, values] of [
  [
    shippingLog,
    'shipping-option logger',
    [
      'let facts = fulfillment_query_context_facts(context);',
      'let shipping_option_id_shape = optional_uuid_shape(shipping_option_id);',
      'let owner_message_presence = text_presence_shape(&error.message);',
      'let owner_message_length = error.message.chars().count();',
      'let diagnostic_error = FulfillmentQueryDiagnosticError;',
      'error = ?diagnostic_error',
      'tenant_id_length = facts.tenant_id_length',
      'actor_kind = facts.actor_kind',
      'actor_id_length = facts.actor_id_length',
      'claim_count = facts.claim_count',
      'role_count = facts.role_count',
      'correlation_id_length = facts.correlation_id_length',
      'context_locale_length = facts.context_locale_length',
      'channel_present = facts.channel_present',
      'channel_length = ?facts.channel_length',
      'deadline_ms = ?facts.deadline_ms',
      'shipping_option_id_shape',
      'requested_locale_length = requested_locale.map(str::len)',
      'tenant_default_locale_length = tenant_default_locale.map(str::len)',
      'owner_code = %error.code',
      'owner_kind = error_kind',
      'owner_message_presence',
      'owner_message_length',
      'owner_retryable = error.retryable',
      'public_code',
      'public_retryable',
      'boundary = GRAPHQL_QUERY_FULFILLMENT_BOUNDARY',
    ],
  ],
  [
    fulfillmentLog,
    'fulfillment logger',
    [
      'let facts = fulfillment_query_context_facts(context);',
      'let error_kind = port_error_kind_name(&error.kind);',
      'let fulfillment_id_shape = optional_uuid_shape(fulfillment_id);',
      'let order_id_shape = optional_uuid_shape(order_id);',
      'let owner_message_presence = text_presence_shape(&error.message);',
      'let owner_message_length = error.message.chars().count();',
      'let public_message_presence = text_presence_shape(public_message);',
      'let public_message_length = public_message.chars().count();',
      'let diagnostic_error = FulfillmentQueryDiagnosticError;',
      'error = ?diagnostic_error',
      'tenant_id_length = facts.tenant_id_length',
      'actor_kind = facts.actor_kind',
      'actor_id_length = facts.actor_id_length',
      'correlation_id_length = facts.correlation_id_length',
      'deadline_ms = ?facts.deadline_ms',
      'fulfillment_id_shape',
      'order_id_shape',
      'owner_code = %error.code',
      'owner_kind = error_kind',
      'owner_message_presence',
      'owner_message_length',
      'owner_retryable = error.retryable',
      'public_message_presence',
      'public_message_length',
      'public_code',
      'public_retryable',
      'boundary = GRAPHQL_QUERY_FULFILLMENT_BOUNDARY',
    ],
  ],
]) {
  for (const value of values) requireText(content, value, label);
  requireText(content, 'tracing::error!(', `${label} technical severity`);
  requireText(content, 'tracing::warn!(', `${label} rejection severity`);
  requireBefore(
    content,
    'let diagnostic_error = FulfillmentQueryDiagnosticError;',
    'tracing::error!(',
    `${label} redaction ordering`,
  );
}

for (const [content, label] of [
  [shippingLog, 'shipping-option logger'],
  [fulfillmentLog, 'fulfillment logger'],
]) {
  for (const value of [
    'error = ?error',
    'error = %error',
    'correlation_id = %context.correlation_id',
    'tenant_id = %context.tenant_id',
    'actor = ?context.actor',
    'owner_kind = ?error.kind',
    'owner_message = %error.message',
    'message = %error.message',
    'public_message,',
  ]) forbidText(content, value, `${label} raw diagnostic`);
}
for (const value of [
  'shipping_option_id = ?shipping_option_id',
  'shipping_option_id = %shipping_option_id',
]) forbidText(shippingLog, value, 'shipping-option raw identity');
for (const value of [
  'fulfillment_id = ?fulfillment_id',
  'fulfillment_id = %fulfillment_id',
  'order_id = ?order_id',
  'order_id = %order_id',
]) forbidText(fulfillmentLog, value, 'fulfillment raw identity');

for (const [pattern, expected, label] of [
  [/let diagnostic_error = FulfillmentQueryDiagnosticError;/g, 2, 'diagnostic token count'],
  [/error = \?diagnostic_error/g, 4, 'redacted error field count'],
  [/tracing::error!\(/g, 2, 'technical event count'],
  [/tracing::warn!\(/g, 2, 'rejection event count'],
]) {
  const count = boundary.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [content, value, label] of [
  [boundary, 'fn public_fulfillment_port_policy(', 'typed public policy'],
  [boundary, 'PortErrorKind::Validation', 'validation classification'],
  [boundary, 'PortErrorKind::NotFound', 'not-found classification'],
  [boundary, 'PortErrorKind::Conflict', 'conflict classification'],
  [boundary, 'PortErrorKind::Unavailable | PortErrorKind::Timeout', 'unavailable classification'],
  [boundary, 'PortErrorKind::Forbidden', 'forbidden classification'],
  [boundary, 'PortErrorKind::InvariantViolation', 'invariant classification'],
  [boundary, '"FULFILLMENT_REQUEST_INVALID"', 'validation code'],
  [boundary, '"FULFILLMENT_RESOURCE_NOT_FOUND"', 'not-found code'],
  [boundary, '"FULFILLMENT_STATE_CONFLICT"', 'conflict code'],
  [boundary, '"FULFILLMENT_TEMPORARILY_UNAVAILABLE"', 'unavailable code'],
  [boundary, '"FULFILLMENT_ACCESS_DENIED"', 'forbidden code'],
  [boundary, '"FULFILLMENT_OPERATION_FAILED"', 'invariant code'],
  [boundary, 'if optional_not_found {', 'shipping optional not-found'],
  [boundary, 'FulfillmentError::ShippingOptionNotFound(shipping_option_id)', 'option not-found bridge'],
  [boundary, 'if matches!(&error.kind, PortErrorKind::NotFound)', 'fulfillment optional not-found'],
  [boundary, 'FulfillmentError::FulfillmentNotFound(', 'fulfillment not-found bridge'],
  [service, '.read_shipping_option_projection(', 'shipping owner lookup'],
  [service, '.list_shipping_option_projections(', 'shipping owner list'],
  [service, '.list_all_shipping_option_projections(', 'shipping admin owner list'],
  [service, '.read_fulfillment_projection(', 'fulfillment owner lookup'],
  [service, '.list_fulfillment_projections(', 'fulfillment owner list'],
  [service, '.find_latest_fulfillment_by_order_projection(', 'fulfillment latest owner read'],
  [shim, 'include!("fulfillment_query_service.rs");', 'service inclusion'],
  [shim, 'include!("fulfillment_query_boundary.rs");', 'boundary inclusion'],
  [shim, 'const GRAPHQL_QUERY_FULFILLMENT_BOUNDARY: &str = "commerce_graphql_query_fulfillment_facade";', 'boundary constant'],
]) requireText(content, value, label);

if (failures.length > 0) {
  console.error('Commerce GraphQL fulfillment-query diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL fulfillment query owner failures retain typed policy and emit bounded redacted diagnostics',
);
