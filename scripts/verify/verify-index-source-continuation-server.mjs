#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  keyring: "apps/server/src/services/index_source_continuation_runtime.rs",
  page: "apps/server/src/services/index_drift_source_page_diagnosis.rs",
  shadow: "apps/server/src/services/index_replay_shadow_transport.rs",
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

requireMarkers("keyring", [
  'const KEYRING_ENV: &str = "RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON";',
  "keys: BTreeMap<String, SecretRef>",
  "const KEY_BYTES: usize = 32;",
  "const ENCODED_KEY_BYTES: usize = 43;",
  "const MAX_CONFIG_BYTES: usize = 16 * 1024;",
  "const MAX_KEYS: usize = 16;",
  "const MAX_SECRET_REFERENCE_BYTES: usize = 256;",
  "if raw.len() > MAX_CONFIG_BYTES",
  "SecretAccessPolicy::Exact",
  "resolve_for_tenant(DEPLOYMENT_SECRET_SCOPE, reference)",
  "if encoded.len() != ENCODED_KEY_BYTES",
  "<[u8; KEY_BYTES]>::try_from(decoded.as_slice())",
  "IndexSourceContinuationCodec::new",
]);

const keyringProduction = content.keyring.split("\n#[cfg(test)]")[0];
for (const forbidden of [
  "DatabaseConnection",
  "tokio::spawn",
  "spawn_blocking",
  "SELECT ",
  "INSERT ",
  "UPDATE ",
  "DELETE FROM",
  "println!",
  "tracing::",
]) {
  if (keyringProduction.includes(forbidden)) {
    throw new Error(`server continuation keyring contains forbidden capability: ${forbidden}`);
  }
}
if (keyringProduction.includes("SecretString") || keyringProduction.includes("Vec<u8>")) {
  throw new Error("server continuation config must retain references, not raw secret material");
}

requireMarkers("page", [
  "pub struct IndexDriftSourcePageDiagnosisSealedOutcome",
  "next_token: Option<rustok_index::IndexSourceContinuationToken>",
  "pub async fn diagnose_source_page_sealed(",
  "authorize_context(context)?;",
  "validate_page_limit(limit)?;",
  "IndexSourceContinuationScope::from_registry(",
  ".resolve_codec()",
  "codec.open_encoded(&scope, encoded, Utc::now())",
  "IndexSourceScanRequest::new(",
  "self.diagnose_request(context, request).await?;",
  "codec.seal(&scope, cursor, Utc::now(), keyring.lifetime())",
]);

const pageProduction = content.page.split("\n#[cfg(test)]")[0];
const sealedStart = pageProduction.indexOf("    pub async fn diagnose_source_page_sealed(");
const sealedEnd = pageProduction.indexOf("\n    async fn diagnose_request(", sealedStart);
const sealed = pageProduction.slice(sealedStart, sealedEnd);
const auth = sealed.indexOf("authorize_context(context)?;");
const limit = sealed.indexOf("validate_page_limit(limit)?;", auth);
const open = sealed.indexOf("codec.open_encoded(&scope, encoded, Utc::now())", limit);
const request = sealed.indexOf("IndexSourceScanRequest::new(", open);
const diagnose = sealed.indexOf("self.diagnose_request(context, request).await?;", request);
const seal = sealed.indexOf("codec.seal(&scope, cursor, Utc::now(), keyring.lifetime())", diagnose);
if (
  sealedStart < 0 ||
  sealedEnd <= sealedStart ||
  auth < 0 ||
  limit <= auth ||
  open <= limit ||
  request <= open ||
  diagnose <= request ||
  seal <= diagnose
) {
  throw new Error(
    "sealed page must authorize, validate, open before request construction, diagnose once, then seal",
  );
}
if (sealed.includes("IndexSourceCursor") || sealed.includes("next_cursor(&self)")) {
  throw new Error("sealed page method must not expose a raw cursor type");
}

