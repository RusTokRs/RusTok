#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-social-graph-privacy-consumer] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};
const requireOrder = (relative, source, markers) => {
  let previous = -1;
  for (const marker of markers) {
    const current = source.indexOf(marker, previous + 1);
    if (current < 0 || current <= previous) fail(`${relative} is missing or reorders ${marker}`);
    previous = current;
  }
};

const adapterPath = 'crates/rustok-social-graph/src/index_privacy.rs';
const adapter = requireMarkers(adapterPath, [
  'pub struct IndexSocialGraphPrivacyReadPort',
  'port: Arc<dyn IndexQueryPort>',
  'pub fn new(runtime: SharedIndexQueryRuntime)',
  'impl SocialGraphPrivacyReadPort for IndexSocialGraphPrivacyReadPort',
  'context.require_policy(PortCallPolicy::read())',
  'validate_source_actor(&context, request.source_user_id)',
  'MAX_SOCIAL_GRAPH_FOLLOW_TARGETS',
  'FilterExpr::Or(vec![',
  'SocialRelationKind::Block.as_str()',
  'SocialRelationKind::Mute.as_str()',
  'SocialRelationKind::Follow.as_str()',
  'FilterExpr::In(contract.target.clone(), target_values)',
  'Pagination::Offset',
  'if page.has_more',
  'IndexQueryExecutionError::SchemaNotReady',
  'social_graph.index_privacy_unavailable',
  'social_graph.index_privacy_contract_invalid',
  'missing_tenant_schema_is_retryable_and_does_not_authorize',
  'user_follow_reads_preserve_source_actor_authorization',
]);
for (const forbidden of [
  'DatabaseConnection',
  'SocialGraphService',
  'relation::Entity',
  'social_graph_relations',
  'sea_orm::',
  'PostgresIndexQueryPort',
  'unwrap_or(false)',
  'unwrap_or_default()',
]) {
  if (adapter.includes(forbidden)) fail(`${adapterPath} contains forbidden marker ${forbidden}`);
}

const metricsPath = 'crates/rustok-telemetry/src/social_graph_index_privacy_shadow_metrics.rs';
const metrics = requireMarkers(metricsPath, [
  'pub enum SocialGraphIndexPrivacyShadowOperation',
  'pub enum SocialGraphIndexPrivacyShadowOutcome',
  'MatchPositive',
  'MatchNegative',
  'FalseNegative',
  'FalsePositive',
  'MatchBatchEmpty',
  'MatchBatchNonempty',
  'BatchMissing',
  'BatchExtra',
  'BatchMixed',
  'rustok_social_graph_index_privacy_shadow_observations_total',
  'rustok_social_graph_index_privacy_shadow_failures_total',
  'rustok_social_graph_index_privacy_shadow_comparison_duration_seconds',
  'rustok_social_graph_index_privacy_shadow_last_observation_timestamp_seconds',
  '&["operation", "outcome"]',
  '&["operation", "error_code", "retryable"]',
  'pub fn ensure_registered()',
  'crate::register_runtime_collector',
  'pub fn record_observation(',
  'pub fn record_failure(',
  'fn bounded_error_code(',
  '"social_graph.index_privacy_unavailable"',
  '"social_graph.index_privacy_contract_invalid"',
  '_ => "other"',
  'error_code_label_is_bounded',
]);
for (const forbidden of [
  'tenant_id',
  'source_user_id',
  'target_user_id',
  'relation_id',
  'entity_id',
  'payload',
  'sql',
  'storage_error',
]) {
  if (metrics.includes(forbidden)) fail(`${metricsPath} contains forbidden identity/cardinality marker ${forbidden}`);
}
requireMarkers('crates/rustok-telemetry/src/lib.rs', [
  'pub mod social_graph_index_privacy_shadow_metrics;',
]);

