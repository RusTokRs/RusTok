#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  producer: "crates/rustok-index/src/application/drift_digest.rs",
  applicationMod: "crates/rustok-index/src/application/mod.rs",
  recorder: "crates/rustok-index/src/infrastructure/postgres/drift_digest_recorder.rs",
  reader: "crates/rustok-index/src/infrastructure/postgres/drift_snapshot_reader.rs",
  diagnosis: "apps/server/src/services/index_drift_diagnosis_operator.rs",
  pageDiagnosis: "apps/server/src/services/index_drift_source_page_diagnosis.rs",
  postgresMod: "crates/rustok-index/src/infrastructure/postgres/mod.rs",
  doc: "crates/rustok-index/docs/m6-drift-digest-producer.md",
  plan: "crates/rustok-index/docs/implementation-plan-current-2026-08-03.md",
};

const content = Object.fromEntries(
  await Promise.all(
    Object.entries(files).map(async ([name, path]) => [name, await readFile(path, "utf8")]),
  ),
);

function requireMarkers(name, markers) {
  for (const marker of markers) {
    if (!content[name].includes(marker)) {
      throw new Error(`${files[name]} missing required marker: ${marker}`);
    }
  }
}

requireMarkers("producer", [
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
  "pub enum IndexDriftMissingEntityCandidateOutcome",
  "NotCandidate",
  "MissingRecorded",
  "pub async fn produce_missing_entity_candidate(",
  "pub async fn produce_missing_entity_candidate_from_pair(",
  "self.validate_pair(&request, &pair)?;",
  "matches!(pair.source(), IndexDriftEntityState::Upsert { .. })",
  "matches!(pair.materialized(), IndexDriftEntityState::Missing { .. })",
  "missing_candidate_records_only_source_upsert_materialized_missing",
  "missing_candidate_skips_every_other_typed_state_combination",
]);

const candidateStart = content.producer.indexOf(
  "    pub async fn produce_missing_entity_candidate_from_pair(",
);
const candidateEnd = content.producer.indexOf("\n    async fn capture_pair(", candidateStart);
if (candidateStart < 0 || candidateEnd < 0) {
  throw new Error("missing-only candidate producer segment is incomplete");
}
const candidate = content.producer.slice(candidateStart, candidateEnd);
const validation = candidate.indexOf("self.validate_pair(&request, &pair)?;");
const sourceUpsert = candidate.indexOf(
  "matches!(pair.source(), IndexDriftEntityState::Upsert { .. })",
);
const materializedMissing = candidate.indexOf(
  "matches!(pair.materialized(), IndexDriftEntityState::Missing { .. })",
);
const notCandidate = candidate.indexOf(
  "return Ok(IndexDriftMissingEntityCandidateOutcome::NotCandidate);",
);
const recorder = candidate.indexOf(".record_mismatch(");
if (
  validation < 0 ||
  sourceUpsert <= validation ||
  materializedMissing <= sourceUpsert ||
  notCandidate <= materializedMissing ||
  recorder <= notCandidate
) {
  throw new Error(
    "missing-only producer must validate, classify Upsert/Missing, skip non-candidates, then record",
  );
}

const outcomeStart = content.producer.indexOf(
  "pub enum IndexDriftMissingEntityCandidateOutcome",
);
const outcomeEnd = content.producer.indexOf("\n}\n\npub struct IndexDriftDigestProducer", outcomeStart);
const outcome = content.producer.slice(outcomeStart, outcomeEnd);
for (const forbidden of [
  "EntityKey",
  "IndexRecord",
  "IndexDriftEntityState",
  "IndexDriftSnapshotPair",
  "boundary",
]) {
  if (outcome.includes(forbidden)) {
    throw new Error(`missing-only public outcome exposes forbidden state detail: ${forbidden}`);
  }
}

