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
  boundary: 'crates/rustok-commerce/src/graphql/safe_query/query_error_boundary.rs',
  regionPorts: 'crates/rustok-region/src/ports.rs',
  evidence:
    'crates/rustok-commerce/contracts/evidence/graphql-query-region-error-safety-source-review.json',
  document: 'crates/rustok-commerce/docs/graphql-query-region-error-safety.md',
};

const query = read(paths.query);
const safeSource = read(paths.safeSource);
const boundary = read(paths.boundary);
const regionPorts = read(paths.regionPorts);
const evidence = JSON.parse(read(paths.evidence));
const document = read(paths.document);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
};

const resolver = between(
  query,
  'async fn storefront_regions(',
  'async fn storefront_shipping_options(',
  'storefront region resolver',
);
const regionMapper = between(
  boundary,
  'impl QueryGraphqlMessage for RegionGraphqlMessage {',
  'impl QueryGraphqlMessage for String {',
  'typed region mapper',
);

for (const [source, value, label] of [
  [resolver, 'RegionService::new(db.clone())', 'region owner service'],
  [resolver, '.list_regions_for_tenant(', 'region owner list operation'],
  [resolver, 'RegionListRequest {', 'typed region request'],
  [resolver, '.with_deadline(std::time::Duration::from_secs(3))', 'region deadline'],
  [resolver, 'format!("{}: {}", error.code, error.message)', 'compatibility format source'],
  [resolver, '.map(|projection| projection.region.into())', 'region projection mapping'],
]) requireText(source, value, label);

const exactRegionFormat = 'format!("{}: {}", error.code, error.message)';
const exactRegionFormatCount = query.split(exactRegionFormat).length - 1;
if (exactRegionFormatCount !== 1) {
  failures.push(
    `${paths.query}: expected exactly one region format expression, found ${exactRegionFormatCount}`,
  );
}

for (const [value, label] of [
  ['macro_rules! format {', 'safe format interception'],
  ['("{}: {}", error.code, error.message) => {', 'exact region format arm'],
  ['super::query_error_boundary::RegionGraphqlMessage::new(error)', 'typed region forwarding'],
  ['::std::format!($($tokens)*)', 'generic format fallback'],
  ['include!("../query.rs");', 'unchanged query inclusion'],
]) requireText(safeSource, value, label);

for (const [value, label] of [
  ['use ::rustok_api::{PortError, PortErrorKind};', 'typed port imports'],
  ['const QUERY_REGION_ERROR_BOUNDARY: &str = "commerce_graphql_query_region";', 'region boundary'],
  ['pub(crate) struct RegionGraphqlMessage {', 'typed message wrapper'],
  ['error: PortError', 'complete typed error ownership'],
  ['pub(crate) fn new(error: PortError) -> Self', 'typed message constructor'],
]) requireText(boundary, value, label);

for (const [kind, message, code] of [
  ['PortErrorKind::Validation', 'Region query is invalid', 'REGION_REQUEST_INVALID'],
  ['PortErrorKind::NotFound', 'Region data was not found', 'REGION_RESOURCE_NOT_FOUND'],
  ['PortErrorKind::Conflict', 'Region state conflicts with this query', 'REGION_STATE_CONFLICT'],
  ['PortErrorKind::Forbidden', 'Region query is not permitted', 'REGION_ACCESS_DENIED'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'Region data is temporarily unavailable', 'REGION_TEMPORARILY_UNAVAILABLE'],
  ['PortErrorKind::InvariantViolation', 'Region query could not be completed safely', 'REGION_OPERATION_FAILED'],
]) {
  for (const marker of [kind, `"${message}"`, `"${code}"`]) {
    requireText(regionMapper, marker, `${paths.boundary}: ${code} policy`);
  }
}

for (const [value, label] of [
  ['let owner_message_presence = text_presence_shape(&self.error.message);', 'owner message presence'],
  ['let owner_message_len = self.error.message.chars().count();', 'owner message length'],
  ['error = ?diagnostic_error', 'redacted diagnostic token'],
  ['owner_code = %self.error.code', 'owner code diagnostic'],
  ['owner_retryable = self.error.retryable', 'owner retryability diagnostic'],
  ['tracing::error!(', 'technical severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['BoundaryError::public(message, code, retryable)', 'typed public result'],
]) requireText(regionMapper, value, label);

for (const forbidden of [
  'error = ?self.error',
  'error = %self.error',
  'owner_message = %self.error.message',
  'message = %self.error.message',
  'BoundaryError::public(self.error.message',
  'BoundaryError::public(&self.error.message',
]) forbidText(regionMapper, forbidden, `${paths.boundary}: raw region owner payload`);

for (const [source, value, label] of [
  [boundary, 'impl QueryGraphqlMessage for String {', 'generic dynamic fallback retained'],
  [boundary, 'impl QueryGraphqlMessage for &str {', 'generic borrowed fallback retained'],
  [regionPorts, 'correlation_id_length: context.correlation_id.chars().count()', 'owner correlation shape'],
  [regionPorts, 'correlation_id_length = context_facts.correlation_id_length', 'owner bounded logger'],
]) requireText(source, value, label);
forbidText(
  regionPorts,
  'correlation_id = %context.correlation_id',
  `${paths.regionPorts}: raw correlation payload`,
);

for (const [key, expected] of Object.entries({
  query_resolver_source_changed: false,
  storefront_region_owner_port_preserved: true,
  exact_region_format_intercept_count: 1,
  generic_format_behavior_preserved: true,
  complete_region_port_error_public: false,
  owner_message_content_public: false,
  owner_code_public: false,
  typed_port_error_kind_policy_preserved: true,
  unavailable_retryable: true,
  other_retryable: false,
  complete_region_port_error_logged: false,
  owner_message_content_logged: false,
  owner_code_kind_retryability_logged: true,
  diagnostic_debug_redacted: true,
  technical_error_severity_preserved: true,
  ordinary_rejection_warning_severity_preserved: true,
  raw_region_owner_correlation_logged: false,
  region_owner_correlation_length_logged: true,
  region_owner_contract_changed: false,
  graphql_fields_or_dtos_changed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract[key] !== expected) {
    failures.push(`${paths.evidence}: ${key} expected ${expected}`);
  }
}

for (const [key, expected] of Object.entries({
  validation_public_code: 'REGION_REQUEST_INVALID',
  not_found_public_code: 'REGION_RESOURCE_NOT_FOUND',
  conflict_public_code: 'REGION_STATE_CONFLICT',
  forbidden_public_code: 'REGION_ACCESS_DENIED',
  unavailable_public_code: 'REGION_TEMPORARILY_UNAVAILABLE',
  invariant_public_code: 'REGION_OPERATION_FAILED',
})) {
  if (evidence.source_contract[key] !== expected) {
    failures.push(`${paths.evidence}: ${key} expected ${expected}`);
  }
}

for (const marker of [
  'Status: `source_closed_unvalidated`',
  'passes the complete typed `PortError` to `RegionGraphqlMessage`',
  '`REGION_TEMPORARILY_UNAVAILABLE`',
  'raw correlation id',
  'No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, or CI were executed',
]) requireText(document, marker, `${paths.document}: documentation contract`);

if (failures.length > 0) {
  console.error('Commerce GraphQL region error safety verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Commerce GraphQL storefront-region errors retain typed kind policy, stable public envelopes, bounded owner diagnostics, and correlation-safe Region logging',
);
