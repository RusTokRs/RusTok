#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  keyring: "apps/server/src/services/index_source_continuation_runtime.rs",
  page: "apps/server/src/services/index_drift_source_page_diagnosis.rs",
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

requireMarkers("keyring", [
  'const KEYRING_ENV: &str = "RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON";',
  "struct IndexSourceContinuationKeyringConfig",
  "active_key_id: String",
  "keys: BTreeMap<String, SecretRef>",
  "const KEY_BYTES: usize = 32;",
  "const ENCODED_KEY_BYTES: usize = 43;",
  "const MAX_CONFIG_BYTES: usize = 16 * 1024;",
  "const MAX_KEYS: usize = 16;",
  "const MAX_KEY_ID_BYTES: usize = 64;",
  "const MAX_SECRET_REFERENCE_BYTES: usize = 256;",
  "const MAX_LIFETIME_SECONDS: u64 = 15 * 60;",
  "if raw.len() > MAX_CONFIG_BYTES",
  "SecretResolverRegistry",
  "SecretAccessPolicy::Exact",
  "URL_SAFE_NO_PAD",
  "resolve_for_tenant(DEPLOYMENT_SECRET_SCOPE, reference)",
  "if encoded.len() != ENCODED_KEY_BYTES",
  "<[u8; KEY_BYTES]>::try_from(decoded.as_slice())",
  "valid_secret_reference(reference)",
  "IndexSourceContinuationCodec::new",
  "field(\"key_count\", &self.keys.len())",
  "resolves_exact_key_bytes_without_exposing_references_in_debug",
  "rejects_encoded_key_material_that_is_not_canonical_32_bytes",
  "rejects_duplicate_references_out_of_range_lifetime_and_unbounded_keys",
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
  ".map(|encoded| codec.open_encoded(&scope, encoded, Utc::now()))",
  "IndexSourceScanRequest::new(",
  "self.diagnose_request(context, request).await?;",
  "codec.seal(&scope, cursor, Utc::now(), keyring.lifetime())",
  "IndexDriftSourcePageDiagnosisSealedOutcome::from_raw(",
  "sealed_outcome_replaces_raw_cursor_with_opaque_token",
]);

const pageProduction = content.page.split("\n#[cfg(test)]")[0];
const sealedStart = pageProduction.indexOf("    pub async fn diagnose_source_page_sealed(");
const sealedEnd = pageProduction.indexOf("\n    async fn diagnose_request(", sealedStart);
if (sealedStart < 0 || sealedEnd <= sealedStart) {
  throw new Error("sealed source-page method segment is incomplete");
}
const sealed = pageProduction.slice(sealedStart, sealedEnd);
const auth = sealed.indexOf("authorize_context(context)?;");
const limit = sealed.indexOf("validate_page_limit(limit)?;", auth);
const scope = sealed.indexOf("IndexSourceContinuationScope::from_registry(", limit);
const resolve = sealed.indexOf(".resolve_codec()", scope);
const open = sealed.indexOf("codec.open_encoded(&scope, encoded, Utc::now())", resolve);
const request = sealed.indexOf("IndexSourceScanRequest::new(", open);
const diagnose = sealed.indexOf("self.diagnose_request(context, request).await?;", request);
const seal = sealed.indexOf("codec.seal(&scope, cursor, Utc::now(), keyring.lifetime())", diagnose);
if (
  auth < 0 ||
  limit <= auth ||
  scope <= limit ||
  resolve <= scope ||
  open <= resolve ||
  request <= open ||
  diagnose <= request ||
  seal <= diagnose
) {
  throw new Error(
    "sealed page must authorize, validate, resolve, open, build, diagnose once, then seal",
  );
}
if (sealed.includes("IndexSourceCursor") || sealed.includes("next_cursor(&self)")) {
  throw new Error("sealed page public method must not expose a raw cursor type");
}

requireMarkers("composition", [
  '#[path = "index_source_continuation_runtime.rs"]',
  "mod source_continuation_runtime;",
  "drift_diagnosis_operator::materialize_index_drift_diagnosis_operator(extensions, db)?;",
  "source_continuation_runtime::materialize_index_source_continuation_keyring()",
  "materialize_index_drift_source_page_diagnosis(",
  "continuation,",
]);
const exactComposition = content.composition.indexOf(
  "drift_diagnosis_operator::materialize_index_drift_diagnosis_operator(extensions, db)?;",
);
const keyringComposition = content.composition.indexOf(
  "source_continuation_runtime::materialize_index_source_continuation_keyring()",
);
const pageComposition = content.composition.indexOf(
  "drift_source_page_diagnosis::materialize_index_drift_source_page_diagnosis(",
);
if (
  exactComposition < 0 ||
  keyringComposition <= exactComposition ||
  pageComposition <= keyringComposition
) {
  throw new Error("sealed continuation must compose after exact diagnosis and before page runtime");
}
if (
  content.composition.includes("extensions.insert(continuation)") ||
  content.composition.includes("extensions.insert(keyring)")
) {
  throw new Error("continuation keyring must remain private to the page runtime");
}

for (const forbidden of [
  "IndexDriftSourcePageDiagnosisSealedOutcome",
  "diagnose_source_page_sealed",
  "IndexSourceContinuationToken",
  "RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON",
]) {
  if (content.graphql.includes(forbidden) || content.graphqlSchema.includes(forbidden)) {
    throw new Error(`sealed source-page capability leaked into GraphQL: ${forbidden}`);
  }
}

requireMarkers("doc", [
  "sealed_internal_source_complete_transport_and_owner_execution_pending",
  "RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON",
  "URL-safe unpadded base64",
  "exactly 32 bytes",
  "diagnose_source_page_sealed",
  "raw cursor is never returned",
]);
requireMarkers("operatorDoc", [
  "diagnose_source_page_sealed",
  "server-owned continuation keyring",
  "not attached to GraphQL",
]);
requireMarkers("plan", [
  "M6 server-owned source continuation keyring and sealed page boundary",
  "source_complete_transport_and_owner_execution_pending",
  "Add one bounded source-page transport",
]);
requireMarkers("aggregate", [
  "'verify-index-source-continuation-server.mjs'",
]);

console.log("Index server sealed source continuation contract verified");