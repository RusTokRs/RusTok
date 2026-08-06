#!/usr/bin/env node

import { readFile } from 'node:fs/promises';

const files = {
  source: 'crates/rustok-index/src/infrastructure/postgres/drift_candidate_reader.rs',
  postgresMod: 'crates/rustok-index/src/infrastructure/postgres/mod.rs',
  lib: 'crates/rustok-index/src/lib.rs',
  doc: 'crates/rustok-index/docs/m6-postgres-drift-candidate-reader.md',
  plan: 'crates/rustok-index/docs/implementation-plan-current-2026-08-03.md',
  aggregate: 'scripts/verify/verify-index-query-contract.mjs',
};

const content = Object.fromEntries(
  await Promise.all(
    Object.entries(files).map(async ([name, path]) => [name, await readFile(path, 'utf8')]),
  ),
);

function requireMarkers(name, markers) {
  for (const marker of markers) {
    if (!content[name].includes(marker)) throw new Error(`${files[name]} missing ${marker}`);
  }
}

requireMarkers('postgresMod', [
  'mod drift_candidate_reader;',
  'PostgresIndexDriftCandidateReader',
  'materialize_postgres_index_drift_candidate_reader',
]);
requireMarkers('lib', [
  'PostgresIndexDriftCandidateReader',
  'IndexDriftCandidateCompositionError',
  'materialize_postgres_index_drift_candidate_reader',
]);
requireMarkers('source', [
  'pub struct PostgresIndexDriftCandidateReader',
  'impl IndexDriftCandidateReader for PostgresIndexDriftCandidateReader',
  'Some(IsolationLevel::RepeatableRead)',
  'Some(AccessMode::ReadOnly)',
  'SELECT txid_current_snapshot()::text AS snapshot_token',
  'txid_visible_in_snapshot((e.xmin::text)::bigint, $5::txid_snapshot)',
  'txid_visible_in_snapshot((l.xmin::text)::bigint, $5::txid_snapshot)',
  'txid_visible_in_snapshot((s.xmin::text)::bigint, $5::txid_snapshot)',
  'txid_visible_in_snapshot((t.xmin::text)::bigint, $5::txid_snapshot)',
  't.is_deleted = TRUE AND txid_visible_in_snapshot',
  'const SCOPE_DIGEST_DOMAIN: &[u8] = b"index_drift_candidate_scope_v1";',
  'wire.scope_digest != scope_digest(request.scope())',
  'request.limit() + 1',
  'remaining + 1',
  'ORDER BY entity_id ASC, locale_key ASC',
  'ORDER BY l.source_entity_id ASC, l.source_locale_key ASC, l.link_name ASC, l.ordinal ASC',
  'AND (entity_id, locale_key) > ($6::uuid, $7)',
  'AND is_deleted = FALSE AND source_version > 0',
  'CursorPhaseWire::Stale',
  'CursorPhaseWire::Orphan',
  'validate_snapshot_token(',
  'URL_SAFE_NO_PAD.encode',
  'URL_SAFE_NO_PAD.decode',
  'materialize_postgres_index_drift_candidate_reader(',
  'snapshot_token_validation_is_bounded_and_canonical',
]);

const production = content.source.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'INSERT INTO',
  'UPDATE index_',
  'DELETE FROM',
  'FOR UPDATE',
  'pg_advisory',
  'tokio::spawn',
  'SharedIndexSourceRegistry',
  'IndexSourceLoadRequest',
  'record_mismatch',
  'resolve_finding',
  'ignore_finding',
  'IndexDriftRepairService',
  'PostgresMutationStore',
  'async_graphql',
  'Router::new',
  'ModuleRuntimeExtensions',
]) {
  if (production.includes(forbidden)) {
    throw new Error(`PostgreSQL drift candidate reader contains forbidden capability: ${forbidden}`);
  }
}
for (const forbiddenSql of ['SELECT *', ' OFFSET ', 'LIMIT 1000']) {
  if (production.includes(forbiddenSql)) throw new Error(`forbidden candidate SQL: ${forbiddenSql}`);
}

const begin = production.indexOf('.begin_with_config(');
const read = production.indexOf('self.read_in_transaction(&transaction, &request)', begin);
const commit = production.indexOf('.commit()', read);
if (begin < 0 || read <= begin || commit <= read) {
  throw new Error('candidate page must begin read-only transaction, read, then commit');
}

const page = production.indexOf('async fn read_in_transaction(');
const fence = production.indexOf('resolve_fence(transaction, request).await?', page);
const cursor = production.indexOf('decode_request_phase(request)?', fence);
const stale = production.indexOf('load_stale_rows(', cursor);
const orphan = production.indexOf('append_orphan_rows(', stale);
const construct = production.indexOf('IndexDriftCandidatePage::new(', orphan);
if (page < 0 || fence <= page || cursor <= fence || stale <= cursor || orphan <= stale || construct <= orphan) {
  throw new Error('candidate reader must fence, decode, scan stale/orphan, then construct page');
}

requireMarkers('doc', [
  'Status: `source_complete_downstream_repair_composition_complete`.',
  '`REPEATABLE READ READ ONLY`',
  '`txid_current_snapshot()::text`',
  'domain-separated SHA-256 digest',
  'inside 512 bytes',
  '`txid_visible_in_snapshot`',
  '`limit + 1`',
  'one concrete missing-entity repair path',
  'reader itself still has no source call',
]);
requireMarkers('plan', [
  'M6 - add prepared repair recovery policy',
  'M6 PostgreSQL drift candidate reader: `source_complete`',
  'source_complete_recovery_policy_pending',
]);
requireMarkers('aggregate', [
  "'verify-index-postgres-drift-candidate-reader.mjs'",
  "'verify-index-missing-entity-repair-composition.mjs'",
]);

console.log('Index PostgreSQL drift candidate reader verified');
