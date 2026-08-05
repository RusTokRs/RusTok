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
const graphqlTransportPath = 'apps/server/src/graphql/index_drift_diagnosis.rs';
const docsPath = 'apps/server/docs/index-reconciliation-operator-runtime.md';
const graphqlDocsPath = 'apps/server/docs/index-drift-diagnosis-graphql-transport.md';
const pageDocsPath = 'apps/server/docs/index-drift-source-page-diagnosis.md';
const planPath = 'crates/rustok-index/docs/implementation-plan-current-2026-08-03.md';
const recheckPath =
  'crates/rustok-index/docs/implementation-recheck-2026-08-05-explicit-absence-watermark.md';

const composition = requireMarkers(compositionPath, [
  '#[path = "index_reconciliation_operator.rs"]',
  'mod reconciliation_operator;',
  '#[path = "index_drift_diagnosis_operator.rs"]',
  'mod drift_diagnosis_operator;',
  '#[path = "index_drift_source_page_diagnosis.rs"]',
  'mod drift_source_page_diagnosis;',
  'IndexDriftDiagnosisOperatorError, IndexDriftDiagnosisOperatorRuntime,',
  'IndexDriftSourcePageDiagnosisRuntime,',
  'materialize_postgres_index_replay_runtime(extensions, db.clone())',
  'reconciliation_operator::materialize_index_reconciliation_operator(extensions, db.clone())?;',
  'drift_diagnosis_operator::materialize_index_drift_diagnosis_operator(extensions, db)?;',
  'drift_source_page_diagnosis::materialize_index_drift_source_page_diagnosis(extensions)?;',
  'extensions.contains::<IndexDriftDiagnosisOperatorRuntime>()',
  'extensions.contains::<IndexDriftSourcePageDiagnosisRuntime>()',
]);
const replay = composition.indexOf(
  'materialize_postgres_index_replay_runtime(extensions, db.clone())',
);
const reconciliation = composition.indexOf(
  'reconciliation_operator::materialize_index_reconciliation_operator(extensions, db.clone())?;',
);
const diagnosis = composition.indexOf(
  'drift_diagnosis_operator::materialize_index_drift_diagnosis_operator(extensions, db)?;',
);
const pageDiagnosis = composition.indexOf(
  'drift_source_page_diagnosis::materialize_index_drift_source_page_diagnosis(extensions)?;',
);
if (
  replay < 0 ||
  reconciliation <= replay ||
  diagnosis <= reconciliation ||
  pageDiagnosis <= diagnosis
) {
  fail(
    `${compositionPath} must freeze replay before reconciliation, exact diagnosis, and page diagnosis publication`,
  );
}

const operator = requireMarkers(operatorPath, [
  'pub struct IndexReconciliationOperatorContext',
  'tenant_id.is_nil() || actor_id.is_nil()',
  'permissions_for(&self.tenant_id, &self.actor_id)',
  'Permission::MODULES_MANAGE',
  'pub struct IndexReconciliationOperatorRuntime',
  'inner: rustok_index::PostgresIndexReconciliationRunner,',
  'PostgresIndexReconciliationDeadLetterInspector,',
  'PostgresIndexDriftFindingInspector,',
  'PostgresIndexReconciliationRecoveryStore,',
  'DriftInspection(#[from] rustok_index::infrastructure::postgres::IndexDriftFindingInspectionError)',
  'context.authorize_for(request.tenant_id())?;',
  'pub async fn request_cancel(',
  'pub async fn inspect_dead_letter(',
  'pub async fn inspect_drift_finding(',
  'pub async fn requeue_dead_letter(',
  'IndexReconciliationRequeueRequest::new(',
  'context.tenant_id(),',
  'context.actor_id(),',
  '.requeue_failed(request)',
  'drift_finding_inspection_authorizes_before_adapter_validation',
]);
const production = operator.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'tokio::spawn',
  'spawn_blocking',
  'SELECT ',
  'INSERT ',
  'UPDATE ',
  'DELETE ',
  'Router::new',
  'async_graphql',
  'ModuleWorkScheduler',
  'StopHandle',
  'PostgresIndexDriftSnapshotReader',
  'IndexDriftDigestProducer',
  'PostgresIndexDriftFindingWriter',
  'SharedIndexSourceAbsenceRegistry',
]) {
  if (production.includes(forbidden)) fail(`${operatorPath} contains ${forbidden}`);
}

