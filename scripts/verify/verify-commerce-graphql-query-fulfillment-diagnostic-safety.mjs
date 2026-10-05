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
  'crates/modules/rustok-commerce/src/graphql/safe_query/source/fulfillment_query_boundary.rs',
);
const service = read(
  'crates/modules/rustok-commerce/src/graphql/safe_query/source/fulfillment_query_service.rs',
);
const shim = read(
  'crates/modules/rustok-commerce/src/graphql/safe_query/source/rustok_fulfillment_shim.rs',
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
  '#[allow(clippy::too_many_arguments)]\nfn log_fulfillment_port_error(',
  'diagnostic helper block',
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
  ['if value.is_empty() { "empty" } else { "present" }', 'text presence branch'],
]) requireText(helperBlock, value, label);

for (const [value, label] of [
  ['let facts = fulfillment_query_context_facts(context);', 'context facts derivation'],
  ['let fulfillment_id_shape = optional_uuid_shape(fulfillment_id);', 'fulfillment id fact'],
  ['let order_id_shape = optional_uuid_shape(order_id);', 'order id fact'],
  ['let owner_message_presence = text_presence_shape(&error.message);', 'owner message presence fact'],
  ['let owner_message_length = error.message.chars().count();', 'owner message length fact'],
  ['let diagnostic_error = FulfillmentQueryDiagnosticError;', 'diagnostic error binding'],
  ['error = ?diagnostic_error', 'redacted error field'],
  ['owner = "rustok_fulfillment"', 'owner tag'],
  ['tenant_id_length = facts.tenant_id_length', 'tenant length field'],
  ['actor_kind = facts.actor_kind', 'actor kind field'],
  ['actor_id_length = facts.actor_id_length', 'actor length field'],
  ['claim_count = facts.claim_count', 'claim count field'],
  ['role_count = facts.role_count', 'role count field'],
  ['correlation_id_length = facts.correlation_id_length', 'correlation length field'],
  ['context_locale_length = facts.context_locale_length', 'locale length field'],
  ['channel_present = facts.channel_present', 'channel presence field'],
  ['channel_length = ?facts.channel_length', 'channel length field'],
  ['deadline_ms = ?facts.deadline_ms', 'deadline field'],
  ['fulfillment_id_shape', 'fulfillment id field'],
  ['order_id_shape', 'order id field'],
  ['owner_code = %error.code', 'owner code field'],
  ['owner_kind = error_kind', 'owner kind field'],
  ['owner_message_presence', 'owner message presence field'],
  ['owner_message_length', 'owner message length field'],
  ['owner_retryable = error.retryable', 'owner retryable field'],
  ['public_code', 'public code field'],
  ['public_retryable', 'public retryable field'],
  ['boundary = GRAPHQL_QUERY_FULFILLMENT_BOUNDARY', 'boundary constant field'],
]) requireText(fulfillmentLog, value, label);

requireText(
  fulfillmentLog,
  'tracing::error!(',
  'fulfillment logger technical severity',
);
requireText(
  fulfillmentLog,
  'tracing::warn!(',
  'fulfillment logger rejection severity',
);
requireBefore(
  fulfillmentLog,
  'let diagnostic_error = FulfillmentQueryDiagnosticError;',
  'tracing::error!(',
  'fulfillment logger redaction ordering',
);

for (const value of [
  'error = ?error',
  'error = %error',
  'owner_message =',
  'error_message =',
  'error.message,',
  'correlation_id = %context.correlation_id',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'actor = %context.actor',
  'channel = %context.channel',
  'channel = ?context.channel',
  'fulfillment_id = ?fulfillment_id',
  'fulfillment_id = %fulfillment_id',
  'order_id = ?order_id',
  'order_id = %order_id',
  'owner_kind = ?error.kind',
  'public_message,',
]) forbidText(boundary, value, 'raw diagnostic leak');

for (const [pattern, expected, label] of [
  [/struct FulfillmentQueryDiagnosticError;/g, 1, 'diagnostic token count'],
  [/error = \?diagnostic_error/g, 2, 'redacted error field count'],
  [/tracing::error!\(/g, 1, 'technical event count'],
  [/tracing::warn!\(/g, 1, 'rejection event count'],
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
  [boundary, 'if matches!(&error.kind, PortErrorKind::NotFound)', 'fulfillment optional not-found'],
  [boundary, 'FulfillmentError::FulfillmentNotFound(', 'fulfillment not-found bridge'],
  [service, '.read_fulfillment_projection(', 'fulfillment owner lookup'],
  [service, '.list_fulfillment_projections(', 'fulfillment owner list'],
  [service, '.find_latest_fulfillment_by_order_projection(', 'fulfillment latest owner read'],
  [shim, 'mod fulfillment_query_service;', 'service module declaration'],
  [shim, 'mod fulfillment_query_boundary;', 'boundary module declaration'],
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
