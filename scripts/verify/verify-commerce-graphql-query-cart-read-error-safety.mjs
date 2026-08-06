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
  cartShim: 'crates/rustok-commerce/src/graphql/safe_query/source/rustok_cart_shim.rs',
  ownerPorts: 'crates/rustok-cart/src/ports.rs',
  apiPorts: 'crates/rustok-api/src/ports.rs',
  evidence:
    'crates/rustok-commerce/contracts/evidence/graphql-query-cart-read-error-safety-source-review.json',
  document: 'crates/rustok-commerce/docs/graphql-query-cart-read-error-safety.md',
};

const query = read(paths.query);
const safeSource = read(paths.safeSource);
const cartShim = read(paths.cartShim);
const ownerPorts = read(paths.ownerPorts);
const apiPorts = read(paths.apiPorts);
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
  'use rustok_cart::{CartStorefrontReadRequest, in_process_cart_storefront_port};',
  'graphql_cart_port_context(tenant_id, cart_id)',
  'graphql_cart_port_context(tenant_id, id)',
  'graphql_cart_port_context(tenant.id, cart_id)',
  'CartStorefrontReadRequest { cart_id }',
  'CartStorefrontReadRequest { cart_id: id }',
  'Err(error) if error.code == "cart.cart_not_found" => return Ok(None)',
  'Err(error) => return Err(error.message.into())',
  '.map_err(|error| async_graphql::Error::new(error.message))?',
]) requireText(query, marker, `${paths.query}: unchanged cart read resolver contract`);

const ownerCallCount = countText(query, '.read_storefront_cart(');
if (ownerCallCount !== 3) {
  failures.push(`${paths.query}: expected three cart storefront owner reads, found ${ownerCallCount}`);
}
const notFoundBranchCount = countText(
  query,
  'Err(error) if error.code == "cart.cart_not_found" => return Ok(None)',
);
if (notFoundBranchCount !== 2) {
  failures.push(`${paths.query}: expected two cart not-found None branches, found ${notFoundBranchCount}`);
}
const intoMessageCount = countText(
  query,
  'Err(error) => return Err(error.message.into())',
);
if (intoMessageCount !== 2) {
  failures.push(`${paths.query}: expected two typed message into conversions, found ${intoMessageCount}`);
}

for (const marker of [
  '#[path = "source/rustok_cart_shim.rs"]',
  'mod rustok_cart_shim;',
  'use self::rustok_cart_shim as rustok_cart;',
  'include!("../query.rs");',
]) requireText(safeSource, marker, `${paths.safeSource}: mounted cart facade`);

for (const marker of [
  'use ::rustok_cart::CartStorefrontPort as OwnerCartStorefrontPort;',
  'pub(crate) use ::rustok_cart::CartStorefrontReadRequest;',
  'inner: Arc<dyn OwnerCartStorefrontPort>',
  'inner: ::rustok_cart::in_process_cart_storefront_port(db)',
  'pub(crate) async fn read_storefront_cart(',
  '.read_storefront_cart(context, request)',
  'CartQueryPortError::new(error, diagnostic_context)',
]) requireText(cartShim, marker, `${paths.cartShim}: canonical owner delegation`);

for (const marker of [
  'pub(crate) code: String',
  'code: error.code.clone()',
  'pub(crate) message: CartGraphqlMessage',
  'message: CartGraphqlMessage { error, context }',
  'impl From<CartGraphqlMessage> for BoundaryError',
  'message.into_query_boundary()',
]) requireText(cartShim, marker, `${paths.cartShim}: typed compatibility envelope`);

for (const [kind, message, code] of [
  ['PortErrorKind::Validation', 'Cart query is invalid', 'CART_REQUEST_INVALID'],
  ['PortErrorKind::NotFound', 'Cart was not found', 'CART_RESOURCE_NOT_FOUND'],
  ['PortErrorKind::Conflict', 'Cart state conflicts with this query', 'CART_STATE_CONFLICT'],
  ['PortErrorKind::Forbidden', 'Cart query is not permitted', 'CART_ACCESS_DENIED'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'Cart data is temporarily unavailable', 'CART_TEMPORARILY_UNAVAILABLE'],
  ['PortErrorKind::InvariantViolation', 'Cart query could not be completed safely', 'CART_OPERATION_FAILED'],
]) {
  for (const marker of [kind, `"${message}"`, `"${code}"`]) {
    requireText(cartShim, marker, `${paths.cartShim}: ${code} transport policy`);
  }
}