requireMarkers("shadow", [
  "pub struct IndexReplayShadowTransportRuntime",
  "locale: Option<rustok_index::LocaleKey>",
  "context.authorize_for(context.tenant_id())?;",
  "IndexSourceContinuationScope::for_locale(",
  "IndexSourceContinuationScope::from_registry(",
  ".resolve_codec()",
  "codec.open_encoded(&scope, encoded, Utc::now())",
  "IndexReplayDryRunRequest::for_locale(",
  "IndexReplayDryRunRequest::new(",
  "self.operator.run_shadow(context, request).await?;",
  "codec.seal(&scope, cursor, Utc::now(), keyring.lifetime())",
]);
const shadowProduction = content.shadow.split("\n#[cfg(test)]")[0];
for (const forbidden of [
  "DatabaseConnection",
  "IndexSourceContinuationKeyringRuntime>()",
  "extensions.insert(keyring)",
  "tokio::spawn",
  ".execute(",
]) {
  if (shadowProduction.includes(forbidden)) {
    throw new Error(`Shadow continuation adapter contains forbidden capability: ${forbidden}`);
  }
}

requireMarkers("composition", [
  '#[path = "index_source_continuation_runtime.rs"]',
  '#[path = "index_replay_shadow_transport.rs"]',
  "source_continuation_runtime::materialize_index_source_continuation_keyring()",
  "materialize_index_replay_shadow_transport(",
  "continuation.clone()",
  "materialize_index_drift_source_page_diagnosis(",
  "continuation,",
]);
if (
  content.composition.includes("extensions.insert(continuation)") ||
  content.composition.includes("extensions.insert(keyring)")
) {
  throw new Error("continuation keyring must remain private to sealed server runtimes");
}

for (const forbidden of [
  "diagnose_index_source_page",
  "IndexDriftSourcePageDiagnosisRuntime",
  "IndexDriftSourcePageDiagnosisSealedOutcome",
  "IndexSourceContinuationToken",
]) {
  if (content.exactGraphql.includes(forbidden)) {
    throw new Error(`exact diagnosis transport gained source-page authority: ${forbidden}`);
  }
}

requireMarkers("pageGraphql", [
  "async fn diagnose_index_source_page(",
  ".get::<IndexDriftSourcePageDiagnosisRuntime>()",
  ".diagnose_source_page_sealed(",
  "continuation.as_deref()",
  "pub continuation: Option<String>",
]);
const pageGraphqlProduction = content.pageGraphql.split("\n#[cfg(test)]")[0];
for (const forbidden of [
  "IndexSourceCursor",
  "IndexSourceContinuationKeyringRuntime",
  "SecretRef",
  "RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON",
  ".diagnose_source_page(",
]) {
  if (pageGraphqlProduction.includes(forbidden)) {
    throw new Error(`sealed source-page GraphQL transport contains ${forbidden}`);
  }
}

requireMarkers("graphqlSchema", [
  "use super::index_drift_source_page_diagnosis::IndexDriftSourcePageDiagnosisMutation;",
  "IndexDriftSourcePageDiagnosisMutation,",
]);
requireMarkers("doc", [
  "graphql_sealed_transport_source_complete_owner_execution_pending",
  "diagnose_source_page_sealed",
  "raw cursor is never returned",
]);
requireMarkers("transportDoc", [
  "diagnoseIndexSourcePage(input: IndexDriftSourcePageDiagnosisInput!)",
  "No raw `IndexSourceCursor`",
]);
requireMarkers("operatorDoc", [
  "diagnose_source_page_sealed",
  "diagnoseIndexSourcePage",
  "server-owned continuation keyring",
]);
requireMarkers("plan", [
  "M6 bounded GraphQL sealed source-page diagnosis transport",
  "source_complete_owner_execution_pending",
]);
requireMarkers("aggregate", [
  "'verify-index-source-continuation-server.mjs'",
  "'verify-index-drift-source-page-graphql-transport.mjs'",
  "'verify-index-replay-shadow-graphql-transport.mjs'",
]);

console.log("Index server sealed source continuation contract verified for drift-page plus schema-wide/exact-locale Shadow consumers");
