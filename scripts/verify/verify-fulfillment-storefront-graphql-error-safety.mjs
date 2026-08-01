#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const rootPath = configuredRoot
  ? path.resolve(configuredRoot)
  : fileURLToPath(new URL('../../', import.meta.url));
const read = (relativePath) => readFileSync(path.join(rootPath, relativePath), 'utf8');

const cargo = read('crates/rustok-fulfillment/storefront/Cargo.toml');
const transport = read('crates/rustok-fulfillment/storefront/src/transport.rs');
const safety = read(
  'crates/rustok-fulfillment/storefront/src/transport/graphql_error_safety.rs',
);
const adapter = read(
  'crates/rustok-fulfillment/storefront/src/transport/graphql_adapter.rs',
);
const native = read(
  'crates/rustok-fulfillment/storefront/src/transport/native_server_adapter/server_functions.rs',
);
const evidence = JSON.parse(
  read(
    'crates/rustok-fulfillment/contracts/evidence/storefront-graphql-error-safety-source.json',
  ),
);
const review = JSON.parse(
  read(
    'crates/rustok-fulfillment/contracts/evidence/storefront-graphql-error-safety-source-review.json',
  ),
);
const document = read('crates/rustok-fulfillment/docs/storefront-graphql-error-safety.md');

const failures = [];
const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['rustok-graphql.workspace = true', 'typed GraphQL error dependency'],
  ['tracing.workspace = true', 'structured diagnostics dependency'],
  ['uuid.workspace = true', 'correlation id dependency'],
]) requireText(cargo, value, label);

for (const [value, label] of [
  ['mod graphql_adapter;', 'private GraphQL adapter module'],
  ['mod graphql_error_safety;', 'private GraphQL safety module'],
  ['mod native_server_adapter;', 'private native adapter module'],
  ['execute_selected_transport(', 'explicit selected transport'],
  ['move || native_server_adapter::select_shipping_option(native_request)', 'unchanged native closure'],
  ['let context = graphql_error_safety::GraphqlCallContext::new(&request);', 'pre-call GraphQL context'],
  ['graphql_adapter::select_shipping_option(request)', 'unchanged GraphQL adapter call'],
  ['.map_err(|error| context.map_error(error))', 'GraphQL safety mapping'],
  ['UiTransportPath::NativeServer', 'native feature path'],
  ['UiTransportPath::Graphql', 'GraphQL feature path'],
]) requireText(transport, value, label);
for (const value of [
  'fallback_failed(',
  'native_server_adapter::select_shipping_option(native_request).await',
  'PaymentTransportError',
]) forbidText(transport, value, 'transport selection drift');

for (const [value, label] of [
  ['use rustok_graphql::GraphqlHttpError;', 'typed GraphQL error policy'],
  ['GraphqlHttpError::from_str(raw_error.as_str())', 'typed display handoff parsing'],
  ['const FULFILLMENT_STOREFRONT_GRAPHQL_OWNER', 'GraphQL owner constant'],
  ['const FULFILLMENT_STOREFRONT_GRAPHQL_OPERATION', 'GraphQL operation constant'],
  ['const FULFILLMENT_STOREFRONT_GRAPHQL_BOUNDARY', 'GraphQL boundary constant'],
  ['Uuid::new_v4()', 'per-call correlation id'],
  ['let ShippingSelectionTransportError::Graphql(raw_error) = error else {', 'GraphQL-only mapping'],
  ['return error;', 'non-GraphQL pass-through'],
  ['let raw_error_present = !raw_error.trim().is_empty();', 'raw display presence fact'],
  ['let raw_error_length = raw_error.chars().count();', 'raw display length fact'],
  ['let parsed_error_valid = parsed_error.is_ok();', 'typed parse validity fact'],
  ['GraphqlHttpError::Network', 'network policy'],
  ['GraphqlHttpError::Http(_)', 'HTTP policy'],
  ['GraphqlHttpError::Unauthorized', 'authentication policy'],
  ['GraphqlHttpError::Graphql(_)', 'GraphQL rejection policy'],
  ['"network"', 'closed network category'],
  ['"http"', 'closed HTTP category'],
  ['"unauthorized"', 'closed unauthorized category'],
  ['"graphql"', 'closed GraphQL category'],
  ['"unknown"', 'closed unknown category'],
  ['"fulfillment.storefront_graphql_network_unavailable"', 'network stable code'],
  ['"fulfillment.storefront_graphql_http_unavailable"', 'HTTP stable code'],
  ['"fulfillment.storefront_graphql_authentication_required"', 'auth stable code'],
  ['"fulfillment.storefront_graphql_request_rejected"', 'request stable code'],
  ['"fulfillment.storefront_graphql_unknown_failure"', 'unknown stable code'],
  ['"Shipping selection is temporarily unavailable"', 'technical public envelope'],
  ['"Shipping selection authentication is required"', 'auth public envelope'],
  ['"Shipping selection request could not be completed"', 'request public envelope'],
  ['tracing::error!(', 'technical event severity'],
  ['tracing::warn!(', 'ordinary event severity'],
  ['raw_error_present,', 'bounded raw display presence logging'],
  ['raw_error_length,', 'bounded raw display length logging'],
  ['parsed_error_valid,', 'bounded typed parse logging'],
  ['owner = FULFILLMENT_STOREFRONT_GRAPHQL_OWNER', 'truthful owner'],
  ['owner_operation = FULFILLMENT_STOREFRONT_GRAPHQL_OPERATION', 'exact operation'],
  ['correlation_id = %self.correlation_id', 'correlation diagnostics'],
  ['tenant_slug_configured = self.tenant_slug_length.is_some()', 'tenant configuration fact'],
  ['tenant_slug_length = ?self.tenant_slug_length', 'tenant length fact'],
  ['cart_id_length = self.cart_id_length', 'cart id length fact'],
  ['delivery_group_count = self.delivery_group_count', 'delivery group count fact'],
  ['shipping_profile_slug_length = self.shipping_profile_slug_length', 'profile length fact'],
  ['seller_id_present = self.seller_id_present', 'seller presence fact'],
  ['shipping_option_id_present = self.shipping_option_id_present', 'option presence fact'],
  ['error_kind,', 'closed error category logging'],
  ['code,', 'stable code logging'],
  ['boundary = FULFILLMENT_STOREFRONT_GRAPHQL_BOUNDARY', 'GraphQL boundary diagnostics'],
  ['ShippingSelectionTransportError::Graphql(public_message.to_string())', 'static GraphQL envelope'],
]) requireText(safety, value, label);

