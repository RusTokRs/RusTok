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
  graphqlRuntime: 'crates/rustok-commerce/src/graphql_runtime.rs',
  paymentRuntime: 'crates/rustok-commerce/src/graphql_runtime/payment_reads.rs',
  paymentLib: 'crates/rustok-payment/src/lib.rs',
  adminRead: 'crates/rustok-payment/src/admin_read.rs',
  orderRead: 'crates/rustok-payment/src/order_read.rs',
  cartRead: 'crates/rustok-payment/src/cart_read.rs',
  ownerService: 'crates/rustok-payment/src/services/payment.rs',
  plan: 'crates/rustok-commerce/docs/implementation-plan.md',
  evidence:
    'crates/rustok-commerce/contracts/evidence/graphql-query-payment-error-safety-source-review.json',
  document: 'crates/rustok-commerce/docs/graphql-query-payment-error-safety.md',
};

const sources = Object.fromEntries(
  Object.entries(paths).map(([key, relativePath]) => [key, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (source, value) => source.split(value).length - 1;

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
]) requireText(sources.query, marker, `${paths.query}: unchanged payment query contract`);

for (const [marker, expected] of [
  ['PaymentService::new(db.clone())', 7],
  ['.find_reusable_collection_by_cart(', 1],
  ['.find_latest_collection_by_order(', 1],
  ['.get_collection(', 1],
  ['.list_collections(', 1],
  ['.get_refund(', 1],
  ['.list_refunds(', 2],
]) {
  const actual = countText(sources.query, marker);
  if (actual !== expected) {
    failures.push(`${paths.query}: expected ${expected} occurrences of ${marker}, found ${actual}`);
  }
}

for (const marker of [
  '#[path = "source/rustok_payment_shim.rs"]',
  'mod rustok_payment_shim;',
  'use self::rustok_payment_shim as rustok_payment;',
  'include!("../query.rs");',
]) requireText(sources.safeSource, marker, `${paths.safeSource}: mounted Payment facade`);

for (const marker of [
  'PaymentAdminReadPort',
  'PaymentOrderReadPort',
  'PaymentCartReadPort',
  'payment_read_runtime_for_current_graphql_scope',
  'payment_read_call_context_for_current_graphql_scope',
  'PortContext::new(',
  '.with_deadline(std::time::Duration::from_secs(2))',
  'context.with_channel(channel)',
  'ReusablePaymentCollectionByCartRequest { cart_id }',
  'LatestPaymentCollectionByOrderRequest { order_id }',
  'ReadPaymentCollectionProjectionRequest { collection_id }',
  'ListPaymentCollectionProjectionsRequest {',
  'ReadRefundProjectionRequest { refund_id }',
  'ListRefundProjectionsRequest {',
  'pub(crate) enum PaymentError {',
  'PaymentCollectionNotFound(PaymentQueryError)',
  'RefundNotFound(PaymentQueryError)',
  'Other(PaymentQueryError)',
  'PaymentError::from_owner_port',
  'pub(crate) fn to_string(self) -> BoundaryError',
  'fn is_configuration_error(error: &OwnerPortError)',
  '"payment.admin_read_configuration"',
  '"payment.order_read_configuration"',
  '"payment.cart_read_configuration"',
  'PAYMENT_REQUEST_INVALID',
  'PAYMENT_RESOURCE_NOT_FOUND',
  'PAYMENT_STATE_CONFLICT',
  'PAYMENT_TEMPORARILY_UNAVAILABLE',
  'PAYMENT_RECONCILIATION_REQUIRED',
  'PAYMENT_CONFIGURATION_ERROR',
  'error = ?diagnostic_error',
  'owner_detail_length',
  'BoundaryError::Public {',
]) requireText(sources.paymentShim, marker, `${paths.paymentShim}: owner-port facade`);

for (const forbidden of [
  '::rustok_payment::PaymentService',
  'inner: ::rustok_payment::PaymentService',
  'error = ?self.error',
  'error = %self.error',
  'message = %self.error',
  'self.error.message',
  'self.error.to_string()',
  'owner_code =',
  'provider_id =',
  'provider_operation =',
  'validation_message =',
]) forbidText(sources.paymentShim, forbidden, `${paths.paymentShim}: concrete/raw Payment payload`);

for (const marker of [
  'pub struct CommercePaymentReadRuntime',
  'admin_reads: PaymentAdminReadRuntime',
  'order_reads: PaymentOrderReadRuntime',
  'cart_reads: PaymentCartReadRuntime',
  'pub fn admin_read_port(&self)',
  'pub fn order_read_port(&self)',
  'pub fn cart_read_port(&self)',
  'CURRENT_COMMERCE_PAYMENT_READ_RUNTIME',
  'CURRENT_COMMERCE_PAYMENT_READ_CALL_CONTEXT',
  'scope_current_payment_reads',
  'ctx.data_opt::<AuthContext>()',
  'ctx.data_opt::<RequestContext>()',
  'PortActor::service("rustok-commerce.graphql-payment-query")',
]) requireText(sources.paymentRuntime, marker, `${paths.paymentRuntime}: scoped Payment runtime`);

for (const marker of [
  'mod payment_reads;',
  'pub use payment_reads::CommercePaymentReadRuntime;',
  'payment_reads::scope_current_payment_reads(',
  'payment_read_runtime: CommercePaymentReadRuntime',
  'pub fn payment_read_runtime(&self) -> CommercePaymentReadRuntime',
  '.shared_get::<CommercePaymentReadRuntime>()',
  '.shared_get::<rustok_payment::PaymentAdminReadRuntime>()',
  '.shared_get::<rustok_payment::PaymentOrderReadRuntime>()',
  '.shared_get::<rustok_payment::PaymentCartReadRuntime>()',
  'PaymentAdminReadRuntime::in_process(inputs.db_clone())',
  'PaymentOrderReadRuntime::in_process(inputs.db_clone())',
  'PaymentCartReadRuntime::in_process(inputs.db_clone())',
  'pub(crate) fn payment_read_call_context_for_current_graphql_scope(',
  '-> (PortActor, Option<String>, Option<String>)',
]) requireText(sources.graphqlRuntime, marker, `${paths.graphqlRuntime}: host/runtime bridge`);

for (const marker of [
  'mod cart_read;',
  'PaymentCartReadPort',
  'PaymentCartReadRuntime',
  'ReusablePaymentCollectionByCartRequest',
  'in_process_payment_cart_read_port',
]) requireText(sources.paymentLib, marker, `${paths.paymentLib}: cart read exports`);

for (const marker of [
  'pub trait PaymentCartReadPort',
  'pub struct PaymentCartReadRuntime',
  'context.require_policy(PortCallPolicy::read())',
  '.find_reusable_collection_by_cart(tenant_id, request.cart_id)',
  '"payment.cart_read_configuration"',
  '"payment.cart_read_unavailable"',
  'error_variant',
  'cart_id_non_nil',
]) requireText(sources.cartRead, marker, `${paths.cartRead}: Payment cart owner read`);
for (const forbidden of ['error = ?error', 'error = %error']) {
  forbidText(sources.cartRead, forbidden, `${paths.cartRead}: raw owner error`);
}

for (const marker of [
  'pub trait PaymentAdminReadPort',
  'context.require_policy(PortCallPolicy::read())',
  '"payment.admin_read_configuration"',
  '"payment.admin_read_unavailable"',
]) requireText(sources.adminRead, marker, `${paths.adminRead}: Payment admin owner read`);
forbidText(sources.adminRead, 'error = ?error', `${paths.adminRead}: raw owner debug`);
forbidText(sources.adminRead, 'error = %error', `${paths.adminRead}: raw owner display`);

for (const marker of [
  'pub trait PaymentOrderReadPort',
  '.find_latest_collection_by_order(tenant_id, request.order_id)',
  '"payment.order_read_configuration"',
  '"payment.order_read_unavailable"',
]) requireText(sources.orderRead, marker, `${paths.orderRead}: Payment order owner read`);

for (const marker of [
  'pub struct PaymentService',
  'pub async fn find_reusable_collection_by_cart(',
  'pub async fn find_latest_collection_by_order(',
  'pub async fn get_collection(',
  'pub async fn list_collections(',
  'pub async fn get_refund(',
  'pub async fn list_refunds(',
]) requireText(sources.ownerService, marker, `${paths.ownerService}: preserved owner service methods`);

const expectedSourceContract = {
  query_resolver_source_changed: false,
  payment_owner_service_preserved: true,
  payment_owner_read_methods_preserved: true,
  payment_success_dtos_preserved: true,
  graphql_concrete_payment_service_removed: true,
  payment_admin_read_port_reused: true,
  payment_order_read_port_reused: true,
  payment_cart_read_port_added: true,
  graphql_payment_runtime_scoped: true,
  host_shared_owner_runtimes_preferred: true,
  embedded_in_process_owner_fallback_retained: true,
  trusted_actor_channel_locale_propagated: true,
  typed_port_error_retained_to_transport: true,
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
  admin_read_raw_owner_debug_removed: true,
  graphql_fields_or_dtos_changed: false,
  payment_owner_contract_changed: true,
  commerce_ffa_status_changed: false,
  commerce_fba_status_changed: false,
  payment_ffa_status_changed: false,
  payment_fba_status_changed: false,
  broad_ecommerce_cleanup_closed: false,
  runtime_evidence_claimed: false,
};
for (const [key, expected] of Object.entries(expectedSourceContract)) {
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
  'remote_adapter_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push(`${paths.evidence}: execution must remain empty`);
}

for (const marker of [
  '# Commerce GraphQL payment query owner-port and error safety',
  'Status: `source_closed_unvalidated`',
  '`PaymentCartReadPort` / `PaymentCartReadRuntime` are the only new Payment owner API',
  '`CommercePaymentReadRuntime`',
  '`PAYMENT_TEMPORARILY_UNAVAILABLE`',
  '`PAYMENT_RECONCILIATION_REQUIRED`',
  'The broad Commerce topology P0 remains open',
  'No compile, runtime, mounted GraphQL, remote-adapter, or parity evidence is claimed.',
]) requireText(sources.document, marker, `${paths.document}: truthful source contract`);

requireText(
  sources.plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,',
  `${paths.plan}: broad topology remains open`,
);
requireText(
  sources.plan,
  'Payment, and Fulfillment concrete services behind host-composed owner ports.',
  `${paths.plan}: broad topology continuation`,
);

if (failures.length > 0) {
  console.error('Commerce GraphQL payment owner-read verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Commerce GraphQL Payment reads are source-routed through typed owner ports with bounded envelopes',
);
