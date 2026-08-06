#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const paths = {
  query: 'crates/rustok-commerce/src/graphql/query.rs',
  safeSource: 'crates/rustok-commerce/src/graphql/safe_query/source.rs',
  paymentShim:
    'crates/rustok-commerce/src/graphql/safe_query/source/rustok_payment_shim.rs',
  ownerService: 'crates/rustok-payment/src/services/payment.rs',
  ownerError: 'crates/rustok-payment/src/error.rs',
  evidence:
    'crates/rustok-commerce/contracts/evidence/graphql-query-payment-error-safety-source-review.json',
  document: 'crates/rustok-commerce/docs/graphql-query-payment-error-safety.md',
};

const query = read(paths.query);
const safeSource = read(paths.safeSource);
const paymentShim = read(paths.paymentShim);
const ownerService = read(paths.ownerService);
const ownerError = read(paths.ownerError);
const evidence = JSON.parse(read(paths.evidence));
const document = read(paths.document);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (source, value) => source.split(value).length - 1;

function blockBetween(source, start, end, label) {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
}

for (const marker of [
  'use rustok_payment::PaymentService;',
  '.find_reusable_collection_by_cart(tenant.id, cart.id)',
  '.find_latest_collection_by_order(tenant_id, id)',
  '.get_collection(tenant_id, id)',
  '.list_collections(',
  '.get_refund(tenant_id, id)',
  '.list_refunds(',
  'Err(rustok_payment::error::PaymentError::PaymentCollectionNotFound(_))',
  'Err(rustok_payment::error::PaymentError::RefundNotFound(_))',
]) requireText(query, marker, `${paths.query}: unchanged payment query contract`);

for (const [marker, expected] of [
  ['PaymentService::new(db.clone())', 7],
  ['.find_reusable_collection_by_cart(', 1],
  ['.find_latest_collection_by_order(', 1],
  ['.get_collection(', 1],
  ['.list_collections(', 1],
  ['.get_refund(', 1],
  ['.list_refunds(', 2],
]) {
  const actual = countText(query, marker);
  if (actual !== expected) {
    failures.push(`${paths.query}: expected ${expected} occurrences of ${marker}, found ${actual}`);
  }
}

for (const marker of [
  '#[path = "source/rustok_payment_shim.rs"]',
  'mod rustok_payment_shim;',
  'use self::rustok_payment_shim as rustok_payment;',
  'include!("../query.rs");',
]) requireText(safeSource, marker, `${paths.safeSource}: mounted Payment facade`);

for (const marker of [
  'inner: ::rustok_payment::PaymentService',
  'inner: ::rustok_payment::PaymentService::new(db)',
  'pub(crate) async fn find_reusable_collection_by_cart(',
  '.find_reusable_collection_by_cart(tenant_id, cart_id)',
  'pub(crate) async fn find_latest_collection_by_order(',
  '.find_latest_collection_by_order(tenant_id, order_id)',
  'pub(crate) async fn get_collection(',
  '.get_collection(tenant_id, collection_id)',
  'pub(crate) async fn list_collections(',
  '.list_collections(tenant_id, input)',
  'pub(crate) async fn get_refund(',
  '.get_refund(tenant_id, refund_id)',
  'pub(crate) async fn list_refunds(',
  '.list_refunds(tenant_id, input)',
]) requireText(paymentShim, marker, `${paths.paymentShim}: canonical owner delegation`);

for (const marker of [
  'pub(crate) enum PaymentError {',
  'PaymentCollectionNotFound(PaymentQueryError)',
  'RefundNotFound(PaymentQueryError)',
  'Other(PaymentQueryError)',
  'OwnerPaymentError::PaymentCollectionNotFound(_)',
  'OwnerPaymentError::RefundNotFound(_)',
  'pub(crate) fn to_string(self) -> BoundaryError',
  'error.into_boundary()',
]) requireText(paymentShim, marker, `${paths.paymentShim}: typed compatibility conversion`);

const mapper = blockBetween(
  paymentShim,
  'fn into_boundary(self) -> BoundaryError {',
  'pub(crate) mod error {',
  'typed Payment GraphQL mapper',
);

for (const [variant, message, code] of [
  ['OwnerPaymentError::Validation(_)', 'Payment query is invalid', 'PAYMENT_REQUEST_INVALID'],
  ['OwnerPaymentError::PaymentCollectionNotFound(_)', 'Payment resource was not found', 'PAYMENT_RESOURCE_NOT_FOUND'],
  ['OwnerPaymentError::InvalidTransition { .. }', 'Payment state conflicts with this query', 'PAYMENT_STATE_CONFLICT'],
  ['OwnerPaymentError::ProviderUnavailable { .. }', 'Payment data is temporarily unavailable', 'PAYMENT_TEMPORARILY_UNAVAILABLE'],
  ['OwnerPaymentError::ProviderInvalidResponse { .. }', 'Payment state requires reconciliation', 'PAYMENT_RECONCILIATION_REQUIRED'],
  ['OwnerPaymentError::ProviderConfiguration { .. }', 'Payment provider configuration is invalid', 'PAYMENT_CONFIGURATION_ERROR'],
]) {
  for (const marker of [variant, `"${message}"`, `"${code}"`]) {
    requireText(mapper, marker, `${paths.paymentShim}: ${code} transport policy`);
  }
}

for (const marker of [
  'owner_detail(&self.error)',
  'error = ?diagnostic_error',
  'owner = "rustok_payment"',
  'owner_operation = self.operation',
  'correlation_id',
  'tenant_id = %self.tenant_id',
  'resource_kind',
  'resource_id_shape',
  'owner_detail_shape',
  'owner_detail_length',
  'public_code = code',
  'boundary = GRAPHQL_QUERY_PAYMENT_BOUNDARY',
  'tracing::error!(',
  'tracing::warn!(',
  'BoundaryError::Public {',
]) requireText(mapper, marker, `${paths.paymentShim}: bounded Payment diagnostics`);

