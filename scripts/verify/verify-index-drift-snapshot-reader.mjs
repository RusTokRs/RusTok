#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  reader: "crates/rustok-index/src/infrastructure/postgres/drift_snapshot_reader.rs",
  test: "crates/rustok-index/tests/drift_snapshot_reader_postgres_test.rs",
  diagnosis: "apps/server/src/services/index_drift_diagnosis_operator.rs",
  productProvider: "crates/rustok-distribution/src/product_index/absence.rs",
  doc: "crates/rustok-index/docs/m6-postgres-drift-snapshot-reader.md",
  recheck: "crates/rustok-index/docs/implementation-recheck-2026-08-05-explicit-absence-watermark.md",
  plan: "crates/rustok-index/docs/implementation-plan-current-2026-08-03.md",
};

const c = Object.fromEntries(
  await Promise.all(
    Object.entries(files).map(async ([name, path]) => [name, await readFile(path, "utf8")]),
  ),
);

function requireMarkers(name, markers) {
  for (const marker of markers) {
    if (!c[name].includes(marker)) throw new Error(`${files[name]} missing ${marker}`);
  }
}

requireMarkers("reader", [
  "pub struct PostgresIndexDriftSnapshotReader",
  "impl IndexDriftSnapshotReader for PostgresIndexDriftSnapshotReader",
  "SharedIndexSourceAbsenceRegistry",
  "IndexSourceLoadRequest::new(vec![request.key().clone()])",
  "load_source_observation",
  "absence.provider_for_schema(&request.key().schema)",
  "IndexDriftSourceObservation::missing(",
  "index_drift_source_watermark_missing",
  "index_drift_source_changed_during_capture",
  "begin_with_config(",
  "IsolationLevel::RepeatableRead",
  "AccessMode::ReadOnly",
  "SELECT txid_current_snapshot()::text AS snapshot_token",
  "let observed_again = match self.load_source_observation(request).await {",
  "source.absence_source_version.is_some()",
  "error.code() == SOURCE_WATERMARK_MISSING",
  "return Err(retryable_failure(SOURCE_CHANGED));",
  "if &observed_again != source",
  "FROM index_entities WHERE tenant_id = $1",
  "FROM index_links WHERE tenant_id = $1",
  "fingerprint != registered.fingerprint.to_string()",
  "targets.sort_by_key(|(ordinal, _)| *ordinal)",
  "index_drift_postgres_source_version_boundary_v1",
  "explicit_source_absence_watermark_v1",
  "source.absence_source_version",
  "source_version.to_be_bytes()",
  "pub fn materialize_postgres_index_drift_snapshot_reader",
]);

const production = c.reader.split("\n#[cfg(test)]")[0];
for (const forbidden of [
  "INSERT INTO",
  "UPDATE ",
  "DELETE FROM",
  "tokio::spawn",
  "spawn_blocking",
  "repair_finding",
  "resolve_finding",
  "ignore_finding",
  "Router::new",
  "async_graphql",
]) {
  if (production.includes(forbidden)) throw new Error(`snapshot reader contains ${forbidden}`);
}

requireMarkers("test", [
  'const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";',
  "for migration in IndexModule.migrations()",
  "PostgresSchemaRegistrationStore",
  "PostgresMutationStore",
  "source_version_fence_captures_and_rejects_unstable_postgres_snapshots",
  'assert_eq!(changed.code(), "index_drift_source_changed_during_capture")',
  'assert_eq!(missing.code(), "index_drift_source_watermark_missing")',
  'DROP SCHEMA IF EXISTS',
]);
requireMarkers("diagnosis", [
  "materialize_index_source_absence_registry(extensions)",
  "SharedIndexSourceAbsenceRegistry",
  "PostgresIndexDriftSnapshotReader::new(",
  "reader.with_absence_registry(absence)",
  "IndexDriftDigestProducer::new(",
  "PostgresIndexDriftFindingWriter::new(db)",
  "pub async fn diagnose_entity(",
  "authorize_for(&context, key.tenant_id)?;",
  "IndexDriftDigestRequest::new(key)?;",
]);
requireMarkers("productProvider", [
  "impl IndexSourceAbsenceProvider for ProductLocaleAbsenceProvider",
  "CAST(projection.projection_epoch AS TEXT) AS source_version_text",
  "FROM product_translations translation",
  "FROM product_index_tombstones tombstone",
]);
requireMarkers("doc", [
  "source_complete_owner_execution_pending",
  "REPEATABLE READ READ ONLY",
  "txid_current_snapshot()",
  "An empty targeted load alone is never converted to `Missing`.",
  "explicit_source_absence_watermark_v1",
  "Existing\nUpsert/Delete boundary derivation is unchanged.",
  "Product locale provider",
  "No retained PostgreSQL execution evidence is claimed",
]);
requireMarkers("recheck", [
  "Audited baseline: `main@368c79b78549e97a68120358021552b2552b800c`",
  "`product-locale-absence-postgres`",
  "index_drift_source_changed_during_capture",
  "index_drift_source_watermark_missing",
]);
requireMarkers("plan", [
  "M6 source-version-fenced PostgreSQL drift snapshot reader",
  "M6 guarded exact-entity drift diagnosis operator",
  "M6 explicit source absence watermark registry, Product provider, and reader fence",
  "source_complete_owner_execution_pending",
  "real-migration Product locale-absence scenario",
]);

for (const claim of [
  "tests passed",
  "PostgreSQL execution passed",
  "retained evidence admitted",
  "diagnosis transport is complete",
  "repair is complete",
]) {
  if (
    c.doc.toLowerCase().includes(claim.toLowerCase()) ||
    c.recheck.toLowerCase().includes(claim.toLowerCase())
  ) {
    throw new Error(`forbidden completion claim: ${claim}`);
  }
}

console.log("Index PostgreSQL drift snapshot reader contract verified");