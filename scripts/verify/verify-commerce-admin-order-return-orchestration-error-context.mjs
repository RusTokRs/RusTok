#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const returns = read('crates/rustok-commerce/src/controllers/admin/returns.rs');
const ownerDecision = read(
  'crates/rustok-commerce/src/services/return_decision_owner_orchestration.rs',
);
const orderErrors = read('crates/rustok-order/src/error.rs');
const paymentErrors = read('crates/rustok-payment/src/error.rs');
const paymentOrchestration = read(
  'crates/rustok-commerce/src/services/payment_orchestration.rs',
);
const postOrder = read('crates/rustok-commerce/src/services/post_order.rs');
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
  returns,
  'fn admin_order_error_policy(',
  'fn admin_order_port_error_policy(',
  'admin order policy',
);
const orderPortPolicy = between(
  returns,
  'fn admin_order_port_error_policy(',
  'fn admin_payment_error_policy(',
  'admin Order port policy',
);
const paymentPolicy = between(
  returns,
  'fn admin_payment_error_policy(',
  'fn admin_reserved_refund_error_policy(',
  'admin payment policy',
);
const reservedRefundPolicy = between(
  returns,
  'fn admin_reserved_refund_error_policy(',
  'fn map_admin_order_return_error(',
  'reserved refund policy',
);
const ownerPortMapper = between(
  returns,
  'fn map_admin_return_decision_order_port_error(',
  'fn map_admin_order_return_orchestration_error(',
  'return-decision Order owner-port mapper',
);
const orchestrationMapper = between(
  returns,
  'fn map_admin_order_return_orchestration_error(',
  '#[utoipa::path(',
  'admin order-return orchestration mapper',
);
const decisionRoute = between(
  returns,
  'pub async fn create_order_return_decision(',
  '#[utoipa::path(\n    get,\n    path = "/admin/returns"',
  'return decision route',
);
const completeRoute = between(
  returns,
  'pub async fn complete_order_return(',
  '#[utoipa::path(\n    post,\n    path = "/admin/returns/{id}/cancel"',
  'return completion route',
);

for (const [value, label] of [
  ['use rustok_payment::error::PaymentError;', 'typed payment error import'],
  ['PaymentOrchestrationError,', 'typed payment orchestration import'],
  ['PostOrderOrchestrationError,', 'typed post-order import'],
  ['ReturnDecisionOwnerOrchestrationError,', 'typed return-decision owner error import'],
  [
    'const ADMIN_ORDER_RETURN_ORCHESTRATION_OWNER: &str =',
    'orchestration owner constant',
  ],
  [
    '"rustok_commerce.admin_order_return_orchestration";',
    'orchestration owner value',
  ],
  [
    'const ADMIN_ORDER_RETURN_BOUNDARY: &str = "commerce_admin_order_return_http";',
    'HTTP boundary constant',
  ],
  ['struct AdminOrderReturnOrchestrationErrorContext {', 'orchestration error context'],
  ['tenant_id: Uuid,', 'tenant field'],
  ['actor_id: Uuid,', 'actor field'],
  ['order_id: Option<Uuid>,', 'order identity field'],
  ['return_id: Option<Uuid>,', 'return identity field'],
  ['refund_id: Option<Uuid>,', 'refund identity field'],
  ["operation: &'static str,", 'operation field'],
  ['refund_id: None,', 'truthful absent refund default'],
]) requireText(returns, value, label);

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
]) requireText(orderPolicy, value, label);

for (const [value, label] of [
  ['PortErrorKind::Validation', 'port validation kind'],
  ['PortErrorKind::NotFound', 'port not-found kind'],
  ['PortErrorKind::Conflict', 'port conflict kind'],
  ['PortErrorKind::Forbidden', 'port forbidden kind'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'port unavailable kind'],
  ['PortErrorKind::InvariantViolation', 'port invariant kind'],
  ['"commerce_admin_order_invalid"', 'port validation code'],
  ['"commerce_admin_order_state_conflict"', 'port conflict code'],
  ['"commerce_admin_order_storage_unavailable"', 'port storage code'],
  ['"commerce_admin_order_failed"', 'port fail-closed code'],
]) requireText(orderPortPolicy, value, label);

