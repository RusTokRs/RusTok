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
  pricingShim:
    'crates/rustok-commerce/src/graphql/safe_query/source/rustok_pricing_shim.rs',
  ownerPorts: 'crates/rustok-pricing/src/ports.rs',
  evidence:
    'crates/rustok-commerce/contracts/evidence/graphql-query-pricing-error-safety-source-review.json',
  document: 'crates/rustok-commerce/docs/graphql-query-pricing-error-safety.md',
};

const query = read(paths.query);
const safeSource = read(paths.safeSource);
const pricingShim = read(paths.pricingShim);
const ownerPorts = read(paths.ownerPorts);
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

const adminPricing = blockBetween(
  query,
  'async fn admin_pricing_product(',
  'async fn storefront_pricing_channels(',
  'admin pricing resolver',
);
const activeLists = blockBetween(
  query,
  'async fn storefront_active_price_lists(',
  'async fn storefront_pricing_product(',
  'active price-list resolver',
);
const storefrontPricing = blockBetween(
  query,
  'async fn storefront_pricing_product(',
  'async fn storefront_regions(',
  'storefront pricing resolver',
);
const effectivePrices = functionBody(query, 'attach_effective_prices');
const compatibilityMapping = blockBetween(
  pricingShim,
  'impl From<PortError> for PricingQueryPortError {',
  '#[::async_trait::async_trait]',
  'pricing compatibility mapping',
);
const mapper = blockBetween(
  pricingShim,
  'impl QueryGraphqlMessage for PricingGraphqlMessage {',
  'pub(crate) struct PricingQueryPortError {',
  'typed pricing GraphQL mapper',
);

for (const [block, operation, request] of [
  [
    adminPricing,
    'read_admin_product_pricing_projection',
    'AdminProductPricingProjectionRequest',
  ],
  [activeLists, 'list_active_price_list_projections', 'ActivePriceListProjectionRequest'],
  [
    storefrontPricing,
    'read_storefront_product_pricing_projection',
    'StorefrontProductPricingProjectionRequest',
  ],
  [effectivePrices, 'resolve_product_price', 'ResolveProductPriceRequest'],
]) {
  requireText(block, `.${operation}(`, `${paths.query}: preserved ${operation} call`);
  requireText(block, `${request} {`, `${paths.query}: preserved ${operation} request`);
}

const pricingCallCount = [
  '.read_admin_product_pricing_projection(',
  '.list_active_price_list_projections(',
  '.read_storefront_product_pricing_projection(',
  '.resolve_product_price(',
].reduce((count, marker) => count + query.split(marker).length - 1, 0);
if (pricingCallCount !== 4) {
  failures.push(`${paths.query}: expected four mounted pricing owner calls, found ${pricingCallCount}`);
}

for (const [block, label] of [
  [adminPricing, 'admin pricing'],
  [activeLists, 'active price lists'],
  [storefrontPricing, 'storefront pricing'],
  [effectivePrices, 'effective price'],
]) {
  requireText(
    block,
    'async_graphql::Error::new(error.message)',
    `${paths.query}: unchanged ${label} compatibility expression`,
  );
}
for (const [block, label] of [
  [adminPricing, 'admin not-found'],
  [effectivePrices, 'effective-price not-found'],
]) {
  requireText(
    block,
    'error.kind == rustok_api::PortErrorKind::NotFound',
    `${paths.query}: preserved ${label} kind branch`,
  );
  requireText(block, '=> None', `${paths.query}: preserved ${label} None behavior`);
}

for (const marker of [
  '#[path = "source/rustok_pricing_shim.rs"]',
  'mod rustok_pricing_shim;',
  'use self::rustok_pricing_shim as rustok_pricing;',
  'include!("../query.rs");',
]) requireText(safeSource, marker, `${paths.safeSource}: mounted pricing facade`);

for (const marker of [
  'use ::rustok_pricing::PricingReadPort as OwnerPricingReadPort;',
  'pub(crate) trait PricingReadPort: Send + Sync',
  'inner: Arc<dyn OwnerPricingReadPort>',
  'inner: ::rustok_pricing::in_process_pricing_read_port(db, event_bus)',
  'Arc<dyn PricingReadPort>',
]) requireText(pricingShim, marker, `${paths.pricingShim}: canonical owner facade`);

for (const [operation, request] of [
  ['resolve_product_price', 'ResolveProductPriceRequest'],
  ['list_active_price_list_projections', 'ActivePriceListProjectionRequest'],
  ['read_admin_product_pricing_projection', 'AdminProductPricingProjectionRequest'],
  ['read_storefront_product_pricing_projection', 'StorefrontProductPricingProjectionRequest'],
]) {
  requireText(pricingShim, `async fn ${operation}(`, `${paths.pricingShim}: ${operation} facade`);
  requireText(pricingShim, `request: ${request}`, `${paths.pricingShim}: ${operation} request`);
  requireText(
    pricingShim,
    `.${operation}(context, request)`,
    `${paths.pricingShim}: ${operation} delegation`,
  );
}

for (const marker of [
  'pub(crate) enum PricingQueryKind',
  'Validation,',
  'NotFound,',
  'Conflict,',
  'Forbidden,',
  'Unavailable,',
  'Timeout,',
  'InvariantViolation,',
  'impl From<&PortErrorKind> for PricingQueryKind',
  'impl PartialEq<PortErrorKind> for PricingQueryKind',
]) requireText(pricingShim, marker, `${paths.pricingShim}: closed compatibility kind`);

