#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  writer: "crates/rustok-index/src/infrastructure/postgres/drift_finding_writer.rs",
  inspector: "crates/rustok-index/src/infrastructure/postgres/drift_finding_inspector.rs",
  recorder: "crates/rustok-index/src/infrastructure/postgres/drift_digest_recorder.rs",
  migration: "crates/rustok-index/src/migrations/m20260804_000005_relax_index_finding_locale_scope.rs",
  legacyMigration: "crates/rustok-index/src/migrations/m20260727_000003_create_index_operations.rs",
  migrationsMod: "crates/rustok-index/src/migrations/mod.rs",
  keyTest: "crates/rustok-index/tests/drift_finding_locale_key_contract.rs",
  postgresTest: "crates/rustok-index/tests/drift_finding_locale_scope_postgres_test.rs",
  doc: "crates/rustok-index/docs/m6-drift-finding-locale-scope.md",
  plan: "crates/rustok-index/docs/implementation-plan-current-2026-08-03.md",
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

requireMarkers(files.writer, contents.writer, [
  'const NO_LOCALE_KEY_COMPONENT: &[u8] = b"\\0";',
  "IndexDriftFindingScope::EntityWithoutLocale",
  "locale_key: None",
  "locale.as_str().as_bytes()",
  "hash_component(&mut hasher, NO_LOCALE_KEY_COMPONENT)",
]);
requireMarkers(files.inspector, contents.inspector, [
  "EntityWithoutLocale",
  "match locale_key",
  "Some(stored_locale)",
  "None => Ok(IndexDriftFindingScope::EntityWithoutLocale",
]);
requireMarkers(files.recorder, contents.recorder, [
  "match key.locale.clone()",
  "Some(locale) => IndexDriftFindingScope::Entity",
  "None => IndexDriftFindingScope::EntityWithoutLocale",
  "PostgresIndexDriftFindingWriter::record_digest_mismatch",
]);
requireMarkers(files.migration, contents.migration, [
  "RELAXED_SCOPE_CHECK",
  "STRICT_SCOPE_CHECK",
  "pg_get_constraintdef(c.oid)",
  "current_schema()",
  "DROP CONSTRAINT",
  "ADD CONSTRAINT",
  "rebuild_sqlite_table",
  "CREATE UNIQUE INDEX uq_index_consistency_finding_key",
  "CREATE INDEX idx_index_consistency_open",
]);
requireMarkers(files.legacyMigration, contents.legacyMigration, [
  "scope_kind = 'entity'",
  "entity_id IS NOT NULL AND locale_key IS NOT NULL",
]);
requireMarkers(files.migrationsMod, contents.migrationsMod, [
  "mod m20260804_000005_relax_index_finding_locale_scope;",
  "Box::new(m20260804_000005_relax_index_finding_locale_scope::Migration)",
  '"m20260804_000005_relax_index_finding_locale_scope"',
  'vec!["m20260803_000004_create_index_reconciliation_recovery"]',
]);
requireMarkers(files.keyTest, contents.keyTest, [
  "legacy_locale_key",
  "assert_eq!(",
  "IndexDriftFindingScope::EntityWithoutLocale",
  "assert_ne!(no_locale_request.finding_key(), locale_request.finding_key())",
]);
requireMarkers(files.postgresTest, contents.postgresTest, [
  "for migration in IndexModule.migrations()",
  "IndexDriftFindingScope::EntityWithoutLocale",
  "PostgresIndexDriftFindingWriter",
  "PostgresIndexDriftFindingInspector",
  "ORDER BY locale_key NULLS FIRST",
  "assert_eq!(first_locale, None)",
  "IndexDriftFindingWriteOutcome::Refreshed",
  'DROP SCHEMA IF EXISTS',
]);
requireMarkers(files.doc, contents.doc, [
  "source_complete_owner_execution_pending",
  "existing locale-bearing finding key",
  "length-prefixed NUL byte",
  "The historical M3 migration remains unchanged.",
  "retained execution evidence until the repository owner runs it",
]);
requireMarkers(files.plan, contents.plan, [
  "M6 locale-optional persisted entity finding scope",
  "Extend persisted entity finding scope to locale-free",
  "drift_finding_locale_scope_postgres_test",
  "authoritative production snapshot capture",
]);
requireMarkers(files.queryVerifier, contents.queryVerifier, [
  "'verify-index-drift-finding-locale-scope.mjs'",
]);

for (const obsolete of [
  "index_drift_locale_free_scope_unsupported",
  "let Some(locale) = key.locale.clone() else",
]) {
  if (contents.recorder.includes(obsolete) || contents.doc.includes(obsolete)) {
    throw new Error(`locale-complete boundary retains obsolete marker: ${obsolete}`);
  }
}

for (const claim of [
  "tests passed",
  "PostgreSQL execution passed",
  "retained evidence admitted",
  "snapshot reader is complete",
  "repair is complete",
]) {
  if (contents.doc.toLowerCase().includes(claim.toLowerCase())) {
    throw new Error(`documentation makes forbidden completion claim: ${claim}`);
  }
}

console.log("Index drift-finding locale scope contract verified");
