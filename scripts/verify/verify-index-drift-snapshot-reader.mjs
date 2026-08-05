#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  reader: "crates/rustok-index/src/infrastructure/postgres/drift_snapshot_reader.rs",
  postgresMod: "crates/rustok-index/src/infrastructure/postgres/mod.rs",
  lib: "crates/rustok-index/src/lib.rs",
  test: "crates/rustok-index/tests/drift_snapshot_reader_postgres_test.rs",
  doc: "crates/rustok-index/docs/m6-postgres-drift-snapshot-reader.md",
  recheck:
    "crates/rustok-index/docs/implementation-recheck-2026-08-04-drift-snapshot-reader.md",
  plan: "crates/rustok-index/docs/implementation-plan-current-2026-08-03.md",
  readme: "crates/rustok-index/docs/README.md",
  queryVerifier: "scripts/verify/verify-index-query-contract.mjs",
};

const contents = Object.fromEntries(
  await Promise.all(
    Object.entries(files).map(async ([name, path]) => [name, await readFile(path, "utf8")]),
  ),
);

function requireMarkers(label, content, markers) {
  for (const marker of markers) {
    if (!content.includes(marker)) {
      throw new Error(`${label} is missing required marker: ${marker}`);
    }
  }
}

requireMarkers(files.reader, contents.reader, [
  "pub struct PostgresIndexDriftSnapshotReader",
  "impl IndexDriftSnapshotReader for PostgresIndexDriftSnapshotReader",
  "IndexSourceLoadRequest::new(vec![request.key().clone()])",
  "index_drift_source_watermark_missing",
  "index_drift_source_changed_during_capture",
  "begin_with_config(",
  "IsolationLevel::RepeatableRead",
  "AccessMode::ReadOnly",
  "SELECT txid_current_snapshot()::text AS snapshot_token",
  "let observed_again = self.load_source_state(request).await?;",
  "if &observed_again != source",
  "FROM index_entities WHERE tenant_id = $1",
  "FROM index_links WHERE tenant_id = $1",
  "fingerprint != registered.fingerprint.to_string()",
  "targets.sort_by_key(|(ordinal, _)| *ordinal)",
  "index_drift_postgres_source_version_boundary_v1",
  "pub fn materialize_postgres_index_drift_snapshot_reader",
]);

const production = contents.reader.split("\n#[cfg(test)]")[0];
for (const forbidden of [
  "INSERT INTO",
  "UPDATE ",
  "DELETE FROM",
  "tokio::spawn",
  "spawn_blocking",
  "tokio::time::sleep",
  "repair_finding",
  "resolve_finding",
  "ignore_finding",
  "Router::new",
  "async_graphql",
]) {
  if (production.includes(forbidden)) {
    throw new Error(`snapshot reader contains forbidden marker: ${forbidden}`);
  }
}

requireMarkers(files.postgresMod, contents.postgresMod, [
  "mod drift_snapshot_reader;",
  "PostgresIndexDriftSnapshotReader",
  "materialize_postgres_index_drift_snapshot_reader",
]);
requireMarkers(files.lib, contents.lib, [
  "materialize_postgres_index_drift_snapshot_reader",
  "IndexDriftSnapshotCompositionError",
  "PostgresIndexDriftSnapshotReader",
]);
requireMarkers(files.test, contents.test, [
  'const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";',
  "for migration in IndexModule.migrations()",
  "PostgresSchemaRegistrationStore",
  "PostgresMutationStore",
  "materialize_postgres_index_drift_snapshot_reader",
  "source_version_fence_captures_and_rejects_unstable_postgres_snapshots",
  'assert_eq!(changed.code(), "index_drift_source_changed_during_capture")',
  'assert_eq!(missing.code(), "index_drift_source_watermark_missing")',
  'DROP SCHEMA IF EXISTS',
]);
requireMarkers(files.doc, contents.doc, [
  "source_complete_host_diagnosis_composition_and_owner_execution_pending",
  "source-version fence",
  "REPEATABLE READ READ ONLY",
  "txid_current_snapshot()",
  "unproven absence is never converted to `Missing`",
  "retained execution evidence only after the repository owner runs and admits it",
]);
requireMarkers(files.recheck, contents.recheck, [
  "Audited baseline: `main@5da25b28be5e1bf4f9cd9802337a3efa560179a4`",
  "source-version fence",
  "Compose the reader, existing digest producer, and finding writer",
]);
requireMarkers(files.plan, contents.plan, [
  "M6 source-version-fenced PostgreSQL drift snapshot reader",
  "source_complete_host_diagnosis_composition_owner_execution_pending",
  "drift_snapshot_reader_postgres_test",
  "Compose the reader, digest producer, and finding writer",
  "explicit retained absence/tombstone watermark contract",
]);
requireMarkers(files.readme, contents.readme, [
  "[M6 PostgreSQL Drift Snapshot Reader](./m6-postgres-drift-snapshot-reader.md)",
]);
requireMarkers(files.queryVerifier, contents.queryVerifier, [
  "'verify-index-drift-snapshot-reader.mjs'",
]);

for (const claim of [
  "tests passed",
  "PostgreSQL execution passed",
  "retained evidence admitted",
  "server diagnosis is complete",
  "repair is complete",
]) {
  if (
    contents.doc.toLowerCase().includes(claim.toLowerCase()) ||
    contents.recheck.toLowerCase().includes(claim.toLowerCase())
  ) {
    throw new Error(`snapshot reader documentation makes forbidden claim: ${claim}`);
  }
}

console.log("Index PostgreSQL drift snapshot reader contract verified");
