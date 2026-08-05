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

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (source, value, expected, label) => {
  const count = source.split(value).length - 1;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
};
const between = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
};

const checkout = read('crates/rustok-commerce/src/controllers/store/checkout.rs');
const runtime = read('crates/rustok-commerce/src/storefront_staged_checkout_runtime.rs');
const evidencePath =
  'crates/rustok-commerce/contracts/evidence/storefront-checkout-http-diagnostic-safety-source-review.json';
const docPath = 'crates/rustok-commerce/docs/storefront-checkout-http-diagnostic-safety.md';
const evidence = JSON.parse(read(evidencePath));
const documentation = read(docPath);
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');

const checkoutContext = between(
  checkout,
  'struct StorefrontCheckoutErrorContext {',
  '#[derive(Clone, Copy)]\nstruct StorefrontPaymentCollectionErrorContext {',
  'checkout route context',
);
const paymentContext = between(
  checkout,
  'struct StorefrontPaymentCollectionErrorContext {',
  '#[derive(Clone, Copy)]\nstruct StorefrontCheckoutRuntimeErrorFacts {',
  'payment route context',
);
const paymentRoute = between(
  checkout,
  'pub async fn create_payment_collection(',
  '/// Complete storefront cart checkout',
  'payment route',
);
const checkoutRoute = between(
  checkout,
  'pub async fn complete_cart_checkout(',
  'fn required_idempotency_key(',
  'checkout route',
);
const checkoutPolicy = between(
  checkout,
  'fn storefront_checkout_error_policy(',
  'fn storefront_checkout_runtime_error_facts(',
  'checkout policy',
);
const checkoutFacts = between(
  checkout,
  'fn storefront_checkout_runtime_error_facts(',
  'fn storefront_checkout_http_error(',
  'checkout facts',
);
const checkoutMapper = between(
  checkout,
  'fn storefront_checkout_http_error(',
  'fn payment_collection_error_policy(',
  'checkout mapper',
);
const paymentPolicy = between(
  checkout,
  'fn payment_collection_error_policy(',
  'fn storefront_payment_collection_error_facts(',
  'payment policy',
);
const paymentFacts = between(
  checkout,
  'fn storefront_payment_collection_error_facts(',
  'fn payment_collection_http_error(',
  'payment facts',
);
const paymentMapper = checkout.slice(checkout.indexOf('fn payment_collection_http_error('));

for (const marker of [
  'const STOREFRONT_CHECKOUT_OWNER: &str = "rustok_commerce.storefront_staged_checkout_runtime";',
  'const STOREFRONT_CHECKOUT_BOUNDARY: &str = "commerce_storefront_checkout_http";',
  'const STOREFRONT_PAYMENT_COLLECTION_OWNER: &str = "rustok_payment.storefront_payment_collections";',
  '"commerce_storefront_payment_collection_http";',
  'type StorefrontCheckoutHttpPolicy = (StatusCode, &\'static str);',
  'type StorefrontPaymentCollectionHttpPolicy =',
]) requireText(checkout, marker, 'stable HTTP boundary');

for (const [block, label, markers] of [
  [
    checkoutContext,
    'checkout context',
    [
      'tenant_id_non_nil: bool,',
      'actor_id_non_nil: bool,',
      'cart_id_non_nil: bool,',
      'channel_id_present: bool,',
      'channel_id_non_nil: Option<bool>,',
      'channel_slug_present: bool,',
      'channel_slug_length: Option<usize>,',
      'locale_length: usize,',
      'tenant_id_non_nil: !tenant_id.is_nil(),',
      'actor_id_non_nil: !actor_id.is_nil(),',
      'cart_id_non_nil: !cart_id.is_nil(),',
      'channel_id_present: request_context.channel_id.is_some(),',
      'channel_id_non_nil: request_context.channel_id.map(|value| !value.is_nil()),',
      'channel_slug_present: request_context.channel_slug.is_some(),',
      'locale_length: request_context.locale.chars().count(),',
    ],
  ],
  [
    paymentContext,
    'payment context',
    [
      'customer_id_present: bool,',
      'customer_id_non_nil: Option<bool>,',
      'customer_id_present: customer_id.is_some(),',
      'customer_id_non_nil: customer_id.map(|value| !value.is_nil()),',
      'channel_slug_length: Option<usize>,',
      'locale_length: usize,',
    ],
  ],
]) {
  for (const marker of markers) requireText(block, marker, label);
  for (const raw of [
    'tenant_id: Uuid,',
    'actor_id: Uuid,',
    'cart_id: Uuid,',
    'customer_id: Option<Uuid>,',
    'channel_id: Option<Uuid>,',
    'channel_slug: Option<&',
    'locale: &',
  ]) forbidText(block, raw, `${label} raw field`);
}

