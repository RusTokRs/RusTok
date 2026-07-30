#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import './verify-commerce-storefront-aggregate-error-safety.mjs';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const rootPath = configuredRoot
  ? path.resolve(configuredRoot)
  : fileURLToPath(new URL('../../', import.meta.url));
const read = (relativePath) => readFileSync(path.join(rootPath, relativePath), 'utf8');

const cargo = read('crates/rustok-commerce/storefront/Cargo.toml');
const native = read(
  'crates/rustok-commerce/storefront/src/transport/native_server_adapter.rs',
);
const shared = read(
  'crates/rustok-commerce/storefront/src/transport/shared_adapter.rs',
);
const evidence = JSON.parse(
  read(
    'crates/rustok-commerce/contracts/evidence/storefront-transport-error-safety-source.json',
  ),
);
const recheckDocPath =
  'crates/rustok-commerce/docs/storefront-request-context-regression.md';
const recheckDoc = read(recheckDocPath);

const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (source, value) => source.split(value).length - 1;

requireText(cargo, 'tracing.workspace = true', 'storefront diagnostics dependency');

for (const [value, label] of [
  ['fn map_storefront_native_request_context_error<E: std::fmt::Debug>(', 'native request-context mapper'],
  ['fn map_storefront_native_tenant_context_error<E: std::fmt::Debug>(', 'native tenant-context mapper'],
  ['fn map_storefront_native_validation_error(', 'native validation mapper'],
  ['fn map_storefront_native_owner_error<E: std::fmt::Debug>(', 'native owner mapper'],
  ['owner_operation = "extract_request_context"', 'native request-context operation'],
  ['owner_operation = "extract_tenant_context"', 'native tenant-context operation'],
  ['code = "commerce.storefront_request_context_unavailable"', 'native request-context stable code'],
  ['code = "commerce.storefront_tenant_context_unavailable"', 'native tenant-context stable code'],
  ['"Storefront request context is temporarily unavailable"', 'native request-context public envelope'],
  ['"Storefront tenant context is temporarily unavailable"', 'native tenant-context public envelope'],
  ['.map_err(map_storefront_native_request_context_error)?', 'native request-context mapping'],
  ['map_storefront_native_tenant_context_error(&request_context, error)', 'native tenant-context mapping'],
  ['request_tenant_id = %request_context.tenant_id', 'native request tenant identity log'],
  ['user_id = ?request_context.user_id', 'native request user identity log'],
  ['tenant_id = %tenant_id', 'native resolved tenant log'],
  ['channel_id = ?request_context.channel_id', 'native channel id log'],
  ['channel_slug = ?request_context.channel_slug', 'native channel slug log'],
  ['locale = %request_context.locale', 'native locale log'],
  ['owner_operation,', 'native owner operation log'],
  ['consumer_operation = STOREFRONT_COMMERCE_CONSUMER_OPERATION', 'native consumer operation log'],
  ['code = "commerce.storefront_cart_id_invalid"', 'native validation stable code'],
  ['"rustok_cart.storefront",\n                "fetch_cart",', 'native cart owner callsite'],
  ['"rustok_payment.storefront",\n                    "fetch_payment_collection",', 'native payment owner callsite'],
  ['"commerce.storefront_cart_unavailable"', 'native cart stable code'],
  ['"commerce.storefront_payment_collection_unavailable"', 'native payment stable code'],
  ['ServerFnError::new("Invalid cart selection")', 'native validation public envelope'],
  ['"Storefront cart data is temporarily unavailable"', 'native cart public envelope'],
  ['"Storefront payment collection is temporarily unavailable"', 'native payment public envelope'],
]) requireText(native, value, label);

if (countText(native, 'user_id = ?request_context.user_id') !== 3) {
  failures.push('native tenant, validation, and owner diagnostics must all retain request user identity');
}

for (const value of [
  'request_context.correlation_id',
  '.map_err(ServerFnError::new)',
  '.map_err(|err| ServerFnError::new(err.to_string()))',
  'ServerFnError::new(error.to_string())',
  'ServerFnError::new(err.to_string())',
]) forbidText(native, value, 'native stale context or raw public transport mapping');

