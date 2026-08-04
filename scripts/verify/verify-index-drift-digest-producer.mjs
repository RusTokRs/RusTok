#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  producer: "crates/rustok-index/src/application/drift_digest.rs",
  applicationMod: "crates/rustok-index/src/application/mod.rs",
  recorder: "crates/rustok-index/src/infrastructure/postgres/drift_digest_recorder.rs",
  reader: "crates/rustok-index/src/infrastructure/postgres/drift_snapshot_reader.rs",
  diagnosis: "apps/server/src/services/index_drift_diagnosis_operator.rs",
  postgresMod: "crates/rustok-index/src/infrastructure/postgres/mod.rs",
  doc: "crates/rustok-index/docs/m6-drift-digest-producer.md",
  plan: "crates/rustok-index/docs/implementation-plan-current-2026-08-03.md",
};

const [producer, applicationMod, recorder, reader, diagnosis, postgresMod, doc, plan] =
  await Promise.all(Object.values(files).map((path) => readFile(path, "utf8")));

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

requireMarkers(files.reader, reader, [
  "impl IndexDriftSnapshotReader for PostgresIndexDriftSnapshotReader",
  "IsolationLevel::RepeatableRead",
  "AccessMode::ReadOnly",
  "index_drift_source_watermark_missing",
  "index_drift_source_changed_during_capture",
]);

requireMarkers(files.diagnosis, diagnosis, [
  "type IndexDriftDiagnosisProducer = rustok_index::IndexDriftDigestProducer<",
  "rustok_index::PostgresIndexDriftSnapshotReader",
  "PostgresIndexDriftFindingWriter",
  "pub async fn diagnose_entity(",
  "authorize_for(&context, key.tenant_id)?;",
  "IndexDriftDigestRequest::new(key)?;",
  "self.inner.produce(request).await",
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
  "producer_reader_locale_scope_and_guarded_diagnosis_source_complete",
  "same bounded opaque `IndexDriftSnapshotBoundary`",
  "Equal digests return `Consistent` and never call the recorder.",
  "`locale: None` maps to `IndexDriftFindingScope::EntityWithoutLocale`",
  "IndexDriftDiagnosisOperatorRuntime",
  "does not define PostgreSQL snapshot export",
]);
requireMarkers(files.plan, plan, [
  "M6 snapshot-pair digest producer and mismatch-only recorder delegation",
  "M6 locale-optional persisted entity finding scope",
  "M6 source-version-fenced PostgreSQL drift snapshot reader",
  "M6 guarded exact-entity drift diagnosis operator",
  "source_complete_transport_and_owner_execution_pending",
  "explicit retained absence/tombstone watermark contract",
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

const diagnosisProduction = diagnosis.split("\n#[cfg(test)]")[0];
for (const forbidden of [
  "tokio::spawn",
  "spawn_blocking",
  "Router::new",
  "async_graphql",
  ".scan(",
  "repair_finding",
  "resolve_finding",
  "ignore_finding",
]) {
  if (diagnosisProduction.includes(forbidden)) {
    throw new Error(`guarded diagnosis contains forbidden marker: ${forbidden}`);
  }
}

for (const claim of [
  "tests passed",
  "diagnosis transport is complete",
  "repair is complete",
  "retained evidence admitted",
]) {
  if (doc.toLowerCase().includes(claim.toLowerCase())) {
    throw new Error(`documentation makes forbidden completion claim: ${claim}`);
  }
}

console.log("Index drift digest producer contract verified");
