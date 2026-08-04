#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  producer: "crates/rustok-index/src/application/drift_digest.rs",
  applicationMod: "crates/rustok-index/src/application/mod.rs",
  recorder: "crates/rustok-index/src/infrastructure/postgres/drift_digest_recorder.rs",
  postgresMod: "crates/rustok-index/src/infrastructure/postgres/mod.rs",
  doc: "crates/rustok-index/docs/m6-drift-digest-producer.md",
  plan: "crates/rustok-index/docs/implementation-plan-current-2026-08-03.md",
};

const [producer, applicationMod, recorder, postgresMod, doc, plan] = await Promise.all(
  Object.values(files).map((path) => readFile(path, "utf8")),
);

function requireMarkers(label, content, markers) {
  for (const marker of markers) {
    if (!content.includes(marker)) {
      throw new Error(`${label} is missing required marker: ${marker}`);
    }
  }
}

requireMarkers(files.producer, producer, [
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

requireMarkers(files.recorder, recorder, [
  "impl IndexDriftMismatchRecorder for PostgresIndexDriftFindingWriter",
  'const CHECK_NAME: &str = "source_index_digest_mismatch";',
  "match key.locale.clone()",
  "Some(locale) => IndexDriftFindingScope::Entity",
  "None => IndexDriftFindingScope::EntityWithoutLocale",
  "PostgresIndexDriftFindingWriter::record_digest_mismatch",
  "IndexDriftMismatchRecordStatus::Suppressed",
]);

requireMarkers(files.applicationMod, applicationMod, [
  "mod drift_digest;",
  "IndexDriftDigestProducer",
  "IndexDriftSnapshotReader",
]);
requireMarkers(files.postgresMod, postgresMod, [
  "mod drift_digest_recorder;",
  "mod drift_snapshot_reader;",
]);
requireMarkers(files.doc, doc, [
  "producer_contract_and_locale_scope_source_complete_snapshot_reader_pending",
  "same bounded opaque `IndexDriftSnapshotBoundary`",
  "Equal digests return `Consistent` and never call the recorder.",
  "`locale: None` maps to `IndexDriftFindingScope::EntityWithoutLocale`",
  "does not define PostgreSQL snapshot export",
]);
requireMarkers(files.plan, plan, [
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
  if (producer.includes(marker)) {
    throw new Error(`database-neutral producer contains forbidden dependency marker: ${marker}`);
  }
}

for (const obsolete of [
  "index_drift_locale_free_scope_unsupported",
  "let Some(locale) = key.locale.clone() else",
]) {
  if (recorder.includes(obsolete) || doc.includes(obsolete)) {
    throw new Error(`producer adapter retains obsolete locale limitation: ${obsolete}`);
  }
}

for (const claim of [
  "tests passed",
  "repair is complete",
  "retained evidence admitted",
]) {
  if (doc.toLowerCase().includes(claim.toLowerCase())) {
    throw new Error(`documentation makes forbidden completion claim: ${claim}`);
  }
}

console.log("Index drift digest producer contract verified");
