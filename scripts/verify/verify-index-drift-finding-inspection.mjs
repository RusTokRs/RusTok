#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-drift-finding-inspection] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const inspectorPath =
  'crates/rustok-index/src/infrastructure/postgres/drift_finding_inspector.rs';
const inspector = requireMarkers(inspectorPath, [
  'pub enum IndexDriftFindingSeverity',
  'pub enum IndexDriftFindingScope',
  'EntityWithoutLocale',
  'pub struct IndexDriftFindingInspection',
  'pub struct PostgresIndexDriftFindingInspector',
  'pub async fn inspect(',
  'if tenant_id.is_nil()',
  'if finding_id.is_nil()',
  'fn decode_open_finding(',
  'fn decode_scope(',
  'fn decode_schema(',
  'fn validate_check_name(',
  'fn validate_digest(',
  'fn select_open_finding_sql(',
  'IndexDriftFindingInspectionError::InvalidStoredFinding',
  'inspection_is_tenant_scoped_open_and_bounded',
  'inspection_fails_closed_on_invalid_digest',
  'inspection_fails_closed_on_scope_mismatch',
]);

const production = inspector.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'INSERT INTO',
  'UPDATE ',
  'DELETE ',
  'tokio::spawn',
  'spawn_blocking',
  'tokio::time::sleep',
  'Router::new',
  'async_graphql',
  'PostgresMutationStore',
  '.sources.scan(',
  '.sources.load(',
  'repair_finding',
  'resolve_finding',
]) {
  if (production.includes(forbidden)) {
    fail(`${inspectorPath} production boundary contains forbidden marker ${forbidden}`);
  }
}

const sqlStart = production.indexOf('fn select_open_finding_sql(');
const errorStart = production.indexOf('\n#[derive(Debug, Error', sqlStart);
if (sqlStart < 0 || errorStart <= sqlStart) {
  fail(`${inspectorPath} bounded SELECT segment is missing`);
}
const sql = production.slice(sqlStart, errorStart);
for (const marker of [
  'SELECT finding_key, check_name, severity, scope_kind, module_name, entity_name,',
  'schema_version_value, entity_id, locale_key, expected_digest, actual_digest',
  'FROM index_consistency_findings',
  'WHERE tenant_id = {prefix}1',
  'finding_id = {prefix}2',
  "state = 'open'",
  'LIMIT 1',
]) {
  if (!sql.includes(marker)) fail(`${inspectorPath} SELECT is missing ${marker}`);
}
for (const forbidden of [
  'SELECT tenant_id',
  'details',
  'first_detected_at',
  'last_detected_at',
  'closed_at',
  'job_id',
  'lease_owner',
  'cursor',
]) {
  if (sql.includes(forbidden)) {
    fail(`${inspectorPath} SELECT exposes forbidden field ${forbidden}`);
  }
}

for (const marker of [
  'value.len() != DIGEST_BYTES',
  "matches!(byte, b'a'..=b'f')",
  'value.len() > MAX_CHECK_NAME_BYTES',
  'value.trim() != value',
  'value.chars().any(char::is_control)',
  '"global" =>',
  '"schema" =>',
  '"entity" =>',
  'entity_id.filter(|value| !value.is_nil())',
  'match locale_key',
  'LocaleKey::new(&stored_locale)',
  'locale.as_str() != stored_locale',
  'None => Ok(IndexDriftFindingScope::EntityWithoutLocale',
  'version == 0',
]) {
  if (!production.includes(marker)) {
    fail(`${inspectorPath} fail-closed decoding is missing ${marker}`);
  }
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod drift_finding_inspector;',
  'pub use drift_finding_inspector::{',
  'IndexDriftFindingInspection, IndexDriftFindingInspectionError, IndexDriftFindingScope,',
  'IndexDriftFindingSeverity, PostgresIndexDriftFindingInspector,',
]);

const operatorPath = 'apps/server/src/services/index_reconciliation_operator.rs';
const operator = requireMarkers(operatorPath, [
  'PostgresIndexDriftFindingInspector,',
  'DriftInspection(#[from] rustok_index::infrastructure::postgres::IndexDriftFindingInspectionError)',
  'pub async fn inspect_drift_finding(',
  'Option<rustok_index::infrastructure::postgres::IndexDriftFindingInspection>',
  'self.drift_findings',
  '.inspect(context.tenant_id(), finding_id)',
  'PostgresIndexDriftFindingInspector::new(db.clone())',
  'drift_finding_inspection_authorizes_before_adapter_validation',
  'IndexDriftFindingInspectionError::NilFindingId',
]);
const operatorProduction = operator.split('\n#[cfg(test)]')[0];
const driftStart = operatorProduction.indexOf('    pub async fn inspect_drift_finding(');
const requeueStart = operatorProduction.indexOf('    pub async fn requeue_dead_letter(', driftStart);
if (driftStart < 0 || requeueStart <= driftStart) {
  fail(`${operatorPath} drift inspection method segment is missing`);
}
const drift = operatorProduction.slice(driftStart, requeueStart);
const authorization = drift.indexOf('context.authorize_for(context.tenant_id())?;');
const delegation = drift.indexOf('.inspect(context.tenant_id(), finding_id)');
if (drift.includes('tenant_id: Uuid') || authorization < 0 || delegation <= authorization) {
  fail(`${operatorPath} drift inspection must authorize before tenant-bound delegation`);
}
for (const forbidden of [
  'SELECT ',
  'INSERT ',
  'UPDATE ',
  'DELETE ',
  'details',
  'first_detected_at',
  'last_detected_at',
  'closed_at',
  'PostgresMutationStore',
  'repair_finding',
  'resolve_finding',
]) {
  if (drift.includes(forbidden)) {
    fail(`${operatorPath} drift inspection contains forbidden marker ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/docs/m6-drift-finding-inspection.md', [
  'Status: `source_complete_server_authorized_snapshot_reader_and_repair_pending`.',
  '`PostgresIndexDriftFindingInspector`',
  "`state = 'open'`",
  'raw `details` JSON',
  '`EntityWithoutLocale`',
  '`inspect_drift_finding(context, finding_id)`',
  'requires effective `modules:manage`',
  'Missing request authority and `modules:read` fail before nil-finding validation or database access.',
  'No automatic finding closure or mutation is allowed from inspection alone.',
  'authoritative production source/index snapshot reader composition',
  'The canonical roadmap item `Add drift diagnosis, targeted repair commands, and admitted repair evidence` remains open.',
  'maintainer-run',
]);
requireMarkers('apps/server/docs/index-reconciliation-operator-runtime.md', [
  '`PostgresIndexDriftFindingInspector` for bounded read-only open-finding diagnosis',
  '`inspect_drift_finding(context, finding_id)`',
  'Both inspection methods and requeue authorize before adapter or recovery-request validation',
  'Drift inspection is read-only and is not scheduled.',
]);
requireMarkers('crates/rustok-index/docs/README.md', [
  '[M6 Drift Finding Inspection](./m6-drift-finding-inspection.md)',
  '[M6 Locale-Optional Drift-Finding Scope](./m6-drift-finding-locale-scope.md)',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
  '- [ ] Add in-page interruption/timeouts, dry-run, and targeted/full/shadow rebuild modes.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-drift-finding-inspection.mjs'",
  "'verify-index-server-reconciliation-guard.mjs'",
]);

console.log('[verify-index-drift-finding-inspection] OK');
