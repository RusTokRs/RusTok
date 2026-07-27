#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const wrapper = read('crates/rustok-order/src/checkout_compensation_local_context.rs');
const lib = read('crates/rustok-order/src/lib.rs');
const compensation = read('crates/rustok-order/src/checkout_compensation.rs');
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

const wrapperImpl = between(
  wrapper,
  'impl CheckoutOrderCompensationPort for InProcessCheckoutOrderCompensationPort {',
  'pub fn in_process_checkout_order_compensation_port(',
  'compensation local wrapper implementation',
);
const mapper = wrapper.slice(
  wrapper.indexOf('fn map_checkout_order_compensation_local_port_error('),
);
const ownerMapper = compensation.slice(compensation.indexOf('fn order_error_to_port_error('));

for (const [value, label] of [
  ['mod checkout_compensation_local_context;', 'private compensation local implementation'],
  ['#[path = "checkout_owner_context.rs"]', 'owner-context implementation path'],
  ['mod checkout_owner_context_impl;', 'private owner-context implementation'],
  ['pub mod checkout_owner_context {', 'public compatibility facade'],
  ['pub use crate::checkout_compensation_local_context::{', 'facade compensation export'],
  ['pub use crate::checkout_owner_context_impl::{', 'facade settlement export'],
  ['pub use checkout_compensation_local_context::{', 'root compensation export'],
  ['pub use checkout_owner_context_impl::{', 'root settlement export'],
]) requireText(lib, value, label);

for (const [value, label] of [
  ['const ORDER_COMPENSATION_OWNER: &str = "rustok_order.checkout_compensation";', 'truthful compensation owner'],
  ['const ORDER_COMPENSATION_BOUNDARY: &str = "checkout_order_compensation_port";', 'compensation boundary'],
  ['const COMPENSATE_OPERATION: &str = "compensate_checkout_order";', 'public compensation operation'],
  ['checkout_owner_context_impl::in_process_checkout_order_compensation_port(', 'owner-context delegation'],
  ['checkout_owner_context_impl::InProcessCheckoutOrderCompensationPort::with_identity_port(', 'identity constructor delegation'],
  ['let diagnostic_context = context.clone();', 'delegated context retention'],
  ['let result = self.inner.compensate_checkout_order(context, request).await;', 'unchanged owner delegation'],
  ['result.map_err(|error| {', 'post-delegation local mapping'],
  ['map_checkout_order_compensation_local_port_error(&diagnostic_context, error)', 'retained context mapper call'],
]) requireText(wrapper, value, label);

const delegationIndex = wrapperImpl.indexOf(
  'self.inner.compensate_checkout_order(context, request).await',
);
const mapperIndex = wrapperImpl.indexOf(
  'map_checkout_order_compensation_local_port_error(',
);
if (!(delegationIndex >= 0 && delegationIndex < mapperIndex)) {
  failures.push('compensation wrapper must delegate before mapping the returned local PortError');
}

for (const [code, message, localOperation, label] of [
  [
    'order.checkout_compensation_identity_invalid',
    'checkout compensation request is invalid',
    'validate_request',
    'request validation outcome',
  ],
  [
    'order.checkout_compensation_identity_conflict',
    'checkout order identity conflicts with the compensation request',
    'validate_durable_checkout_identity',
    'durable identity conflict outcome',
  ],
  [
    'order.checkout_compensation_state_conflict',
    'checkout order changed while compensation was being applied',
    'adopt_cancelled_after_transition_race',
    'cancellation race outcome',
  ],
  [
    'order.checkout_compensation_manual_reconciliation',
    'checkout requires manual reconciliation',
    'require_manual_reconciliation',
    'manual reconciliation outcome',
  ],
]) {
  requireText(mapper, `"${code}"`, `${label} code`);
  requireText(mapper, `"${message}"`, `${label} message`);
  requireText(mapper, `"${localOperation}"`, `${label} local operation`);
  requireText(compensation, `"${code}"`, `${label} preserved owner code`);
  requireText(compensation, `"${message}"`, `${label} preserved public message`);
}

for (const [value, label] of [
  ['match (error.code.as_str(), error.message.as_str()) {', 'exact code-and-message classification'],
  ['_ => return error,', 'unmatched error passthrough'],
  ['let integrity_failure = matches!(', 'integrity severity classification'],
  ['"validate_durable_checkout_identity" | "require_manual_reconciliation"', 'integrity outcome set'],
  ['tracing::error!(', 'integrity error event'],
  ['tracing::warn!(', 'ordinary warning event'],
  ['error = ?error', 'mapped error evidence'],
  ['owner = ORDER_COMPENSATION_OWNER', 'truthful owner diagnostic'],
  ['operation = COMPENSATE_OPERATION', 'exact public operation diagnostic'],
  ['local_operation,', 'exact local operation diagnostic'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['internal_code = %error.code', 'stable code diagnostic'],
  ['internal_message = %error.message', 'stable message diagnostic'],
  ['error_kind = ?error.kind', 'typed kind diagnostic'],
  ['retryable = error.retryable', 'retryability diagnostic'],
  ['boundary = ORDER_COMPENSATION_BOUNDARY', 'exact boundary diagnostic'],
  ['error\n}', 'same mapped error returned'],
]) requireText(mapper, value, label);

for (const [value, label] of [
  ['"order checkout compensation local integrity outcome retained delegated context"', 'integrity event message'],
  ['"order checkout compensation local outcome retained delegated context"', 'ordinary event message'],
  ['fn manual_reconciliation(', 'manual reconciliation helper preserved'],
  ['reason = %reason', 'manual reconciliation internal reason preserved'],
  ['Err(OrderError::InvalidTransition { from, to })', 'cancellation race cause capture preserved'],
  ['current_state = ?current.status_kind()', 'cancellation race state evidence preserved'],
  ['"order lifecycle conflicts with checkout compensation"', 'service transition log preserved'],
  ['"checkout order lifecycle conflicts with compensation"', 'service transition envelope preserved'],
  ['order_error_to_port_error(&context, "read_checkout_order_for_compensation", error)', 'order read mapping preserved'],
]) requireText(compensation, value, label);

forbidText(
  mapper,
  '"checkout order lifecycle conflicts with compensation"',
  'service transition envelope must not be classified as cancellation race',
);
for (const value of [
  'PortError::validation(',
  'PortError::conflict(',
  'PortError::new(',
  'PortError::unavailable(',
  'PortError::invariant_violation(',
]) forbidText(mapper, value, 'local mapper must not construct a replacement envelope');

for (const [value, label] of [
  ['OrderError::Database(error)', 'database mapper preserved'],
  ['OrderError::Validation(cause)', 'service validation mapper preserved'],
  ['OrderError::InvalidTransition { from, to }', 'service transition mapper preserved'],
  ['OrderError::Core(error)', 'core mapper preserved'],
]) requireText(ownerMapper, value, label);

for (const [pattern, expected, label] of [
  [/map_checkout_order_compensation_local_port_error\(/g, 2, 'local mapper definition/use count'],
  [/"validate_request"/g, 1, 'request operation count'],
  [/"validate_durable_checkout_identity"/g, 2, 'identity operation classification/severity count'],
  [/"adopt_cancelled_after_transition_race"/g, 1, 'race operation count'],
  [/"require_manual_reconciliation"/g, 2, 'reconciliation operation classification/severity count'],
]) {
  const count = wrapper.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

if (failures.length > 0) {
  console.error('Order compensation local context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Order compensation request, identity, cancellation-race, and reconciliation outcomes retain full delegated context and unchanged PortError envelopes',
);
