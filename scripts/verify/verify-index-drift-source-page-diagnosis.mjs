#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  producer: "crates/rustok-index/src/application/drift_digest.rs",
  runtime: "apps/server/src/services/index_drift_source_page_diagnosis.rs",
  diagnosis: "apps/server/src/services/index_drift_diagnosis_operator.rs",
  composition: "apps/server/src/services/index_replay_runtime_composition.rs",
  graphql: "apps/server/src/graphql/index_drift_diagnosis.rs",
  graphqlSchema: "apps/server/src/graphql/schema.rs",
  continuation: "crates/rustok-index/src/application/source_continuation.rs",
  doc: "apps/server/docs/index-drift-source-page-diagnosis.md",
  operatorDoc: "apps/server/docs/index-reconciliation-operator-runtime.md",
  plan: "crates/rustok-index/docs/implementation-plan-current-2026-08-03.md",
  aggregate: "scripts/verify/verify-index-query-contract.mjs",
};

const content = Object.fromEntries(
  await Promise.all(
    Object.entries(files).map(async ([name, path]) => [name, await readFile(path, "utf8")]),
  ),
);

function requireMarkers(name, markers) {
  for (const marker of markers) {
    if (!content[name].includes(marker)) {
      throw new Error(`${files[name]} missing ${marker}`);
    }
  }
}

requireMarkers("producer", [
  "pub enum IndexDriftMissingEntityCandidateOutcome",
  "pub async fn produce_missing_entity_candidate(",
  "pub async fn produce_missing_entity_candidate_from_pair(",
  "matches!(pair.source(), IndexDriftEntityState::Upsert { .. })",
  "matches!(pair.materialized(), IndexDriftEntityState::Missing { .. })",
]);
requireMarkers("diagnosis", [
  "pub async fn diagnose_missing_entity_candidate(",
  ".produce_missing_entity_candidate(request)",
  "missing_candidate_diagnosis_authorizes_before_request_validation",
]);
requireMarkers("runtime", [
  "const MAX_SOURCE_PAGE_DIAGNOSIS_SIZE: usize = 32;",
  "pub enum IndexDriftSourcePageDiagnosisError",
  "pub struct IndexDriftSourcePageDiagnosisOutcome",
  "pub struct IndexDriftSourcePageDiagnosisRuntime",
  "sources: rustok_index::SharedIndexSourceRegistry",
  "exact: IndexDriftDiagnosisOperatorRuntime",
  "pub async fn diagnose_source_page(",
  "authorize_and_build_scan_request(context, schema, cursor, limit)?;",
  "let page = self.sources.scan(request).await?;",
  ".diagnose_missing_entity_candidate(context, key)",
  "permissions_for(&context.tenant_id(), &context.actor_id())",
  "has_effective_permission(&permissions, &Permission::MODULES_MANAGE)",
  "if !(1..=MAX_SOURCE_PAGE_DIAGNOSIS_SIZE).contains(&limit)",
  "IndexSourceScanRequest::new(context.tenant_id(), schema, cursor, limit)",
  "matches!(&mutation, rustok_index::IndexMutation::Delete { .. })",
  "IndexDriftMissingEntityCandidateOutcome::NotCandidate",
  "IndexDriftMissingEntityCandidateOutcome::MissingRecorded",
  "non_missing_count",
  "missing_recorded_count",
  "receipts.push(receipt);",
  "materialize_index_drift_source_page_diagnosis",
  "authorization_precedes_page_limit_validation",
  "one_page_skips_deletes_and_classifies_each_upsert_once",
]);

const production = content.runtime.split("\n#[cfg(test)]")[0];
const auth = production.indexOf(
  "let permissions = permissions_for(&context.tenant_id(), &context.actor_id())",
);
const limit = production.indexOf(
  "if !(1..=MAX_SOURCE_PAGE_DIAGNOSIS_SIZE).contains(&limit)",
  auth,
);
const request = production.indexOf(
  "IndexSourceScanRequest::new(context.tenant_id(), schema, cursor, limit)",
  limit,
);
const scan = production.indexOf("let page = self.sources.scan(request).await?;");
if (auth < 0 || limit <= auth || request <= limit || scan <= request) {
  throw new Error(
    "source-page diagnosis must authorize, validate the bounded request, then scan once",
  );
}

const pageLoop = production.indexOf("for (position, mutation) in mutations.into_iter().enumerate()");
const deleteSkip = production.indexOf(
  "matches!(&mutation, rustok_index::IndexMutation::Delete { .. })",
  pageLoop,
);
const exactDelegate = production.indexOf(
  ".diagnose_missing_entity_candidate(context, key)",
);
const nonCandidate = production.indexOf(
  "IndexDriftMissingEntityCandidateOutcome::NotCandidate",
  pageLoop,
);
const missingRecorded = production.indexOf(
  "IndexDriftMissingEntityCandidateOutcome::MissingRecorded",
  nonCandidate,
);
if (
  pageLoop < 0 ||
  deleteSkip <= pageLoop ||
  exactDelegate < 0 ||
  nonCandidate <= pageLoop ||
  missingRecorded <= nonCandidate
) {
  throw new Error(
    "source-page diagnosis must skip deletes and classify exact missing-only outcomes",
  );
}

