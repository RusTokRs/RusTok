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
const pageDiagnosisPath = 'apps/server/src/services/index_drift_source_page_diagnosis.rs';
const keyringPath = 'apps/server/src/services/index_source_continuation_runtime.rs';
const continuationPath = 'crates/rustok-index/src/application/source_continuation.rs';
const graphqlTransportPath = 'apps/server/src/graphql/index_drift_diagnosis.rs';
const graphqlSchemaPath = 'apps/server/src/graphql/schema.rs';
const docsPath = 'apps/server/docs/index-reconciliation-operator-runtime.md';
const graphqlDocsPath = 'apps/server/docs/index-drift-diagnosis-graphql-transport.md';
const pageDocsPath = 'apps/server/docs/index-drift-source-page-diagnosis.md';
const continuationDocsPath = 'crates/rustok-index/docs/m6-source-continuation-codec.md';
const serverContinuationDocsPath =
  'crates/rustok-index/docs/m6-source-continuation-server-keyring.md';
const planPath = 'crates/rustok-index/docs/implementation-plan-current-2026-08-03.md';
const recheckPath =
  'crates/rustok-index/docs/implementation-recheck-2026-08-05-explicit-absence-watermark.md';

const composition = requireMarkers(compositionPath, [
  '#[path = "index_reconciliation_operator.rs"]',
  '#[path = "index_drift_diagnosis_operator.rs"]',
  '#[path = "index_source_continuation_runtime.rs"]',
  '#[path = "index_drift_source_page_diagnosis.rs"]',
  'IndexDriftSourcePageDiagnosisRuntime, IndexDriftSourcePageDiagnosisSealedOutcome,',
  'materialize_postgres_index_replay_runtime(extensions, db.clone())',
  'reconciliation_operator::materialize_index_reconciliation_operator(extensions, db.clone())?;',
  'drift_diagnosis_operator::materialize_index_drift_diagnosis_operator(extensions, db)?;',
  'source_continuation_runtime::materialize_index_source_continuation_keyring()',
  'drift_source_page_diagnosis::materialize_index_drift_source_page_diagnosis(',
  'extensions.contains::<IndexDriftDiagnosisOperatorRuntime>()',
  'extensions.contains::<IndexDriftSourcePageDiagnosisRuntime>()',
]);
const replay = composition.indexOf(
  'materialize_postgres_index_replay_runtime(extensions, db.clone())',
);
const reconciliation = composition.indexOf(
  'reconciliation_operator::materialize_index_reconciliation_operator(extensions, db.clone())?;',
);
const diagnosisComposition = composition.indexOf(
  'drift_diagnosis_operator::materialize_index_drift_diagnosis_operator(extensions, db)?;',
);
const keyringComposition = composition.indexOf(
  'source_continuation_runtime::materialize_index_source_continuation_keyring()',
);
const pageComposition = composition.indexOf(
  'drift_source_page_diagnosis::materialize_index_drift_source_page_diagnosis(',
);
if (
  replay < 0 ||
  reconciliation <= replay ||
  diagnosisComposition <= reconciliation ||
  keyringComposition <= diagnosisComposition ||
  pageComposition <= keyringComposition
) {
  fail(
    `${compositionPath} must freeze replay before reconciliation, exact diagnosis, private keyring, and page diagnosis publication`,
  );
}
for (const forbidden of ['extensions.insert(continuation)', 'extensions.insert(keyring)']) {
  if (composition.includes(forbidden)) {
    fail(`${compositionPath} exposes the private continuation keyring: ${forbidden}`);
  }
}

const operator = requireMarkers(operatorPath, [
  'pub struct IndexReconciliationOperatorContext',
  'tenant_id.is_nil() || actor_id.is_nil()',
  'permissions_for(&self.tenant_id, &self.actor_id)',
  'Permission::MODULES_MANAGE',
  'pub struct IndexReconciliationOperatorRuntime',
  'pub async fn request_cancel(',
  'pub async fn inspect_dead_letter(',
  'pub async fn inspect_drift_finding(',
  'pub async fn requeue_dead_letter(',
  'context.authorize_for(request.tenant_id())?;',
  'context.actor_id(),',
]);
const operatorProduction = operator.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'tokio::spawn',
  'spawn_blocking',
  'SELECT ',
  'INSERT ',
  'UPDATE ',
  'DELETE ',
  'Router::new',
  'async_graphql',
  'PostgresIndexDriftSnapshotReader',
  'IndexDriftDigestProducer',
  'IndexSourceContinuationCodec',
  'IndexSourceContinuationToken',
]) {
  if (operatorProduction.includes(forbidden)) fail(`${operatorPath} contains ${forbidden}`);
}

