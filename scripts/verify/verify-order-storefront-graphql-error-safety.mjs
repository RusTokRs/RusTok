#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const rootPath = configuredRoot
  ? path.resolve(configuredRoot)
  : fileURLToPath(new URL('../../', import.meta.url));
const read = (relativePath) => readFileSync(path.join(rootPath, relativePath), 'utf8');

const cargo = read('crates/rustok-order/storefront/Cargo.toml');
const transport = read('crates/rustok-order/storefront/src/transport.rs');
const adapter = read('crates/rustok-order/storefront/src/transport/graphql_adapter.rs');
const safety = read('crates/rustok-order/storefront/src/transport/graphql_error_safety.rs');
const native = read(
  'crates/rustok-order/storefront/src/transport/native_server_adapter/server_functions.rs',
);
const docs = read('crates/rustok-order/docs/storefront-graphql-error-safety.md');
const evidence = JSON.parse(
  read('crates/rustok-order/contracts/evidence/storefront-graphql-error-safety-source.json'),
);
const review = JSON.parse(
  read(
    'crates/rustok-order/contracts/evidence/storefront-graphql-error-safety-source-review.json',
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

for (const [value, label] of [
  ['rustok-graphql.workspace = true', 'typed GraphQL error dependency'],
  ['tracing.workspace = true', 'structured diagnostics dependency'],
  ['uuid.workspace = true', 'correlation id dependency'],
]) requireText(cargo, value, label);

for (const [value, label] of [
  ['mod graphql_error_safety;', 'private GraphQL safety module'],
  ['let context = graphql_error_safety::GraphqlCallContext::new(&request);', 'per-call context'],
  ['graphql_adapter::complete_checkout(request)', 'private adapter delegation'],
  ['.map_err(|error| context.map_error(error))', 'consumer-boundary mapper'],
  ['move || native_server_adapter::complete_checkout(native_request)', 'native path preservation'],
  ['selected_transport_path()', 'explicit selected transport'],
]) requireText(transport, value, label);

for (const [value, label] of [
  ['pub(super) struct GraphqlCallContext', 'private call context'],
  ['ORDER_STOREFRONT_GRAPHQL_OWNER', 'owner constant'],
  ['ORDER_STOREFRONT_GRAPHQL_OPERATION', 'owner operation constant'],
  ['ORDER_STOREFRONT_GRAPHQL_BOUNDARY', 'boundary constant'],
  ['GraphqlHttpError::from_str(raw_error.as_str())', 'typed display reparse'],
  ['let CheckoutCompletionTransportError::Graphql(raw_error) = error else', 'GraphQL-only mapping'],
  ['return error;', 'non-GraphQL pass-through'],
  ['Uuid::new_v4()', 'unique correlation id'],
  ['let raw_error_present = !raw_error.trim().is_empty();', 'raw display presence fact'],
  ['let raw_error_length = raw_error.chars().count();', 'raw display length fact'],
  ['let parsed_error_valid = parsed_error.is_ok();', 'typed parse validity fact'],
  ['correlation_id = %self.correlation_id', 'correlation diagnostics'],
  ['tenant_slug_configured = self.tenant_slug_length.is_some()', 'tenant configured fact'],
  ['tenant_slug_length = ?self.tenant_slug_length', 'tenant length fact'],
  ['cart_id_length = self.cart_id_length', 'cart id length fact'],
  ['idempotency_key_length = self.idempotency_key_length', 'idempotency length fact'],
  ['create_fulfillment = self.create_fulfillment', 'fulfillment policy fact'],
  ['raw_error_present,', 'bounded raw display presence logging'],
  ['raw_error_length,', 'bounded raw display length logging'],
  ['parsed_error_valid,', 'bounded typed parse logging'],
  ['error_kind,', 'closed error category logging'],
  ['code,', 'stable code logging'],
  ['boundary = ORDER_STOREFRONT_GRAPHQL_BOUNDARY', 'boundary diagnostics'],
]) requireText(safety, value, label);

for (const [value, label] of [
  ['GraphqlHttpError::Network', 'network policy'],
  ['GraphqlHttpError::Http(_)', 'HTTP policy'],
  ['GraphqlHttpError::Unauthorized', 'authentication policy'],
  ['GraphqlHttpError::Graphql(_)', 'GraphQL rejection policy'],
  ['"network"', 'closed network category'],
  ['"http"', 'closed HTTP category'],
  ['"unauthorized"', 'closed unauthorized category'],
  ['"graphql"', 'closed GraphQL category'],
  ['"unknown"', 'closed unknown category'],
  ['order.storefront_graphql_network_unavailable', 'network stable code'],
  ['order.storefront_graphql_http_unavailable', 'HTTP stable code'],
  ['order.storefront_graphql_authentication_required', 'authentication stable code'],
  ['order.storefront_graphql_request_rejected', 'GraphQL rejection stable code'],
  ['order.storefront_graphql_unknown_failure', 'unknown stable code'],
  ['Checkout completion is temporarily unavailable', 'technical public message'],
  ['Checkout authentication is required', 'authentication public message'],
  ['Checkout request could not be completed', 'rejection public message'],
  ['CheckoutCompletionTransportError::Graphql(public_message.to_string())', 'static public envelope'],
]) requireText(safety, value, label);

for (const value of [
  'raw_error = %raw_error',
  'raw_error = ?raw_error',
  'parsed_error = ?parsed_error',
  'parsed_error = %parsed_error',
  'tenant_slug = %',
  'cart_id = %',
  'idempotency_key = %',
  'metadata = ?',
  'source_module = %',
  'source_surface = %',
  'command = %',
  'owner_module = %',
]) forbidText(safety, value, 'raw diagnostic payload');

for (const [value, label] of [
  ['COMPLETE_STOREFRONT_CHECKOUT_MUTATION', 'checkout mutation'],
  ['checkout idempotency key must contain 1 to 191 bytes', 'idempotency validation'],
  ['cart_id must be a valid UUID', 'cart UUID validation'],
  ['CheckoutCompletionTransportError::Graphql(error.to_string())', 'private adapter handoff'],
  ['configured_tenant_slug()', 'tenant configuration handoff'],
]) requireText(adapter, value, label);

if (countText(adapter, 'CheckoutCompletionTransportError::Graphql(error.to_string())') !== 1) {
  failures.push('private adapter must retain exactly one typed GraphQL display handoff');
}
if (countText(transport, 'graphql_error_safety::GraphqlCallContext::new(&request)') !== 1) {
  failures.push('the sole public GraphQL operation must create exactly one call context');
}

for (const value of [
  'move || graphql_adapter::complete_checkout(request)',
  'UiTransportError::graphql("order", error)',
]) forbidText(transport, value, 'unsanitized public GraphQL delegation');

for (const [value, label] of [
  ['CheckoutCompletionTransportError::ServerFn(', 'native outer transport variant'],
  ['Checkout transport is temporarily unavailable', 'native outer public message'],
  ['native_checkout_runtime_error', 'native runtime mapper'],
  ['complete_storefront_checkout', 'native owner operation'],
]) requireText(native, value, label);

for (const [value, label] of [
  ['Status: source-unvalidated', 'documentation source status'],
  ['Raw GraphQL display text is not written to the event.', 'raw diagnostic removal'],
  ['Debug output from the parsed typed error is not written to the event.', 'typed debug removal'],
  ['raw-display presence and character length', 'bounded display shape'],
  ['The broad ecommerce correlation-safe mapper cleanup remains open.', 'broad-plan nonclaim'],
  ['No tests, verifiers, Cargo commands, formatting, workflows, or CI were run', 'execution nonclaim'],
]) requireText(docs, value, label);

if (evidence.status !== 'order_storefront_graphql_error_safety_source_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  public_consumer_boundary_sanitized: true,
  private_graphql_adapter_changed: false,
  graphql_document_changed: false,
  request_response_dto_changed: false,
  idempotency_validation_changed: false,
  native_server_functions_changed: false,
  transport_selection_changed: false,
  native_to_graphql_fallback_added: false,
  graphql_http_error_reparsed: true,
  raw_graphql_detail_logged: false,
  parsed_graphql_error_debug_logged: false,
  raw_graphql_detail_shape_logged: true,
  typed_parse_validity_logged: true,
  closed_graphql_error_category_logged: true,
  raw_cart_id_logged: false,
  raw_idempotency_key_logged: false,
  raw_tenant_slug_logged: false,
  raw_command_metadata_logged: false,
  raw_graphql_error_public: false,
  non_graphql_errors_pass_through: true,
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
  'graphql_runtime_proven',
  'browser_runtime_proven',
  'mounted_parity_proven',
  'production_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

if (
  review.status !==
  'order_storefront_graphql_error_safety_source_reviewed_unvalidated'
) failures.push(`review status mismatch: ${review.status}`);
for (const [key, expected] of Object.entries({
  typed_graphql_variant_policy: true,
  static_public_graphql_messages: true,
  raw_graphql_detail_logging_removed: true,
  parsed_graphql_error_debug_logging_removed: true,
  bounded_graphql_error_shape_retained: true,
  per_call_correlation_id: true,
  safe_shape_only_request_facts: true,
  private_graphql_adapter_changed: false,
  native_path_changed: false,
  validation_path_changed: false,
  graphql_document_changed: false,
  transport_selection_changed: false,
  request_response_dto_changed: false,
  runtime_evidence_claimed: false,
})) {
  if (review.implementation_review?.[key] !== expected) {
    failures.push(`review implementation_review.${key} must be ${expected}`);
  }
}

if (failures.length > 0) {
  console.error('Order storefront GraphQL error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ order storefront GraphQL failures retain bounded error/request shape and static public envelopes; runtime evidence remains open',
);