const runStart = production.indexOf('    pub async fn run(');
const cancelStart = production.indexOf('    pub async fn request_cancel(', runStart);
const deadLetterStart = production.indexOf('    pub async fn inspect_dead_letter(', cancelStart);
const driftStart = production.indexOf('    pub async fn inspect_drift_finding(', deadLetterStart);
const requeueStart = production.indexOf('    pub async fn requeue_dead_letter(', driftStart);
const runtimeEnd = production.indexOf('\n}\n\nimpl fmt::Debug', requeueStart);
if (
  [runStart, cancelStart, deadLetterStart, driftStart, requeueStart, runtimeEnd].some(
    (value) => value < 0,
  )
) {
  fail(`${operatorPath} guarded method segments are incomplete`);
}
const run = production.slice(runStart, cancelStart);
const cancel = production.slice(cancelStart, deadLetterStart);
const deadLetter = production.slice(deadLetterStart, driftStart);
const drift = production.slice(driftStart, requeueStart);
const requeue = production.slice(requeueStart, runtimeEnd);
if (
  run.indexOf('context.authorize_for(request.tenant_id())?;') < 0 ||
  run.indexOf('self.inner.run(request)') <=
    run.indexOf('context.authorize_for(request.tenant_id())?;')
) {
  fail(`${operatorPath} run must authorize before delegation`);
}
for (const [name, body, marker] of [
  ['cancel', cancel, '.request_cancel(context.tenant_id(), job_id)'],
  ['dead-letter inspection', deadLetter, '.inspect(context.tenant_id(), job_id)'],
  ['drift-finding inspection', drift, '.inspect(context.tenant_id(), finding_id)'],
]) {
  const auth = body.indexOf('context.authorize_for(context.tenant_id())?;');
  const delegate = body.indexOf(marker);
  if (body.includes('tenant_id: Uuid') || auth < 0 || delegate <= auth) {
    fail(`${operatorPath} ${name} must bind tenant to authorized context`);
  }
}
const requeueAuth = requeue.indexOf('context.authorize_for(context.tenant_id())?;');
const requeueRequest = requeue.indexOf('IndexReconciliationRequeueRequest::new(');
const requeueTenant = requeue.indexOf('context.tenant_id(),', requeueRequest);
const requeueActor = requeue.indexOf('context.actor_id(),', requeueTenant);
const requeueDelegate = requeue.indexOf('.requeue_failed(request)');
if (
  requeueAuth < 0 ||
  requeueRequest <= requeueAuth ||
  requeueTenant <= requeueRequest ||
  requeueActor <= requeueTenant ||
  requeueDelegate <= requeueActor
) {
  fail(`${operatorPath} requeue must authorize, bind tenant/actor, then delegate`);
}

const driftComposition = production.indexOf('let drift_findings =');
const driftConstructor = production.indexOf(
  'PostgresIndexDriftFindingInspector::new(db.clone())',
  driftComposition,
);
const runnerComposition = production.indexOf(
  'PostgresIndexReconciliationRunner::new(db, sources, schemas.shared())',
);
const runtimeComposition = production.indexOf(
  'IndexReconciliationOperatorRuntime::new(',
  runnerComposition,
);
const driftArgument = production.indexOf('drift_findings,', runtimeComposition);
if (
  driftComposition < 0 ||
  driftConstructor <= driftComposition ||
  runnerComposition <= driftConstructor ||
  runtimeComposition <= runnerComposition ||
  driftArgument <= runtimeComposition
) {
  fail(`${operatorPath} must privately compose drift inspection before runtime publication`);
}

