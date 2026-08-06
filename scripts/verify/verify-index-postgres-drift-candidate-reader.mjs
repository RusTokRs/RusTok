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
    if (!content[name].includes(marker)) {
      throw new Error(`${files[name]} missing ${marker}`);
    }
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
  'scope_digest: String',
  'wire.scope_digest != scope_digest(request.scope())',
  'scope_digest: scope_digest(request.scope())',
  'request.limit() + 1',
  'remaining + 1',
  'load_stale_rows(',
  'load_orphan_rows(',
  'ORDER BY entity_id ASC, locale_key ASC',
  'ORDER BY l.source_entity_id ASC, l.source_locale_key ASC, l.link_name ASC, l.ordinal ASC',
  'AND (entity_id, locale_key) > ($6::uuid, $7)',
  'AND (l.source_entity_id, l.source_locale_key, l.link_name, l.ordinal',
  'AND is_deleted = FALSE AND source_version > 0',
  'CursorPhaseWire::Stale',
  'CursorPhaseWire::Orphan',
  'validate_wire_scope(',
  'validate_snapshot_token(',
  'URL_SAFE_NO_PAD.encode',
  'URL_SAFE_NO_PAD.decode',
  'index_drift_candidate_storage_unavailable',
  'index_drift_candidate_cursor_invalid',
  'index_drift_candidate_fence_invalid',
  'index_drift_candidate_materialized_invalid',
  'materialize_postgres_index_drift_candidate_reader(',
  'snapshot_token_validation_is_bounded_and_canonical',
  'compact_fence_remains_scope_bound',
  'cursor_is_scope_bound_and_phase_typed',
]);

const production = content.source.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'INSERT INTO',
  'UPDATE index_',
  'DELETE FROM',
  'FOR UPDATE',
  'pg_advisory',
  'tokio::spawn',
  'spawn_blocking',
  'SharedIndexSourceRegistry',
  'IndexSourceLoadRequest',
  'PostgresIndexDriftFindingWriter',
  'record_mismatch',
  'resolve_finding',
  'ignore_finding',
  'repair_finding',
  'async_graphql',
  'Router::new',
  'ModuleRuntimeExtensions',
]) {
  if (production.includes(forbidden)) {
    throw new Error(`PostgreSQL drift candidate reader contains forbidden capability: ${forbidden}`);
  }
}

for (const forbiddenSql of ['SELECT *', ' OFFSET ', 'LIMIT 1000']) {
  if (production.includes(forbiddenSql)) {
    throw new Error(`PostgreSQL drift candidate reader contains forbidden SQL shape: ${forbiddenSql}`);
  }
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
  throw new Error('candidate reader must fence, decode, scan stale/orphan phases, then construct page');
}

const staleSql = production.indexOf('FROM index_entities e');
const staleLimit = production.indexOf('LIMIT $6', staleSql);
const orphanSql = production.indexOf('FROM index_links l JOIN index_entities s', staleLimit);
const targetFence = production.indexOf('txid_visible_in_snapshot((t.xmin::text)::bigint', orphanSql);
const orphanLimit = production.indexOf('LIMIT $6', targetFence);
if (
  staleSql < 0 ||
  staleLimit <= staleSql ||
  orphanSql <= staleLimit ||
  targetFence <= orphanSql ||
  orphanLimit <= targetFence
) {
  throw new Error('candidate reader must retain bounded stale, source, target, and orphan SQL');
}

requireMarkers('doc', [
  'Status: `source_complete_candidate_confirmation_pending`.',
  '`REPEATABLE READ READ ONLY`',
  '`txid_current_snapshot()::text`',
  'domain-separated SHA-256 digest',
  'keeps the fence inside its 512-byte contract',
  'deleted target rows must have an',
  'current target row whose',
  'post-fence is skipped',
  '`limit + 1`',
  'does not record a finding',
  'candidate confirmation boundary',
  'No tests, verifiers, formatting, Cargo checks',
]);
requireMarkers('plan', [
  'M6 - confirm bounded stale and orphan candidates',
  'M6 PostgreSQL drift candidate reader',
  'source_complete_candidate_confirmation_pending',
]);
requireMarkers('aggregate', [
  "'verify-index-postgres-drift-candidate-reader.mjs'",
]);

console.log('Index PostgreSQL drift candidate reader verified');
