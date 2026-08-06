#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/rustok-commerce/src/controllers/admin/payments.rs');
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

const diagnosticContext = between(
  source,
  'struct AdminPaymentDiagnosticContext {',
  '#[utoipa::path(',
  'admin payment diagnostic projection',
);
const paymentPolicy = between(
  source,
  'fn payment_error_policy(',
  'fn reserved_refund_error_policy(',
  'payment HTTP policy',
);
const reservedRefundPolicy = between(
  source,
  'fn reserved_refund_error_policy(',
  'fn adopt_payment_error_identity(',
  'reserved refund HTTP policy',
);
const identityAdoption = between(
  source,
  'fn adopt_payment_error_identity(',
  'fn admin_payment_http_error<E>(',
  'payment identity adoption',
);
const diagnosticHelper = between(
  source,
  'fn admin_payment_http_error<E>(',
  'fn map_admin_payment_error(',
  'shared admin payment diagnostic helper',
);
const ownerMapper = between(
  source,
  'fn map_admin_payment_error(',
  'fn map_admin_payment_orchestration_error(',
  'admin payment owner mapper',
);
const orchestrationMapper = between(
  source,
  'fn map_admin_payment_orchestration_error(',
  'fn refund_creation_key(',
  'admin payment orchestration mapper',
);

for (const [value, label] of [
  ['struct AdminPaymentErrorContext {', 'typed error context'],
  ['tenant_id: Uuid,', 'typed tenant identity'],
  ['actor_id: Uuid,', 'typed actor identity'],
  ['payment_collection_id: Option<Uuid>,', 'typed collection identity'],
  ['refund_id: Option<Uuid>,', 'typed refund identity'],
  ['order_id: Option<Uuid>,', 'typed order identity'],
  ['cart_id: Option<Uuid>,', 'typed cart identity'],
  ['customer_id: Option<Uuid>,', 'typed customer identity'],
  ["operation: &'static str,", 'typed operation'],
  ['struct AdminPaymentDiagnosticContext {', 'bounded diagnostic context'],
  ['impl From<&AdminPaymentErrorContext> for AdminPaymentDiagnosticContext', 'typed-to-diagnostic conversion'],
  ['struct AdminPaymentDiagnosticError;', 'bounded diagnostic error'],
  ['formatter.write_str("redacted")', 'redacted Debug output'],
  ["fn uuid_shape(value: Uuid) -> &'static str", 'required UUID shape helper'],
  ["fn optional_uuid_shape(value: Option<Uuid>) -> &'static str", 'optional UUID shape helper'],
  ['"nil"', 'nil UUID shape'],
  ['"non_nil"', 'non-nil UUID shape'],
  ['"absent"', 'absent optional shape'],
  ['"present_nil"', 'present nil optional shape'],
  ['"present_non_nil"', 'present non-nil optional shape'],
]) requireText(source, value, label);

for (const field of [
  'tenant_id',
  'actor_id',
  'payment_collection_id',
  'refund_id',
  'order_id',
  'cart_id',
  'customer_id',
]) {
  requireText(diagnosticContext, `${field}:`, `${field} diagnostic field`);
}
for (const [value, label] of [
  ['tenant_id: uuid_shape(context.tenant_id)', 'tenant projection'],
  ['actor_id: uuid_shape(context.actor_id)', 'actor projection'],
  ['payment_collection_id: optional_uuid_shape(context.payment_collection_id)', 'collection projection'],
  ['refund_id: optional_uuid_shape(context.refund_id)', 'refund projection'],
  ['order_id: optional_uuid_shape(context.order_id)', 'order projection'],
  ['cart_id: optional_uuid_shape(context.cart_id)', 'cart projection'],
  ['customer_id: optional_uuid_shape(context.customer_id)', 'customer projection'],
  ['operation: context.operation', 'static operation projection'],
]) requireText(diagnosticContext, value, label);

for (const [value, label] of [
  ['PaymentError::PaymentCollectionNotFound(_)', 'collection not-found policy'],
  ['PaymentError::PaymentNotFound(_)', 'payment not-found policy'],
  ['PaymentError::RefundNotFound(_)', 'refund not-found policy'],
  ['PaymentError::Validation(_)', 'validation policy'],
  ['PaymentError::InvalidTransition { .. }', 'transition policy'],
  ['PaymentError::ProviderRejected { .. }', 'provider rejection policy'],
  ['PaymentError::ProviderUnavailable { .. }', 'provider unavailable policy'],
  ['PaymentError::ProviderInvalidResponse { .. }', 'provider invalid-response policy'],
  ['PaymentError::ProviderOutcomeUnknown { .. }', 'provider unknown-outcome policy'],
  ['PaymentError::ProviderConfiguration { .. }', 'provider configuration policy'],
  ['PaymentError::Database(_)', 'database policy'],
  ['StatusCode::BAD_GATEWAY', 'bad-gateway policy'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'service-unavailable policy'],
  ['StatusCode::CONFLICT', 'conflict policy'],
]) requireText(paymentPolicy, value, label);

for (const [value, label] of [
  ['"commerce_admin_refund_reconciliation_required"', 'reserved refund reconciliation code'],
  ['"commerce_admin_refund_provider_unavailable"', 'reserved refund unavailable code'],
  ['error => payment_error_policy(error)', 'reserved refund policy fallback'],
]) requireText(reservedRefundPolicy, value, label);