const diagnosis = requireMarkers(diagnosisPath, [
  'type IndexDriftDiagnosisProducer = rustok_index::IndexDriftDigestProducer<',
  'pub struct IndexDriftDiagnosisOperatorRuntime',
  'pub async fn diagnose_entity(',
  'pub async fn diagnose_missing_entity_candidate(',
  'authorize_for(&context, key.tenant_id)?;',
  'IndexDriftDigestRequest::new(key)?;',
  'self.inner.produce(request).await',
  '.produce_missing_entity_candidate(request)',
  'materialize_index_source_absence_registry(extensions)',
  'reader.with_absence_registry(absence)',
  'IndexDriftDigestProducer::new(',
]);
const diagnosisProduction = diagnosis.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'tokio::spawn',
  'spawn_blocking',
  'Router::new',
  'async_graphql',
  '.scan(',
  'IndexSourceContinuationCodec',
  'IndexSourceContinuationToken',
  'repair_finding',
  'resolve_finding',
  'ignore_finding',
]) {
  if (diagnosisProduction.includes(forbidden)) fail(`${diagnosisPath} contains ${forbidden}`);
}

const keyring = requireMarkers(keyringPath, [
  'RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON',
  'keys: BTreeMap<String, SecretRef>',
  'const KEY_BYTES: usize = 32;',
  'const MAX_KEYS: usize = 16;',
  'SecretResolverRegistry',
  'SecretAccessPolicy::Exact',
  'resolve_for_tenant(DEPLOYMENT_SECRET_SCOPE, reference)',
  '<[u8; KEY_BYTES]>::try_from(decoded.as_slice())',
  'IndexSourceContinuationCodec::new',
  'finish_non_exhaustive()',
]);
const keyringProduction = keyring.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'DatabaseConnection',
  'ModuleRuntimeExtensions',
  'tokio::spawn',
  'spawn_blocking',
  'SELECT ',
  'INSERT ',
  'UPDATE ',
  'DELETE FROM',
  'tracing::',
]) {
  if (keyringProduction.includes(forbidden)) fail(`${keyringPath} contains ${forbidden}`);
}
if (keyringProduction.includes('SecretString') || keyringProduction.includes('Vec<u8>')) {
  fail(`${keyringPath} persists raw secret material instead of bounded references`);
}

const page = requireMarkers(pageDiagnosisPath, [
  'const MAX_SOURCE_PAGE_DIAGNOSIS_SIZE: usize = 32;',
  'pub struct IndexDriftSourcePageDiagnosisOutcome',
  'pub struct IndexDriftSourcePageDiagnosisSealedOutcome',
  'next_token: Option<rustok_index::IndexSourceContinuationToken>',
  'pub struct IndexDriftSourcePageDiagnosisRuntime',
  'continuation: Option<IndexSourceContinuationKeyringRuntime>',
  'pub async fn diagnose_source_page(',
  'pub async fn diagnose_source_page_sealed(',
  'authorize_context(context)?;',
  'validate_page_limit(limit)?;',
  'IndexSourceContinuationScope::from_registry(',
  '.resolve_codec()',
  'codec.open_encoded(&scope, encoded, Utc::now())',
  'IndexSourceScanRequest::new(',
  'let page = self.sources.scan(request).await?;',
  '.diagnose_missing_entity_candidate(context, key)',
  'codec.seal(&scope, cursor, Utc::now(), keyring.lifetime())',
  'sealed_outcome_replaces_raw_cursor_with_opaque_token',
]);
const pageProduction = page.split('\n#[cfg(test)]')[0];
const sealedStart = pageProduction.indexOf('    pub async fn diagnose_source_page_sealed(');
const sealedEnd = pageProduction.indexOf('\n    async fn diagnose_request(', sealedStart);
const sealed = pageProduction.slice(sealedStart, sealedEnd);
const pageAuth = sealed.indexOf('authorize_context(context)?;');
const pageLimit = sealed.indexOf('validate_page_limit(limit)?;', pageAuth);
const pageOpen = sealed.indexOf('codec.open_encoded(&scope, encoded, Utc::now())', pageLimit);
const pageRequest = sealed.indexOf('IndexSourceScanRequest::new(', pageOpen);
const pageDelegate = sealed.indexOf('self.diagnose_request(context, request).await?;', pageRequest);
const pageSeal = sealed.indexOf(
  'codec.seal(&scope, cursor, Utc::now(), keyring.lifetime())',
  pageDelegate,
);
if (
  sealedStart < 0 ||
  sealedEnd <= sealedStart ||
  pageAuth < 0 ||
  pageLimit <= pageAuth ||
  pageOpen <= pageLimit ||
  pageRequest <= pageOpen ||
  pageDelegate <= pageRequest ||
  pageSeal <= pageDelegate
) {
  fail(`${pageDiagnosisPath} sealed page ordering is invalid`);
}
if ((pageProduction.match(/self\.sources\.scan\(request\)/g) ?? []).length !== 1) {
  fail(`${pageDiagnosisPath} must scan exactly one source page`);
}
for (const forbidden of [
  'tokio::spawn',
  'spawn_blocking',
  'DatabaseConnection',
  'SELECT ',
  'INSERT ',
  'UPDATE ',
  'DELETE FROM',
  'while ',
  'loop {',
  'ModuleWorkScheduler',
  'repair_finding',
  'resolve_finding',
  'ignore_finding',
]) {
  if (pageProduction.includes(forbidden)) fail(`${pageDiagnosisPath} contains ${forbidden}`);
}