if (countText(native, 'ApiError::ServerFn(error.to_string())') !== 1) {
  failures.push(
    'native outer server-function wrapper must be the only remaining error.to_string conversion',
  );
}

for (const [value, label] of [
  ['fn is_cart_id_validation_error(', 'shared validation classifier'],
  ['tracing::warn!(', 'shared validation diagnostics'],
  ['tracing::error!(', 'shared owner diagnostics'],
  ['failed_path = failed_path.as_str()', 'shared failed-path diagnostics'],
  ['fallback_attempted = error.fallback_attempted', 'shared fallback diagnostics'],
  ['owner = "rustok_cart.storefront"', 'shared cart owner identity'],
  ['owner = "rustok_payment.storefront"', 'shared payment owner identity'],
  ['code = "commerce.storefront_cart_id_invalid"', 'shared validation stable code'],
  ['code = "commerce.storefront_cart_unavailable"', 'shared cart stable code'],
  ['code = "commerce.storefront_payment_collection_unavailable"', 'shared payment stable code'],
  ['ApiError::Validation("cart_id must be a valid UUID".to_string())', 'shared validation public envelope'],
  ['ApiError::ServerFn("Storefront cart data is temporarily unavailable".to_string())', 'shared preserved cart error variant'],
  ['"Storefront payment collection is temporarily unavailable".to_string()', 'shared payment public envelope'],
]) requireText(shared, value, label);

for (const value of [
  'error = ?error',
  'let message = error.to_string();',
  'ApiError::ServerFn(error.to_string())',
  'ApiError::Graphql(error.to_string())',
  'ApiError::ServerFn(message)',
  'ApiError::Graphql("Storefront cart data is temporarily unavailable".to_string())',
]) forbidText(shared, value, 'shared raw transport cause or variant drift');

if (evidence.schema_version !== 3) {
  failures.push(`evidence schema_version mismatch: ${evidence.schema_version}`);
}
if (evidence.status !== 'storefront_transport_error_safety_source_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  native_static_public_envelopes: true,
  native_context_logging: true,
  native_request_context_static_public_envelope: true,
  native_tenant_context_static_public_envelope: true,
  native_context_extraction_raw_text_public: false,
  native_correlation_logging: false,
  native_request_identity_logging: true,
  shared_static_public_envelopes: true,
  shared_raw_cause_logging: false,
  cart_error_variant_preserved: true,
  foreign_transport_error_text_public: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
if (evidence.recheck?.documentation !== recheckDocPath) {
  failures.push('evidence recheck documentation path drift');
}
if (evidence.recheck?.removed_request_context_field !== 'correlation_id') {
  failures.push('evidence removed RequestContext field drift');
}
if (evidence.recheck?.guard_forbids_removed_field !== true) {
  failures.push('evidence must retain the removed-field guard');
}
if (evidence.recheck?.master_plan_item_10_complete !== false) {
  failures.push('evidence must not claim ecommerce master-plan item 10 complete');
}
for (const [key, expected] of Object.entries({
  request_context_unavailable: 'Storefront request context is temporarily unavailable',
  tenant_context_unavailable: 'Storefront tenant context is temporarily unavailable',
})) {
  if (evidence.stable_public_messages?.[key] !== expected) {
    failures.push(`evidence stable_public_messages.${key} mismatch`);
  }
}
for (const marker of [
  'Status: `source_corrected_unvalidated`.',
  'immediate execution item 10',
  '`RequestContext.correlation_id`',
  'Customer admin',
  'Pricing admin',
  'No command above was run by the implementation agent.',
]) requireText(recheckDoc, marker, 'storefront RequestContext regression documentation');
for (const key of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
  'native_runtime_proven',
  'graphql_runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

if (failures.length > 0) {
  console.error('Commerce storefront transport error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ commerce storefront request/tenant context plus cart/payment transport failures retain actual request identity, static public envelopes, and no client-side raw causes; runtime evidence remains open',
);
