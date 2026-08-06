#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  transport: "apps/server/src/graphql/index_drift_source_page_diagnosis.rs",
  exactTransport: "apps/server/src/graphql/index_drift_diagnosis.rs",
  schema: "apps/server/src/graphql/schema.rs",
  service: "apps/server/src/services/index_drift_source_page_diagnosis.rs",
  doc: "apps/server/docs/index-drift-source-page-graphql-transport.md",
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

requireMarkers("transport", [
  "pub struct IndexDriftSourcePageDiagnosisInput",
  "pub module_name: String",
  "pub entity_name: String",
  "pub schema_version: String",
  "pub limit: String",
  "pub continuation: Option<String>",
  "const MAX_CONTINUATION_BYTES: usize = 16 * 1024;",
  "const MAX_PAGE_LIMIT: usize = 32;",
  "pub struct IndexDriftSourcePageDiagnosisPayload",
  "pub findings: Vec<IndexDriftSourcePageFindingReceipt>",
  "pub complete: bool",
  "async fn diagnose_index_source_page(",
  "ctx.data::<AuthContext>()",
  "ctx.data::<TenantContext>()?",
  "prepare_authorized_source_page_request(tenant.id, auth.user_id, input)",
  ".get::<IndexDriftSourcePageDiagnosisRuntime>()",
  ".diagnose_source_page_sealed(",
  "continuation.as_deref()",
  "permissions_for(&tenant_id, &actor_id)",
  "has_effective_permission(&permissions, &Permission::MODULES_MANAGE)",
  "let schema = parse_schema(",
  "bounded_text(\"limit\"",
  "bounded_text(\"continuation\"",
  "INDEX_SOURCE_CONTINUATION_INVALID",
  "INDEX_SOURCE_CONTINUATION_EXPIRED",
  "INDEX_SOURCE_PAGE_DEPENDENCY_FAILED",
  "source_page_transport_authorizes_before_parsing_untrusted_input",
  "source_page_transport_builds_one_schema_and_bounded_page_request",
]);

const production = content.transport.split("\n#[cfg(test)]")[0];
const preparation = production.indexOf("fn prepare_authorized_source_page_request(");
const permission = production.indexOf(
  "let permissions = permissions_for(&tenant_id, &actor_id)",
  preparation,
);
const schema = production.indexOf("let schema = parse_schema(", permission);
const limit = production.indexOf('bounded_text("limit"', schema);
const token = production.indexOf('bounded_text("continuation"', limit);
if (
  preparation < 0 ||
  permission <= preparation ||
  schema <= permission ||
  limit <= schema ||
  token <= limit
) {
  throw new Error(
    "source-page GraphQL transport must authorize before schema, limit, and token parsing",
  );
}

const resolver = production.slice(
  production.indexOf("    async fn diagnose_index_source_page("),
  production.indexOf("\n}\n\nfn prepare_authorized_source_page_request(")
);
if ((resolver.match(/\.diagnose_source_page_sealed\(/g) ?? []).length !== 1) {
  throw new Error("source-page GraphQL transport must delegate exactly once to the sealed method");
}
if (resolver.includes(".diagnose_source_page(")) {
  throw new Error("source-page GraphQL transport must never call the raw cursor method");
}

for (const forbidden of [
  "IndexSourceCursor",
  "IndexSourceContinuationKeyringRuntime",
  "SecretRef",
  "RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON",
  "DatabaseConnection",
  "sea_orm",
  "tokio::spawn",
  "spawn_blocking",
  "SELECT ",
  "INSERT ",
  "UPDATE ",
  "DELETE FROM",
  "while ",
  "loop {",
  "entity_id: String",
  "tenant_id: String",
  "actor_id: String",
  "source_name: String",
  "owner_module: String",
  "repair_finding",
  "resolve_finding",
  "ignore_finding",
]) {
  if (production.includes(forbidden)) {
    throw new Error(`source-page GraphQL transport contains forbidden capability: ${forbidden}`);
  }
}

requireMarkers("schema", [
  "use super::index_drift_source_page_diagnosis::IndexDriftSourcePageDiagnosisMutation;",
  "IndexDriftSourcePageDiagnosisMutation,",
]);
requireMarkers("service", [
  "pub async fn diagnose_source_page_sealed(",
  "IndexDriftSourcePageDiagnosisSealedOutcome",
]);

for (const forbidden of [
  "diagnose_index_source_page",
  "IndexDriftSourcePageDiagnosisRuntime",
  "IndexDriftSourcePageDiagnosisSealedOutcome",
  "IndexSourceContinuationToken",
]) {
  if (content.exactTransport.includes(forbidden)) {
    throw new Error(`exact diagnosis transport gained source-page authority: ${forbidden}`);
  }
}

requireMarkers("doc", [
  "Status: `source_complete_owner_execution_pending`.",
  "diagnoseIndexSourcePage(input: IndexDriftSourcePageDiagnosisInput!)",
  "authorization runs before schema, limit, or continuation parsing",
  "delegates exactly once to `diagnose_source_page_sealed`",
  "No raw `IndexSourceCursor`",
  "No tests, verifiers, formatting, Cargo checks",
]);
requireMarkers("plan", [
  "M6 bounded GraphQL sealed source-page diagnosis transport",
  "source_complete_owner_execution_pending",
]);
requireMarkers("aggregate", [
  "'verify-index-drift-source-page-graphql-transport.mjs'",
]);

console.log("Index sealed source-page GraphQL transport contract verified");
