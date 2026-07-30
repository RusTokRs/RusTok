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
    if (current < 0 || current <= previous) {
      fail(`${relative} is missing or reorders ${marker}`);
    }
    previous = current;
  }
};

const ownerPath = 'crates/rustok-social-graph/src/index_privacy.rs';
const owner = requireMarkers(ownerPath, [
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
  if (owner.includes(forbidden)) {
    fail(`${ownerPath} contains forbidden owner-storage or permissive marker ${forbidden}`);
  }
}

requireMarkers('crates/rustok-social-graph/src/lib.rs', [
  '#[cfg(feature = "index")]',
  'pub mod index_privacy;',
  'pub use index_privacy::IndexSocialGraphPrivacyReadPort;',
]);

const policyPath = 'apps/server/src/services/notification_recipient_policy.rs';
const policy = requireMarkers(policyPath, [
  'pub const SOCIAL_GRAPH_INDEX_PRIVACY_READS_ENABLED_ENV',
  'RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_READS_ENABLED',
  'pub(crate) fn social_graph_index_privacy_reads_enabled()',
  'Err(std::env::VarError::NotPresent) => Ok(false)',
  'pub fn compose_with_index_runtime(',
  'runtime: SharedIndexQueryRuntime',
  'IndexSocialGraphPrivacyReadPort::new(runtime)',
  'SocialGraphPrivacyRuntime::new(graph_port)',
  'NotificationBlockReadRuntime::new',
  'NotificationMuteReadRuntime::new',
  'evaluate_profile_privacy',
  'blocks_notification',
  'mutes_notification',
  'map_port_error',
]);
for (const forbidden of [
  'social_graph_relations',
  'relation::Entity',
  'PostgresIndexQueryPort::new',
  'unwrap_or(false)',
]) {
  if (policy.includes(forbidden)) {
    fail(`${policyPath} contains forbidden table/runtime bypass marker ${forbidden}`);
  }
}

const finalHostPath = 'apps/server/src/services/mod.rs';
const finalHost = requireMarkers(finalHostPath, [
  'materialize_postgres_index_query_runtime(&mut extensions, db.clone())',
  'social_graph_index_privacy_reads_enabled()',
  'Index query runtime is required when Social Graph Index privacy reads are enabled',
  'get::<rustok_index::SharedIndexQueryRuntime>()',
  'compose_with_index_runtime(',
  'extensions.insert(policy);',
]);
requireOrder(finalHostPath, finalHost, [
  'materialize_postgres_index_query_runtime(&mut extensions, db.clone())',
  'social_graph_index_privacy_reads_enabled()',
  'get::<rustok_index::SharedIndexQueryRuntime>()',
  'compose_with_index_runtime(',
  'extensions.insert(policy);',
  'Ok(Arc::new(extensions))',
]);
for (const forbidden of [
  'SocialGraphService::new',
  'PostgresIndexQueryPort::new',
  'unwrap_or(false)',
  'unwrap_or_default()',
]) {
  if (finalHost.includes(forbidden)) {
    fail(`${finalHostPath} contains forbidden activated-path fallback marker ${forbidden}`);
  }
}

const contractPath = 'crates/rustok-social-graph/contracts/social-graph-notification-policy.json';
const contract = JSON.parse(read(contractPath));
if (contract.schema_version !== 3) fail(`${contractPath} must use schema_version 3`);
if (contract.index_privacy !== ownerPath) fail(`${contractPath} must point to the owner adapter`);
if (contract.privacy_semantics?.index_readiness_failure_suppresses_allow_when_enabled !== true) {
  fail(`${contractPath} must record fail-closed Index readiness after activation`);
}
if (contract.server_composition?.index_query_runtime_required_when_enabled !== true) {
  fail(`${contractPath} must require the shared Index query runtime after activation`);
}
if (contract.server_composition?.notification_block_mute_index_cutover_source_complete !== true) {
  fail(`${contractPath} must record source-complete notification block/mute cutover`);
}
if (contract.server_composition?.index_privacy_activation_env !== 'RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_READS_ENABLED') {
  fail(`${contractPath} must retain the exact activation environment variable`);
}
if (contract.server_composition?.index_privacy_default_enabled !== false) {
  fail(`${contractPath} must keep Index privacy reads default-off`);
}
if (contract.server_composition?.authoritative_table_path_before_activation !== true) {
  fail(`${contractPath} must retain the owner read path before activation`);
}
if (contract.server_composition?.authoritative_table_fallback_after_activation !== false) {
  fail(`${contractPath} must forbid owner-table fallback after activation`);
}

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-social-graph-privacy-consumer.mjs'",
]);
requireMarkers('crates/rustok-index/docs/m4-social-graph-privacy-consumer.md', [
  'Status: `source_complete_execution_pending`',
  '`IndexSocialGraphPrivacyReadPort`',
  'block in either direction',
  '`RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_READS_ENABLED`',
  'default-off',
  'retryable fail-closed',
  'does not authorize from missing or stale Index state',
  'Not run by the implementation agent',
]);
requireMarkers('crates/rustok-index/docs/m4-query-planner.md', [
  'M4 first authorized consumer cutover source: `source_complete_execution_pending`',
  '`IndexSocialGraphPrivacyReadPort`',
  'notification block/mute policy',
  'default-off activation gate',
]);
requireMarkers('crates/rustok-social-graph/CRATE_API.md', [
  '`IndexSocialGraphPrivacyReadPort`',
  'notification block/mute policy',
  '`RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_READS_ENABLED`',
  'retryable fail-closed',
]);

console.log('[verify-index-social-graph-privacy-consumer] OK');
