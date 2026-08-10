#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const changes = read('crates/rustok-commerce/src/controllers/admin/changes.rs');
const orchestration = read('crates/rustok-commerce/src/services/order_change_orchestration.rs');
const postOrder = read('crates/rustok-commerce/src/services/post_order.rs');
const paymentOrchestration = read(
  'crates/rustok-commerce/src/services/payment_orchestration.rs',
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

const portPolicy = between(
  changes,
  'fn admin_order_change_port_error_policy(',
  'fn admin_order_change_payment_error_policy(',
  'order-change port policy',
);
const portMapper = between(
  changes,
  'fn map_admin_order_change_port_error(',
  'fn map_admin_order_change_orchestration_error(',
  'order-change owner-port mapper',
);
const legacyMapper = between(
  changes,
  'fn map_admin_order_change_orchestration_error(',
  'fn map_admin_order_change_apply_error(',
  'legacy cross-domain mapper',
);
const applyMapper = between(
  changes,
  'fn map_admin_order_change_apply_error(',
  '/// Create admin order change preview',
  'apply boundary mapper',
);
const applyRoute = between(
  changes,
  'pub async fn apply_order_change(',
  '/// Cancel admin order change',
  'apply order change route',
);
const ownerMethod = between(
  orchestration,
  'pub async fn apply_order_change_with_owner_ports(',
  '\n    }\n}',
  'owner-port orchestration method',
);

for (const [value, label] of [
  ['PortErrorKind::Validation', 'validation mapping'],
  ['PortErrorKind::NotFound', 'not-found mapping'],
  ['PortErrorKind::Conflict', 'conflict mapping'],
  ['PortErrorKind::Forbidden', 'forbidden mapping'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'unavailable mapping'],
  ['PortErrorKind::InvariantViolation', 'invariant mapping'],
  ['"commerce_admin_order_invalid"', 'validation public code'],
  ['"commerce_admin_not_found"', 'not-found public code'],
  ['"commerce_admin_order_state_conflict"', 'conflict public code'],
  ['"commerce_admin_order_storage_unavailable"', 'unavailable public code'],
  ['"commerce_admin_order_failed"', 'fail-closed public code'],
]) requireText(portPolicy, value, label);

for (const [value, label] of [
  ['owner = "rustok_order"', 'owner diagnostic'],
  ['owner_operation,', 'owner operation diagnostic'],
  ['consumer_operation = "apply_order_change"', 'consumer operation diagnostic'],
  ['correlation_id = %context.correlation_id', 'correlation diagnostic'],
  ['tenant_id_non_empty = !context.tenant_id.is_empty()', 'tenant shape diagnostic'],
  ['actor_id_non_nil = !actor_id.is_nil()', 'actor shape diagnostic'],
  ['order_change_id_non_nil = !order_change_id.is_nil()', 'change shape diagnostic'],
  ['owner_error_kind = ?error.kind', 'owner kind diagnostic'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner code diagnostic'],
  ['retryable = error.retryable', 'retryability diagnostic'],
  ['HttpError::new(status, code, message)', 'stable public envelope'],
]) requireText(portMapper, value, label);
for (const value of ['error = ?error', 'error.message', 'error.to_string()', 'internal_message']) {
  forbidText(portMapper, value, 'raw owner-port diagnostic');
}

for (const [value, label] of [
  ['OrderChangeOrchestrationError::OrderRead(source)', 'read-owner error branch'],
  ['OrderChangeOrchestrationError::OrderCommand(source)', 'command-owner error branch'],
  ['OrderChangeOrchestrationError::PostOrder(source)', 'cross-domain fallback branch'],
  ['"read_order_change_projection"', 'read owner operation'],
  ['"apply_change"', 'command owner operation'],
  ['map_admin_order_change_orchestration_error(context, source)', 'legacy cross-domain handoff'],
]) requireText(applyMapper, value, label);

for (const [value, label] of [
  ['PostOrderOrchestrationError::Order(source)', 'legacy order branch'],
  ['PostOrderOrchestrationError::Payment(source)', 'legacy payment branch'],
  ['PostOrderOrchestrationError::PaymentOrchestration(source)', 'legacy payment orchestration branch'],
  ['PostOrderOrchestrationError::Validation(_)', 'legacy validation branch'],
  ['PaymentOrchestrationError::ProviderAfterRefundReservation {', 'reserved-refund branch'],
  ['"commerce_admin_refund_reconciliation_required"', 'reserved reconciliation envelope'],
  ['"commerce_admin_refund_provider_unavailable"', 'reserved unavailable envelope'],
]) requireText(legacyMapper, value, label);

for (const [value, label] of [
  ['request_context: RequestContext,', 'request context extractor'],
  ['admin_order_change_read_context(&tenant, &auth, &request_context, id)', 'read context construction'],
  ['admin_order_change_apply_context(&tenant, &auth, &request_context, id)', 'write context construction'],
  ['OrderChangeOrchestrationService::from_order_ports(', 'owner-port orchestration construction'],
  ['runtime.order_read_port()', 'host-selected read port'],
  ['runtime.order_post_order_command_port()', 'host-selected command port'],
  ['.apply_order_change_with_owner_ports(', 'owner-port orchestration call'],
  ['read_context.clone()', 'read context forwarding'],
  ['command_context.clone()', 'command context forwarding'],
  ['map_admin_order_change_apply_error(', 'typed apply mapper'],
]) requireText(applyRoute, value, label);

for (const [value, label] of [
  ['pub enum OrderChangeOrchestrationError', 'typed orchestration error'],
  ['OrderRead(PortError)', 'read owner variant'],
  ['OrderCommand(PortError)', 'command owner variant'],
  ['PostOrder(#[from] PostOrderOrchestrationError)', 'cross-domain variant'],
  ['.read_order_change_projection(', 'Order read port call'],
  ['ReadOrderChangeProjectionRequest { change_id }', 'typed read request'],
  ['.apply_change(', 'Order command port call'],
  ['ApplyOrderChangeRequest {', 'typed apply request'],
  ['.map_err(OrderChangeOrchestrationError::OrderRead)', 'read error preservation'],
  ['.map_err(OrderChangeOrchestrationError::OrderCommand)', 'command error preservation'],
]) requireText(orchestration, value, label);

for (const value of [
  'OrderService::new(',
  '.get_order_change(',
  '.apply_order_change(tenant_id, change_id,',
]) forbidText(ownerMethod, value, 'owner-port REST orchestration must not construct concrete Order service');

for (const [ownerSource, value, label] of [
  [postOrder, 'Order(#[from] rustok_order::error::OrderError)', 'legacy post-order Order variant'],
  [postOrder, 'Payment(#[from] rustok_payment::error::PaymentError)', 'legacy post-order Payment variant'],
  [postOrder, 'PaymentOrchestration(#[from] PaymentOrchestrationError)', 'legacy payment orchestration variant'],
  [paymentOrchestration, 'ProviderAfterRefundReservation {', 'payment reserved-refund variant'],
]) requireText(ownerSource, value, label);

if (failures.length > 0) {
  console.error('Commerce admin order-change owner-port orchestration error verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ mounted REST order-change apply preserves typed owner-port errors and legacy cross-domain envelopes',
);
