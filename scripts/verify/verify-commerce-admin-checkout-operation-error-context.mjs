#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const controller = read(
  'crates/rustok-commerce/src/controllers/admin/checkout_operations.rs',
);
const sweep = read(
  'crates/rustok-commerce/src/services/checkout_compensation_sweep.rs',
);
const operationErrors = read(
  'crates/rustok-commerce/src/services/checkout_operation.rs',
);
const compensationErrors = read(
  'crates/rustok-commerce/src/services/checkout_compensation.rs',
);
const reservationErrors = read(
  'crates/rustok-commerce/src/services/checkout_inventory_reservation_journal.rs',
);
const paymentErrors = read('crates/rustok-payment/src/error.rs');
const orderErrors = read('crates/rustok-order/src/error.rs');
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

const showRoute = between(
  controller,
  'pub async fn show_checkout_operation(',
  'pub async fn compensate_checkout_operation(',
  'show checkout operation route',
);
const compensateRoute = between(
  controller,
  'pub async fn compensate_checkout_operation(',
  'pub async fn sweep_checkout_compensations(',
  'compensate checkout operation route',
);
const sweepRoute = between(
  controller,
  'pub async fn sweep_checkout_compensations(',
  'fn map_operation(',
  'checkout compensation sweep route',
);
const operationPolicy = between(
  controller,
  'fn checkout_operation_error_policy(',
  'fn payment_error_policy(',
  'checkout operation policy',
);
const paymentPolicy = between(
  controller,
  'fn payment_error_policy(',
  'fn reserved_refund_error_policy(',
  'checkout payment policy',
);
const reservedRefundPolicy = between(
  controller,
  'fn reserved_refund_error_policy(',
  'fn order_error_policy(',
  'checkout reserved refund policy',
);
const orderPolicy = between(
  controller,
  'fn order_error_policy(',
  'fn adopt_operation_error_identity(',
  'checkout order policy',
);
const logger = between(
  controller,
  'fn admin_checkout_operation_http_error<E>(',
  'fn map_operation_error(',
  'checkout operation logger',
);
const compensationMapper = between(
  controller,
  'fn map_compensation_error(',
  'fn map_sweep_error(',
  'checkout compensation mapper',
);
const sweepMapperStart = controller.indexOf('fn map_sweep_error(');
const sweepMapper = sweepMapperStart < 0 ? '' : controller.slice(sweepMapperStart);
if (sweepMapperStart < 0) {
  failures.push('checkout sweep mapper: unable to isolate source block');
}

for (const [value, label] of [
  ['use rustok_order::error::OrderError;', 'typed order error import'],
  ['use rustok_payment::error::PaymentError;', 'typed payment error import'],
  ['use sea_orm::DbErr;', 'typed sweep database import'],
  ['CheckoutCompensationError,', 'typed compensation error import'],
  ['CheckoutInventoryReservationError,', 'typed reservation error import'],
  ['CheckoutOperationError,', 'typed operation error import'],
  ['PaymentOrchestrationError,', 'typed payment orchestration import'],
  [
    'const ADMIN_CHECKOUT_OPERATION_OWNER: &str = "rustok_commerce.admin_checkout_operation";',
    'owner constant',
  ],
  [
    'const ADMIN_CHECKOUT_OPERATION_BOUNDARY: &str = "commerce_admin_checkout_operation_http";',
    'HTTP boundary constant',
  ],
  ['type AdminCheckoutOperationHttpPolicy = (', 'static policy type'],
  ['struct AdminCheckoutOperationErrorContext {', 'typed checkout context'],
  ['tenant_id: Uuid,', 'tenant field'],
  ['actor_id: Uuid,', 'actor field'],
  ['checkout_operation_id: Option<Uuid>,', 'checkout operation identity field'],
  ['reservation_id: Option<Uuid>,', 'reservation identity field'],
  ['payment_collection_id: Option<Uuid>,', 'payment collection identity field'],
  ['payment_id: Option<Uuid>,', 'payment identity field'],
  ['refund_id: Option<Uuid>,', 'refund identity field'],
  ['order_id: Option<Uuid>,', 'order identity field'],
  ['order_return_id: Option<Uuid>,', 'order return identity field'],
  ['order_change_id: Option<Uuid>,', 'order change identity field'],
  ["operation: &'static str,", 'route operation field'],
]) requireText(controller, value, label);

