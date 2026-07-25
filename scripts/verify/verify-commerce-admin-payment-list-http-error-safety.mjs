#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const payments = read('crates/rustok-commerce/src/controllers/admin/payments.rs');
const paymentErrors = read('crates/rustok-payment/src/error.rs');
const paymentOrchestration = read(
  'crates/rustok-commerce/src/services/payment_orchestration.rs',
);
const paymentService = read('crates/rustok-payment/src/services/payment.rs');
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

const listCollectionsRoute = between(
  payments,
  'pub async fn list_payment_collections(',
  'pub async fn show_payment_collection(',
  'payment collection list route',
);
const listRefundsRoute = between(
  payments,
  'pub async fn list_refunds(',
  'pub async fn show_refund(',
  'refund list route',
);
const paymentPolicy = between(
  payments,
  'fn payment_error_policy(',
  'fn reserved_refund_error_policy(',
  'admin payment policy',
);
const reservedRefundPolicy = between(
  payments,
  'fn reserved_refund_error_policy(',
  'fn adopt_payment_error_identity(',
  'admin reserved refund policy',
);
const logger = between(
  payments,
  'fn admin_payment_http_error<E>(',
  'fn map_admin_payment_error(',
  'admin payment logger',
);
const orchestrationMapper = between(
  payments,
  'fn map_admin_payment_orchestration_error(',
  'fn refund_creation_key(',
  'admin payment orchestration mapper',
);
const serviceList = between(
  paymentService,
  'pub async fn list_collections(',
  'pub async fn attach_order_to_collection(',
  'payment service list operation',
);

for (const [value, label] of [
  ['use rustok_payment::error::PaymentError;', 'typed payment error import'],
  ['use crate::PaymentOrchestrationError;', 'typed orchestration import'],
  [
    'const ADMIN_PAYMENT_OWNER: &str = "rustok_payment.admin_payments";',
    'admin payment owner constant',
  ],
  [
    'const ADMIN_PAYMENT_BOUNDARY: &str = "commerce_admin_payment_http";',
    'admin payment boundary constant',
  ],
  ['type AdminPaymentHttpPolicy = (', 'static payment policy type'],
  ['struct AdminPaymentErrorContext {', 'typed payment context'],
  ['tenant_id: Uuid,', 'tenant context field'],
  ['actor_id: Uuid,', 'actor context field'],
  ['payment_collection_id: Option<Uuid>,', 'collection context field'],
  ['refund_id: Option<Uuid>,', 'refund context field'],
  ['order_id: Option<Uuid>,', 'order context field'],
  ['cart_id: Option<Uuid>,', 'cart context field'],
  ['customer_id: Option<Uuid>,', 'customer context field'],
  ["operation: &'static str,", 'operation context field'],
  ['fn map_admin_payment_error(', 'local owner mapper'],
  [
    'fn map_admin_payment_orchestration_error(',
    'local orchestration mapper',
  ],
]) requireText(payments, value, label);