if (production.includes(".diagnose_entity(context, key)")) {
  throw new Error("source-page diagnosis must not delegate to the general mismatch recorder path");
}
for (const forbidden of [
  "tokio::spawn",
  "spawn_blocking",
  "DatabaseConnection",
  "SELECT ",
  "INSERT ",
  "UPDATE ",
  "DELETE FROM",
  "while ",
  "loop {",
  "ModuleWorkScheduler",
  "request_cancel",
  "requeue_dead_letter",
  "resolve_finding",
  "ignore_finding",
  "repair_finding",
  "IndexSourceContinuationCodec",
  "IndexSourceContinuationToken",
]) {
  if (production.includes(forbidden)) {
    throw new Error(`source-page diagnosis contains forbidden capability: ${forbidden}`);
  }
}

requireMarkers("composition", [
  '#[path = "index_drift_source_page_diagnosis.rs"]',
  "mod drift_source_page_diagnosis;",
  "IndexDriftSourcePageDiagnosisRuntime,",
  "drift_diagnosis_operator::materialize_index_drift_diagnosis_operator(extensions, db)?;",
  "drift_source_page_diagnosis::materialize_index_drift_source_page_diagnosis(extensions)?;",
  "extensions.contains::<IndexDriftSourcePageDiagnosisRuntime>()",
  "shared_get::<IndexDriftSourcePageDiagnosisRuntime>()",
]);
const exactComposition = content.composition.indexOf(
  "drift_diagnosis_operator::materialize_index_drift_diagnosis_operator(extensions, db)?;",
);
const pageComposition = content.composition.indexOf(
  "drift_source_page_diagnosis::materialize_index_drift_source_page_diagnosis(extensions)?;",
);
if (exactComposition < 0 || pageComposition <= exactComposition) {
  throw new Error("source-page diagnosis must compose after exact diagnosis");
}

for (const forbidden of [
  "diagnose_index_source_page",
  "IndexDriftSourcePageDiagnosisRuntime",
  "IndexSourceCursor",
  "IndexSourceContinuationCodec",
  "IndexSourceContinuationToken",
  "diagnose_missing_entity_candidate",
]) {
  if (content.graphql.includes(forbidden) || content.graphqlSchema.includes(forbidden)) {
    throw new Error(`source-page capability leaked into GraphQL: ${forbidden}`);
  }
}

requireMarkers("continuation", [
  "pub struct IndexSourceContinuationCodec",
  "pub struct IndexSourceContinuationScope",
  "pub struct IndexSourceContinuationToken",
  "pub fn from_registry(",
  "Aes256Gcm::new_from_slice",
]);
requireMarkers("doc", [
  "Status: `missing_only_source_complete_transport_and_owner_execution_pending`.",
  "one page limit in `1..=32`",
  "skips retained source `Delete` mutations",
  "source `Upsert` plus materialized `Missing`",
  "non-missing candidate count",
  "The raw cursor is not attached to GraphQL",
  "Confidential continuation prerequisite",
  "The codec is not yet composed into this server runtime",
  "server continuation-key configuration",
  "No tests, verifiers, formatting, Cargo checks",
]);
requireMarkers("operatorDoc", [
  "IndexDriftSourcePageDiagnosisRuntime",
  "diagnose_missing_entity_candidate(context, key)",
  "IndexSourceContinuationCodec",
  "does **not** yet compose that codec",
  "transport-neutral confidential continuation codec",
]);
requireMarkers("plan", [
  "M6 missing-only entity candidate outcome",
  "M6 bounded source-page missing-entity diagnosis",
  "M6 authenticated and confidential source continuation codec",
  "source_complete_server_key_composition_pending",
]);
requireMarkers("aggregate", [
  "'verify-index-drift-source-page-diagnosis.mjs'",
  "'verify-index-source-continuation.mjs'",
]);

for (const claim of [
  "tests passed",
  "source-page transport is complete",
  "server continuation composition is complete",
  "retained evidence admitted",
  "repair is complete",
]) {
  if (
    content.doc.toLowerCase().includes(claim.toLowerCase()) ||
    content.operatorDoc.toLowerCase().includes(claim.toLowerCase()) ||
    content.plan.toLowerCase().includes(claim.toLowerCase())
  ) {
    throw new Error(`forbidden completion claim: ${claim}`);
  }
}

console.log("Index one-page missing-entity diagnosis contract verified");
