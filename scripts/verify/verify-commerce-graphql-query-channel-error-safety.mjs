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
  channelShim:
    'crates/rustok-commerce/src/graphql/safe_query/source/rustok_channel_shim.rs',
  ownerService: 'crates/rustok-channel/src/services/channel_service.rs',
  ownerError: 'crates/rustok-channel/src/error.rs',
  evidence:
    'crates/rustok-commerce/contracts/evidence/graphql-query-channel-error-safety-source-review.json',
  document: 'crates/rustok-commerce/docs/graphql-query-channel-error-safety.md',
};

const query = read(paths.query);
const safeSource = read(paths.safeSource);
const channelShim = read(paths.channelShim);
const ownerService = read(paths.ownerService);
const ownerError = read(paths.ownerError);
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

const channelResolver = blockBetween(
  query,
  'async fn storefront_pricing_channels(',
  'async fn storefront_active_price_lists(',
  'storefront pricing-channel resolver',
);
const mapper = blockBetween(
  channelShim,
  'impl QueryGraphqlMessage for ChannelGraphqlMessage {',
  'pub(crate) struct ChannelQueryError {',
  'typed Channel GraphQL mapper',
);
const typedConversion = blockBetween(
  channelShim,
  'impl ChannelQueryError {',
  'pub(crate) struct ChannelService {',
  'typed Channel compatibility conversion',
);

for (const marker of [
  'rustok_channel::ChannelService::new(db.clone())',
  '.list_channels(tenant_id, 1, 250)',
  'async_graphql::Error::new(err.to_string())',
  'Ok(channels.into_iter().map(Into::into).collect())',
]) requireText(channelResolver, marker, `${paths.query}: unchanged resolver contract`);

const ownerCallCount = channelResolver.split('.list_channels(').length - 1;
if (ownerCallCount !== 1) {
  failures.push(`${paths.query}: expected one mounted Channel owner call, found ${ownerCallCount}`);
}

for (const marker of [
  '#[path = "source/rustok_channel_shim.rs"]',
  'mod rustok_channel_shim;',
  'use self::rustok_channel_shim as rustok_channel;',
  'include!("../query.rs");',
]) requireText(safeSource, marker, `${paths.safeSource}: mounted Channel facade`);

for (const marker of [
  'use ::rustok_channel::{ChannelError, ChannelResponse};',
  'inner: ::rustok_channel::ChannelService',
  'inner: ::rustok_channel::ChannelService::new(db)',
  'pub(crate) async fn list_channels(',
  '.list_channels(tenant_id, page, per_page)',
  '.map_err(Into::into)',
]) requireText(channelShim, marker, `${paths.channelShim}: canonical owner delegation`);

for (const marker of [
  'pub(crate) fn to_string(self) -> ChannelGraphqlMessage',
  'ChannelGraphqlMessage { error: self.error }',
]) requireText(typedConversion, marker, `${paths.channelShim}: typed compatibility conversion`);
for (const forbidden of [
  'self.error.to_string()',
  'format!("{}", self.error)',
  'format!("{:?}", self.error)',
]) forbidText(typedConversion, forbidden, `${paths.channelShim}: owner display conversion`);

for (const [variant, message, code] of [
  ['ChannelError::InvalidTargetType(_)', 'Channel query is invalid', 'CHANNEL_REQUEST_INVALID'],
  ['ChannelError::NotFound(_)', 'Channel data was not found', 'CHANNEL_RESOURCE_NOT_FOUND'],
  ['ChannelError::InactiveChannel(_)', 'Channel state conflicts with this query', 'CHANNEL_STATE_CONFLICT'],
  ['ChannelError::Database(_)', 'Channel data is temporarily unavailable', 'CHANNEL_TEMPORARILY_UNAVAILABLE'],
  ['ChannelError::Serialization(_)', 'Channel query could not be completed safely', 'CHANNEL_OPERATION_FAILED'],
]) {
  for (const marker of [variant, `"${message}"`, `"${code}"`]) {
    requireText(mapper, marker, `${paths.channelShim}: ${code} transport policy`);
  }
}