for (const [value, label] of [
  ['PaymentError::PaymentCollectionNotFound(_)', 'collection not-found variant'],
  ['PaymentError::PaymentNotFound(_)', 'payment not-found variant'],
  ['PaymentError::RefundNotFound(_)', 'refund not-found variant'],
  ['PaymentError::Validation(_)', 'payment validation variant'],
  ['PaymentError::InvalidTransition { .. }', 'payment transition variant'],
  ['PaymentError::ProviderRejected { .. }', 'provider rejected variant'],
  ['PaymentError::ProviderUnavailable { .. }', 'provider unavailable variant'],
  ['PaymentError::ProviderInvalidResponse { .. }', 'provider invalid-response variant'],
  ['PaymentError::ProviderOutcomeUnknown { .. }', 'provider unknown-outcome variant'],
  ['PaymentError::ProviderConfiguration { .. }', 'provider configuration variant'],
  ['PaymentError::Database(_)', 'payment database variant'],
  ['"commerce_admin_payment_invalid"', 'payment validation code'],
  ['"commerce_admin_payment_state_conflict"', 'payment conflict code'],
  ['"commerce_admin_payment_provider_unavailable"', 'provider unavailable code'],
  ['"commerce_admin_payment_provider_invalid_response"', 'provider invalid-response code'],
  ['"commerce_admin_payment_reconciliation_required"', 'payment reconciliation code'],
  ['"commerce_admin_payment_provider_not_configured"', 'provider configuration code'],
  ['"commerce_admin_payment_storage_unavailable"', 'payment storage code'],
  ['StatusCode::BAD_GATEWAY', 'provider invalid-response status'],
]) requireText(paymentPolicy, value, label);

for (const [value, label] of [
  ['PaymentError::ProviderOutcomeUnknown { .. }', 'reserved unknown-outcome variant'],
  ['PaymentError::ProviderInvalidResponse { .. }', 'reserved invalid-response variant'],
  ['PaymentError::ProviderUnavailable { .. }', 'reserved unavailable variant'],
  ['"commerce_admin_refund_reconciliation_required"', 'reserved reconciliation code'],
  ['"Refund remains reserved while the provider outcome is reconciled"', 'reserved reconciliation message'],
  ['"commerce_admin_refund_provider_unavailable"', 'reserved unavailable code'],
  [
    '"Refund remains reserved and the provider operation may be retried safely"',
    'reserved retry message',
  ],
  ['error => admin_payment_error_policy(error)', 'reserved fallback policy'],
]) requireText(reservedRefundPolicy, value, label);

for (const [value, label] of [
  ['error: PostOrderOrchestrationError,', 'owned post-order cause'],
  ['PostOrderOrchestrationError::Order(source)', 'order source branch'],
  ['PostOrderOrchestrationError::Payment(source)', 'payment source branch'],
  ['PostOrderOrchestrationError::PaymentOrchestration(source)', 'payment orchestration branch'],
  ['PostOrderOrchestrationError::Validation(_)', 'post-order validation branch'],
  ['PaymentOrchestrationError::Provider(source)', 'provider branch'],
  ['PaymentOrchestrationError::Payment(source)', 'payment branch'],
  [
    'PaymentOrchestrationError::ProviderAfterRefundReservation {',
    'reserved refund branch',
  ],
  ['context.order_id = Some(*id)', 'typed order identity adoption'],
  ['context.return_id = Some(*id)', 'typed return identity adoption'],
  ['context.refund_id = Some(*id)', 'typed missing-refund identity adoption'],
  ['context.refund_id = Some(*refund_id)', 'reserved refund identity adoption'],
  ['"commerce_admin_post_order_invalid"', 'post-order validation code'],
  ['"Post-order request is invalid"', 'static post-order validation message'],
  ['error = ?error', 'typed top-level error log'],
  ['owner = ADMIN_ORDER_RETURN_ORCHESTRATION_OWNER', 'orchestration owner log'],
  ['source_owner,', 'source owner log'],
  ['tenant_id = %context.tenant_id', 'tenant log'],
  ['actor_id = %context.actor_id', 'actor log'],
  ['order_id = ?context.order_id', 'order identity log'],
  ['return_id = ?context.return_id', 'return identity log'],
  ['refund_id = ?context.refund_id', 'refund identity log'],
  ['operation = %context.operation', 'operation log'],
  ['error_kind,', 'error-kind log'],
  ['public_code = code', 'public-code log'],
  ['status = %status', 'status log'],
  ['boundary = ADMIN_ORDER_RETURN_BOUNDARY', 'boundary log'],
  ['HttpError::new(status, code, message)', 'single static envelope constructor'],
]) requireText(orchestrationMapper, value, label);

