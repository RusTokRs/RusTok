#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-server-reconciliation-guard] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const compositionPath = 'apps/server/src/services/index_replay_runtime_composition.rs';
const operatorPath = 'apps/server/src/services/index_reconciliation_operator.rs';
const diagnosisPath = 'apps/server/src/services/index_drift_diagnosis_operator.rs';
const pagePath = 'apps/server/src/services/index_drift_source_page_diagnosis.rs';
const keyringPath = 'apps/server/src/services/index_source_continuation_runtime.rs';
const continuationPath = 'crates/rustok-index/src/application/source_continuation.rs';
const exactGraphqlPath = 'apps/server/src/graphql/index_drift_diagnosis.rs';
const pageGraphqlPath = 'apps/server/src/graphql/index_drift_source_page_diagnosis.rs';
const graphqlSchemaPath = 'apps/server/src/graphql/schema.rs';
const docsPath = 'apps/server/docs/index-reconciliation-operator-runtime.md';
const exactDocsPath = 'apps/server/docs/index-drift-diagnosis-graphql-transport.md';
const pageDocsPath = 'apps/server/docs/index-drift-source-page-diagnosis.md';
const pageTransportDocsPath = 'apps/server/docs/index-drift-source-page-graphql-transport.md';
const planPath = 'crates/rustok-index/docs/implementation-plan-current-2026-08-03.md';
const recheckPath =
  'crates/rustok-index/docs/implementation-recheck-2026-08-05-explicit-absence-watermark.md';

const composition = requireMarkers(compositionPath, [
  '#[path = "index_reconciliation_operator.rs"]',
  '#[path = "index_drift_diagnosis_operator.rs"]',
  '#[path = "index_source_continuation_runtime.rs"]',
  '#[path = "index_drift_source_page_diagnosis.rs"]',
  'materialize_postgres_index_replay_runtime(extensions, db.clone())',
  'reconciliation_operator::materialize_index_reconciliation_operator(extensions, db.clone())?;',
  'drift_diagnosis_operator::materialize_index_drift_diagnosis_operator(extensions, db)?;',
  'source_continuation_runtime::materialize_index_source_continuation_keyring()',
  'drift_source_page_diagnosis::materialize_index_drift_source_page_diagnosis(',
]);
const replay = composition.indexOf('materialize_postgres_index_replay_runtime(extensions, db.clone())');
const reconciliation = composition.indexOf(
  'reconciliation_operator::materialize_index_reconciliation_operator(extensions, db.clone())?;',
);
const diagnosis = composition.indexOf(
  'drift_diagnosis_operator::materialize_index_drift_diagnosis_operator(extensions, db)?;',
);
const keyring = composition.indexOf(
  'source_continuation_runtime::materialize_index_source_continuation_keyring()',
);
const page = composition.indexOf(
  'drift_source_page_diagnosis::materialize_index_drift_source_page_diagnosis(',
);
if (
  replay < 0 ||
  reconciliation <= replay ||
  diagnosis <= reconciliation ||
  keyring <= diagnosis ||
  page <= keyring
) {
  fail(`${compositionPath} composition ordering is invalid`);
}
for (const forbidden of ['extensions.insert(continuation)', 'extensions.insert(keyring)']) {
  if (composition.includes(forbidden)) fail(`${compositionPath} exposes ${forbidden}`);
}

const operator = requireMarkers(operatorPath, [
  'pub struct IndexReconciliationOperatorContext',
  'Permission::MODULES_MANAGE',
  'pub async fn request_cancel(',
  'pub async fn inspect_dead_letter(',
  'pub async fn inspect_drift_finding(',
  'pub async fn requeue_dead_letter(',
]);
const operatorProduction = operator.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'tokio::spawn',
  'async_graphql',
  'IndexSourceContinuationCodec',
  'IndexSourceContinuationToken',
]) {
  if (operatorProduction.includes(forbidden)) fail(`${operatorPath} contains ${forbidden}`);
}

const exact = requireMarkers(diagnosisPath, [
  'pub struct IndexDriftDiagnosisOperatorRuntime',
  'pub async fn diagnose_entity(',
  'pub async fn diagnose_missing_entity_candidate(',
  'authorize_for(&context, key.tenant_id)?;',
  'IndexDriftDigestRequest::new(key)?;',
  '.produce_missing_entity_candidate(request)',
  'reader.with_absence_registry(absence)',
]);
const exactProduction = exact.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'tokio::spawn',
  'async_graphql',
  '.scan(',
  'IndexSourceContinuationCodec',
  'repair_finding',
]) {
  if (exactProduction.includes(forbidden)) fail(`${diagnosisPath} contains ${forbidden}`);
}

const keyringSource = requireMarkers(keyringPath, [
  'RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON',
  'keys: BTreeMap<String, SecretRef>',
  'const KEY_BYTES: usize = 32;',
  'const ENCODED_KEY_BYTES: usize = 43;',
  'const MAX_CONFIG_BYTES: usize = 16 * 1024;',
  'const MAX_KEYS: usize = 16;',
  'const MAX_SECRET_REFERENCE_BYTES: usize = 256;',
  'SecretAccessPolicy::Exact',
  'resolve_for_tenant(DEPLOYMENT_SECRET_SCOPE, reference)',
  'IndexSourceContinuationCodec::new',
]);
const keyringProduction = keyringSource.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'DatabaseConnection',
  'tokio::spawn',
  'tracing::',
  'Vec<u8>',
]) {
  if (keyringProduction.includes(forbidden)) fail(`${keyringPath} contains ${forbidden}`);
}