for (const forbidden of [
  'error = ?self.error',
  'error = %self.error',
  'owner_error = ?self.error',
  'owner_error = %self.error',
  'message = %self.error',
  'self.error.to_string()',
  'format!("{}", self.error)',
  'format!("{:?}", self.error)',
  'provider_id =',
  'provider_operation =',
  'validation_message =',
]) forbidText(mapper, forbidden, `${paths.paymentShim}: raw Payment owner payload`);

for (const marker of [
  'fn owner_detail(error: &OwnerPaymentError)',
  'OwnerPaymentError::Database(_) => ("database_redacted", 0)',
  '"provider_operation_values"',
  'provider_id.chars()',
  'operation.chars().count()',
  'from.chars().count().saturating_add(to.chars().count())',
  'fn uuid_shape(value: &Uuid)',
]) requireText(paymentShim, marker, `${paths.paymentShim}: bounded owner detail projection`);

for (const marker of [
  'pub struct PaymentService',
  'pub async fn find_reusable_collection_by_cart(',
  'pub async fn find_latest_collection_by_order(',
  'pub async fn get_collection(',
  'pub async fn list_collections(',
  'pub async fn get_refund(',
  'pub async fn list_refunds(',
  'PaymentResult<Option<PaymentCollectionResponse>>',
  'PaymentResult<PaymentCollectionResponse>',
  'PaymentResult<(Vec<PaymentCollectionResponse>, u64)>',
  'PaymentResult<RefundResponse>',
  'PaymentResult<(Vec<RefundResponse>, u64)>',
]) requireText(ownerService, marker, `${paths.ownerService}: preserved owner contract`);

for (const marker of [
  'pub enum PaymentError',
  'Validation(String)',
  'PaymentCollectionNotFound(Uuid)',
  'PaymentNotFound(Uuid)',
  'RefundNotFound(Uuid)',
  'InvalidTransition { from: String, to: String }',
  'ProviderUnavailable {',
  'ProviderRejected {',
  'ProviderInvalidResponse {',
  'ProviderOutcomeUnknown {',
  'ProviderConfiguration { provider_id: String }',
  'Database(#[from] DbErr)',
]) requireText(ownerError, marker, `${paths.ownerError}: exhaustive owner variants`);

for (const [key, expected] of Object.entries({
  query_resolver_source_changed: false,
  payment_owner_service_preserved: true,
  payment_owner_read_methods_preserved: true,
  payment_success_dtos_preserved: true,
  typed_payment_error_retained_to_transport: true,
  payment_collection_not_found_branch_preserved: true,
  refund_not_found_branch_preserved: true,
  owner_error_display_used_for_public_response: false,
  complete_payment_error_public: false,
  provider_or_validation_content_public: false,
  structural_payment_error_policy_preserved: true,
  unavailable_retryable: true,
  other_retryable: false,
  complete_payment_error_logged: false,
  provider_or_validation_content_logged: false,
  owner_detail_shape_length_logged: true,
  operation_and_correlation_logged: true,
  diagnostic_debug_redacted: true,
  technical_error_severity_preserved: true,
  ordinary_rejection_warning_severity_preserved: true,
  graphql_fields_or_dtos_changed: false,
  payment_owner_contract_changed: false,
  commerce_ffa_status_changed: false,
  commerce_fba_status_changed: false,
  payment_ffa_status_changed: false,
  payment_fba_status_changed: false,
  broad_ecommerce_cleanup_closed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}

for (const [key, expected] of Object.entries({
  validation_public_code: 'PAYMENT_REQUEST_INVALID',
  not_found_public_code: 'PAYMENT_RESOURCE_NOT_FOUND',
  conflict_public_code: 'PAYMENT_STATE_CONFLICT',
  unavailable_public_code: 'PAYMENT_TEMPORARILY_UNAVAILABLE',
  reconciliation_public_code: 'PAYMENT_RECONCILIATION_REQUIRED',
  configuration_public_code: 'PAYMENT_CONFIGURATION_ERROR',
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}

for (const key of [
  'tests_run',
  'verifiers_run',
  'cargo_run',
  'format_run',
  'mounted_graphql_run',
  'workflow_checks_run',
  'ci_run',
  'compile_proven',
  'runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push(`${paths.evidence}: execution must remain empty`);
}

for (const marker of [
  '# Commerce GraphQL payment query error safety',
  'Status: `source_closed_unvalidated`',
  'The compatibility resolver source in `crates/rustok-commerce/src/graphql/query.rs` remains unchanged.',
  '`find_reusable_collection_by_cart`',
  '`find_latest_collection_by_order`',
  '`get_collection`',
  '`list_collections`',
  '`get_refund`',
  '`list_refunds`',
  'It does not format the Payment owner error into a public string.',
  '`PAYMENT_TEMPORARILY_UNAVAILABLE`',
  '`PAYMENT_RECONCILIATION_REQUIRED`',
  'Commerce and Payment FFA/FBA status is unchanged.',
  'The broad ecommerce mapper and public-envelope cleanup remains open.',
  'No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, or CI were executed.',
]) requireText(document, marker, `${paths.document}: truthful source contract`);

if (failures.length > 0) {
  console.error('Commerce GraphQL payment error-safety verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Commerce GraphQL payment reads preserve canonical owner calls and route typed failures through bounded structural envelopes',
);