for (const [value, label] of [
  ['PaymentError::PaymentCollectionNotFound(id) | PaymentError::PaymentNotFound(id)', 'collection identity variants'],
  ['context.payment_collection_id = Some(*id);', 'collection identity adoption'],
  ['PaymentError::RefundNotFound(id)', 'refund identity variant'],
  ['context.refund_id = Some(*id)', 'refund identity adoption'],
]) requireText(identityAdoption, value, label);

for (const [value, label] of [
  ['let context = AdminPaymentDiagnosticContext::from(context);', 'diagnostic context shadow'],
  ['let error = AdminPaymentDiagnosticError;', 'diagnostic error shadow'],
  ['error = ?error', 'redacted error event field'],
  ['owner = ADMIN_PAYMENT_OWNER', 'owner event field'],
  ['source_owner,', 'source-owner event field'],
  ['tenant_id = %context.tenant_id', 'tenant shape event field'],
  ['actor_id = %context.actor_id', 'actor shape event field'],
  ['payment_collection_id = ?context.payment_collection_id', 'collection shape event field'],
  ['refund_id = ?context.refund_id', 'refund shape event field'],
  ['order_id = ?context.order_id', 'order shape event field'],
  ['cart_id = ?context.cart_id', 'cart shape event field'],
  ['customer_id = ?context.customer_id', 'customer shape event field'],
  ['operation = %context.operation', 'operation event field'],
  ['error_kind,', 'error-kind event field'],
  ['public_code = code', 'public-code event field'],
  ['status = %status', 'status event field'],
  ['boundary = ADMIN_PAYMENT_BOUNDARY', 'boundary event field'],
  ['HttpError::new(status, code, message)', 'static HTTP envelope'],
]) requireText(diagnosticHelper, value, label);
requireBefore(
  diagnosticHelper,
  'let context = AdminPaymentDiagnosticContext::from(context);',
  'tracing::error!(',
  'context shadow ordering',
);
requireBefore(
  diagnosticHelper,
  'let error = AdminPaymentDiagnosticError;',
  'tracing::error!(',
  'error shadow ordering',
);

for (const [value, label] of [
  ['adopt_payment_error_identity(&mut context, &error);', 'owner identity adoption'],
  ['let policy = payment_error_policy(&error);', 'owner typed policy'],
  ['admin_payment_http_error(&context, &error, "rustok_payment", policy)', 'owner shared helper delegation'],
]) requireText(ownerMapper, value, label);

for (const [value, label] of [
  ['PaymentOrchestrationError::Payment(source)', 'payment orchestration variant'],
  ['PaymentOrchestrationError::Provider(source)', 'provider orchestration variant'],
  ['adopt_payment_error_identity(&mut context, source);', 'orchestration identity adoption'],
  ['payment_error_policy(source)', 'orchestration typed policy'],
  ['PaymentOrchestrationError::ProviderAfterRefundReservation { refund_id, source }', 'reserved refund orchestration variant'],
  ['context.refund_id = Some(*refund_id);', 'reserved refund identity adoption'],
  ['reserved_refund_error_policy(source)', 'reserved refund typed policy'],
  ['admin_payment_http_error(&context, &error, "rustok_payment", policy)', 'orchestration shared helper delegation'],
]) requireText(orchestrationMapper, value, label);

const ownerMapperUses = source.match(/map_admin_payment_error\(/g) ?? [];
if (ownerMapperUses.length !== 5) {
  failures.push(`expected helper definition plus four owner mapper callsites, found ${ownerMapperUses.length}`);
}
const orchestrationMapperUses =
  source.match(/map_admin_payment_orchestration_error\(/g) ?? [];
if (orchestrationMapperUses.length !== 7) {
  failures.push(
    `expected helper definition plus six orchestration mapper callsites, found ${orchestrationMapperUses.length}`,
  );
}

for (const [value, label] of [
  ['pub async fn list_payment_collections(', 'collection list route'],
  ['pub async fn show_payment_collection(', 'collection detail route'],
  ['pub async fn authorize_payment_collection(', 'authorize route'],
  ['pub async fn capture_payment_collection(', 'capture route'],
  ['pub async fn cancel_payment_collection(', 'collection cancel route'],
  ['pub async fn create_refund(', 'refund create route'],
  ['pub async fn list_refunds(', 'refund list route'],
  ['pub async fn show_refund(', 'refund detail route'],
  ['pub async fn complete_refund(', 'refund complete route'],
  ['pub async fn cancel_refund(', 'refund cancel route'],
  ['[Permission::PAYMENTS_READ]', 'read permission'],
  ['[Permission::PAYMENTS_UPDATE]', 'update permission'],
  ['.with_provider_registry(runtime.payment_provider_registry())', 'provider registry composition'],
  ['refund_creation_key(&headers)?', 'refund idempotency-key validation'],
]) requireText(source, value, label);

for (const value of [
  'error.to_string()',
  'err.to_string()',
  'format!("Payment request is invalid:',
  'format!("Payment provider',
  'HttpError::bad_request("commerce_admin_payment_invalid", error',
]) forbidText(source, value, 'unsafe admin payment public mapping');

if (failures.length > 0) {
  console.error('Commerce admin payment diagnostic safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin payment diagnostics retain typed policy and identities while emitting only bounded shapes',
);
