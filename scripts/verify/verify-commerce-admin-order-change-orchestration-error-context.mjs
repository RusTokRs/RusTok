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
const orderErrors = read('crates/rustok-order/src/error.rs');
const paymentErrors = read('crates/rustok-payment/src/error.rs');
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

const orderPolicy = between(
  changes,
  'fn admin_order_change_order_error_policy(',
  'fn admin_order_change_payment_error_policy(',
  'order policy',
);
const paymentPolicy = between(
  changes,
  'fn admin_order_change_payment_error_policy(',
  'fn admin_order_change_reserved_refund_error_policy(',
  'payment policy',
);
const reservedRefundPolicy = between(
  changes,
  'fn admin_order_change_reserved_refund_error_policy(',
  'fn adopt_order_change_order_error_identity(',
  'reserved refund policy',
);
const mapper = between(
  changes,
  'fn map_admin_order_change_orchestration_error(',
  '/// Create admin order change preview',
  'order-change orchestration mapper',
);
const applyRoute = between(
  changes,
  'pub async fn apply_order_change(',
  '/// Cancel admin order change',
  'apply order change route',
);

for (const [value, label] of [
  ['use rustok_payment::error::PaymentError;', 'typed payment import'],
  ['PaymentOrchestrationError,', 'typed payment orchestration import'],
  ['PostOrderOrchestrationError,', 'typed post-order import'],
  [
    'const ADMIN_ORDER_CHANGE_ORCHESTRATION_OWNER: &str =',
    'orchestration owner constant',
  ],
  [
    'const ADMIN_ORDER_CHANGE_BOUNDARY: &str = "commerce_admin_order_change_http";',
    'HTTP boundary constant',
  ],
  ['type AdminOrderChangeHttpPolicy = (', 'static HTTP policy type'],
  ['struct AdminOrderChangeOrchestrationErrorContext {', 'orchestration context'],
  ['tenant_id: Uuid,', 'tenant field'],
  ['actor_id: Uuid,', 'actor field'],
  ['order_id: Option<Uuid>,', 'order identity field'],
  ['order_change_id: Option<Uuid>,', 'change identity field'],
  ['payment_collection_id: Option<Uuid>,', 'collection identity field'],
  ['payment_id: Option<Uuid>,', 'payment identity field'],
  ['refund_id: Option<Uuid>,', 'refund identity field'],
  ["operation: &'static str,", 'operation field'],
]) requireText(changes, value, label);

for (const [value, label] of [
  ['OrderError::Validation(_)', 'order validation variant'],
  ['OrderError::OrderNotFound(_)', 'order not-found variant'],
  ['OrderError::OrderReturnNotFound(_)', 'return not-found variant'],
  ['OrderError::OrderChangeNotFound(_)', 'change not-found variant'],
  ['OrderError::InvalidTransition { .. }', 'order transition variant'],
  ['OrderError::Database(_)', 'order database variant'],
  ['OrderError::Core(_)', 'order core variant'],
  ['"commerce_admin_order_invalid"', 'order validation code'],
  ['"commerce_admin_order_state_conflict"', 'order conflict code'],
  ['"commerce_admin_order_storage_unavailable"', 'order storage code'],
  ['"commerce_admin_order_failed"', 'order fail-closed code'],
  ['"Order request is invalid"', 'static order validation message'],
  ['"Order operation could not be completed safely"', 'static order fail-closed message'],
]) requireText(orderPolicy, value, label);

