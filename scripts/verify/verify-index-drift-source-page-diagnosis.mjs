#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  producer: "crates/rustok-index/src/application/drift_digest.rs",
  runtime: "apps/server/src/services/index_drift_source_page_diagnosis.rs",
  diagnosis: "apps/server/src/services/index_drift_diagnosis_operator.rs",
  composition: "apps/server/src/services/index_replay_runtime_composition.rs",
  exactGraphql: "apps/server/src/graphql/index_drift_diagnosis.rs",
  pageGraphql: "apps/server/src/graphql/index_drift_source_page_diagnosis.rs",
  graphqlSchema: "apps/server/src/graphql/schema.rs",
  doc: "apps/server/docs/index-drift-source-page-diagnosis.md",
  transportDoc: "apps/server/docs/index-drift-source-page-graphql-transport.md",
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
  "matches!(pair.source(), IndexDriftEntityState::Upsert { .. })",
  "matches!(pair.materialized(), IndexDriftEntityState::Missing { .. })",
]);
requireMarkers("diagnosis", [
  "pub async fn diagnose_missing_entity_candidate(",
  ".produce_missing_entity_candidate(request)",
]);
requireMarkers("runtime", [
  "const MAX_SOURCE_PAGE_DIAGNOSIS_SIZE: usize = 32;",
  "pub struct IndexDriftSourcePageDiagnosisOutcome",
  "pub struct IndexDriftSourcePageDiagnosisSealedOutcome",
  "pub struct IndexDriftSourcePageDiagnosisRuntime",
  "continuation: Option<IndexSourceContinuationKeyringRuntime>",
  "pub async fn diagnose_source_page(",
  "pub async fn diagnose_source_page_sealed(",
  "let page = self.sources.scan(request).await?;",
  ".diagnose_missing_entity_candidate(context, key)",
  "IndexSourceContinuationScope::from_registry(",
  "codec.open_encoded(&scope, encoded, Utc::now())",
  "codec.seal(&scope, cursor, Utc::now(), keyring.lifetime())",
  "matches!(&mutation, rustok_index::IndexMutation::Delete { .. })",
  "IndexDriftMissingEntityCandidateOutcome::NotCandidate",
  "IndexDriftMissingEntityCandidateOutcome::MissingRecorded",
]);

const production = content.runtime.split("\n#[cfg(test)]")[0];
const sealedStart = production.indexOf("    pub async fn diagnose_source_page_sealed(");
const requestStart = production.indexOf("    async fn diagnose_request(", sealedStart);
const sealed = production.slice(sealedStart, requestStart);
const auth = sealed.indexOf("authorize_context(context)?;");
const limit = sealed.indexOf("validate_page_limit(limit)?;", auth);
const open = sealed.indexOf("codec.open_encoded(&scope, encoded, Utc::now())", limit);
const request = sealed.indexOf("IndexSourceScanRequest::new(", open);
const delegate = sealed.indexOf("self.diagnose_request(context, request).await?;", request);
const seal = sealed.indexOf("codec.seal(&scope, cursor, Utc::now(), keyring.lifetime())", delegate);
if (
  sealedStart < 0 ||
  requestStart <= sealedStart ||
  auth < 0 ||
  limit <= auth ||
  open <= limit ||
  request <= open ||
  delegate <= request ||
  seal <= delegate
) {
  throw new Error("sealed diagnosis must authorize, open, diagnose one page, then seal");
}

const requestBody = production.slice(requestStart);
if ((requestBody.match(/self\.sources\.scan\(request\)/g) ?? []).length !== 1) {
  throw new Error("source-page diagnosis must scan exactly one owner page");
}
if (production.includes(".diagnose_entity(context, key)")) {
  throw new Error("source-page diagnosis must not use the general mismatch recorder path");
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
  "repair_finding",
  "resolve_finding",
  "ignore_finding",
]) {
  if (production.includes(forbidden)) {
    throw new Error(`source-page diagnosis contains forbidden capability: ${forbidden}`);
  }
}

requireMarkers("composition", [
  '#[path = "index_source_continuation_runtime.rs"]',
  '#[path = "index_drift_source_page_diagnosis.rs"]',
  "source_continuation_runtime::materialize_index_source_continuation_keyring()",
  "drift_source_page_diagnosis::materialize_index_drift_source_page_diagnosis(",
]);

for (const forbidden of [
  "diagnose_index_source_page",
  "IndexDriftSourcePageDiagnosisRuntime",
  "IndexDriftSourcePageDiagnosisSealedOutcome",
  "diagnose_source_page_sealed",
]) {
  if (content.exactGraphql.includes(forbidden)) {
    throw new Error(`exact diagnosis transport gained source-page authority: ${forbidden}`);
  }
}
requireMarkers("pageGraphql", [
  "async fn diagnose_index_source_page(",
  ".diagnose_source_page_sealed(",
  "pub continuation: Option<String>",
  "pub complete: bool",
]);
const pageGraphqlProduction = content.pageGraphql.split("\n#[cfg(test)]")[0];
for (const forbidden of [
  "IndexSourceCursor",
  "IndexSourceContinuationKeyringRuntime",
  ".diagnose_source_page(",
  "entity_id: String",
  "source_name: String",
  "owner_module: String",
]) {
  if (pageGraphqlProduction.includes(forbidden)) {
    throw new Error(`source-page GraphQL transport contains ${forbidden}`);
  }
}
requireMarkers("graphqlSchema", [
  "IndexDriftSourcePageDiagnosisMutation,",
]);

requireMarkers("doc", [
  "Status: `graphql_sealed_transport_source_complete_owner_execution_pending`.",
  "one page limit in `1..=32`",
  "source `Upsert` plus materialized `Missing`",
  "diagnose_source_page_sealed",
  "raw cursor is never returned",
  "diagnoseIndexSourcePage",
]);
requireMarkers("transportDoc", [
  "diagnoseIndexSourcePage(input: IndexDriftSourcePageDiagnosisInput!)",
  "authorization runs before schema, limit, or continuation parsing",
]);
requireMarkers("operatorDoc", [
  "IndexDriftSourcePageDiagnosisRuntime",
  "diagnose_source_page_sealed",
  "diagnoseIndexSourcePage",
]);
requireMarkers("plan", [
  "M6 bounded GraphQL sealed source-page diagnosis transport",
  "source_complete_owner_execution_pending",
]);
requireMarkers("aggregate", [
  "'verify-index-drift-source-page-diagnosis.mjs'",
  "'verify-index-drift-source-page-graphql-transport.mjs'",
]);

console.log("Index one-page sealed missing-entity diagnosis contract verified");
