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

const shadowPath = 'crates/rustok-social-graph/src/index_privacy_shadow.rs';
const shadow = requireMarkers(shadowPath, [
  'pub struct IndexShadowSocialGraphPrivacyReadPort',
  'authoritative: Arc<dyn SocialGraphPrivacyReadPort>',
  'projected: IndexSocialGraphPrivacyReadPort',
  'impl SocialGraphPrivacyReadPort for IndexShadowSocialGraphPrivacyReadPort',
  '.blocks_between(context.clone(), request)',
  '.source_mutes_target(context.clone(), request)',
  '.source_follows_target(context.clone(), request)',
  '.source_follows_targets(context.clone(), request.clone())',
  'observe_bool(',
  'observe_batch(',
  'Ok(authoritative)',
  'Social Graph Index privacy shadow mismatch',
  'Social Graph Index privacy shadow read failed',
]);
const authoritativeReturns = shadow.match(/Ok\(authoritative\)/g) ?? [];
if (authoritativeReturns.length !== 4) {
  fail(`${shadowPath} must return the authoritative result from all four privacy methods`);
}
for (const forbidden of [
  'tenant_id =',
  'source_user_id =',
  'target_user_id =',
  'relation_id =',
  'entity_id =',
  'PostgresIndexQueryPort',
  'DatabaseConnection',
  'unwrap_or(false)',
  'unwrap_or_default()',
]) {
  if (shadow.includes(forbidden)) fail(`${shadowPath} contains forbidden telemetry/runtime marker ${forbidden}`);
}

requireMarkers('crates/rustok-social-graph/src/lib.rs', [
  'pub mod index_privacy;',
  'pub mod index_privacy_shadow;',
  'pub use index_privacy::IndexSocialGraphPrivacyReadPort;',
  'pub use index_privacy_shadow::IndexShadowSocialGraphPrivacyReadPort;',
]);

const policyPath = 'apps/server/src/services/notification_recipient_policy.rs';
const policy = requireMarkers(policyPath, [
  'pub const SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED_ENV',
  'RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED',
  'pub(crate) fn social_graph_index_privacy_shadow_enabled()',
  'Err(std::env::VarError::NotPresent) => Ok(false)',
  'pub fn compose_with_index_shadow_runtime(',
  'runtime: SharedIndexQueryRuntime',
  'SocialGraphService::new(db.clone())',
  'IndexShadowSocialGraphPrivacyReadPort::new(authoritative, runtime)',
  'NotificationBlockReadRuntime::new',
  'NotificationMuteReadRuntime::new',
  'evaluate_profile_privacy',
  'blocks_notification',
  'mutes_notification',
]);
for (const forbidden of ['social_graph_relations', 'relation::Entity', 'PostgresIndexQueryPort::new']) {
  if (policy.includes(forbidden)) fail(`${policyPath} contains forbidden table/runtime bypass marker ${forbidden}`);
}

const finalHostPath = 'apps/server/src/services/mod.rs';
const finalHost = requireMarkers(finalHostPath, [
  'materialize_postgres_index_query_runtime(&mut extensions, db.clone())',
  'social_graph_index_privacy_shadow_enabled()',
  'Index query runtime is required when Social Graph Index privacy shadow is enabled',
  'get::<rustok_index::SharedIndexQueryRuntime>()',
  'compose_with_index_shadow_runtime(',
  'extensions.insert(policy);',
]);
requireOrder(finalHostPath, finalHost, [
  'materialize_postgres_index_query_runtime(&mut extensions, db.clone())',
  'social_graph_index_privacy_shadow_enabled()',
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
if (contract.schema_version !== 3) fail(`${contractPath} must use schema_version 3`);
if (contract.index_privacy !== adapterPath) fail(`${contractPath} must point to the Index adapter`);
if (contract.index_privacy_shadow !== shadowPath) fail(`${contractPath} must point to the shadow wrapper`);
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

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-social-graph-privacy-consumer.mjs'",
]);
requireMarkers('crates/rustok-index/docs/m4-social-graph-privacy-consumer.md', [
  'Status: `source_complete_execution_pending`',
  '`IndexSocialGraphPrivacyReadPort`',
  '`IndexShadowSocialGraphPrivacyReadPort`',
  '`RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED`',
  'default-off',
  'always returns the owner result',
  'never authorizes',
  'Not run by the implementation agent',
]);
requireMarkers('crates/rustok-index/docs/m4-query-planner.md', [
  'M4 first consumer parity shadow: `source_complete_execution_pending`',
  '`IndexShadowSocialGraphPrivacyReadPort`',
  'notification block/mute policy',
  'default-off shadow gate',
]);
requireMarkers('crates/rustok-social-graph/CRATE_API.md', [
  '`IndexSocialGraphPrivacyReadPort`',
  '`IndexShadowSocialGraphPrivacyReadPort`',
  '`RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED`',
  'owner result',
  'never authorizes',
]);

console.log('[verify-index-social-graph-privacy-consumer] OK');