for (const [value, label] of [
  ['PaymentError::PaymentCollectionNotFound(_)', 'collection not-found variant'],
  ['PaymentError::PaymentNotFound(_)', 'payment not-found variant'],
  ['PaymentError::RefundNotFound(_)', 'refund not-found variant'],
  ['PaymentError::Validation(_)', 'payment validation variant'],
  ['PaymentError::InvalidTransition { .. }', 'payment transition variant'],
  ['PaymentError::ProviderRejected { .. }', 'provider rejection variant'],
  ['PaymentError::ProviderUnavailable { .. }', 'provider unavailable variant'],
  ['PaymentError::ProviderInvalidResponse { .. }', 'provider invalid-response variant'],
  ['PaymentError::ProviderOutcomeUnknown { .. }', 'provider unknown-outcome variant'],
  ['PaymentError::ProviderConfiguration { .. }', 'provider configuration variant'],
  ['PaymentError::Database(_)', 'payment database variant'],
  ['StatusCode::BAD_GATEWAY', 'bad-gateway status'],
  ['"commerce_admin_payment_invalid"', 'payment validation code'],
  ['"commerce_admin_payment_state_conflict"', 'payment conflict code'],
  ['"commerce_admin_payment_provider_unavailable"', 'provider unavailable code'],
  ['"commerce_admin_payment_provider_invalid_response"', 'provider response code'],
  ['"commerce_admin_payment_reconciliation_required"', 'payment reconciliation code'],
  ['"commerce_admin_payment_provider_not_configured"', 'provider configuration code'],
  ['"commerce_admin_payment_storage_unavailable"', 'payment storage code'],
]) requireText(paymentPolicy, value, label);

for (const [value, label] of [
  ['PaymentError::ProviderOutcomeUnknown { .. }', 'reserved unknown-outcome variant'],
  ['PaymentError::ProviderInvalidResponse { .. }', 'reserved invalid-response variant'],
  ['PaymentError::ProviderUnavailable { .. }', 'reserved unavailable variant'],
  ['"commerce_admin_refund_reconciliation_required"', 'refund reconciliation code'],
  ['"Refund remains reserved while the provider outcome is reconciled"', 'refund reconciliation message'],
  ['"commerce_admin_refund_provider_unavailable"', 'refund retry code'],
  [
    '"Refund remains reserved and the provider operation may be retried safely"',
    'refund retry message',
  ],
  ['error => admin_order_change_payment_error_policy(error)', 'reserved fallback policy'],
]) requireText(reservedRefundPolicy, value, label);

for (const [value, label] of [
  ['error: PostOrderOrchestrationError,', 'owned top-level cause'],
  ['PostOrderOrchestrationError::Order(source)', 'nested order branch'],
  ['PostOrderOrchestrationError::Payment(source)', 'nested payment branch'],
  ['PostOrderOrchestrationError::PaymentOrchestration(source)', 'payment orchestration branch'],
  ['PostOrderOrchestrationError::Validation(_)', 'orchestration validation branch'],
  ['PaymentOrchestrationError::Provider(source)', 'provider branch'],
  ['PaymentOrchestrationError::Payment(source)', 'payment owner branch'],
  ['PaymentOrchestrationError::ProviderAfterRefundReservation {', 'reserved refund branch'],
  ['context.refund_id = Some(*refund_id);', 'reserved refund identity adoption'],
  ['adopt_order_change_order_error_identity(&mut context, source)', 'order identity adoption'],
  ['adopt_order_change_payment_error_identity(&mut context, source)', 'payment identity adoption'],
  ['"commerce_admin_post_order_invalid"', 'post-order validation code'],
  ['"Post-order request is invalid"', 'post-order validation message'],
  ['error = ?error', 'typed top-level cause log'],
  ['owner = ADMIN_ORDER_CHANGE_ORCHESTRATION_OWNER', 'orchestration owner log'],
  ['source_owner,', 'source owner log'],
  ['tenant_id = %context.tenant_id', 'tenant log'],
  ['actor_id = %context.actor_id', 'actor log'],
  ['order_id = ?context.order_id', 'order identity log'],
  ['order_change_id = ?context.order_change_id', 'change identity log'],
  ['payment_collection_id = ?context.payment_collection_id', 'collection identity log'],
  ['payment_id = ?context.payment_id', 'payment identity log'],
  ['refund_id = ?context.refund_id', 'refund identity log'],
  ['operation = %context.operation', 'operation log'],
  ['error_kind,', 'error-kind log'],
  ['public_code = code', 'public-code log'],
  ['status = %status', 'status log'],
  ['boundary = ADMIN_ORDER_CHANGE_BOUNDARY', 'boundary log'],
  ['HttpError::new(status, code, message)', 'single static envelope constructor'],
]) requireText(mapper, value, label);