const diagnosisSource = requireMarkers(diagnosisPath, [
  'type IndexDriftDiagnosisProducer = rustok_index::IndexDriftDigestProducer<',
  'rustok_index::PostgresIndexDriftSnapshotReader,',
  'PostgresIndexDriftFindingWriter,',
  'pub struct IndexDriftDiagnosisOperatorRuntime',
  'inner: Arc<IndexDriftDiagnosisProducer>,',
  'pub async fn diagnose_entity(',
  'authorize_for(&context, key.tenant_id)?;',
  'IndexDriftDigestRequest::new(key)?;',
  'self.inner.produce(request).await',
  'Permission::MODULES_MANAGE',
  'permissions_for(&context.tenant_id(), &context.actor_id())',
  'materialize_index_source_absence_registry(extensions)',
  'SharedIndexSourceAbsenceRegistry',
  'extensions.insert(absence);',
  'PostgresIndexDriftSnapshotReader::new(',
  'reader.with_absence_registry(absence)',
  'PostgresIndexDriftFindingWriter::new(db)',
  'IndexDriftDigestProducer::new(',
  'exact_entity_diagnosis_authorizes_before_request_validation',
  'IndexDriftDigestError::NilEntityId',
]);
const diagnosisProduction = diagnosisSource.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'tokio::spawn',
  'spawn_blocking',
  'SELECT ',
  'INSERT ',
  'UPDATE ',
  'DELETE ',
  'Router::new',
  'async_graphql',
  '.scan(',
  'repair_finding',
  'targeted_repair',
  'resolve_finding',
  'ignore_finding',
]) {
  if (diagnosisProduction.includes(forbidden)) fail(`${diagnosisPath} contains ${forbidden}`);
}
const methodStart = diagnosisProduction.indexOf('    pub async fn diagnose_entity(');
const methodEnd = diagnosisProduction.indexOf('\n    }\n}', methodStart);
if (methodStart < 0 || methodEnd < 0) fail(`${diagnosisPath} diagnosis method is incomplete`);
const method = diagnosisProduction.slice(methodStart, methodEnd);
const diagnosisAuth = method.indexOf('authorize_for(&context, key.tenant_id)?;');
const diagnosisRequest = method.indexOf('IndexDriftDigestRequest::new(key)?;');
const diagnosisDelegate = method.indexOf('self.inner.produce(request).await');
if (
  diagnosisAuth < 0 ||
  diagnosisRequest <= diagnosisAuth ||
  diagnosisDelegate <= diagnosisRequest
) {
  fail(`${diagnosisPath} must authorize, validate one exact key, then delegate`);
}
if (method.includes('tenant_id:') || method.includes('actor_id:') || method.includes('Vec<')) {
  fail(`${diagnosisPath} must not accept caller-selected authority or batch scope`);
}

const absenceMaterialization = diagnosisProduction.indexOf(
  'materialize_index_source_absence_registry(extensions)',
);
const absenceInsert = diagnosisProduction.indexOf(
  'extensions.insert(absence);',
  absenceMaterialization,
);
const readerConstructor = diagnosisProduction.indexOf(
  'PostgresIndexDriftSnapshotReader::new(',
  absenceInsert,
);
const readerAttachment = diagnosisProduction.indexOf(
  'reader.with_absence_registry(absence)',
  readerConstructor,
);
const producerConstructor = diagnosisProduction.indexOf(
  'IndexDriftDigestProducer::new(',
  readerConstructor,
);
if (
  absenceMaterialization < 0 ||
  absenceInsert <= absenceMaterialization ||
  readerConstructor <= absenceInsert ||
  readerAttachment <= readerConstructor ||
  producerConstructor <= readerConstructor
) {
  fail(`${diagnosisPath} must freeze optional absence evidence before producer publication`);
}

