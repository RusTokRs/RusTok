#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const rootPath = configuredRoot
  ? path.resolve(configuredRoot)
  : fileURLToPath(new URL('../../', import.meta.url));
const read = (relativePath) => readFileSync(path.join(rootPath, relativePath), 'utf8');

const cargoPath = 'crates/rustok-payment/storefront/Cargo.toml';
const sourcePath =
  'crates/rustok-payment/storefront/src/transport/native_server_adapter/server_functions.rs';
const evidencePath =
  'crates/rustok-payment/contracts/evidence/payment-storefront-native-error-safety-source.json';
const docPath = 'crates/rustok-payment/docs/storefront-native-error-safety.md';

const cargo = read(cargoPath);
const source = read(sourcePath);
const evidence = JSON.parse(read(evidencePath));
const doc = read(docPath);

const failures = [];
const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (content, value) => content.split(value).length - 1;

function functionBody(content, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(content);
  if (!match) {
    failures.push(`${sourcePath}: missing function ${functionName}`);
    return '';
  }
  const openBrace = content.indexOf('{', match.index);
  if (openBrace === -1) {
    failures.push(`${sourcePath}: missing body for ${functionName}`);
    return '';
  }
  let depth = 0;
  for (let index = openBrace; index < content.length; index += 1) {
    if (content[index] === '{') depth += 1;
    if (content[index] === '}') {
      depth -= 1;
      if (depth === 0) return content.slice(openBrace, index + 1);
    }
  }
  failures.push(`${sourcePath}: unterminated body for ${functionName}`);
  return '';
}

requireText(cargo, 'tracing.workspace = true', 'payment storefront diagnostics dependency');

for (const [value, label] of [
  ['const PAYMENT_STOREFRONT_NATIVE_OWNER', 'native owner constant'],
  ['const PAYMENT_STOREFRONT_NATIVE_BOUNDARY', 'native boundary constant'],
  ['fn map_request_context_error<E>(', 'request context mapper'],
  ['fn map_tenant_context_error<E>(', 'tenant context mapper'],
  ['fn map_auth_context_error<E>(', 'auth context mapper'],
  ['fn map_owner_runtime_error<E>(', 'owner runtime mapper'],
  ['boundary = PAYMENT_STOREFRONT_NATIVE_BOUNDARY', 'boundary diagnostics'],
]) requireText(source, value, label);

for (const obsolete of [
  'fn map_request_context_error<E: std::fmt::Debug>(',
  'fn map_tenant_context_error<E: std::fmt::Debug>(',
  'fn map_auth_context_error<E: std::fmt::Debug>(',
  'fn map_owner_runtime_error<E: std::fmt::Debug>(',
]) forbidText(source, obsolete, 'obsolete payment storefront diagnostic contract');

for (const functionName of [
  'map_request_context_error',
  'map_tenant_context_error',
  'map_auth_context_error',
  'map_owner_runtime_error',
]) {
  const body = functionBody(source, functionName);
  requireText(
    body,
    'let error_type = std::any::type_name::<E>();',
    `${functionName} bounded error type`,
  );
  requireText(body, 'error_type', `${functionName} error type diagnostic`);
  for (const forbidden of [
    'error = ?error',
    'error = %error',
    'error = ?_error',
    'error = %_error',
  ]) forbidText(body, forbidden, `${functionName} complete error payload`);
}

if (countText(source, 'let error_type = std::any::type_name::<E>();') !== 4) {
  failures.push('expected exactly four type-only payment storefront error mapper sites');
}
if (countText(source, 'correlation_id = %request_context.correlation_id') !== 4) {
  failures.push('expected correlation diagnostics in tenant, auth, owner, and runtime blocks');
}
if (countText(source, 'tenant_id_non_nil = !tenant_id.is_nil()') !== 3) {
  failures.push('expected tenant non-nil facts in auth, owner, and runtime blocks');
}
for (const marker of [
  'channel_id_present = request_context.channel_id.is_some()',
  'channel_id_non_nil = ?request_context.channel_id.map(|value| !value.is_nil())',
  'channel_slug_present = request_context.channel_slug.is_some()',
  'channel_slug_length = ?request_context.channel_slug.as_ref().map(|value| value.chars().count())',
  'locale_present = !request_context.locale.trim().is_empty()',
  'locale_length = request_context.locale.chars().count()',
]) {
  if (countText(source, marker) !== 4) {
    failures.push(`expected four bounded request-context diagnostic sites for ${marker}`);
  }
}

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
  countText(source, '.map_err(|error| map_request_context_error(owner_operation, error))?') !== 3
) failures.push('all three native operations must sanitize request-context extraction');
if (
  countText(source, 'map_tenant_context_error(&request_context, owner_operation, error)') !== 3
) failures.push('all three native operations must sanitize tenant-context extraction');
if (
  countText(source, 'map_auth_context_error(&request_context, tenant_id, owner_operation, error)') !== 3
) failures.push('all three native operations must sanitize authentication-context extraction');
if (countText(source, 'checkout_runtime(&request_context, tenant_id, owner_operation)?;') !== 3) {
  failures.push('all three native operations must compose runtime with correlation context');
}
if (countText(source, 'PaymentTransportError::ServerFn(error.to_string())') !== 3) {
  failures.push('the three outer native transport wrappers must remain unchanged');
}

for (const value of [
  'error = ?error',
  'error = %error',
  'tenant_id = %tenant_id',
  'channel_id = ?request_context.channel_id',
  'channel_slug = ?request_context.channel_slug',
  'locale = %request_context.locale',
  '.map_err(ServerFnError::new)',
  'ServerFnError::new(error.to_string())',
  'ServerFnError::new(err.to_string())',
  '.map_err(|error| ServerFnError::new(error.to_string()))',
  'payment storefront native transport requires TransactionalEventBus in host runtime context',
]) forbidText(source, value, 'unsafe payment storefront native diagnostic or public mapping');

if (evidence.status !== 'payment_storefront_native_error_safety_source_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  request_context_static_public_envelope: true,
  tenant_context_static_public_envelope: true,
  auth_context_static_public_envelope: true,
  runtime_composition_static_public_envelope: true,
  owner_runtime_static_public_envelopes: true,
  framework_error_type_only: true,
  owner_runtime_error_type_only: true,
  complete_internal_error_logged: false,
  correlation_logging: true,
  tenant_identity_shape_only: true,
  channel_context_shape_only: true,
  locale_shape_only: true,
  raw_tenant_channel_locale_logged: false,
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
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push('evidence execution must remain empty');
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

requireText(doc, 'Status: **source-complete / unvalidated**', 'documentation status');
requireText(doc, 'complete framework and owner errors are not logged', 'documentation error policy');
requireText(doc, 'tenant, channel, slug, and locale values are not logged', 'documentation identity policy');

if (failures.length > 0) {
  console.error('Payment storefront native error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ payment storefront native diagnostics use correlation-safe type/shape only; execution evidence remains open',
);