const shadowPath = 'crates/rustok-social-graph/src/index_privacy_shadow.rs';
const shadow = requireMarkers(shadowPath, [
  'pub const SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_TARGET',
  'rustok_social_graph::index_privacy_shadow',
  'pub enum IndexPrivacyShadowOperation',
  'pub enum IndexPrivacyShadowOutcome',
  'pub enum IndexPrivacyShadowFailureCode',
  'pub struct IndexPrivacyShadowObservation',
  'pub trait IndexPrivacyShadowObserver',
  'fn observe(&self, observation: IndexPrivacyShadowObservation)',
  'pub struct IndexShadowSocialGraphPrivacyReadPort',
  'observer: Arc<dyn IndexPrivacyShadowObserver>',
  'pub fn with_observer(',
  'projected: Arc::new(IndexSocialGraphPrivacyReadPort::new(runtime))',
  'impl SocialGraphPrivacyReadPort for IndexShadowSocialGraphPrivacyReadPort',
  '.blocks_between(context.clone(), request)',
  '.source_mutes_target(context.clone(), request)',
  '.source_follows_target(context.clone(), request)',
  '.source_follows_targets(context.clone(), request.clone())',
  'let operation_started_at = Instant::now();',
  'let budget = shadow_budget(&context);',
  'projected_within_remaining_budget(',
  'tokio::time::timeout(remaining, future)',
  'social graph Index privacy shadow exceeded the caller deadline budget',
  'observer.observe(IndexPrivacyShadowObservation {',
  'IndexPrivacyShadowOutcome::FalseNegative',
  'IndexPrivacyShadowOutcome::FalsePositive',
  'IndexPrivacyShadowOutcome::BatchMissing',
  'IndexPrivacyShadowOutcome::BatchExtra',
  'IndexPrivacyShadowOutcome::BatchMixed',
  'IndexPrivacyShadowFailureCode::from_port_error',
  'Ok(authoritative)',
  'comparison_duration_ms',
  'boolean_outcomes_distinguish_negative_safety',
  'batch_outcomes_distinguish_missing_extra_and_mixed',
  'mismatch_returns_authoritative_boolean_and_observes_false_negative',
  'projected_error_returns_authoritative_batch_and_bounded_failure',
  'projected_timeout_returns_authoritative_result_within_caller_budget',
]);
const authoritativeReturns = shadow.match(/Ok\(authoritative\)/g) ?? [];
if (authoritativeReturns.length !== 4) {
  fail(`${shadowPath} must return the authoritative result from all four privacy methods`);
}
const shadowWithoutAllowedDeadlineDefaults = shadow
  .replace('Duration::from_millis(context.deadline_ms.unwrap_or_default())', '')
  .replace('.checked_sub(operation_started_at.elapsed())\n        .unwrap_or_default()', '');
if (shadowWithoutAllowedDeadlineDefaults.includes('unwrap_or_default()')) {
  fail(`${shadowPath} contains an unapproved default fallback outside fail-closed deadline accounting`);
}
for (const forbidden of [
  'rustok_telemetry',
  'tenant_id =',
  'source_user_id =',
  'target_user_id =',
  'relation_id =',
  'entity_id =',
  'PostgresIndexQueryPort',
  'DatabaseConnection',
  'unwrap_or(false)',
]) {
  if (shadow.includes(forbidden)) fail(`${shadowPath} contains forbidden telemetry/runtime marker ${forbidden}`);
}

requireMarkers('crates/rustok-social-graph/src/lib.rs', [
  'pub mod index_privacy;',
  'pub mod index_privacy_shadow;',
  'pub use index_privacy::IndexSocialGraphPrivacyReadPort;',
  'IndexPrivacyShadowFailureCode',
  'IndexPrivacyShadowObservation',
  'IndexPrivacyShadowObserver',
  'IndexPrivacyShadowOperation',
  'IndexPrivacyShadowOutcome',
  'IndexShadowSocialGraphPrivacyReadPort',
]);
requireMarkers('crates/rustok-social-graph/Cargo.toml', [
  'index = ["dep:rustok-index"]',
  'tokio.workspace = true',
]);
const socialCargo = read('crates/rustok-social-graph/Cargo.toml');
if (socialCargo.includes('rustok-telemetry')) {
  fail('crates/rustok-social-graph/Cargo.toml must keep the Prometheus adapter host-owned');
}