const continuation = requireMarkers(continuationPath, [
  'pub struct IndexSourceContinuationScope',
  'pub fn from_registry(',
  '.source_for_schema(&schema)',
  'pub struct IndexSourceContinuationToken(String);',
  'pub struct IndexSourceContinuationCodec',
  'Aes256Gcm::new_from_slice',
  'OsRng.fill_bytes(&mut nonce);',
  'pub fn seal(',
  'pub fn open_encoded(',
  'validate_claims(&claims, expected_scope, now)?;',
]);
const continuationProduction = continuation.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'DatabaseConnection',
  'sea_orm',
  'async_graphql',
  'std::env',
  'SecretResolverRegistry',
  'tokio::spawn',
]) {
  if (continuationProduction.includes(forbidden)) {
    fail(`${continuationPath} contains server dependency ${forbidden}`);
  }
}

const graphql = requireMarkers(graphqlTransportPath, [
  'pub struct IndexDriftDiagnosisInput',
  'async fn diagnose_index_entity(',
  'permissions_for(&tenant_id, &actor_id)',
  'let key = parse_entity_key(tenant_id, input)?;',
  '.diagnose_entity(operator_context, key)',
]);
const graphqlSchema = read(graphqlSchemaPath);
for (const forbidden of [
  'IndexDriftSourcePageDiagnosisRuntime',
  'IndexDriftSourcePageDiagnosisSealedOutcome',
  'IndexSourceCursor',
  'IndexSourceContinuationCodec',
  'IndexSourceContinuationToken',
  'diagnose_source_page_sealed',
  'RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON',
]) {
  if (graphql.includes(forbidden) || graphqlSchema.includes(forbidden)) {
    fail(`sealed source-page capability leaked into GraphQL: ${forbidden}`);
  }
}

requireMarkers(docsPath, [
  'Status: `sealed_source_page_source_complete_transport_and_owner_execution_pending`.',
  'effective `Permission::MODULES_MANAGE`',
  '`diagnose_entity(context, key)`',
  '`diagnose_missing_entity_candidate(context, key)`',
  '`diagnose_source_page_sealed(context, schema, continuation, limit)`',
  'server-owned continuation keyring',
  'not attached to GraphQL',
  '`SharedIndexSourceAbsenceRegistry`',
  'maintainer-run',
]);
requireMarkers(graphqlDocsPath, [
  'Status: `source_complete_owner_execution_pending`.',
  'diagnoseIndexEntity(input: $input)',
  'Tenant and actor identities are never accepted',
]);
requireMarkers(pageDocsPath, [
  'Status: `sealed_internal_source_complete_transport_and_owner_execution_pending`.',
  'one page limit in `1..=32`',
  'diagnose_source_page_sealed',
  'raw cursor is never returned',
  'RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON',
]);
requireMarkers(continuationDocsPath, [
  'Status: `source_complete_server_key_composition_pending`.',
  'AES-256-GCM',
]);
requireMarkers(serverContinuationDocsPath, [
  'Status: `source_complete_transport_and_owner_execution_pending`.',
  'SecretRef',
  'exactly 32 bytes',
  'diagnose_source_page_sealed',
]);
requireMarkers(recheckPath, [
  'Audited baseline: `main@368c79b78549e97a68120358021552b2552b800c`.',
  '`product-locale-absence-postgres`',
  'Confidential source continuation',
  'Server continuation keyring and sealed page boundary',
  'did not run tests,',
]);
requireMarkers(planPath, [
  'M6 guarded exact-entity drift diagnosis operator',
  'M6 bounded GraphQL exact-entity diagnosis transport',
  'M6 missing-only entity candidate outcome',
  'M6 bounded source-page missing-entity diagnosis',
  'M6 authenticated and confidential source continuation codec',
  'M6 server-owned source continuation keyring and sealed page boundary',
  'source_complete_transport_and_owner_execution_pending',
  '[x] Compose a server-owned continuation keyring from bounded secret references',
  '[x] Add a sealed internal page method',
  'M6 - expose only the sealed source-page boundary through bounded transport',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-server-reconciliation-guard.mjs'",
  "'verify-index-drift-source-page-diagnosis.mjs'",
  "'verify-index-source-continuation.mjs'",
  "'verify-index-source-continuation-server.mjs'",
]);

console.log('[verify-index-server-reconciliation-guard] OK');