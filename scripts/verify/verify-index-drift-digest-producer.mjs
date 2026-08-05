#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  producer: "crates/rustok-index/src/application/drift_digest.rs",
  applicationMod: "crates/rustok-index/src/application/mod.rs",
  recorder: "crates/rustok-index/src/infrastructure/postgres/drift_digest_recorder.rs",
  snapshotReader:
    "crates/rustok-index/src/infrastructure/postgres/drift_snapshot_reader.rs",
  postgresMod: "crates/rustok-index/src/infrastructure/postgres/mod.rs",
  doc: "crates/rustok-index/docs/m6-drift-digest-producer.md",
  plan: "crates/rustok-index/docs/implementation-plan-current-2026-08-03.md",
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

requireMarkers(files.producer, contents.producer, [
  'b"index_drift_entity_state_digest_v1"',
  "pub trait IndexDriftSnapshotReader",
  "pub trait IndexDriftMismatchRecorder",
  "pub struct IndexDriftSnapshotPair",
  "SnapshotBoundaryMismatch",
  "SnapshotScopeMismatch",
  "registry.validate_mutation(&mutation)",
  "postcard::to_allocvec(state)",
  "if source_digest == materialized_digest",
  ".record_digest_mismatch(&mismatch)",
  "IndexDriftEntityState::Missing",
  "IndexDriftEntityState::Delete",
  "IndexDriftEntityState::Upsert",
]);

requireMarkers(files.recorder, contents.recorder, [
  "impl IndexDriftMismatchRecorder for PostgresIndexDriftFindingWriter",
  'const CHECK_NAME: &str = "source_index_digest_mismatch";',
  "match key.locale.clone()",
  "Some(locale) => IndexDriftFindingScope::Entity",
  "None => IndexDriftFindingScope::EntityWithoutLocale",
  "PostgresIndexDriftFindingWriter::record_digest_mismatch",
  "IndexDriftMismatchRecordStatus::Suppressed",
]);
requireMarkers(files.snapshotReader, contents.snapshotReader, [
  "impl IndexDriftSnapshotReader for PostgresIndexDriftSnapshotReader",
  "IsolationLevel::RepeatableRead",
  "AccessMode::ReadOnly",
  "index_drift_source_changed_during_capture",
  "index_drift_source_watermark_missing",
]);

requireMarkers(files.applicationMod, contents.applicationMod, [
  "mod drift_digest;",
  "IndexDriftDigestProducer",
  "IndexDriftSnapshotReader",
]);
requireMarkers(files.postgresMod, contents.postgresMod, [
  "mod drift_digest_recorder;",
  "mod drift_snapshot_reader;",
]);
requireMarkers(files.doc, contents.doc, [
  "producer_reader_and_locale_scope_source_complete_host_diagnosis_pending",
  "same bounded opaque `IndexDriftSnapshotBoundary`",
  "Equal digests return `Consistent` and never call the recorder.",
  "`locale: None` maps to `IndexDriftFindingScope::EntityWithoutLocale`",
  "`PostgresIndexDriftSnapshotReader` is now source complete",
  "server-owned composition of reader, producer, and writer",
]);
requireMarkers(files.plan, contents.plan, [
  "M6 snapshot-pair digest producer and mismatch-only recorder delegation",
  "M6 source-version-fenced PostgreSQL drift snapshot reader",
  "source_complete_host_diagnosis_composition_owner_execution_pending",
  "Compose the reader, digest producer, and finding writer",
]);

const forbiddenProducerMarkers = [
  "sea_orm",
  "DatabaseConnection",
  "SELECT ",
  "INSERT ",
  "UPDATE ",
  "DELETE FROM",
  "IndexSource::scan",
  "index_entities",
  "index_links",
];
for (const marker of forbiddenProducerMarkers) {
  if (contents.producer.includes(marker)) {
    throw new Error(`database-neutral producer contains forbidden dependency marker: ${marker}`);
  }
}

for (const obsolete of [
  "index_drift_locale_free_scope_unsupported",
  "let Some(locale) = key.locale.clone() else",
]) {
  if (contents.recorder.includes(obsolete) || contents.doc.includes(obsolete)) {
    throw new Error(`producer adapter retains obsolete locale limitation: ${obsolete}`);
  }
}

for (const claim of [
  "tests passed",
  "repair is complete",
  "retained evidence admitted",
]) {
  if (contents.doc.toLowerCase().includes(claim.toLowerCase())) {
    throw new Error(`documentation makes forbidden completion claim: ${claim}`);
  }
}

console.log("Index drift digest producer contract verified");