const policyPath = 'apps/server/src/services/notification_recipient_policy.rs';
const policy = requireMarkers(policyPath, [
  'pub const SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED_ENV',
  'RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED',
  'struct TelemetryIndexPrivacyShadowObserver;',
  'impl IndexPrivacyShadowObserver for TelemetryIndexPrivacyShadowObserver',
  'fn observe(&self, observation: IndexPrivacyShadowObservation)',
  'metric_operation(observation.operation)',
  'record_failure(',
  'record_observation(',
  'fn metric_operation(',
  'fn metric_outcome(',
  'pub(crate) fn social_graph_index_privacy_shadow_enabled()',
  'Err(std::env::VarError::NotPresent) => Ok(false)',
  'pub fn compose_with_index_shadow_runtime(',
  'runtime: SharedIndexQueryRuntime',
  'SocialGraphService::new(db.clone())',
  'IndexShadowSocialGraphPrivacyReadPort::with_observer(',
  'Arc::new(TelemetryIndexPrivacyShadowObserver)',
  'NotificationBlockReadRuntime::new',
  'NotificationMuteReadRuntime::new',
  'evaluate_profile_privacy',
  'blocks_notification',
  'mutes_notification',
]);
for (const forbidden of ['social_graph_relations', 'relation::Entity', 'PostgresIndexQueryPort::new']) {
  if (policy.includes(forbidden)) fail(`${policyPath} contains forbidden table/runtime bypass marker ${forbidden}`);
}
for (const forbidden of [
  'tenant_id =',
  'source_user_id =',
  'target_user_id =',
  'relation_id =',
  'entity_id =',
]) {
  const observerStart = policy.indexOf('struct TelemetryIndexPrivacyShadowObserver;');
  const policyStart = policy.indexOf('#[derive(Clone)]\npub struct ServerNotificationRecipientPolicy');
  const observerSource = observerStart >= 0 && policyStart > observerStart
    ? policy.slice(observerStart, policyStart)
    : '';
  if (observerSource.includes(forbidden)) {
    fail(`${policyPath} telemetry observer contains forbidden identity marker ${forbidden}`);
  }
}

const finalHostPath = 'apps/server/src/services/mod.rs';
const finalHost = requireMarkers(finalHostPath, [
  'materialize_postgres_index_query_runtime(&mut extensions, db.clone())',
  'social_graph_index_privacy_shadow_enabled()',
  'social_graph_index_privacy_shadow_metrics::ensure_registered()',
  'Social Graph Index privacy shadow metrics registration failed',
  'Index query runtime is required when Social Graph Index privacy shadow is enabled',
  'get::<rustok_index::SharedIndexQueryRuntime>()',
  'compose_with_index_shadow_runtime(',
  'extensions.insert(policy);',
]);
requireOrder(finalHostPath, finalHost, [
  'materialize_postgres_index_query_runtime(&mut extensions, db.clone())',
  'social_graph_index_privacy_shadow_enabled()',
  'social_graph_index_privacy_shadow_metrics::ensure_registered()',
  'get::<rustok_index::SharedIndexQueryRuntime>()',
  'compose_with_index_shadow_runtime(',
  'extensions.insert(policy);',
  'Ok(Arc::new(extensions))',
]);
for (const forbidden of ['SocialGraphService::new', 'PostgresIndexQueryPort::new', 'unwrap_or(false)']) {
  if (finalHost.includes(forbidden)) fail(`${finalHostPath} contains forbidden final-host marker ${forbidden}`);
}