for (const marker of [
  'let kind = PricingQueryKind::from(&error.kind);',
  'kind,',
  'message: PricingGraphqlMessage { error }',
]) requireText(compatibilityMapping, marker, `${paths.pricingShim}: typed compatibility mapping`);
for (const forbidden of ['error.code', 'error.message']) {
  forbidText(
    compatibilityMapping,
    forbidden,
    `${paths.pricingShim}: owner string compatibility control flow`,
  );
}

for (const [kind, message, code] of [
  ['PortErrorKind::Validation', 'Pricing query is invalid', 'PRICING_REQUEST_INVALID'],
  ['PortErrorKind::NotFound', 'Pricing data was not found', 'PRICING_RESOURCE_NOT_FOUND'],
  ['PortErrorKind::Conflict', 'Pricing state conflicts with this query', 'PRICING_STATE_CONFLICT'],
  ['PortErrorKind::Forbidden', 'Pricing query is not permitted', 'PRICING_ACCESS_DENIED'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout',
    'Pricing data is temporarily unavailable',
    'PRICING_TEMPORARILY_UNAVAILABLE',
  ],
  [
    'PortErrorKind::InvariantViolation',
    'Pricing query could not be completed safely',
    'PRICING_OPERATION_FAILED',
  ],
]) {
  for (const marker of [kind, `"${message}"`, `"${code}"`]) {
    requireText(mapper, marker, `${paths.pricingShim}: ${code} transport policy`);
  }
}

for (const marker of [
  'owner_message_present = !self.error.message.is_empty()',
  'owner_message_length = self.error.message.chars().count()',
  'error = ?diagnostic_error',
  'owner = "rustok_pricing"',
  'owner_code = %self.error.code',
  'owner_retryable = self.error.retryable',
  'public_code = code',
  'boundary = GRAPHQL_QUERY_PRICING_BOUNDARY',
  'tracing::error!(',
  'tracing::warn!(',
  'BoundaryError::Public {',
]) requireText(mapper, marker, `${paths.pricingShim}: bounded pricing diagnostics`);
for (const forbidden of [
  'error = ?self.error',
  'error = %self.error',
  'owner_message = %self.error.message',
  'message = %self.error.message',
  'message: self.error.message',
  'code: self.error.code',
]) forbidText(mapper, forbidden, `${paths.pricingShim}: raw owner payload`);

for (const marker of [
  'pub trait PricingReadPort: Send + Sync',
  'async fn resolve_product_price(',
  'async fn list_active_price_list_projections(',
  'async fn read_admin_product_pricing_projection(',
  'async fn read_storefront_product_pricing_projection(',
  'pub fn in_process_pricing_read_port(',
]) requireText(ownerPorts, marker, `${paths.ownerPorts}: preserved owner contract`);

for (const [key, expected] of Object.entries({
  query_resolver_source_changed: false,
  pricing_owner_call_count: 4,
  pricing_owner_port_preserved: true,
  pricing_owner_requests_preserved: true,
  pricing_port_contexts_preserved: true,
  pricing_success_projections_preserved: true,
  admin_not_found_none_preserved: true,
  effective_price_not_found_none_preserved: true,
  storefront_projection_option_preserved: true,
  not_found_derived_from_port_error_kind: true,
  owner_code_used_for_control_flow: false,
  complete_pricing_port_error_public: false,
  owner_message_content_public: false,
  typed_port_error_kind_policy_preserved: true,
  unavailable_retryable: true,
  other_retryable: false,
  complete_pricing_port_error_logged: false,
  owner_message_content_logged: false,
  owner_code_kind_retryability_logged: true,
  diagnostic_debug_redacted: true,
  technical_error_severity_preserved: true,
  ordinary_rejection_warning_severity_preserved: true,
  graphql_fields_or_dtos_changed: false,
  pricing_owner_contract_changed: false,
  pricing_ffa_status_changed: false,
  pricing_fba_status_changed: false,
  broad_ecommerce_cleanup_closed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}

for (const [key, expected] of Object.entries({
  validation_public_code: 'PRICING_REQUEST_INVALID',
  not_found_public_code: 'PRICING_RESOURCE_NOT_FOUND',
  conflict_public_code: 'PRICING_STATE_CONFLICT',
  forbidden_public_code: 'PRICING_ACCESS_DENIED',
  unavailable_public_code: 'PRICING_TEMPORARILY_UNAVAILABLE',
  invariant_public_code: 'PRICING_OPERATION_FAILED',
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
  '# Commerce GraphQL pricing error safety',
  'Status: `source_closed_unvalidated`',
  'The compatibility resolver source in `crates/rustok-commerce/src/graphql/query.rs` remains unchanged',
  'derived only from `PortErrorKind`',
  '`PRICING_TEMPORARILY_UNAVAILABLE`',
  'Owner code strings and owner message text are not used for control flow.',
  'Pricing and Commerce FFA/FBA status is unchanged.',
  'No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, or CI were executed.',
]) requireText(document, marker, `${paths.document}: truthful source contract`);

if (failures.length > 0) {
  console.error('Commerce GraphQL pricing error-safety verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Commerce GraphQL pricing reads preserve owner calls and not-found behavior while routing remaining failures through typed bounded envelopes',
);