for (const marker of [
  'struct CartQueryDiagnosticError;',
  'formatter.write_str("redacted")',
  'CartQueryDiagnosticContext::new(&context, &request.cart_id)',
  'owner = "rustok_cart"',
  'owner_operation = "read_storefront_cart"',
  'correlation_id = %self.context.correlation_id',
  'tenant_id_shape = self.context.tenant_id_shape',
  'actor_id_shape = self.context.actor_id_shape',
  'cart_id_shape = self.context.cart_id_shape',
  'owner_code = %self.error.code',
  'owner_message_shape',
  'owner_message_length',
  'owner_retryable = self.error.retryable',
  'public_code = code',
  'boundary = GRAPHQL_QUERY_CART_BOUNDARY',
  'tracing::error!(',
  'tracing::warn!(',
  'BoundaryError::Public {',
]) requireText(cartShim, marker, `${paths.cartShim}: bounded cart diagnostics`);

for (const forbidden of [
  'error = ?self.error',
  'error = %self.error',
  'owner_error = ?self.error',
  'owner_error = %self.error',
  'owner_message = %self.error.message',
  'owner_message = ?self.error.message',
  'message = %self.error.message',
  'message = ?self.error.message',
  'async_graphql::Error::new(self.error.message)',
  'BoundaryError::from(self.error.message)',
  'format!("{}", self.error)',
  'format!("{:?}", self.error)',
]) forbidText(cartShim, forbidden, `${paths.cartShim}: raw cart owner payload`);

for (const marker of [
  'pub trait CartStorefrontPort: Send + Sync',
  'async fn read_storefront_cart(',
  'context: PortContext',
  'request: CartStorefrontReadRequest',
  'Result<CartResponse, PortError>',
  'pub fn in_process_cart_storefront_port(db: DatabaseConnection)',
  'Arc::new(crate::CartService::new(db))',
]) requireText(ownerPorts, marker, `${paths.ownerPorts}: preserved owner contract`);

for (const marker of [
  'pub struct PortError',
  'pub kind: PortErrorKind',
  'pub code: String',
  'pub message: String',
  'pub retryable: bool',
  'pub enum PortErrorKind',
  'Validation',
  'NotFound',
  'Conflict',
  'Forbidden',
  'Unavailable',
  'Timeout',
  'InvariantViolation',
]) requireText(apiPorts, marker, `${paths.apiPorts}: exhaustive PortError contract`);

for (const [key, expected] of Object.entries({
  query_resolver_source_changed: false,
  cart_owner_call_count: 3,
  cart_owner_port_preserved: true,
  cart_owner_arguments_preserved: true,
  cart_success_projection_preserved: true,
  cart_not_found_code_guard_preserved: true,
  cart_not_found_none_branch_count: 2,
  typed_cart_port_error_retained_to_transport: true,
  owner_message_used_for_public_response: false,
  complete_cart_error_public: false,
  owner_message_content_public: false,
  structural_cart_error_policy_preserved: true,
  unavailable_retryable: true,
  other_retryable: false,
  complete_cart_error_logged: false,
  owner_message_content_logged: false,
  owner_message_shape_length_logged: true,
  owner_code_logged: true,
  correlation_id_logged: true,
  request_context_shapes_logged: true,
  diagnostic_debug_redacted: true,
  technical_error_severity_preserved: true,
  ordinary_rejection_warning_severity_preserved: true,
  graphql_fields_or_dtos_changed: false,
  cart_owner_contract_changed: false,
  commerce_ffa_status_changed: false,
  commerce_fba_status_changed: false,
  cart_ffa_status_changed: false,
  cart_fba_status_changed: false,
  broad_ecommerce_cleanup_closed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}

for (const [key, expected] of Object.entries({
  validation_public_code: 'CART_REQUEST_INVALID',
  not_found_public_code: 'CART_RESOURCE_NOT_FOUND',
  conflict_public_code: 'CART_STATE_CONFLICT',
  forbidden_public_code: 'CART_ACCESS_DENIED',
  unavailable_public_code: 'CART_TEMPORARILY_UNAVAILABLE',
  invariant_public_code: 'CART_OPERATION_FAILED',
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
  '# Commerce GraphQL cart read error safety',
  'Status: `source_closed_unvalidated`',
  'The compatibility resolver source in `crates/rustok-commerce/src/graphql/query.rs` remains unchanged.',
  'Its three existing calls to `read_storefront_cart`',
  'Two existing resolver branches still compare the owner code `cart.cart_not_found` and return `None`.',
  '`message` is a typed `CartGraphqlMessage`, not the owner message string',
  '`CART_TEMPORARILY_UNAVAILABLE`',
  'The broad ecommerce mapper and public-envelope cleanup remains open.',
  'No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, or CI were executed.',
]) requireText(document, marker, `${paths.document}: truthful source contract`);

if (failures.length > 0) {
  console.error('Commerce GraphQL cart read error-safety verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Commerce GraphQL storefront cart reads preserve owner calls and not-found behavior while retaining typed PortError envelopes',
);