for (const [value, label] of [
  ['CheckoutOperationError::NotFound(_)', 'operation not-found variant'],
  ['CheckoutOperationError::Conflict(_)', 'operation conflict variant'],
  ['CheckoutOperationError::Validation(_)', 'operation validation variant'],
  ['CheckoutOperationError::Database(_)', 'operation database variant'],
  ['"checkout_operation_not_found"', 'operation not-found code'],
  ['"checkout_operation_conflict"', 'operation conflict code'],
  ['"checkout_operation_invalid"', 'operation validation code'],
  ['"internal_error"', 'operation internal code'],
  ['"Checkout operation not found"', 'static operation not-found message'],
  [
    '"Checkout operation conflicts with the current state"',
    'static operation conflict message',
  ],
  [
    '"Checkout operation request is invalid"',
    'static operation validation message',
  ],
  [
    '"Checkout operation storage is unavailable"',
    'static operation storage message',
  ],
]) requireText(operationPolicy, value, label);

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
  ['"commerce_admin_payment_invalid"', 'payment invalid code'],
  ['"commerce_admin_payment_state_conflict"', 'payment conflict code'],
  [
    '"commerce_admin_payment_provider_unavailable"',
    'payment provider unavailable code',
  ],
  [
    '"commerce_admin_payment_provider_invalid_response"',
    'payment provider response code',
  ],
  [
    '"commerce_admin_payment_reconciliation_required"',
    'payment reconciliation code',
  ],
  [
    '"commerce_admin_payment_provider_not_configured"',
    'payment provider configuration code',
  ],
  [
    '"commerce_admin_payment_storage_unavailable"',
    'payment storage code',
  ],
]) requireText(paymentPolicy, value, label);

for (const [value, label] of [
  ['PaymentError::ProviderOutcomeUnknown { .. }', 'reserved unknown-outcome variant'],
  ['PaymentError::ProviderInvalidResponse { .. }', 'reserved invalid-response variant'],
  ['PaymentError::ProviderUnavailable { .. }', 'reserved unavailable variant'],
  [
    '"commerce_admin_refund_reconciliation_required"',
    'reserved refund reconciliation code',
  ],
  [
    '"Refund remains reserved while the provider outcome is reconciled"',
    'reserved refund reconciliation message',
  ],
  [
    '"commerce_admin_refund_provider_unavailable"',
    'reserved refund retry code',
  ],
  [
    '"Refund remains reserved and the provider operation may be retried safely"',
    'reserved refund retry message',
  ],
  ['error => payment_error_policy(error)', 'reserved refund fallback'],
]) requireText(reservedRefundPolicy, value, label);

for (const [value, label] of [
  ['OrderError::Validation(_)', 'order validation variant'],
  ['OrderError::OrderNotFound(_)', 'order not-found variant'],
  ['OrderError::OrderReturnNotFound(_)', 'return not-found variant'],
  ['OrderError::OrderChangeNotFound(_)', 'change not-found variant'],
  ['OrderError::InvalidTransition { .. }', 'order transition variant'],
  ['OrderError::Database(_)', 'order database variant'],
  ['OrderError::Core(_)', 'order core variant'],
  ['"commerce_admin_order_invalid"', 'order invalid code'],
  ['"commerce_admin_order_state_conflict"', 'order conflict code'],
  ['"commerce_admin_order_storage_unavailable"', 'order storage code'],
  ['"commerce_admin_order_failed"', 'order fail-closed code'],
]) requireText(orderPolicy, value, label);

for (const [value, label] of [
  ['error = ?error', 'typed cause log'],
  ['owner = ADMIN_CHECKOUT_OPERATION_OWNER', 'owner log'],
  ['source_owner,', 'source owner log'],
  ['tenant_id = %context.tenant_id', 'tenant log'],
  ['actor_id = %context.actor_id', 'actor log'],
  [
    'checkout_operation_id = ?context.checkout_operation_id',
    'checkout operation identity log',
  ],
  ['reservation_id = ?context.reservation_id', 'reservation identity log'],
  [
    'payment_collection_id = ?context.payment_collection_id',
    'payment collection identity log',
  ],
  ['payment_id = ?context.payment_id', 'payment identity log'],
  ['refund_id = ?context.refund_id', 'refund identity log'],
  ['order_id = ?context.order_id', 'order identity log'],
  ['order_return_id = ?context.order_return_id', 'order return identity log'],
  ['order_change_id = ?context.order_change_id', 'order change identity log'],
  ['operation = %context.operation', 'route operation log'],
  ['error_kind,', 'error-kind log'],
  ['public_code = code', 'public-code log'],
  ['status = %status', 'status log'],
  ['boundary = ADMIN_CHECKOUT_OPERATION_BOUNDARY', 'boundary log'],
  ['HttpError::new(status, code, message)', 'single static envelope constructor'],
]) requireText(logger, value, label);

