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
  customerShim:
    'crates/rustok-commerce/src/graphql/safe_query/source/rustok_customer_shim.rs',
  customerReadContext: 'crates/rustok-customer/src/read_context.rs',
  customerPorts: 'crates/rustok-customer/src/ports.rs',
  evidence:
    'crates/rustok-commerce/contracts/evidence/graphql-query-customer-error-safety-source-review.json',
  document: 'crates/rustok-commerce/docs/graphql-query-customer-error-safety.md',
};

const query = read(paths.query);
const safeSource = read(paths.safeSource);
const customerShim = read(paths.customerShim);
const customerReadContext = read(paths.customerReadContext);
const customerPorts = read(paths.customerPorts);
const evidence = JSON.parse(read(paths.evidence));
const document = read(paths.document);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

function blockBetween(source, start, end, label) {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
}

function functionBody(source, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(source);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return '';
  }
  const openBrace = source.indexOf('{', match.index);
  let depth = 0;
  for (let index = openBrace; index >= 0 && index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    if (source[index] === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated function ${functionName}`);
  return '';
}

const storefrontMe = blockBetween(
  query,
  'async fn storefront_me(',
  'async fn storefront_returns(',
  'storefrontMe customer read',
);
const ownershipHelper = blockBetween(
  query,
  'async fn load_storefront_customer_order(',
  'fn normalize_pricing_channel_slug(',
  'storefront order ownership customer read',
);
const optionalHelper = blockBetween(
  query,
  'async fn resolve_optional_storefront_customer_id(',
  'fn graphql_customer_port_context(',
  'optional storefront customer read',
);
const mapper = blockBetween(
  customerShim,
  'impl QueryGraphqlMessage for CustomerGraphqlMessage {',
  'pub(crate) struct CustomerQueryPortError {',
  'typed customer GraphQL mapper',
);
const compatibilityMapping = blockBetween(
  customerShim,
  'impl From<PortError> for CustomerQueryPortError {',
  '/// Compatibility facade for the unchanged Commerce query source.',
  'customer compatibility mapping',
);

for (const block of [storefrontMe, ownershipHelper, optionalHelper]) {
  for (const marker of [
    'in_process_customer_read_port(db.clone())',
    '.read_customer_projection_by_user(',
    'CustomerUserProjectionRequest {',
    'graphql_customer_port_context(tenant_id, auth.user_id)',
  ]) requireText(block, marker, `${paths.query}: preserved customer call`);
}

const readCallCount = query.split('.read_customer_projection_by_user(').length - 1;
if (readCallCount !== 3) {
  failures.push(`${paths.query}: expected 3 customer-by-user reads, found ${readCallCount}`);
}
const missingCode = 'customer.customer_by_user_not_found';
const missingCodeCount = query.split(missingCode).length - 1;
if (missingCodeCount !== 3) {
  failures.push(`${paths.query}: expected 3 compatibility missing-code checks, found ${missingCodeCount}`);
}

for (const marker of [
  '<FieldError as GraphQLError>::unauthenticated()',
  'match error.code.as_str()',
]) {
  requireText(storefrontMe, marker, `${paths.query}: storefrontMe missing identity`);
  requireText(ownershipHelper, marker, `${paths.query}: ownership missing identity`);
}
for (const marker of [
  `Err(error) if error.code == "${missingCode}" => Ok(None)`,
  'Err(error) => Err(async_graphql::Error::new(error.message))',
]) requireText(optionalHelper, marker, `${paths.query}: optional identity behavior`);

for (const marker of [
  '#[path = "source/rustok_customer_shim.rs"]',
  'mod rustok_customer_shim;',
  'use self::rustok_customer_shim as rustok_customer;',
  'include!("../query.rs");',
]) requireText(safeSource, marker, `${paths.safeSource}: mounted customer shim`);

for (const marker of [
  'use ::rustok_customer::{CustomerReadPort, CustomerResponse};',
  'pub(crate) use ::rustok_customer::CustomerUserProjectionRequest;',
  'inner: Arc<dyn CustomerReadPort>',
  'inner: ::rustok_customer::in_process_customer_read_port(db)',
  '.read_customer_projection_by_user(context, request)',
  '.map_err(Into::into)',
]) requireText(customerShim, marker, `${paths.customerShim}: owner delegation`);

for (const marker of [
  'let identity_missing = matches!(&error.kind, PortErrorKind::NotFound);',
  'code: CustomerQueryCode { identity_missing }',
  'message: CustomerGraphqlMessage { error }',
]) requireText(compatibilityMapping, marker, `${paths.customerShim}: typed identity classification`);
for (const forbidden of [
  'error.code',
  'error.message',
  'customer.customer_by_user_not_found',
]) forbidText(
  compatibilityMapping,
  forbidden,
  `${paths.customerShim}: owner string control flow`,
);

for (const marker of [
  'if self.identity_missing',
  'CUSTOMER_BY_USER_NOT_FOUND_CODE',
  'CUSTOMER_OTHER_CODE',
  'impl PartialEq<&str> for CustomerQueryCode',
]) requireText(customerShim, marker, `${paths.customerShim}: compatibility sentinel`);

for (const [kind, message, code] of [
  ['PortErrorKind::Validation', 'Customer query is invalid', 'CUSTOMER_REQUEST_INVALID'],
  ['PortErrorKind::NotFound', 'Customer data was not found', 'CUSTOMER_RESOURCE_NOT_FOUND'],
  ['PortErrorKind::Conflict', 'Customer state conflicts with this query', 'CUSTOMER_STATE_CONFLICT'],
  ['PortErrorKind::Forbidden', 'Customer query is not permitted', 'CUSTOMER_ACCESS_DENIED'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'Customer data is temporarily unavailable', 'CUSTOMER_TEMPORARILY_UNAVAILABLE'],
  ['PortErrorKind::InvariantViolation', 'Customer query could not be completed safely', 'CUSTOMER_OPERATION_FAILED'],
]) {
  for (const marker of [kind, `"${message}"`, `"${code}"`]) {
    requireText(mapper, marker, `${paths.customerShim}: ${code} policy`);
  }
}

for (const marker of [
  'owner_message_present = !self.error.message.is_empty()',
  'owner_message_length = self.error.message.chars().count()',
  'error = ?diagnostic_error',
  'owner_code = %self.error.code',
  'owner_retryable = self.error.retryable',
  'tracing::error!(',
  'tracing::warn!(',
  'BoundaryError::Public {',
]) requireText(mapper, marker, `${paths.customerShim}: bounded diagnostics`);
for (const forbidden of [
  'error = ?self.error',
  'error = %self.error',
  'owner_message = %self.error.message',
  'message = %self.error.message',
  'message: self.error.message',
  'code: self.error.code',
]) forbidText(mapper, forbidden, `${paths.customerShim}: raw owner payload`);

for (const marker of [
  'pub fn in_process_customer_read_port(db: DatabaseConnection) -> Arc<dyn CustomerReadPort>',
  'CustomerReadPort::read_customer_projection_by_user(&self.inner, context, request)',
]) requireText(customerReadContext, marker, `${paths.customerReadContext}: canonical provider`);
for (const marker of [
  'async fn read_customer_projection_by_user(',
  'CustomerError::CustomerByUserNotFound(_)',
  'PortError::not_found(',
  '"customer.customer_by_user_not_found"',
]) requireText(customerPorts, marker, `${paths.customerPorts}: preserved owner contract`);

for (const [key, expected] of Object.entries({
  query_resolver_source_changed: false,
  customer_read_call_count: 3,
  customer_owner_port_preserved: true,
  customer_owner_request_preserved: true,
  customer_port_context_preserved: true,
  auth_required_not_found_unauthenticated_preserved: true,
  optional_not_found_none_preserved: true,
  identity_absence_derived_from_port_error_kind: true,
  owner_code_used_for_control_flow: false,
  complete_customer_port_error_public: false,
  owner_message_content_public: false,
  typed_port_error_kind_policy_preserved: true,
  unavailable_retryable: true,
  other_retryable: false,
  complete_customer_port_error_logged: false,
  owner_message_content_logged: false,
  owner_code_kind_retryability_logged: true,
  diagnostic_debug_redacted: true,
  technical_error_severity_preserved: true,
  ordinary_rejection_warning_severity_preserved: true,
  graphql_fields_or_dtos_changed: false,
  customer_owner_contract_changed: false,
  customer_ffa_status_changed: false,
  customer_fba_status_changed: false,
  broad_ecommerce_cleanup_closed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract[key] !== expected) {
    failures.push(`${paths.evidence}: ${key} expected ${expected}`);
  }
}

for (const [key, expected] of Object.entries({
  validation_public_code: 'CUSTOMER_REQUEST_INVALID',
  not_found_public_code: 'CUSTOMER_RESOURCE_NOT_FOUND',
  conflict_public_code: 'CUSTOMER_STATE_CONFLICT',
  forbidden_public_code: 'CUSTOMER_ACCESS_DENIED',
  unavailable_public_code: 'CUSTOMER_TEMPORARILY_UNAVAILABLE',
  invariant_public_code: 'CUSTOMER_OPERATION_FAILED',
})) {
  if (evidence.source_contract[key] !== expected) {
    failures.push(`${paths.evidence}: ${key} expected ${expected}`);
  }
}

for (const marker of [
  'Status: `source_closed_unvalidated`',
  'The compatibility resolver source in `crates/rustok-commerce/src/graphql/query.rs` remains unchanged',
  'derives the compatibility sentinel only from `PortErrorKind::NotFound`',
  '`CUSTOMER_TEMPORARILY_UNAVAILABLE`',
  'Owner code strings are therefore no longer a control-flow boundary',
  'No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, or CI were executed',
]) requireText(document, marker, `${paths.document}: documentation contract`);

if (failures.length > 0) {
  console.error('Commerce GraphQL customer error safety verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Commerce GraphQL customer reads preserve identity absence while routing non-identity failures through typed bounded envelopes',
);