for (const [value, label] of [
  ['[Permission::ORDERS_UPDATE]', 'update permission'],
  ['let actor_id = auth.user_id;', 'actor capture'],
  ['.with_payment_provider_registry(runtime.payment_provider_registry())', 'provider registry contract'],
  ['.apply_order_change(tenant.id, id, input.difference_refund, input.metadata)', 'service contract'],
  ['.map_err(|error| {', 'typed mapping closure'],
  ['map_admin_order_change_orchestration_error(', 'local mapper handoff'],
  ['AdminOrderChangeOrchestrationErrorContext::new(', 'context construction'],
  ['tenant.id,\n                    actor_id,\n                    id,\n                    "apply_order_change",', 'truthful route context'],
]) requireText(applyRoute, value, label);

const mapperUses =
  changes.match(
    /map_admin_order_change_orchestration_error\(\s+AdminOrderChangeOrchestrationErrorContext::new\(/g,
  ) ?? [];
if (mapperUses.length !== 1) {
  failures.push(`expected one context-aware order-change orchestration callsite, found ${mapperUses.length}`);
}

for (const [ownerSource, value, label] of [
  [orderErrors, 'Validation(String)', 'owner order validation variant'],
  [orderErrors, 'OrderNotFound(Uuid)', 'owner order not-found variant'],
  [orderErrors, 'OrderReturnNotFound(Uuid)', 'owner return not-found variant'],
  [orderErrors, 'OrderChangeNotFound(Uuid)', 'owner change not-found variant'],
  [orderErrors, 'InvalidTransition { from: String, to: String }', 'owner order transition variant'],
  [orderErrors, 'Database(#[from] DbErr)', 'owner order database variant'],
  [orderErrors, 'Core(#[from] rustok_core::Error)', 'owner order core variant'],
  [paymentErrors, 'PaymentCollectionNotFound(Uuid)', 'owner collection variant'],
  [paymentErrors, 'PaymentNotFound(Uuid)', 'owner payment variant'],
  [paymentErrors, 'RefundNotFound(Uuid)', 'owner refund variant'],
  [paymentErrors, 'ProviderUnavailable {', 'owner provider unavailable variant'],
  [paymentErrors, 'ProviderRejected {', 'owner provider rejection variant'],
  [paymentErrors, 'ProviderInvalidResponse {', 'owner provider response variant'],
  [paymentErrors, 'ProviderOutcomeUnknown {', 'owner provider outcome variant'],
  [paymentErrors, 'ProviderConfiguration { provider_id: String }', 'owner provider configuration variant'],
  [postOrder, 'Order(#[from] rustok_order::error::OrderError)', 'post-order order variant'],
  [postOrder, 'Payment(#[from] rustok_payment::error::PaymentError)', 'post-order payment variant'],
  [postOrder, 'PaymentOrchestration(#[from] PaymentOrchestrationError)', 'post-order orchestration variant'],
  [postOrder, 'Validation(String)', 'post-order validation variant'],
  [paymentOrchestration, 'Provider(#[source] PaymentError)', 'payment orchestration provider variant'],
  [paymentOrchestration, 'ProviderAfterRefundReservation {', 'payment orchestration reserved variant'],
  [paymentOrchestration, 'Payment(#[from] PaymentError)', 'payment orchestration owner variant'],
]) requireText(ownerSource, value, label);

for (const value of [
  '.map_err(super::map_post_order_orchestration_error)?;',
  'format!("Post-order request is invalid:',
  'format!("Payment request is invalid:',
  'error.to_string()',
  'err.to_string()',
  'other.to_string()',
  'HttpError::bad_request("commerce_operation_failed"',
]) forbidText(changes, value, 'unsafe admin order-change orchestration public conversion');

if (failures.length > 0) {
  console.error('Commerce admin order-change orchestration error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin order-change orchestration retains typed causes and static public envelopes',
);