for (const marker of [
  'StorefrontStagedCheckoutRuntimeError::Validation(_)',
  'StorefrontStagedCheckoutRuntimeError::CartAccess',
  'StorefrontStagedCheckoutRuntimeError::AuthenticationRequired',
  'StorefrontStagedCheckoutRuntimeError::TemporarilyUnavailable',
  'StorefrontStagedCheckoutRuntimeError::CheckoutFailed',
  'StorefrontStagedCheckoutRuntimeError::CompensationPending',
  'StorefrontStagedCheckoutRuntimeError::ReconciliationRequired',
  'StatusCode::BAD_REQUEST',
  'StatusCode::NOT_FOUND',
  'StatusCode::UNAUTHORIZED',
  'StatusCode::SERVICE_UNAVAILABLE',
  'StatusCode::INTERNAL_SERVER_ERROR',
  'StatusCode::CONFLICT',
  '"validation"',
  '"cart_access"',
  '"authentication_required"',
  '"temporarily_unavailable"',
  '"checkout_failed"',
  '"compensation_pending"',
  '"reconciliation_required"',
]) requireText(checkoutPolicy, marker, 'checkout HTTP policy');

for (const marker of [
  'StorefrontCheckoutRuntimeErrorFacts',
  'error_variant: "validation"',
  'text_field_count: 1',
  'text_total_length: message.chars().count()',
  'error_variant: "cart_access"',
  'error_variant: "authentication_required"',
  'error_variant: "temporarily_unavailable"',
  'error_variant: "checkout_failed"',
  'error_variant: "compensation_pending"',
  'error_variant: "reconciliation_required"',
]) requireText(checkoutFacts, marker, 'checkout error shape');
for (const raw of ['error = ?', 'message =', 'cause =', 'error.to_string()']) {
  forbidText(checkoutFacts, raw, 'checkout error payload');
}

for (const marker of [
  'let error_facts = storefront_checkout_runtime_error_facts(&error);',
  'owner = STOREFRONT_CHECKOUT_OWNER',
  'tenant_id_non_nil = context.tenant_id_non_nil',
  'actor_id_non_nil = context.actor_id_non_nil',
  'cart_id_non_nil = context.cart_id_non_nil',
  'channel_id_present = context.channel_id_present',
  'channel_id_non_nil = ?context.channel_id_non_nil',
  'channel_slug_present = context.channel_slug_present',
  'channel_slug_length = ?context.channel_slug_length',
  'locale_length = context.locale_length',
  'operation = context.operation',
  'error_variant = error_facts.error_variant',
  'error_text_field_count = error_facts.text_field_count',
  'error_text_total_length = error_facts.text_total_length',
  'public_code = code',
  'retryable = error.retryable()',
  'status = status.as_u16()',
  'boundary = STOREFRONT_CHECKOUT_BOUNDARY',
  'HttpError::new(status, code, message)',
]) requireText(checkoutMapper, marker, 'bounded checkout mapper');
for (const raw of [
  'error = ?error',
  'tenant_id = %context.',
  'actor_id = %context.',
  'cart_id = %context.',
  'channel_id = ?context.',
  'channel = ?context.',
  'locale = %context.',
]) forbidText(checkoutMapper, raw, 'checkout mapper raw diagnostic');

for (const marker of [
  'PaymentError::Validation(_)',
  'PaymentError::PaymentCollectionNotFound(_)',
  'PaymentError::PaymentNotFound(_)',
  'PaymentError::RefundNotFound(_)',
  'PaymentError::InvalidTransition { .. }',
  'PaymentError::ProviderUnavailable { .. }',
  'PaymentError::ProviderConfiguration { .. }',
  'PaymentError::ProviderRejected { .. }',
  'PaymentError::ProviderInvalidResponse { .. }',
  'PaymentError::ProviderOutcomeUnknown { .. }',
  'PaymentError::Database(_)',
  '"payment_request_invalid"',
  '"payment_resource_not_found"',
  '"payment_state_conflict"',
  '"payment_temporarily_unavailable"',
  '"payment_provider_rejected"',
  '"payment_reconciliation_required"',
  '"payment_storage_unavailable"',
]) requireText(paymentPolicy, marker, 'payment HTTP policy');

for (const marker of [
  'StorefrontPaymentCollectionErrorFacts',
  '"validation"',
  '"payment_collection_not_found"',
  '"payment_not_found"',
  '"refund_not_found"',
  '"invalid_transition"',
  '"provider_unavailable"',
  '"provider_rejected"',
  '"provider_invalid_response"',
  '"provider_outcome_unknown"',
  '"provider_configuration"',
  'PaymentError::Database(_) => ("database", 0, 0, 0, 0, true)',
  'value.chars().count()',
  'from.chars().count() + to.chars().count()',
  'provider_id.chars().count() + operation.chars().count()',
  'if id.is_nil() { 0 } else { 1 }',
]) requireText(paymentFacts, marker, 'payment error shape');
requireCount(paymentFacts, 'if id.is_nil() { 0 } else { 1 }', 3, 'payment UUID variants');
for (const raw of [
  'provider_id =',
  'provider_operation =',
  'from =',
  'to =',
  'resource_id =',
  'error = ?',
  'error.to_string()',
]) forbidText(paymentFacts, raw, 'payment error payload');