const pageSource = requireMarkers(pagePath, [
  'const MAX_SOURCE_PAGE_DIAGNOSIS_SIZE: usize = 32;',
  'pub struct IndexDriftSourcePageDiagnosisSealedOutcome',
  'pub async fn diagnose_source_page_sealed(',
  'authorize_context(context)?;',
  'validate_page_limit(limit)?;',
  'IndexSourceContinuationScope::from_registry(',
  '.resolve_codec()',
  'codec.open_encoded(&scope, encoded, Utc::now())',
  'IndexSourceScanRequest::new(',
  'self.diagnose_request(context, request).await?;',
  'codec.seal(&scope, cursor, Utc::now(), keyring.lifetime())',
  '.diagnose_missing_entity_candidate(context, key)',
]);
const pageProduction = pageSource.split('\n#[cfg(test)]')[0];
if ((pageProduction.match(/self\.sources\.scan\(request\)/g) ?? []).length !== 1) {
  fail(`${pagePath} must scan exactly one page`);
}
for (const forbidden of [
  'tokio::spawn',
  'DatabaseConnection',
  'while ',
  'loop {',
  'ModuleWorkScheduler',
  'repair_finding',
  'resolve_finding',
  'ignore_finding',
]) {
  if (pageProduction.includes(forbidden)) fail(`${pagePath} contains ${forbidden}`);
}

const continuation = requireMarkers(continuationPath, [
  'pub struct IndexSourceContinuationScope',
  'pub struct IndexSourceContinuationToken(String);',
  'pub struct IndexSourceContinuationCodec',
  'Aes256Gcm::new_from_slice',
  'OsRng.fill_bytes(&mut nonce);',
  'validate_claims(&claims, expected_scope, now)?;',
]);
const continuationProduction = continuation.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'DatabaseConnection',
  'sea_orm',
  'async_graphql',
  'std::env',
  'SecretResolverRegistry',
]) {
  if (continuationProduction.includes(forbidden)) {
    fail(`${continuationPath} contains server dependency ${forbidden}`);
  }
}

const exactGraphql = requireMarkers(exactGraphqlPath, [
  'async fn diagnose_index_entity(',
  '.diagnose_entity(operator_context, key)',
]);
for (const forbidden of [
  'diagnose_index_source_page',
  'IndexDriftSourcePageDiagnosisRuntime',
  'diagnose_source_page_sealed',
]) {
  if (exactGraphql.includes(forbidden)) fail(`${exactGraphqlPath} contains ${forbidden}`);
}

const pageGraphql = requireMarkers(pageGraphqlPath, [
  'pub struct IndexDriftSourcePageDiagnosisInput',
  'pub limit: String',
  'pub continuation: Option<String>',
  'async fn diagnose_index_source_page(',
  'permissions_for(&tenant_id, &actor_id)',
  'let schema = parse_schema(',
  'bounded_text("limit"',
  'bounded_text("continuation"',
  '.get::<IndexDriftSourcePageDiagnosisRuntime>()',
  '.diagnose_source_page_sealed(',
  'continuation.as_deref()',
]);
const pageGraphqlProduction = pageGraphql.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'IndexSourceCursor',
  'IndexSourceContinuationKeyringRuntime',
  'SecretRef',
  '.diagnose_source_page(',
  'entity_id: String',
  'source_name: String',
  'owner_module: String',
  'DatabaseConnection',
  'tokio::spawn',
]) {
  if (pageGraphqlProduction.includes(forbidden)) fail(`${pageGraphqlPath} contains ${forbidden}`);
}

requireMarkers(graphqlSchemaPath, [
  'use super::index_drift_source_page_diagnosis::IndexDriftSourcePageDiagnosisMutation;',
  'IndexDriftSourcePageDiagnosisMutation,',
]);
requireMarkers(docsPath, [
  'Status: `sealed_source_page_graphql_source_complete_owner_execution_pending`.',
  'diagnoseIndexSourcePage',
  'diagnose_source_page_sealed',
]);
requireMarkers(exactDocsPath, [
  'Status: `source_complete_owner_execution_pending`.',
  'diagnoseIndexEntity(input: $input)',
]);
requireMarkers(pageDocsPath, [
  'Status: `graphql_sealed_transport_source_complete_owner_execution_pending`.',
  'diagnoseIndexSourcePage',
  'raw cursor is never returned',
]);
requireMarkers(pageTransportDocsPath, [
  'Status: `source_complete_owner_execution_pending`.',
  'diagnoseIndexSourcePage(input: IndexDriftSourcePageDiagnosisInput!)',
  'No raw `IndexSourceCursor`',
]);
requireMarkers(planPath, [
  'M6 bounded GraphQL sealed source-page diagnosis transport',
  '`source_complete_owner_execution_pending`',
  '[x] Expose one bounded source-page GraphQL mutation over the sealed method only.',
  'M6 stale entity, orphan-link, lifecycle, and targeted repair',
]);
requireMarkers(recheckPath, [
  'Bounded GraphQL source-page transport',
  'did not run tests',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-server-reconciliation-guard.mjs'",
  "'verify-index-drift-source-page-diagnosis.mjs'",
  "'verify-index-drift-source-page-graphql-transport.mjs'",
  "'verify-index-source-continuation-server.mjs'",
]);

console.log('[verify-index-server-reconciliation-guard] OK');