requireMarkers("recorder", [
  "impl IndexDriftMismatchRecorder for PostgresIndexDriftFindingWriter",
  'const CHECK_NAME: &str = "source_index_digest_mismatch";',
  "match key.locale.clone()",
  "Some(locale) => IndexDriftFindingScope::Entity",
  "None => IndexDriftFindingScope::EntityWithoutLocale",
  "PostgresIndexDriftFindingWriter::record_digest_mismatch",
  "IndexDriftMismatchRecordStatus::Suppressed",
]);

requireMarkers("reader", [
  "impl IndexDriftSnapshotReader for PostgresIndexDriftSnapshotReader",
  "IsolationLevel::RepeatableRead",
  "AccessMode::ReadOnly",
  "index_drift_source_watermark_missing",
  "index_drift_source_changed_during_capture",
]);

requireMarkers("diagnosis", [
  "type IndexDriftDiagnosisProducer = rustok_index::IndexDriftDigestProducer<",
  "rustok_index::PostgresIndexDriftSnapshotReader",
  "PostgresIndexDriftFindingWriter",
  "pub async fn diagnose_entity(",
  "pub async fn diagnose_missing_entity_candidate(",
  "authorize_for(&context, key.tenant_id)?;",
  "IndexDriftDigestRequest::new(key)?;",
  "self.inner.produce(request).await",
  ".produce_missing_entity_candidate(request)",
  "missing_candidate_diagnosis_authorizes_before_request_validation",
]);
requireMarkers("pageDiagnosis", [
  ".diagnose_missing_entity_candidate(context, key)",
  "IndexDriftMissingEntityCandidateOutcome::NotCandidate",
  "IndexDriftMissingEntityCandidateOutcome::MissingRecorded",
]);

requireMarkers("applicationMod", [
  "mod drift_digest;",
  "IndexDriftDigestProducer",
  "IndexDriftMissingEntityCandidateOutcome",
  "IndexDriftSnapshotReader",
]);
requireMarkers("postgresMod", [
  "mod drift_digest_recorder;",
  "mod drift_snapshot_reader;",
]);
requireMarkers("doc", [
  "producer_missing_candidate_and_guarded_source_page_source_complete",
  "same bounded opaque `IndexDriftSnapshotBoundary`",
  "Equal digests return `Consistent` and never call the recorder.",
  "`IndexDriftMissingEntityCandidateOutcome`",
  "source state is authoritative `Upsert`",
  "materialized state is exact `Missing`",
  "`locale: None` maps to `IndexDriftFindingScope::EntityWithoutLocale`",
  "diagnose_missing_entity_candidate",
  "does not define PostgreSQL snapshot export",
]);
requireMarkers("plan", [
  "M6 snapshot-pair digest producer and mismatch-only recorder delegation",
  "M6 missing-only entity candidate outcome",
  "M6 bounded source-page missing-entity diagnosis",
  "M6 source-version-fenced PostgreSQL drift snapshot reader",
  "M6 guarded exact-entity drift diagnosis operator",
  "source_complete_owner_execution_pending",
]);

for (const marker of [
  "sea_orm",
  "DatabaseConnection",
  "SELECT ",
  "INSERT ",
  "UPDATE ",
  "DELETE FROM",
  "IndexSource::scan",
  "index_entities",
  "index_links",
]) {
  if (content.producer.includes(marker)) {
    throw new Error(`database-neutral producer contains forbidden dependency marker: ${marker}`);
  }
}

for (const obsolete of [
  "index_drift_locale_free_scope_unsupported",
  "let Some(locale) = key.locale.clone() else",
]) {
  if (content.recorder.includes(obsolete) || content.doc.includes(obsolete)) {
    throw new Error(`producer adapter retains obsolete locale limitation: ${obsolete}`);
  }
}

const diagnosisProduction = content.diagnosis.split("\n#[cfg(test)]")[0];
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
  "retained evidence admitted",
  "repair is complete",
]) {
  if (content.doc.toLowerCase().includes(claim.toLowerCase())) {
    throw new Error(`documentation makes forbidden completion claim: ${claim}`);
  }
}

console.log("Index drift digest and missing-only candidate contracts verified");
