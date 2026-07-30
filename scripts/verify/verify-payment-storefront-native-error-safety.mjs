#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const rootPath = configuredRoot
  ? path.resolve(configuredRoot)
  : fileURLToPath(new URL('../../', import.meta.url));
const read = (relativePath) => readFileSync(path.join(rootPath, relativePath), 'utf8');

const cargo = read('crates/rustok-payment/storefront/Cargo.toml');
const source = read(
  'crates/rustok-payment/storefront/src/transport/native_server_adapter/server_functions.rs',
);
const evidence = JSON.parse(
  read(
    'crates/rustok-payment/contracts/evidence/payment-storefront-native-error-safety-source.json',
  ),
);

const failures = [];
const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (content, value) => content.split(value).length - 1;

requireText(cargo, 'tracing.workspace = true', 'payment storefront diagnostics dependency');

for (const [value, label] of [
  ['const PAYMENT_STOREFRONT_NATIVE_OWNER', 'native owner constant'],
  ['const PAYMENT_STOREFRONT_NATIVE_BOUNDARY', 'native boundary constant'],
  ['fn map_request_context_error<E: std::fmt::Debug>(', 'request context mapper'],
  ['fn map_tenant_context_error<E: std::fmt::Debug>(', 'tenant context mapper'],
  ['fn map_auth_context_error<E: std::fmt::Debug>(', 'auth context mapper'],
  ['fn map_owner_runtime_error<E: std::fmt::Debug>(', 'owner runtime mapper'],
  ['correlation_id = %request_context.correlation_id', 'correlation diagnostics'],
  ['tenant_id = %tenant_id', 'tenant diagnostics'],
  ['channel_id = ?request_context.channel_id', 'channel id diagnostics'],
  ['channel_slug = ?request_context.channel_slug', 'channel slug diagnostics'],
  ['locale = %request_context.locale', 'locale diagnostics'],
  ['boundary = PAYMENT_STOREFRONT_NATIVE_BOUNDARY', 'boundary diagnostics'],
  ['error = ?error', 'internal cause diagnostics'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['endpoint = "payment/refund-summary"', 'refund endpoint'],
  ['endpoint = "payment/payment-collection"', 'collection read endpoint'],
  ['endpoint = "payment/create-payment-collection"', 'collection create endpoint'],
  ['let owner_operation = "read_storefront_order_refunds";', 'refund owner operation'],
  ['let owner_operation = "read_storefront_payment_collection";', 'collection read owner operation'],
  ['let owner_operation = "create_storefront_payment_collection";', 'collection create owner operation'],
  ['storefront_checkout_runtime::read_storefront_order_refunds(', 'refund owner call'],
  ['storefront_checkout_runtime::read_storefront_payment_collection(', 'collection read owner call'],
  ['storefront_checkout_runtime::create_storefront_payment_collection(', 'collection create owner call'],
  ['"source_module": metadata.source_module', 'create metadata source module'],
  ['"source_surface": metadata.source_surface', 'create metadata source surface'],
  ['"command": metadata.command', 'create metadata command'],
  ['"owner_module": metadata.owner_module', 'create metadata owner module'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['payment.storefront_request_context_unavailable', 'request context stable code'],
  ['payment.storefront_tenant_context_unavailable', 'tenant context stable code'],
  ['payment.storefront_auth_context_unavailable', 'auth context stable code'],
  ['payment.storefront_runtime_unavailable', 'runtime stable code'],
  ['payment.storefront_refund_summary_unavailable', 'refund stable code'],
  ['payment.storefront_collection_unavailable', 'collection read stable code'],
  ['payment.storefront_collection_create_failed', 'collection create stable code'],
  ['Payment storefront request context is unavailable', 'request context public message'],
  ['Payment storefront tenant context is unavailable', 'tenant context public message'],
  ['Payment storefront authentication context is unavailable', 'auth context public message'],
  ['Payment storefront runtime is temporarily unavailable', 'runtime public message'],
  ['Storefront refund summary is temporarily unavailable', 'refund public message'],
  ['Storefront payment collection is temporarily unavailable', 'collection public message'],
]) requireText(source, value, label);

if (
  countText(
    source,
    '.map_err(|error| map_request_context_error(owner_operation, error))?',
  ) !== 3
) {
  failures.push('all three native operations must sanitize request-context extraction');
}
if (
  countText(
    source,
    'map_tenant_context_error(&request_context, owner_operation, error)',
  ) !== 3
) {
  failures.push('all three native operations must sanitize tenant-context extraction');
}
if (
  countText(
    source,
    'map_auth_context_error(&request_context, tenant_id, owner_operation, error)',
  ) !== 3
) {
  failures.push('all three native operations must sanitize authentication-context extraction');
}
if (
  countText(source, 'checkout_runtime(&request_context, tenant_id, owner_operation)?;') !== 3
) {
  failures.push('all three native operations must compose runtime with correlation context');
}
if (countText(source, 'PaymentTransportError::ServerFn(error.to_string())') !== 3) {
  failures.push('the three outer native transport wrappers must remain unchanged');
}

for (const value of [
  '.map_err(ServerFnError::new)',
  'ServerFnError::new(error.to_string())',
  'ServerFnError::new(err.to_string())',
  '.map_err(|error| ServerFnError::new(error.to_string()))',
  'payment storefront native transport requires TransactionalEventBus in host runtime context',
]) forbidText(source, value, 'raw payment storefront native public mapping');

if (evidence.status !== 'payment_storefront_native_error_safety_source_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  request_context_static_public_envelope: true,
  tenant_context_static_public_envelope: true,
  auth_context_static_public_envelope: true,
  runtime_composition_static_public_envelope: true,
  owner_runtime_static_public_envelopes: true,
  outer_transport_variant_changed: false,
  graphql_adapter_changed: false,
  request_response_dto_changed: false,
  raw_internal_error_public: false,
  collection_read_request_context_for_diagnostics: true,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
  'native_runtime_proven',
  'mounted_parity_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

if (failures.length > 0) {
  console.error('Payment storefront native error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ payment storefront native failures retain server diagnostics and static public envelopes; runtime evidence remains open',
);
