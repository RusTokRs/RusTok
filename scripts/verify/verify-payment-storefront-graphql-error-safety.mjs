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
const transport = read('crates/rustok-payment/storefront/src/transport.rs');
const adapter = read('crates/rustok-payment/storefront/src/transport/graphql_adapter.rs');
const safety = read(
  'crates/rustok-payment/storefront/src/transport/graphql_error_safety.rs',
);
const evidence = JSON.parse(
  read(
    'crates/rustok-payment/contracts/evidence/payment-storefront-graphql-error-safety-source.json',
  ),
);
const review = JSON.parse(
  read(
    'crates/rustok-payment/contracts/evidence/payment-storefront-graphql-error-safety-source-review.json',
  ),
);
const document = read('crates/rustok-payment/docs/storefront-graphql-error-safety.md');

const failures = [];
const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (content, value) => content.split(value).length - 1;
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};

for (const [value, label] of [
  ['rustok-graphql.workspace = true', 'typed GraphQL dependency'],
  ['tracing.workspace = true', 'structured diagnostics dependency'],
  ['uuid.workspace = true', 'correlation dependency'],
]) requireText(cargo, value, label);

for (const [value, label] of [
  ['mod graphql_adapter;', 'private GraphQL adapter'],
  ['mod graphql_error_safety;', 'private GraphQL safety policy'],
  ['mod native_server_adapter;', 'private native adapter'],
  ['UiTransportPath::NativeServer', 'native selection'],
  ['UiTransportPath::Graphql', 'GraphQL selection'],
]) requireText(transport, value, label);

const operations = [
  {
    start: 'pub async fn create_payment_collection(',
    end: 'pub async fn fetch_payment_collection(',
    ownerOperation: 'create_storefront_payment_collection',
    graphqlCall: 'graphql_adapter::create_payment_collection(request)',
    nativeCall: 'native_server_adapter::create_payment_collection(native_request)',
    label: 'create payment collection',
  },
  {
    start: 'pub async fn fetch_payment_collection(',
    end: 'pub async fn fetch_refund_summary(',
    ownerOperation: 'read_storefront_payment_collection',
    graphqlCall: 'graphql_adapter::fetch_payment_collection(request)',
    nativeCall: 'native_server_adapter::fetch_payment_collection(native_request)',
    label: 'fetch payment collection',
  },
  {
    start: 'pub async fn fetch_refund_summary(',
    end: 'pub fn build_payment_collection_create_request(',
    ownerOperation: 'read_storefront_order_refunds',
    graphqlCall: 'graphql_adapter::fetch_refund_summary(request)',
    nativeCall: 'native_server_adapter::fetch_refund_summary(native_request)',
    label: 'fetch refund summary',
  },
];

for (const operation of operations) {
  const block = between(transport, operation.start, operation.end, operation.label);
  for (const [value, detail] of [
    ['let native_request = request.clone();', 'native request retention'],
    [operation.nativeCall, 'unchanged native call'],
    ['move || async move {', 'GraphQL consumer closure'],
    ['graphql_error_safety::GraphqlCallContext::new(', 'call context construction'],
    [`"${operation.ownerOperation}"`, 'exact owner operation'],
    [operation.graphqlCall, 'unchanged GraphQL adapter call'],
    ['.map_err(|error| context.map_error(error))', 'consumer safety mapping'],
  ]) requireText(block, value, `${operation.label} ${detail}`);

  const indexes = [
    block.indexOf(operation.nativeCall),
    block.indexOf('GraphqlCallContext::new('),
    block.indexOf(operation.graphqlCall),
    block.indexOf('|error| context.map_error(error)'),
  ];
  if (!indexes.every((value, index) => value >= 0 && (index === 0 || indexes[index - 1] < value))) {
    failures.push(`${operation.label}: expected native closure then context -> GraphQL call -> mapping order`);
  }
}

if (countText(transport, 'GraphqlCallContext::new(') !== 3) {
  failures.push('all three GraphQL public operations must construct a call context');
}
if (countText(transport, '|error| context.map_error(error)') !== 3) {
  failures.push('all three GraphQL public operations must route errors through the safety policy');
}
if (countText(adapter, 'PaymentTransportError::Graphql(error.to_string())') !== 3) {
  failures.push('private GraphQL adapter must preserve its three typed display handoffs');
}