for (const [value, label] of [
  ['PaymentError::PaymentCollectionNotFound(_)', 'collection not-found variant'],
  ['PaymentError::PaymentNotFound(_)', 'payment not-found variant'],
  ['PaymentError::RefundNotFound(_)', 'refund not-found variant'],
  ['PaymentError::Validation(_)', 'validation variant'],
  ['PaymentError::InvalidTransition { .. }', 'transition variant'],
  ['PaymentError::ProviderRejected { .. }', 'provider rejection variant'],
  ['PaymentError::ProviderUnavailable { .. }', 'provider unavailable variant'],
  ['PaymentError::ProviderInvalidResponse { .. }', 'provider invalid-response variant'],
  ['PaymentError::ProviderOutcomeUnknown { .. }', 'provider unknown-outcome variant'],
  ['PaymentError::ProviderConfiguration { .. }', 'provider configuration variant'],
  ['PaymentError::Database(_)', 'database variant'],
  ['"commerce_admin_not_found"', 'shared not-found code'],
  ['"commerce_admin_payment_invalid"', 'payment invalid code'],
  ['"commerce_admin_payment_state_conflict"', 'payment state code'],
  ['"commerce_admin_payment_provider_unavailable"', 'provider unavailable code'],
  ['"commerce_admin_payment_provider_invalid_response"', 'provider invalid-response code'],
  ['"commerce_admin_payment_reconciliation_required"', 'reconciliation code'],
  ['"commerce_admin_payment_provider_not_configured"', 'provider configuration code'],
  ['"commerce_admin_payment_storage_unavailable"', 'storage unavailable code'],
  ['"Payment request is invalid"', 'static validation message'],
  [
    '"Payment operation conflicts with the current state"',
    'static transition message',
  ],
  [
    '"Payment provider is temporarily unavailable"',
    'static provider unavailable message',
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
  ['error = ?error', 'typed cause log'],
  ['owner = ADMIN_PAYMENT_OWNER', 'owner log'],
  ['source_owner,', 'source owner log'],
  ['tenant_id = %context.tenant_id', 'tenant log'],
  ['actor_id = %context.actor_id', 'actor log'],
  [
    'payment_collection_id = ?context.payment_collection_id',
    'collection identity log',
  ],
  ['refund_id = ?context.refund_id', 'refund identity log'],
  ['order_id = ?context.order_id', 'order identity log'],
  ['cart_id = ?context.cart_id', 'cart identity log'],
  ['customer_id = ?context.customer_id', 'customer identity log'],
  ['operation = %context.operation', 'route operation log'],
  ['error_kind,', 'error-kind log'],
  ['public_code = code', 'public-code log'],
  ['status = %status', 'status log'],
  ['boundary = ADMIN_PAYMENT_BOUNDARY', 'boundary log'],
  ['HttpError::new(status, code, message)', 'single static envelope constructor'],
]) requireText(logger, value, label);

for (const [value, label] of [
  ['PaymentOrchestrationError::Payment(source)', 'owner orchestration variant'],
  ['PaymentOrchestrationError::Provider(source)', 'provider orchestration variant'],
  [
    'PaymentOrchestrationError::ProviderAfterRefundReservation { refund_id, source }',
    'reserved refund orchestration variant',
  ],
  ['adopt_payment_error_identity(&mut context, source)', 'nested identity adoption'],
  ['context.refund_id = Some(*refund_id);', 'reserved refund identity adoption'],
  ['reserved_refund_error_policy(source)', 'reserved refund policy selection'],
  [
    'admin_payment_http_error(&context, &error, "rustok_payment", policy)',
    'single orchestration log handoff',
  ],
]) requireText(orchestrationMapper, value, label);

for (const [value, label] of [
  ['Permission::PAYMENTS_READ', 'payment read permission'],
  ['PaymentService::new(runtime.db_clone())', 'payment service construction'],
  ['.list_collections(', 'payment collection list call'],
  ['tenant.id,', 'tenant argument'],
  ['page: pagination.page', 'page argument'],
  ['per_page: pagination.limit()', 'per-page argument'],
  ['status: params.status', 'status filter argument'],
  ['let order_id = params.order_id;', 'order filter capture'],
  ['let cart_id = params.cart_id;', 'cart filter capture'],
  ['let customer_id = params.customer_id;', 'customer filter capture'],
  ['order_id,', 'order filter forwarding'],
  ['cart_id,', 'cart filter forwarding'],
  ['customer_id,', 'customer filter forwarding'],
  ['"list_payment_collections"', 'list route operation'],
  ['.with_filters(order_id, cart_id, customer_id)', 'list filter context'],
  ['PaginationMeta::new(pagination.page, pagination.limit(), total)', 'pagination response metadata'],
]) requireText(listCollectionsRoute, value, label);

for (const [value, label] of [
  ['let payment_collection_id = params.payment_collection_id;', 'refund collection filter capture'],
  ['let order_id = params.order_id;', 'refund order filter capture'],
  ['payment_collection_id,', 'refund collection filter forwarding'],
  ['order_id,', 'refund order filter forwarding'],
  ['status: params.status', 'refund status filter forwarding'],
  ['"list_refunds"', 'refund list operation'],
  ['.with_payment_collection_id(payment_collection_id)', 'refund collection context'],
  ['.with_filters(order_id, None, None)', 'refund order context'],
]) requireText(listRefundsRoute, value, label);

for (const [handler, permission, operation, serviceCall, identity, mapper, label] of [
  ['show_payment_collection', 'PAYMENTS_READ', 'show_payment_collection', '.get_collection(tenant.id, id)', '.with_payment_collection_id(Some(id))', 'map_admin_payment_error(', 'collection detail'],
  ['authorize_payment_collection', 'PAYMENTS_UPDATE', 'authorize_payment_collection', '.authorize_collection(tenant.id, id, input)', '.with_payment_collection_id(Some(id))', 'map_admin_payment_orchestration_error(', 'collection authorize'],
  ['capture_payment_collection', 'PAYMENTS_UPDATE', 'capture_payment_collection', '.capture_collection(tenant.id, id, input)', '.with_payment_collection_id(Some(id))', 'map_admin_payment_orchestration_error(', 'collection capture'],
  ['cancel_payment_collection', 'PAYMENTS_UPDATE', 'cancel_payment_collection', '.cancel_collection(tenant.id, id, input)', '.with_payment_collection_id(Some(id))', 'map_admin_payment_orchestration_error(', 'collection cancel'],
  ['create_refund', 'PAYMENTS_UPDATE', 'create_refund', '.create_refund_idempotent(tenant.id, id, creation_key, input)', '.with_payment_collection_id(Some(id))', 'map_admin_payment_orchestration_error(', 'refund create'],
  ['show_refund', 'PAYMENTS_READ', 'show_refund', '.get_refund(tenant.id, id)', '.with_refund_id(Some(id))', 'map_admin_payment_error(', 'refund detail'],
  ['complete_refund', 'PAYMENTS_UPDATE', 'complete_refund', '.complete_refund(tenant.id, id, input)', '.with_refund_id(Some(id))', 'map_admin_payment_orchestration_error(', 'refund complete'],
  ['cancel_refund', 'PAYMENTS_UPDATE', 'cancel_refund', '.cancel_refund(tenant.id, id, input)', '.with_refund_id(Some(id))', 'map_admin_payment_orchestration_error(', 'refund cancel'],
]) {
  const start = `pub async fn ${handler}(`;
  const startIndex = payments.indexOf(start);
  const nextIndex = payments.indexOf('\n#[utoipa::path(', startIndex + start.length);
  const block = startIndex < 0
    ? ''
    : payments.slice(startIndex, nextIndex < 0 ? payments.length : nextIndex);
  if (startIndex < 0) failures.push(`${label}: unable to isolate handler`);
  requireText(block, `Permission::${permission}`, `${label} permission`);
  requireText(block, `"${operation}"`, `${label} operation`);
  requireText(block, serviceCall, `${label} service contract`);
  requireText(block, identity, `${label} truthful identity`);
  requireText(block, mapper, `${label} local mapper`);
}

for (const [value, label] of [
  [
    '.with_provider_registry(runtime.payment_provider_registry())',
    'provider registry forwarding',
  ],
  ['refund_creation_key(&headers)?', 'refund idempotency validation'],
  ['"refund_idempotency_key_required"', 'required idempotency code'],
  ['"refund_idempotency_key_invalid"', 'invalid idempotency code'],
  ['MAX_REFUND_CREATION_KEY_LENGTH', 'idempotency length bound'],
  ['Ok((StatusCode::CREATED, Json(refund)))', 'refund creation response'],
]) requireText(payments, value, label);

const ownerMapperUses =
  payments.match(/map_admin_payment_error\(\s+AdminPaymentErrorContext::new\(/g) ?? [];
if (ownerMapperUses.length !== 4) {
  failures.push(
    `expected four context-aware payment owner mapper callsites, found ${ownerMapperUses.length}`,
  );
}
const orchestrationMapperUses =
  payments.match(
    /map_admin_payment_orchestration_error\(\s+AdminPaymentErrorContext::new\(/g,
  ) ?? [];
if (orchestrationMapperUses.length !== 6) {
  failures.push(
    `expected six context-aware payment orchestration mapper callsites, found ${orchestrationMapperUses.length}`,
  );
}
const providerRegistryUses =
  payments.match(/\.with_provider_registry\(runtime\.payment_provider_registry\(\)\)/g) ?? [];
if (providerRegistryUses.length !== 6) {
  failures.push(
    `expected six provider-registry orchestration callsites, found ${providerRegistryUses.length}`,
  );
}

for (const value of [
  '.map_err(super::map_payment_error)?;',
  '.map_err(super::map_payment_orchestration_error)?;',
  'HttpError::new(StatusCode::CONFLICT, "checkout_operation_conflict", message)',
  'err.to_string()',
  'error.to_string()',
  'other.to_string()',
  'commerce_operation_failed',
]) forbidText(payments, value, 'unsafe admin payment public conversion');

for (const [value, label] of [
  ['Validation(String)', 'owner validation variant'],
  ['PaymentCollectionNotFound(Uuid)', 'owner collection variant'],
  ['PaymentNotFound(Uuid)', 'owner payment variant'],
  ['RefundNotFound(Uuid)', 'owner refund variant'],
  ['InvalidTransition { from: String, to: String }', 'owner transition variant'],
  ['ProviderUnavailable {', 'owner unavailable variant'],
  ['ProviderRejected {', 'owner rejection variant'],
  ['ProviderInvalidResponse {', 'owner invalid-response variant'],
  ['ProviderOutcomeUnknown {', 'owner unknown-outcome variant'],
  ['ProviderConfiguration { provider_id: String }', 'owner configuration variant'],
  ['Database(#[from] DbErr)', 'owner database variant'],
]) requireText(paymentErrors, value, label);

for (const [value, label] of [
  ['Provider(#[source] PaymentError)', 'orchestration provider variant'],
  ['ProviderAfterRefundReservation {', 'orchestration reserved-refund variant'],
  ['refund_id: Uuid,', 'orchestration refund identity'],
  ['Payment(#[from] PaymentError)', 'orchestration owner variant'],
]) requireText(paymentOrchestration, value, label);

for (const [value, label] of [
  ['-> PaymentResult<(Vec<PaymentCollectionResponse>, u64)>', 'typed list result'],
  ['input.per_page.clamp(1, 100)', 'service page-size bound'],
  ['payment_collection::Column::TenantId.eq(tenant_id)', 'service tenant filter'],
  ['Self::normalize_collection_status_filter(&status)?', 'typed status validation'],
  ['payment_collection::Column::OrderId.eq(order_id)', 'service order filter'],
  ['payment_collection::Column::CartId.eq(cart_id)', 'service cart filter'],
  ['payment_collection::Column::CustomerId.eq(customer_id)', 'service customer filter'],
  ['query.clone().count(&self.db).await?', 'service count propagation'],
  ['.all(&self.db)', 'service page query'],
  ['items.push(self.build_response(row).await?);', 'response build propagation'],
]) requireText(serviceList, value, label);

if (failures.length > 0) {
  console.error('Commerce admin payment route error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Admin payment routes retain typed causes and use static public envelopes',
);
