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
  mounted: 'crates/rustok-commerce/src/storefront_checkout_runtime_mounted.rs',
  legacy: 'crates/rustok-commerce/src/storefront_checkout_runtime.rs',
  paymentNative:
    'crates/rustok-payment/storefront/src/transport/native_server_adapter/server_functions.rs',
  fulfillmentNative:
    'crates/rustok-fulfillment/storefront/src/transport/native_server_adapter/server_functions.rs',
  evidence:
    'crates/rustok-commerce/contracts/evidence/storefront-checkout-runtime-error-safety-source-review.json',
  document:
    'crates/rustok-commerce/docs/storefront-checkout-runtime-error-safety.md',
};

const mounted = read(paths.mounted);
const legacy = read(paths.legacy);
const paymentNative = read(paths.paymentNative);
const fulfillmentNative = read(paths.fulfillmentNative);
const evidence = JSON.parse(read(paths.evidence));
const document = read(paths.document);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (source, value) => source.split(value).length - 1;

for (const marker of [
  'mod legacy {',
  'include!("storefront_checkout_runtime.rs");',
  'pub struct StorefrontCheckoutRuntimeError {',
  "message: &'static str",
  "code: &'static str",
  'retryable: bool',
  "pub const fn public_message(&self) -> &'static str",
  "pub const fn public_code(&self) -> &'static str",
  'pub const fn retryable(&self) -> bool',
  'struct MountedRuntimeDiagnosticError;',
  'formatter.write_str("redacted")',
  'struct MountedRuntimeErrorContext {',
  'fn map_legacy_runtime_error(',
  '_error: legacy::StorefrontCheckoutRuntimeError',
  'legacy_error_type =',
  'std::any::type_name::<legacy::StorefrontCheckoutRuntimeError>()',
  'owner = "rustok_commerce.storefront_checkout_runtime"',
  'tenant_id_non_nil = context.tenant_id_non_nil',
  'auth_present = context.auth_present',
  'resource_id_non_nil = context.resource_id_non_nil',
  'request_context_present = context.request_context_present',
  'channel_id_present = context.channel_id_present',
  'channel_slug_length = ?context.channel_slug_length',
  'locale_length = ?context.locale_length',
  'public_code = policy.code',
  'public_retryable = policy.retryable',
  'boundary = STOREFRONT_RUNTIME_BOUNDARY',
]) {
  requireText(mounted, marker, `${paths.mounted}: mounted static boundary`);
}

const reexportEnd = mounted.indexOf('const STOREFRONT_RUNTIME_BOUNDARY');
if (reexportEnd < 0) {
  failures.push(`${paths.mounted}: mounted re-export section boundary is missing`);
} else {
  const reexportSection = mounted.slice(0, reexportEnd);
  for (const forbidden of [
    'StorefrontCheckoutRuntimeError',
    'create_storefront_payment_collection',
    'read_storefront_order_refunds',
    'read_storefront_payment_collection',
    'select_storefront_shipping_option',
  ]) {
    forbidText(
      reexportSection,
      forbidden,
      `${paths.mounted}: private legacy error/function re-export`,
    );
  }
}

for (const [operation, policy, code, message] of [
  [
    'read_storefront_payment_collection',
    'PAYMENT_COLLECTION_READ_POLICY',
    'STOREFRONT_PAYMENT_COLLECTION_UNAVAILABLE',
    'Storefront payment collection is temporarily unavailable',
  ],
  [
    'read_storefront_order_refunds',
    'REFUND_SUMMARY_READ_POLICY',
    'STOREFRONT_REFUND_SUMMARY_UNAVAILABLE',
    'Storefront refund summary is temporarily unavailable',
  ],
  [
    'create_storefront_payment_collection',
    'PAYMENT_COLLECTION_CREATE_POLICY',
    'STOREFRONT_PAYMENT_COLLECTION_CREATE_FAILED',
    'Storefront payment collection is temporarily unavailable',
  ],
  [
    'select_storefront_shipping_option',
    'SHIPPING_SELECTION_POLICY',
    'STOREFRONT_SHIPPING_SELECTION_FAILED',
    'Shipping selection is temporarily unavailable',
  ],
]) {
  for (const marker of [
    `pub async fn ${operation}(`,
    `legacy::${operation}(`,
    policy,
    `"${code}"`,
    `"${message}"`,
  ]) {
    requireText(mounted, marker, `${paths.mounted}: ${operation} mounted wrapper`);
  }
}

if (countText(mounted, 'legacy::read_storefront_payment_collection(') !== 1) {
  failures.push(`${paths.mounted}: payment collection read must delegate exactly once`);
}
if (countText(mounted, 'legacy::read_storefront_order_refunds(') !== 1) {
  failures.push(`${paths.mounted}: refund summary read must delegate exactly once`);
}
if (countText(mounted, 'legacy::create_storefront_payment_collection(') !== 1) {
  failures.push(`${paths.mounted}: payment collection create must delegate exactly once`);
}
if (countText(mounted, 'legacy::select_storefront_shipping_option(') !== 1) {
  failures.push(`${paths.mounted}: shipping selection must delegate exactly once`);
}