for (const value of [
  'raw_error = %raw_error',
  'raw_error = ?raw_error',
  'parsed_error = ?parsed_error',
  'parsed_error = %parsed_error',
  'tenant_slug =',
  'cart_id =',
  'shipping_profile_slug =',
  'seller_id =',
  'shipping_option_id =',
  'available_shipping_option_ids =',
  'query =',
  'variables =',
  'endpoint =',
  'token =',
]) forbidText(safety, value, 'raw GraphQL diagnostic payload');

for (const [value, label] of [
  ['SELECT_STOREFRONT_SHIPPING_OPTION_MUTATION', 'unchanged GraphQL document'],
  ['GraphqlRequest::new(', 'unchanged GraphQL request construction'],
  ['build_shipping_selection_updates(&request)?', 'unchanged validation order'],
  ['configured_tenant_slug()', 'unchanged tenant header selection'],
  ['ShippingSelectionTransportError::Graphql(error.to_string())', 'private display handoff'],
]) requireText(adapter, value, label);
for (const value of [
  'graphql_error_safety',
  'tracing::error!(',
  'tracing::warn!(',
]) forbidText(adapter, value, 'private adapter policy leakage');

for (const [value, label] of [
  ['FULFILLMENT_STOREFRONT_NATIVE_BOUNDARY', 'native safety boundary'],
  ['ShippingSelectionTransportError::ServerFn(error.to_string())', 'native transport variant'],
  ['select_storefront_shipping_option(', 'native owner call'],
]) requireText(native, value, label);
forbidText(native, 'graphql_error_safety', 'native GraphQL policy coupling');

if (evidence.status !== 'fulfillment_storefront_graphql_error_safety_source_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  graphql_variant_static_public_envelopes: true,
  typed_graphql_error_policy: true,
  per_call_correlation_id: true,
  safe_request_shape_logging: true,
  raw_graphql_detail_logged: false,
  parsed_graphql_error_debug_logged: false,
  raw_graphql_detail_shape_logged: true,
  typed_parse_validity_logged: true,
  closed_graphql_error_category_logged: true,
  validation_variant_changed: false,
  native_variant_changed: false,
  transport_selection_changed: false,
  graphql_document_changed: false,
  request_response_dto_changed: false,
  native_server_functions_changed: false,
  raw_graphql_detail_public: false,
  broad_ecommerce_cleanup_closed: false,
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
  'browser_runtime_proven',
  'graphql_runtime_proven',
  'mounted_parity_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

if (
  review.status !==
  'fulfillment_storefront_graphql_error_safety_source_reviewed_unvalidated'
) failures.push(`review status mismatch: ${review.status}`);
for (const [key, expected] of Object.entries({
  typed_graphql_variant_policy: true,
  static_public_graphql_messages: true,
  raw_graphql_detail_logging_removed: true,
  parsed_graphql_error_debug_logging_removed: true,
  bounded_graphql_error_shape_retained: true,
  per_call_correlation_id: true,
  safe_shape_only_request_facts: true,
  native_path_changed: false,
  validation_path_changed: false,
  graphql_adapter_changed: false,
  graphql_document_changed: false,
  transport_selection_changed: false,
  request_response_dto_changed: false,
  runtime_evidence_claimed: false,
})) {
  if (review.implementation_review?.[key] !== expected) {
    failures.push(`review implementation_review.${key} must be ${expected}`);
  }
}

for (const marker of [
  'Status: **source-ready / unvalidated**',
  'Raw GraphQL display text is not written to the event.',
  'Debug output from the parsed typed error is not written to the event.',
  'raw-display presence and character length',
  'The broad ecommerce correlation-safe mapper task remains open',
]) requireText(document, marker, 'truthful fulfillment GraphQL documentation');

if (failures.length > 0) {
  console.error('Fulfillment storefront GraphQL error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ fulfillment storefront GraphQL failures retain bounded error/request shape and static public envelopes; runtime evidence remains open',
);