for (const [value, label] of [
  ['use std::str::FromStr;', 'display round-trip parser'],
  ['use rustok_graphql::GraphqlHttpError;', 'typed GraphQL error'],
  ['const PAYMENT_STOREFRONT_GRAPHQL_OWNER', 'owner constant'],
  ['const PAYMENT_STOREFRONT_GRAPHQL_BOUNDARY', 'boundary constant'],
  ['pub(super) struct GraphqlCallContext', 'private call context'],
  ['payment-storefront-graphql:{owner_operation}:{}', 'unique correlation format'],
  ['Uuid::new_v4()', 'unique correlation generation'],
  ['let PaymentTransportError::Graphql(raw_error) = error else', 'non-GraphQL pass-through'],
  ['return error;', 'same non-GraphQL error return'],
  ['let raw_error_present = !raw_error.trim().is_empty();', 'raw display presence fact'],
  ['let raw_error_length = raw_error.chars().count();', 'raw display length fact'],
  ['GraphqlHttpError::from_str(raw_error.as_str())', 'typed display parsing'],
  ['let parsed_error_valid = parsed_error.is_ok();', 'typed parse validity fact'],
  ['Ok(GraphqlHttpError::Network)', 'network policy'],
  ['Ok(GraphqlHttpError::Http(_))', 'HTTP policy'],
  ['Ok(GraphqlHttpError::Unauthorized)', 'authentication policy'],
  ['Ok(GraphqlHttpError::Graphql(_))', 'GraphQL rejection policy'],
  ['Err(_)', 'unknown failure policy'],
  ['"network"', 'closed network category'],
  ['"http"', 'closed HTTP category'],
  ['"unauthorized"', 'closed unauthorized category'],
  ['"graphql"', 'closed GraphQL category'],
  ['"unknown"', 'closed unknown category'],
  ['tracing::error!(', 'technical diagnostics'],
  ['tracing::warn!(', 'rejection diagnostics'],
  ['raw_error_present,', 'bounded raw display presence logging'],
  ['raw_error_length,', 'bounded raw display length logging'],
  ['parsed_error_valid,', 'bounded typed parse logging'],
  ['owner = PAYMENT_STOREFRONT_GRAPHQL_OWNER', 'truthful owner'],
  ['owner_operation = self.owner_operation', 'exact owner operation'],
  ['correlation_id = %self.correlation_id', 'correlation diagnostics'],
  ['tenant_slug_configured = self.tenant_slug_length.is_some()', 'tenant configuration fact'],
  ['tenant_slug_length = ?self.tenant_slug_length', 'tenant length fact'],
  ['error_kind,', 'closed error category'],
  ['code,', 'stable internal code'],
  ['boundary = PAYMENT_STOREFRONT_GRAPHQL_BOUNDARY', 'boundary diagnostics'],
  ['PaymentTransportError::Graphql(public_message.to_string())', 'static public GraphQL envelope'],
]) requireText(safety, value, label);

for (const [code, message, label] of [
  ['payment.storefront_graphql_network_unavailable', 'Payment storefront is temporarily unavailable', 'network'],
  ['payment.storefront_graphql_http_unavailable', 'Payment storefront is temporarily unavailable', 'HTTP'],
  ['payment.storefront_graphql_authentication_required', 'Payment storefront authentication is required', 'authentication'],
  ['payment.storefront_graphql_request_rejected', 'Payment storefront request could not be completed', 'GraphQL rejection'],
  ['payment.storefront_graphql_unknown_failure', 'Payment storefront request could not be completed', 'unknown failure'],
]) {
  requireText(safety, `"${code}"`, `${label} code`);
  requireText(safety, `"${message}"`, `${label} public message`);
}

for (const value of [
  'raw_error = %raw_error',
  'raw_error = ?raw_error',
  'parsed_error = ?parsed_error',
  'parsed_error = %parsed_error',
  'tenant_slug =',
  'tenant_slug = %',
  'graphql_query',
  'variables =',
  'token =',
  'authorization =',
  'endpoint =',
  'cart_id =',
  'order_id =',
  'metadata =',
]) forbidText(safety, value, 'raw payment storefront GraphQL diagnostics');

if (evidence.status !== 'payment_storefront_graphql_error_safety_source_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  private_graphql_adapter_preserved: true,
  public_consumer_safety_policy: true,
  network_static_public_envelope: true,
  http_static_public_envelope: true,
  unauthorized_static_public_envelope: true,
  graphql_rejection_static_public_envelope: true,
  unknown_static_public_envelope: true,
  validation_error_pass_through: true,
  native_transport_changed: false,
  transport_selection_changed: false,
  request_response_dto_changed: false,
  graphql_query_changed: false,
  raw_graphql_detail_logged: false,
  parsed_graphql_error_debug_logged: false,
  raw_graphql_detail_shape_logged: true,
  typed_parse_validity_logged: true,
  closed_graphql_error_category_logged: true,
  raw_graphql_error_public: false,
  raw_tenant_slug_logged: false,
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
  'payment_storefront_graphql_error_safety_source_reviewed_unvalidated'
) failures.push(`review status mismatch: ${review.status}`);
for (const [key, expected] of Object.entries({
  typed_graphql_variant_policy: true,
  static_public_graphql_messages: true,
  raw_graphql_detail_logging_removed: true,
  parsed_graphql_error_debug_logging_removed: true,
  bounded_graphql_error_shape_retained: true,
  all_three_public_operations_preserved: true,
  per_call_correlation_id: true,
  tenant_shape_only: true,
  private_graphql_adapter_changed: false,
  native_path_changed: false,
  validation_path_changed: false,
  graphql_queries_changed: false,
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
  'The ecommerce correlation-safe mapper item remains open',
]) requireText(document, marker, 'truthful payment GraphQL documentation');

if (failures.length > 0) {
  console.error('Payment storefront GraphQL error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ payment storefront GraphQL failures retain bounded error and tenant shape while exposing only static transport messages; runtime evidence remains open',
);