for (const [value, label] of [
  ['CheckoutCompensationError::Operation(source)', 'operation wrapper variant'],
  [
    'CheckoutCompensationError::ReservationJournal(source)',
    'reservation journal variant',
  ],
  ['CheckoutCompensationError::Payment(source)', 'payment wrapper variant'],
  [
    'CheckoutCompensationError::PaymentOrchestration(source)',
    'payment orchestration wrapper variant',
  ],
  ['CheckoutCompensationError::Order(source)', 'order wrapper variant'],
  [
    'CheckoutCompensationError::ManualReconciliation(_)',
    'manual reconciliation variant',
  ],
  ['CheckoutCompensationError::Conflict(_)', 'compensation conflict variant'],
  ['CheckoutCompensationError::Boundary {', 'compensation boundary variant'],
  [
    'CheckoutCompensationError::CompensationAndJournal { .. }',
    'compensation and journal variant',
  ],
  ['PaymentOrchestrationError::Provider(source)', 'provider orchestration variant'],
  ['PaymentOrchestrationError::Payment(source)', 'payment owner orchestration variant'],
  [
    'PaymentOrchestrationError::ProviderAfterRefundReservation {',
    'reserved refund orchestration variant',
  ],
  ['context.refund_id = Some(*refund_id);', 'reserved refund identity adoption'],
  ['adopt_operation_error_identity(&mut context, source)', 'operation identity adoption'],
  [
    'adopt_reservation_error_identity(&mut context, source)',
    'reservation identity adoption',
  ],
  ['adopt_payment_error_identity(&mut context, source)', 'payment identity adoption'],
  ['adopt_order_error_identity(&mut context, source)', 'order identity adoption'],
  ['"checkout_reconciliation_required"', 'manual reconciliation code'],
  ['"checkout_compensation_conflict"', 'compensation conflict code'],
  ['"checkout_compensation_pending"', 'retryable compensation code'],
  ['"Checkout compensation is unavailable"', 'fail-closed compensation message'],
]) requireText(compensationMapper, value, label);

for (const [block, permission, operation, identity, serviceCall, mapper, label] of [
  [
    showRoute,
    '[Permission::ORDERS_READ]',
    '"show_checkout_operation"',
    'tenant.id,\n                    auth.user_id,\n                    Some(id),',
    '.get(tenant.id, id)',
    'map_operation_error(',
    'show route',
  ],
  [
    compensateRoute,
    '[Permission::ORDERS_MANAGE]',
    '"compensate_checkout_operation"',
    'tenant.id,\n                    auth.user_id,\n                    Some(id),',
    '.compensate(',
    'map_compensation_error(',
    'compensate route',
  ],
  [
    sweepRoute,
    '[Permission::ORDERS_MANAGE]',
    '"sweep_checkout_compensations"',
    'tenant.id,\n                auth.user_id,\n                None,',
    '.run(',
    'map_sweep_error(',
    'sweep route',
  ],
]) {
  requireText(block, permission, `${label} permission`);
  requireText(block, '.map_err(|error| {', `${label} typed mapping closure`);
  requireText(block, 'AdminCheckoutOperationErrorContext::new(', `${label} context`);
  requireText(block, operation, `${label} operation`);
  requireText(block, identity, `${label} truthful identity`);
  requireText(block, serviceCall, `${label} service contract`);
  requireText(block, mapper, `${label} mapper handoff`);
}

for (const [value, label] of [
  [
    '.with_payment_provider_registry(runtime.payment_provider_registry())',
    'provider registry forwarding',
  ],
  [
    'rustok_inventory::in_process_inventory_reservation_identity_port(runtime.db_clone())',
    'inventory identity port forwarding',
  ],
  [
    'in_process_cart_checkout_port(runtime.db_clone())',
    'cart checkout port forwarding',
  ],
  ['format!(\n                "admin-checkout-compensation:{}:{}"', 'compensation lease owner'],
  ['format!("admin:{}", auth.user_id)', 'sweep worker identity'],
  ['input.limit', 'sweep limit forwarding'],
  ['Ok(Json(map_operation(operation)))', 'operation response contract'],
  ['scanned: report.scanned', 'sweep scanned response'],
  ['failures: report', 'sweep failure response'],
]) requireText(controller, value, label);

for (const [value, label] of [
  ['error: DbErr,', 'typed sweep database cause'],
  ['"rustok_commerce.checkout_compensation_sweep"', 'sweep source owner'],
  ['"Checkout compensation storage is unavailable"', 'static sweep storage message'],
  ['"database"', 'sweep database kind'],
]) requireText(sweepMapper, value, label);