for (const marker of [
  'pub async fn complete_storefront_checkout(',
  'crate::services::storefront_staged_checkout_runtime::complete_storefront_checkout(',
]) {
  requireText(mounted, marker, `${paths.mounted}: staged completion preservation`);
}

for (const forbidden of [
  'message: String',
  'format!("{error:?}")',
  'format!("{_error:?}")',
  'error.to_string()',
  '_error.to_string()',
  'error = ?_error',
  'error = %_error',
  'legacy_error = ?_error',
  'legacy_error = %_error',
  'message = %_error',
  'message = ?_error',
]) {
  forbidText(mounted, forbidden, `${paths.mounted}: private legacy payload`);
}

for (const marker of [
  'pub struct StorefrontCheckoutRuntimeError {',
  'message: String',
  'fn runtime_error(error: impl std::fmt::Debug) -> StorefrontCheckoutRuntimeError',
  'StorefrontCheckoutRuntimeError::new(format!("{error:?}"))',
]) {
  requireText(legacy, marker, `${paths.legacy}: unchanged private compatibility source`);
}

for (const marker of [
  'map_owner_runtime_error(',
  '"Storefront refund summary is temporarily unavailable"',
  '"Storefront payment collection is temporarily unavailable"',
]) {
  requireText(paymentNative, marker, `${paths.paymentNative}: static native envelope`);
}
for (const marker of [
  'map_owner_runtime_error(',
  'ServerFnError::new("Shipping selection is temporarily unavailable")',
]) {
  requireText(
    fulfillmentNative,
    marker,
    `${paths.fulfillmentNative}: static native envelope`,
  );
}

for (const [key, expected] of Object.entries({
  legacy_runtime_source_changed: false,
  legacy_runtime_error_publicly_reexported: false,
  mounted_runtime_error_static: true,
  mounted_runtime_error_public_message_available: true,
  mounted_runtime_error_public_code_available: true,
  mounted_runtime_error_retryable_available: true,
  payment_collection_read_wrapped: true,
  refund_summary_read_wrapped: true,
  payment_collection_create_wrapped: true,
  shipping_selection_update_wrapped: true,
  legacy_operation_delegation_preserved: true,
  legacy_arguments_preserved: true,
  success_dtos_preserved: true,
  checkout_completion_entrypoint_changed: false,
  legacy_error_formatted_by_mounted_boundary: false,
  legacy_error_content_public: false,
  legacy_error_content_logged: false,
  diagnostic_debug_redacted: true,
  tenant_value_logged: false,
  actor_value_logged: false,
  resource_value_logged: false,
  channel_value_logged: false,
  locale_value_logged: false,
  request_context_shapes_logged: true,
  public_policy_logged: true,
  payment_native_transport_message_changed: false,
  fulfillment_native_transport_message_changed: false,
  rest_routes_changed: false,
  graphql_fields_changed: false,
  owner_contracts_changed: false,
  commerce_ffa_status_changed: false,
  commerce_fba_status_changed: false,
  broad_ecommerce_cleanup_closed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}

for (const [key, expected] of Object.entries({
  payment_collection_read_public_code: 'STOREFRONT_PAYMENT_COLLECTION_UNAVAILABLE',
  refund_summary_read_public_code: 'STOREFRONT_REFUND_SUMMARY_UNAVAILABLE',
  payment_collection_create_public_code: 'STOREFRONT_PAYMENT_COLLECTION_CREATE_FAILED',
  shipping_selection_update_public_code: 'STOREFRONT_SHIPPING_SELECTION_FAILED',
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
  'native_server_functions_run',
  'http_requests_run',
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
  '# Commerce mounted storefront runtime error safety',
  'Status: `source_closed_unvalidated`',
  'The mounted module now keeps the compatibility error private',
  '`STOREFRONT_PAYMENT_COLLECTION_UNAVAILABLE`',
  '`STOREFRONT_REFUND_SUMMARY_UNAVAILABLE`',
  '`STOREFRONT_PAYMENT_COLLECTION_CREATE_FAILED`',
  '`STOREFRONT_SHIPPING_SELECTION_FAILED`',
  'It does not format, retain, log, or publish the private legacy error.',
  'Checkout completion remains on the existing staged owner-port pipeline',
  'The broad ecommerce mapper and compatibility-source cleanup remain open.',
  'No tests, Node verifiers, Cargo commands, formatting, native server functions,',
]) {
  requireText(document, marker, `${paths.document}: truthful source contract`);
}

if (failures.length > 0) {
  console.error('Commerce storefront checkout runtime error-safety verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Mounted storefront checkout helpers delegate unchanged success paths while private compatibility errors remain redacted behind static public envelopes',
);
