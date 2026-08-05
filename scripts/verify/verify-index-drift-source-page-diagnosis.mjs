#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  runtime: "apps/server/src/services/index_drift_source_page_diagnosis.rs",
  composition: "apps/server/src/services/index_replay_runtime_composition.rs",
  graphql: "apps/server/src/graphql/index_drift_diagnosis.rs",
  graphqlSchema: "apps/server/src/graphql/schema.rs",
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
  ".diagnose_entity(context, key)",
  "permissions_for(&context.tenant_id(), &context.actor_id())",
  "has_effective_permission(&permissions, &Permission::MODULES_MANAGE)",
  "if !(1..=MAX_SOURCE_PAGE_DIAGNOSIS_SIZE).contains(&limit)",
  "IndexSourceScanRequest::new(context.tenant_id(), schema, cursor, limit)",
  "matches!(&mutation, rustok_index::IndexMutation::Delete { .. })",
  "receipts.push(receipt);",
  "materialize_index_drift_source_page_diagnosis",
  "authorization_precedes_page_limit_validation",
  "one_page_skips_deletes_and_diagnoses_each_upsert_once",
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
const exactDelegate = production.indexOf(".diagnose_entity(context, key)");
if (pageLoop < 0 || deleteSkip <= pageLoop || exactDelegate < 0) {
  throw new Error("source-page diagnosis must skip retained deletes and delegate exact candidates");
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
]) {
  if (content.graphql.includes(forbidden) || content.graphqlSchema.includes(forbidden)) {
    throw new Error(`source-page capability leaked into GraphQL: ${forbidden}`);
  }
}

requireMarkers("doc", [
  "Status: `source_complete_transport_and_owner_execution_pending`.",
  "one page limit in `1..=32`",
  "skips retained source `Delete` mutations",
  "source-present candidate",
  "The cursor is not attached to GraphQL",
  "missing-only selector over captured typed states remains a separate open slice",
  "No tests, verifiers, formatting, Cargo checks",
]);
requireMarkers("operatorDoc", ["IndexDriftSourcePageDiagnosisRuntime"]);
requireMarkers("plan", [
  "M6 bounded source-page drift candidate diagnosis",
  "source_complete_transport_and_owner_execution_pending",
]);
requireMarkers("aggregate", [
  "'verify-index-drift-source-page-diagnosis.mjs'",
]);

for (const claim of [
  "tests passed",
  "source-page transport is complete",
  "missing-only diagnosis is complete",
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

console.log("Index one-page source candidate diagnosis contract verified");