for (const marker of [
  'let error_facts = storefront_payment_collection_error_facts(&error);',
  'owner = STOREFRONT_PAYMENT_COLLECTION_OWNER',
  'tenant_id_non_nil = context.tenant_id_non_nil',
  'actor_id_non_nil = context.actor_id_non_nil',
  'cart_id_non_nil = context.cart_id_non_nil',
  'customer_id_present = context.customer_id_present',
  'customer_id_non_nil = ?context.customer_id_non_nil',
  'channel_id_present = context.channel_id_present',
  'channel_id_non_nil = ?context.channel_id_non_nil',
  'channel_slug_present = context.channel_slug_present',
  'channel_slug_length = ?context.channel_slug_length',
  'locale_length = context.locale_length',
  'operation = context.operation',
  'error_variant = error_facts.error_variant',
  'error_text_field_count = error_facts.text_field_count',
  'error_text_total_length = error_facts.text_total_length',
  'error_uuid_field_count = error_facts.uuid_field_count',
  'error_uuid_non_nil_count = error_facts.uuid_non_nil_count',
  'error_opaque_payload_present = error_facts.opaque_payload_present',
  'public_code = code',
  'status = status.as_u16()',
  'boundary = STOREFRONT_PAYMENT_COLLECTION_BOUNDARY',
  'HttpError::new(status, code, message)',
]) requireText(paymentMapper, marker, 'bounded payment mapper');
for (const raw of [
  'error = ?error',
  'tenant_id = %context.',
  'actor_id = %context.',
  'cart_id = %context.',
  'customer_id = ?context.',
  'channel_id = ?context.',
  'channel = ?context.',
  'locale = %context.',
]) forbidText(paymentMapper, raw, 'payment mapper raw diagnostic');

for (const marker of [
  'StorefrontCheckoutErrorContext::new(',
  '"complete_cart_checkout"',
  'runtime.payment_provider_registry(),',
  'runtime.product_catalog_read_port(),',
  'idempotency_key,',
  'checkout_input,',
  'Ok(Json(response))',
]) requireText(checkoutRoute, marker, 'checkout route flow');
for (const marker of [
  '.find_reusable_collection_by_cart(tenant.id, cart.id)',
  'StorefrontPaymentCollectionErrorContext::new(',
  '"find_reusable_collection_by_cart"',
  '.create_collection(',
  '"create_collection"',
  'Ok((StatusCode::CREATED, Json(collection)))',
]) requireText(paymentRoute, marker, 'payment route flow');

for (const marker of [
  'pub enum StorefrontStagedCheckoutRuntimeError {',
  'Validation(String),',
  'pub const fn public_code(&self)',
  'pub const fn public_message(&self)',
  'pub const fn retryable(&self)',
]) requireText(runtime, marker, 'runtime public contract');

if (
  evidence.status !==
  'commerce_storefront_checkout_http_diagnostic_safety_source_reviewed_unvalidated'
) failures.push(`${evidencePath}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  checkout_runtime_variant_count: 7,
  payment_error_variant_count: 11,
  complete_checkout_error_logged: false,
  checkout_validation_text_logged: false,
  complete_payment_error_logged: false,
  payment_validation_text_logged: false,
  payment_transition_text_logged: false,
  payment_provider_text_logged: false,
  payment_resource_uuid_values_logged: false,
  raw_route_identifiers_logged: false,
  raw_channel_or_locale_logged: false,
  closed_checkout_variant_logged: true,
  checkout_text_shape_logged: true,
  closed_payment_variant_logged: true,
  payment_text_shape_logged: true,
  payment_uuid_shape_logged: true,
  payment_opaque_payload_presence_logged: true,
  public_checkout_policy_preserved: true,
  public_payment_policy_preserved: true,
  route_flow_changed: false,
  idempotency_contract_changed: false,
  runtime_retryability_changed: false,
  broad_cleanup_remains_open: true,
  runtime_evidence_claimed: false,
})) {
  if (evidence.review_findings?.[key] !== expected) {
    failures.push(`${evidencePath}: review_findings.${key} must be ${expected}`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push(`${evidencePath}: execution must remain empty`);
}
for (const key of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
  'compile_proven',
  'runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${evidencePath}: validation.${key} must remain false`);
  }
}

requireText(documentation, 'Status: **source-ready / unvalidated**', `${docPath}: status`);
requireText(documentation, '`storefront_checkout_http_error`', `${docPath}: checkout mapper`);
requireText(documentation, '`payment_collection_http_error`', `${docPath}: payment mapper`);
requireText(
  documentation,
  'The master ecommerce correlation-safe mapper-cleanup item remains open.',
  `${docPath}: remaining work`,
);
requireText(
  plan,
  'Finish correlation-safe mapper cleanup for order, payment execution/compensation,',
  'implementation plan broad cleanup',
);

if (failures.length > 0) {
  console.error('Commerce storefront checkout HTTP diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Storefront checkout and payment-collection HTTP mappers preserve public envelopes with bounded error and route-context diagnostics',
);