for (const marker of [
  'owner_detail(&self.error)',
  'error = ?diagnostic_error',
  'owner = "rustok_channel"',
  'owner_detail_shape',
  'owner_detail_length',
  'public_code = code',
  'boundary = GRAPHQL_QUERY_CHANNEL_BOUNDARY',
  'tracing::error!(',
  'tracing::warn!(',
  'BoundaryError::Public {',
]) requireText(mapper, marker, `${paths.channelShim}: bounded Channel diagnostics`);
for (const forbidden of [
  'error = ?self.error',
  'error = %self.error',
  'owner_error = ?self.error',
  'owner_error = %self.error',
  'message = %self.error',
  'self.error.to_string()',
]) forbidText(mapper, forbidden, `${paths.channelShim}: raw Channel owner payload`);

for (const marker of [
  'fn owner_detail(error: &ChannelError)',
  'ChannelError::Database(_) => ("database_redacted", 0)',
  'ChannelError::Serialization(_) => ("serialization_redacted", 0)',
  'target_type.chars().count().saturating_add(value.chars().count())',
  'fn uuid_shape(value: &::uuid::Uuid)',
]) requireText(channelShim, marker, `${paths.channelShim}: bounded owner detail projection`);

for (const marker of [
  'pub struct ChannelService',
  'pub async fn list_channels(',
  'tenant_id: Uuid',
  'page: u64',
  'per_page: u64',
  'ChannelResult<(Vec<ChannelResponse>, u64)>',
]) requireText(ownerService, marker, `${paths.ownerService}: preserved owner contract`);

for (const marker of [
  'pub enum ChannelError',
  'SlugAlreadyExists(String)',
  'NotFound(Uuid)',
  'InactiveChannel(Uuid)',
  'InvalidTargetType(String)',
  'InvalidTargetValue(String)',
  'InvalidPolicyDefinition(String)',
  'TargetAlreadyExists(String, String)',
  'PolicySetSlugAlreadyExists(String)',
  'InvalidPolicyOperation(String)',
  'Database(#[from] DbErr)',
  'Serialization(#[from] SerdeJsonError)',
]) requireText(ownerError, marker, `${paths.ownerError}: exhaustive owner variants`);

for (const [key, expected] of Object.entries({
  query_resolver_source_changed: false,
  channel_owner_call_count: 1,
  channel_owner_service_preserved: true,
  channel_owner_arguments_preserved: true,
  channel_success_projection_preserved: true,
  typed_channel_error_retained_to_transport: true,
  owner_error_display_used_for_public_response: false,
  complete_channel_error_public: false,
  owner_detail_content_public: false,
  structural_channel_error_policy_preserved: true,
  database_retryable: true,
  other_retryable: false,
  complete_channel_error_logged: false,
  owner_detail_content_logged: false,
  owner_detail_shape_length_logged: true,
  diagnostic_debug_redacted: true,
  technical_error_severity_preserved: true,
  ordinary_rejection_warning_severity_preserved: true,
  graphql_fields_or_dtos_changed: false,
  channel_owner_contract_changed: false,
  commerce_ffa_status_changed: false,
  commerce_fba_status_changed: false,
  broad_ecommerce_cleanup_closed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}

for (const [key, expected] of Object.entries({
  validation_public_code: 'CHANNEL_REQUEST_INVALID',
  not_found_public_code: 'CHANNEL_RESOURCE_NOT_FOUND',
  conflict_public_code: 'CHANNEL_STATE_CONFLICT',
  database_public_code: 'CHANNEL_TEMPORARILY_UNAVAILABLE',
  serialization_public_code: 'CHANNEL_OPERATION_FAILED',
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
  '# Commerce GraphQL channel error safety',
  'Status: `source_closed_unvalidated`',
  'The compatibility resolver source in `crates/rustok-commerce/src/graphql/query.rs` remains unchanged.',
  'call `list_channels(tenant_id, 1, 250)` exactly once',
  'It does not format the Channel owner error into a public string.',
  '`CHANNEL_TEMPORARILY_UNAVAILABLE`',
  'Commerce and Channel FFA/FBA status is unchanged.',
  'The broad ecommerce mapper and public-envelope cleanup remains open.',
  'No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, or CI were executed.',
]) requireText(document, marker, `${paths.document}: truthful source contract`);

if (failures.length > 0) {
  console.error('Commerce GraphQL channel error-safety verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Commerce GraphQL pricing-channel reads preserve the Channel owner call while routing failures through structural bounded envelopes',
);