for (const [value, label] of [
  ['CheckoutCompensationError::Boundary { .. }', 'sweep boundary code branch'],
  ['CheckoutCompensationError::ReservationJournal(_)', 'sweep reservation code branch'],
  ['CheckoutCompensationError::Payment(_)', 'sweep payment code branch'],
  [
    'CheckoutCompensationError::PaymentOrchestration(_)',
    'sweep payment orchestration code branch',
  ],
  ['CheckoutCompensationError::Order(_)', 'sweep order code branch'],
  [
    'CheckoutCompensationError::ManualReconciliation(_)',
    'sweep manual reconciliation code branch',
  ],
  ['CheckoutCompensationError::Operation(_)', 'sweep operation code branch'],
  ['CheckoutCompensationError::Conflict(_)', 'sweep conflict code branch'],
  [
    'CheckoutCompensationError::CompensationAndJournal { .. }',
    'sweep compensation and journal code branch',
  ],
  [
    '"checkout.compensation_payment_failed"',
    'safe payment sweep code',
  ],
  ['"checkout.compensation_order_failed"', 'safe order sweep code'],
]) requireText(sweep, value, label);

for (const [ownerSource, value, label] of [
  [operationErrors, 'Validation(String)', 'operation validation source variant'],
  [operationErrors, 'NotFound(Uuid)', 'operation not-found source variant'],
  [operationErrors, 'Conflict(String)', 'operation conflict source variant'],
  [operationErrors, 'Database(#[from] sea_orm::DbErr)', 'operation database source variant'],
  [reservationErrors, 'Validation(String)', 'reservation validation source variant'],
  [reservationErrors, 'NotFound(Uuid)', 'reservation not-found source variant'],
  [reservationErrors, 'Conflict(String)', 'reservation conflict source variant'],
  [reservationErrors, 'Database(#[from] sea_orm::DbErr)', 'reservation database source variant'],
  [compensationErrors, 'Operation(#[from] CheckoutOperationError)', 'compensation operation source variant'],
  [compensationErrors, 'ReservationJournal(#[from] CheckoutInventoryReservationError)', 'compensation reservation source variant'],
  [compensationErrors, 'Payment(#[from] PaymentError)', 'compensation payment source variant'],
  [compensationErrors, 'PaymentOrchestration(#[from] PaymentOrchestrationError)', 'compensation payment orchestration source variant'],
  [compensationErrors, 'Order(#[from] OrderError)', 'compensation order source variant'],
  [compensationErrors, 'ManualReconciliation(String)', 'compensation manual source variant'],
  [compensationErrors, 'Conflict(String)', 'compensation conflict source variant'],
  [compensationErrors, 'Boundary {', 'compensation boundary source variant'],
  [compensationErrors, 'CompensationAndJournal {', 'compensation journal source variant'],
  [paymentErrors, 'PaymentCollectionNotFound(Uuid)', 'payment collection source variant'],
  [paymentErrors, 'PaymentNotFound(Uuid)', 'payment source variant'],
  [paymentErrors, 'RefundNotFound(Uuid)', 'refund source variant'],
  [paymentErrors, 'ProviderUnavailable {', 'provider unavailable source variant'],
  [paymentErrors, 'ProviderRejected {', 'provider rejected source variant'],
  [paymentErrors, 'ProviderInvalidResponse {', 'provider invalid-response source variant'],
  [paymentErrors, 'ProviderOutcomeUnknown {', 'provider unknown source variant'],
  [paymentErrors, 'ProviderConfiguration { provider_id: String }', 'provider configuration source variant'],
  [orderErrors, 'OrderNotFound(Uuid)', 'order source variant'],
  [orderErrors, 'OrderReturnNotFound(Uuid)', 'order return source variant'],
  [orderErrors, 'OrderChangeNotFound(Uuid)', 'order change source variant'],
  [paymentOrchestration, 'Provider(#[source] PaymentError)', 'payment orchestration provider source variant'],
  [paymentOrchestration, 'ProviderAfterRefundReservation {', 'payment orchestration reserved source variant'],
  [paymentOrchestration, 'Payment(#[from] PaymentError)', 'payment orchestration owner source variant'],
]) requireText(ownerSource, value, label);

const contextMapperUses =
  controller.match(
    /map_(?:operation|compensation|sweep)_error\(\s+AdminCheckoutOperationErrorContext::new\(/g,
  ) ?? [];
if (contextMapperUses.length !== 3) {
  failures.push(
    `expected three context-aware admin checkout mapper callsites, found ${contextMapperUses.length}`,
  );
}

for (const value of [
  '.map_err(map_operation_error)?;',
  '.map_err(map_compensation_error)?;',
  '.map_err(|_| HttpError::internal(',
  'HttpError::new(StatusCode::CONFLICT, "checkout_operation_conflict", message)',
  'HttpError::bad_request("checkout_operation_invalid", message)',
  'error.to_string()',
  'err.to_string()',
  'other.to_string()',
]) forbidText(controller, value, 'unsafe admin checkout operation public conversion');

if (failures.length > 0) {
  console.error('Commerce admin checkout-operation error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin checkout operations retain typed causes and static public envelopes',
);