const contractPath = 'crates/rustok-social-graph/contracts/social-graph-notification-policy.json';
const contract = JSON.parse(read(contractPath));
if (contract.schema_version !== 4) fail(`${contractPath} must use schema_version 4`);
if (contract.index_privacy !== adapterPath) fail(`${contractPath} must point to the Index adapter`);
if (contract.index_privacy_shadow !== shadowPath) fail(`${contractPath} must point to the shadow wrapper`);
if (contract.index_privacy_shadow_metrics !== metricsPath) fail(`${contractPath} must point to the telemetry collector`);
if (contract.index_privacy_shadow_metrics_adapter !== policyPath) fail(`${contractPath} must point to the server metrics adapter`);
if (contract.privacy_semantics?.authoritative_owner_result_always_returned !== true) {
  fail(`${contractPath} must retain the owner result as authoritative`);
}
if (contract.privacy_semantics?.index_shadow_never_authorizes !== true) {
  fail(`${contractPath} must forbid Index shadow authorization`);
}
if (contract.server_composition?.notification_block_mute_index_shadow_source_complete !== true) {
  fail(`${contractPath} must record source-complete shadow composition`);
}
if (contract.server_composition?.notification_block_mute_index_cutover !== false) {
  fail(`${contractPath} must not claim an authoritative cutover`);
}
if (contract.server_composition?.index_privacy_shadow_env !== 'RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED') {
  fail(`${contractPath} must retain the exact shadow environment variable`);
}
if (contract.server_composition?.index_privacy_shadow_default_enabled !== false) {
  fail(`${contractPath} must keep the shadow default-off`);
}
if (contract.server_composition?.owner_policy_remains_authoritative !== true) {
  fail(`${contractPath} must keep the owner policy authoritative`);
}
if (contract.server_composition?.index_privacy_shadow_metrics_required_when_enabled !== true) {
  fail(`${contractPath} must require metrics registration for an enabled shadow`);
}
if (contract.telemetry?.owner_observation_contract !== shadowPath) {
  fail(`${contractPath} must retain the owner observation contract`);
}
if (contract.telemetry?.prometheus_adapter_owner !== 'rustok-server') {
  fail(`${contractPath} must keep the Prometheus adapter host-owned`);
}
for (const metric of [
  'rustok_social_graph_index_privacy_shadow_observations_total',
  'rustok_social_graph_index_privacy_shadow_failures_total',
  'rustok_social_graph_index_privacy_shadow_comparison_duration_seconds',
  'rustok_social_graph_index_privacy_shadow_last_observation_timestamp_seconds',
]) {
  if (!Object.values(contract.telemetry ?? {}).flat().includes(metric)) {
    fail(`${contractPath} must retain metric ${metric}`);
  }
}
for (const outcome of ['false_negative', 'false_positive', 'batch_missing', 'batch_extra', 'batch_mixed']) {
  if (!contract.telemetry?.outcomes?.includes(outcome)) fail(`${contractPath} must retain outcome ${outcome}`);
}
for (const label of ['tenant_id', 'source_user_id', 'target_user_id', 'relation_id', 'entity_id']) {
  if (!contract.telemetry?.identity_labels_forbidden?.includes(label)) {
    fail(`${contractPath} must forbid identity label ${label}`);
  }
}

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-social-graph-privacy-consumer.mjs'",
]);
requireMarkers('crates/rustok-index/docs/m4-social-graph-privacy-consumer.md', [
  'Status: `source_complete_metrics_execution_pending`',
  '`IndexSocialGraphPrivacyReadPort`',
  '`IndexShadowSocialGraphPrivacyReadPort`',
  '`IndexPrivacyShadowObservation`',
  'host-owned Prometheus adapter',
  'RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED',
  'default-off',
  'always returns the owner result',
  'remaining caller deadline budget',
  '`false_negative`',
  '`batch_missing`',
  'single Prometheus registry',
  'Not run by the implementation agent',
]);
requireMarkers('crates/rustok-index/docs/m4-query-planner.md', [
  'M4 first consumer parity shadow: `source_complete_metrics_execution_pending`',
  '`IndexShadowSocialGraphPrivacyReadPort`',
  '`IndexPrivacyShadowObservation`',
  'host-owned Prometheus adapter',
  'notification block/mute policy',
  'default-off shadow gate',
  'bounded Prometheus outcomes',
]);
requireMarkers('crates/rustok-social-graph/CRATE_API.md', [
  '`IndexSocialGraphPrivacyReadPort`',
  '`IndexShadowSocialGraphPrivacyReadPort`',
  '`IndexPrivacyShadowObservation`',
  'host-owned Prometheus adapter',
  'RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED',
  'owner result',
  'never authorizes',
  'remaining caller deadline budget',
  '`false_negative`',
  '`batch_mixed`',
]);
requireMarkers('crates/rustok-telemetry/CRATE_API.md', [
  '`social_graph_index_privacy_shadow_metrics`',
  'rustok_social_graph_index_privacy_shadow_observations_total',
  '`false_negative`',
  'tenant and user identifiers',
]);

console.log('[verify-index-social-graph-privacy-consumer] OK');
