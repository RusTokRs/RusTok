#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  transport: "apps/server/src/graphql/index_drift_diagnosis.rs",
  graphqlMod: "apps/server/src/graphql/mod.rs",
  schema: "apps/server/src/graphql/schema.rs",
  operator: "apps/server/src/services/index_drift_diagnosis_operator.rs",
  doc: "apps/server/docs/index-drift-diagnosis-graphql-transport.md",
  operatorDoc: "apps/server/docs/index-reconciliation-operator-runtime.md",
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
      throw new Error(`${files[name]} missing ${marker}`);
    }
  }
}

requireMarkers("transport", [
  "pub struct IndexDriftDiagnosisInput",
  "pub module_name: String",
  "pub entity_name: String",
  "pub schema_version: String",
  "pub entity_id: String",
  "pub locale: Option<String>",
  "pub struct IndexDriftDiagnosisPayload",
  "pub struct IndexDriftDiagnosisMutation",
  "async fn diagnose_index_entity(",
  "prepare_authorized_request(tenant.id, auth.user_id, input)",
  "permissions_for(&tenant_id, &actor_id)",
  "has_effective_permission(&permissions, &Permission::MODULES_MANAGE)",
  "let key = parse_entity_key(tenant_id, input)?;",
  ".get::<IndexDriftDiagnosisOperatorRuntime>()",
  ".diagnose_entity(operator_context, key)",
  "IndexDriftDigestOutcome::Consistent",
  "IndexDriftDigestOutcome::MismatchRecorded",
  "dependency_code",
  "transport_authorizes_before_parsing_untrusted_input",
  "transport_derives_tenant_and_builds_one_exact_key",
]);

const prepareStart = content.transport.indexOf("fn prepare_authorized_request(");
const permissionCheck = content.transport.indexOf(
  "let permissions = permissions_for(&tenant_id, &actor_id)",
  prepareStart,
);
const parseCall = content.transport.indexOf("let key = parse_entity_key(tenant_id, input)?;", prepareStart);
if (prepareStart < 0 || permissionCheck < 0 || parseCall < 0 || permissionCheck > parseCall) {
  throw new Error("GraphQL transport must authorize before parsing untrusted entity input");
}

const inputStart = content.transport.indexOf("pub struct IndexDriftDiagnosisInput");
const inputEnd = content.transport.indexOf("}", inputStart);
const inputBlock = content.transport.slice(inputStart, inputEnd);
for (const forbidden of ["tenant", "actor", "user_id", "Vec<", "Uuid"]) {
  if (inputBlock.includes(forbidden)) {
    throw new Error(`GraphQL diagnosis input contains forbidden caller authority: ${forbidden}`);
  }
}

const production = content.transport.split("\n#[cfg(test)]")[0];
for (const forbidden of [
  "DatabaseConnection",
  "sea_orm",
  "tokio::spawn",
  "scan(",
  "repair",
  "resolve_finding",
  "ignore_finding",
  "request_cancel",
  "requeue_dead_letter",
  "Vec<IndexDriftDiagnosisInput>",
  "Vec<rustok_index::EntityKey>",
]) {
  if (production.includes(forbidden)) {
    throw new Error(`GraphQL diagnosis transport contains forbidden capability: ${forbidden}`);
  }
}

requireMarkers("graphqlMod", ["pub mod index_drift_diagnosis;"]);
requireMarkers("schema", [
  "use super::index_drift_diagnosis::IndexDriftDiagnosisMutation;",
  "RootMutation,\n    IndexDriftDiagnosisMutation,",
]);
requireMarkers("operator", [
  "pub async fn diagnose_entity(",
  "authorize_for(&context, key.tenant_id)?;",
  "IndexDriftDigestRequest::new(key)?;",
]);
requireMarkers("doc", [
  "Status: `source_complete_owner_execution_pending`",
  "diagnoseIndexEntity(input: $input)",
  "Tenant and actor identities are never accepted",
  "before parsing module/entity identifiers, schema version, UUID, or locale",
  "delegates once to `diagnose_entity(context, key)`",
  "no owner payload",
  "retained authorization, PostgreSQL, GraphQL execution, or CI evidence",
]);
requireMarkers("operatorDoc", ["IndexDriftDiagnosisOperatorRuntime"]);
requireMarkers("plan", ["M6 guarded exact-entity drift diagnosis operator"]);

for (const claim of [
  "tests passed",
  "GraphQL execution passed",
  "retained evidence admitted",
  "discovery is complete",
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

console.log("Index GraphQL drift diagnosis transport contract verified");