const pageSource = requireMarkers(pageDiagnosisPath, [
  'const MAX_SOURCE_PAGE_DIAGNOSIS_SIZE: usize = 32;',
  'pub struct IndexDriftSourcePageDiagnosisRuntime',
  'sources: rustok_index::SharedIndexSourceRegistry',
  'exact: IndexDriftDiagnosisOperatorRuntime',
  'pub async fn diagnose_source_page(',
  'permissions_for(&context.tenant_id(), &context.actor_id())',
  'IndexSourceScanRequest::new(context.tenant_id(), schema, cursor, limit)',
  'let page = self.sources.scan(request).await?;',
  'matches!(&mutation, rustok_index::IndexMutation::Delete { .. })',
  '.diagnose_entity(context, key)',
  'materialize_index_drift_source_page_diagnosis',
  'one_page_skips_deletes_and_diagnoses_each_upsert_once',
]);
const pageProduction = pageSource.split('\n#[cfg(test)]')[0];
const pageAuth = pageProduction.indexOf(
  'let permissions = permissions_for(&context.tenant_id(), &context.actor_id())',
);
const pageLimit = pageProduction.indexOf(
  'if !(1..=MAX_SOURCE_PAGE_DIAGNOSIS_SIZE).contains(&limit)',
  pageAuth,
);
const pageRequest = pageProduction.indexOf(
  'IndexSourceScanRequest::new(context.tenant_id(), schema, cursor, limit)',
  pageLimit,
);
const pageScan = pageProduction.indexOf('let page = self.sources.scan(request).await?;');
if (pageAuth < 0 || pageLimit <= pageAuth || pageRequest <= pageLimit || pageScan <= pageRequest) {
  fail(`${pageDiagnosisPath} must authorize, validate, and then scan one page`);
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

const graphqlTransport = requireMarkers(graphqlTransportPath, [
  'pub struct IndexDriftDiagnosisInput',
  'pub struct IndexDriftDiagnosisMutation',
  'async fn diagnose_index_entity(',
  'prepare_authorized_request(tenant.id, auth.user_id, input)',
  'permissions_for(&tenant_id, &actor_id)',
  'let key = parse_entity_key(tenant_id, input)?;',
  '.diagnose_entity(operator_context, key)',
  'transport_authorizes_before_parsing_untrusted_input',
]);
const graphqlProduction = graphqlTransport.split('\n#[cfg(test)]')[0];
const graphqlPermission = graphqlProduction.indexOf(
  'let permissions = permissions_for(&tenant_id, &actor_id)',
);
const graphqlParse = graphqlProduction.indexOf('let key = parse_entity_key(tenant_id, input)?;');
if (graphqlPermission < 0 || graphqlParse <= graphqlPermission) {
  fail(`${graphqlTransportPath} must authorize before parsing untrusted identity strings`);
}
for (const forbidden of [
  'DatabaseConnection',
  'sea_orm',
  'tokio::spawn',
  '.scan(',
  'IndexDriftSourcePageDiagnosisRuntime',
  'IndexSourceCursor',
  'repair_finding',
  'resolve_finding',
  'ignore_finding',
  'Vec<IndexDriftDiagnosisInput>',
]) {
  if (graphqlProduction.includes(forbidden)) {
    fail(`${graphqlTransportPath} contains ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/replay_runtime.rs', [
  'register_postgres_index_reconciliation_work(extensions)?;',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/source_reconciliation_scheduler.rs', [
  'impl ModuleWorkRegistration for IndexReconciliationWorkRegistration',
  'PostgresIndexReconciliationRunner',
]);
requireMarkers(docsPath, [
  'Status: `source_page_diagnosis_source_complete_transport_and_owner_execution_pending`.',
  'effective `Permission::MODULES_MANAGE`',
  '`inspect_drift_finding(context, finding_id)`',
  '`IndexDriftDiagnosisOperatorRuntime`',
  '`diagnose_entity(context, key)`',
  '`IndexDriftSourcePageDiagnosisRuntime`',
  '`diagnose_source_page(context, schema, cursor, limit)`',
  'maximum page size of',
  'not claimed as missing-only diagnosis',
  '`diagnoseIndexEntity(input: IndexDriftDiagnosisInput!)`',
  '`SharedIndexSourceAbsenceRegistry`',
  'positive `products.index_revision`',
  'index_drift_source_changed_during_capture',
  'index_drift_source_watermark_missing',
  'maintainer-run',
]);
requireMarkers(graphqlDocsPath, [
  'Status: `source_complete_owner_execution_pending`.',
  'diagnoseIndexEntity(input: $input)',
  'Tenant and actor identities are never accepted',
  'before parsing module/entity identifiers',
  'delegates once to `diagnose_entity(context, key)`',
]);
requireMarkers(pageDocsPath, [
  'Status: `source_complete_transport_and_owner_execution_pending`.',
  'one page limit in `1..=32`',
  'skips retained source `Delete` mutations',
  'The cursor is not attached to GraphQL',
  'missing-only selector over captured typed states remains a separate open slice',
]);
requireMarkers(recheckPath, [
  'Audited baseline: `main@368c79b78549e97a68120358021552b2552b800c`.',
  '`product-locale-absence-postgres`',
  'absence version is domain-tagged into the opaque boundary only for source `Missing`',
  'index_drift_source_changed_during_capture',
  'index_drift_source_watermark_missing',
  'Bounded GraphQL transport continuation',
  'did not run tests,',
]);
requireMarkers(planPath, [
  'M6 guarded exact-entity drift diagnosis operator',
  'M6 bounded GraphQL exact-entity diagnosis transport',
  'M6 bounded source-page drift candidate diagnosis',
  'source_complete_transport_and_owner_execution_pending',
  '[x] Register the Product locale absence provider',
  '[x] Reload and compare the exact positive absence version',
  '[x] Add the source-ready real-migration Product locale-absence scenario',
  '[x] Expose exact-entity diagnosis through bounded GraphQL `diagnoseIndexEntity`',
  '[x] Add one internal server-owned source-page candidate diagnosis runtime',
  'M6 - add missing-only candidate classification without exposing snapshot state',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-server-reconciliation-guard.mjs'",
  "'verify-index-drift-diagnosis-graphql-transport.mjs'",
  "'verify-index-drift-source-page-diagnosis.mjs'",
  "'verify-index-reconciliation-host-scheduler.mjs'",
  "'verify-index-drift-finding-inspection.mjs'",
  "'verify-index-source-absence-watermark.mjs'",
]);

console.log('[verify-index-server-reconciliation-guard] OK');