for (const [value, label] of [
  ['owner = "rustok_order.post_order_command"', 'Order owner label'],
  ['consumer_operation = "create_return_decision"', 'consumer operation'],
  ['correlation_id = %context.correlation_id', 'correlation identity'],
  ['owner_error_kind = ?error.kind', 'bounded owner kind'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner code'],
  ['retryable = error.retryable', 'owner retryability'],
  ['public_code = code', 'public code'],
  ['status = %status', 'public status'],
]) requireText(ownerPortMapper, value, label);
for (const value of ['error = ?error', 'error.message', 'error.to_string()', 'internal_message']) {
  forbidText(ownerPortMapper, value, 'return-decision Order owner raw diagnostics');
}

for (const [value, label] of [
  ['request_context: RequestContext,', 'request context extractor'],
  ['admin_return_decision_order_context(&tenant, &auth, &request_context, id)', 'owner base context'],
  ['ReturnDecisionOwnerOrchestrationService::new(', 'owner-backed orchestration'],
  ['runtime.order_post_order_command_port()', 'host-selected Order command port'],
  ['.create_return_decision(context.clone(), tenant.id, id, input)', 'owner-backed decision contract'],
  ['.map_err(|error| match error {', 'typed decision error dispatch'],
  ['ReturnDecisionOwnerOrchestrationError::OrderCommand(error)', 'Order owner error branch'],
  ['map_admin_return_decision_order_port_error(', 'Order owner mapper handoff'],
  ['ReturnDecisionOwnerOrchestrationError::PostOrder(error)', 'Payment/validation orchestration branch'],
  ['map_admin_order_return_orchestration_error(', 'post-order mapper handoff'],
  ['"create_return_decision"', 'decision operation'],
  ['[Permission::ORDERS_UPDATE]', 'order update permission'],
  ['[Permission::PAYMENTS_UPDATE]', 'payment update permission'],
  ['super::decision_requires_payments_update(', 'decision payment permission gate'],
]) requireText(decisionRoute, value, label);
for (const value of [
  'PostOrderOrchestrationService::new(',
  'OrderService::new(',
  '.create_return(tenant.id, id,',
]) forbidText(decisionRoute, value, 'mounted decision route concrete Order dependency');

for (const [value, label] of [
  ['.map_err(|error| {', 'completion typed mapping closure'],
  ['map_admin_order_return_orchestration_error(', 'completion mapper handoff'],
  ['AdminOrderReturnOrchestrationErrorContext::new(', 'completion context construction'],
  ['"complete_return"', 'completion operation'],
  ['auth.user_id,\n                    None,\n                    Some(id),', 'completion truthful route identity'],
  ['.complete_return(tenant.id, auth.user_id, id, command)', 'completion service contract'],
  ['if input.refund.is_some() {', 'completion payment permission gate'],
]) requireText(completeRoute, value, label);

for (const [value, label] of [
  ['OrderPostOrderCommandPort', 'owner command port dependency'],
  ['.create_return(', 'owner return creation'],
  ['CreateOrderReturnRequest {', 'typed return request'],
  ['.create_change(', 'owner change creation'],
  ['CreateOrderChangeRequest {', 'typed change request'],
  ['.complete_return(', 'owner return completion'],
  ['CompleteOrderReturnRequest {', 'typed completion request'],
  ['PaymentService::new(self.db.clone())', 'deferred Payment compatibility path'],
]) requireText(ownerDecision, value, label);
for (const value of ['OrderService::new(', '.create_order_change(', '.complete_return(tenant_id,']) {
  forbidText(ownerDecision, value, 'owner-backed decision orchestration concrete Order dependency');
}

const orchestrationMapperUses =
  returns.match(
    /map_admin_order_return_orchestration_error\(\s+AdminOrderReturnOrchestrationErrorContext::new\(/g,
  ) ?? [];
if (orchestrationMapperUses.length !== 2) {
  failures.push(
    `expected two context-aware return orchestration mapper callsites, found ${orchestrationMapperUses.length}`,
  );
}

for (const [ownerSource, value, label] of [
  [orderErrors, 'Validation(String)', 'owner order validation variant'],
  [orderErrors, 'OrderNotFound(Uuid)', 'owner order-not-found variant'],
  [orderErrors, 'OrderReturnNotFound(Uuid)', 'owner return-not-found variant'],
  [orderErrors, 'OrderChangeNotFound(Uuid)', 'owner change-not-found variant'],
  [orderErrors, 'InvalidTransition { from: String, to: String }', 'owner order transition variant'],
  [orderErrors, 'Database(#[from] DbErr)', 'owner order database variant'],
  [orderErrors, 'Core(#[from] rustok_core::Error)', 'owner order core variant'],
  [paymentErrors, 'PaymentCollectionNotFound(Uuid)', 'owner collection variant'],
  [paymentErrors, 'PaymentNotFound(Uuid)', 'owner payment variant'],
  [paymentErrors, 'RefundNotFound(Uuid)', 'owner refund variant'],
  [paymentErrors, 'ProviderUnavailable {', 'owner unavailable variant'],
  [paymentErrors, 'ProviderRejected {', 'owner rejected variant'],
  [paymentErrors, 'ProviderInvalidResponse {', 'owner invalid-response variant'],
  [paymentErrors, 'ProviderOutcomeUnknown {', 'owner unknown-outcome variant'],
  [paymentErrors, 'ProviderConfiguration { provider_id: String }', 'owner configuration variant'],
  [paymentErrors, 'Database(#[from] DbErr)', 'owner payment database variant'],
  [paymentOrchestration, 'Provider(#[source] PaymentError)', 'payment orchestration provider variant'],
  [paymentOrchestration, 'ProviderAfterRefundReservation {', 'reserved refund variant'],
  [paymentOrchestration, 'Payment(#[from] PaymentError)', 'payment orchestration payment variant'],
  [postOrder, 'Order(#[from] rustok_order::error::OrderError)', 'post-order order variant'],
  [postOrder, 'Payment(#[from] rustok_payment::error::PaymentError)', 'post-order payment variant'],
  [postOrder, 'PaymentOrchestration(#[from] PaymentOrchestrationError)', 'post-order orchestration variant'],
  [postOrder, 'Validation(String)', 'post-order validation variant'],
]) requireText(ownerSource, value, label);

for (const value of [
  '.map_err(super::map_post_order_orchestration_error)?;',
  'format!("Post-order request is invalid:',
  'error.to_string()',
  'err.to_string()',
  'other.to_string()',
  'HttpError::bad_request("commerce_operation_failed"',
]) forbidText(returns, value, 'unsafe admin return orchestration mapping');

if (failures.length > 0) {
  console.error('Commerce admin order-return orchestration error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin order-return orchestration errors retain route context and static public envelopes',
);
