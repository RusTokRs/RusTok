#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  source: "crates/rustok-index/src/application/source_continuation.rs",
  applicationMod: "crates/rustok-index/src/application/mod.rs",
  cargo: "crates/rustok-index/Cargo.toml",
  pageRuntime: "apps/server/src/services/index_drift_source_page_diagnosis.rs",
  graphql: "apps/server/src/graphql/index_drift_diagnosis.rs",
  doc: "crates/rustok-index/docs/m6-source-continuation-codec.md",
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

requireMarkers("cargo", ["aes-gcm.workspace = true"]);
requireMarkers("applicationMod", [
  "mod source_continuation;",
  "IndexSourceContinuationCodec",
  "IndexSourceContinuationScope",
  "IndexSourceContinuationToken",
]);
requireMarkers("source", [
  'b"rustok-index-source-continuation-v1"',
  "const CONTINUATION_VERSION: u8 = 1;",
  "const KEY_BYTES: usize = 32;",
  "const NONCE_BYTES: usize = 12;",
  "const MAX_KEYS: usize = 16;",
  "const MAX_LIFETIME_MILLIS: u128 = 15 * 60 * 1_000;",
  "const MAX_CLOCK_SKEW_MILLIS: i64 = 30 * 1_000;",
  "pub struct IndexSourceContinuationScope",
  "pub fn from_registry(",
  ".source_for_schema(&schema)",
  "descriptor.owner_module()",
  "descriptor.source_name()",
  "pub struct IndexSourceContinuationToken(String);",
  "pub struct IndexSourceContinuationCodec",
  "active_key_id: String",
  "keys: Arc<BTreeMap<String, [u8; KEY_BYTES]>>",
  "Aes256Gcm::new_from_slice",
  "OsRng.fill_bytes(&mut nonce);",
  "Payload {",
  "pub fn seal(",
  "pub fn open_encoded(",
  "pub fn open(",
  "contract_version: CONTINUATION_VERSION",
  "tenant_id: scope.tenant_id",
  "schema: scope.schema.clone()",
  "owner_module: scope.owner_module.clone()",
  "source_name: scope.source_name.clone()",
  "cursor: cursor.clone()",
  "URL_SAFE_NO_PAD.encode(decoded)",
  "URL_SAFE_NO_PAD.decode(token.as_str())",
  "validate_claims(&claims, expected_scope, now)?;",
  "IndexSourceContinuationError::TenantMismatch",
  "IndexSourceContinuationError::SchemaMismatch",
  "IndexSourceContinuationError::SourceOwnerMismatch",
  "IndexSourceContinuationError::SourceNameMismatch",
  "IndexSourceContinuationError::IssuedAtInFuture",
  "IndexSourceContinuationError::Expired",
  "IndexSourceContinuationError::KeyUnavailable",
  "sealed_cursor_round_trips_only_under_exact_scope",
  "tampering_fails_authentication",
  "rotation_decodes_retained_old_key_and_rejects_removed_key",
  "token_and_codec_debug_do_not_expose_secret_material",
]);

const production = content.source.split("\n#[cfg(test)]")[0];
for (const forbidden of [
  "sea_orm",
  "DatabaseConnection",
  "SELECT ",
  "INSERT ",
  "UPDATE ",
  "DELETE FROM",
  "tokio::spawn",
  "spawn_blocking",
  "async_graphql",
  "std::env",
  "SecretResolverRegistry",
]) {
  if (production.includes(forbidden)) {
    throw new Error(`source continuation codec contains forbidden dependency: ${forbidden}`);
  }
}

const decode = production.indexOf("let decoded = URL_SAFE_NO_PAD.decode(token.as_str())?;");
const decodedBound = production.indexOf(
  "if decoded.len() > MAX_DECODED_TOKEN_BYTES",
  decode,
);
const decrypt = production.indexOf(".decrypt(", decodedBound);
const claims = production.indexOf("let claims: ContinuationClaims", decrypt);
const validate = production.indexOf("validate_claims(&claims, expected_scope, now)?;", claims);
if (decode < 0 || decodedBound <= decode || decrypt <= decodedBound || claims <= decrypt || validate <= claims) {
  throw new Error("continuation opening must bound, authenticate, decode, then validate claims");
}

const scopeValidation = production.indexOf("fn validate_claims(");
for (const marker of [
  "claims.tenant_id != expected_scope.tenant_id",
  "claims.schema != expected_scope.schema",
  "claims.owner_module != expected_scope.owner_module",
  "claims.source_name != expected_scope.source_name",
  "claims.issued_at_unix_millis > latest_acceptable_issue",
  "claims.expires_at_unix_millis <= now",
]) {
  if (production.indexOf(marker, scopeValidation) <= scopeValidation) {
    throw new Error(`claim validation is missing ${marker}`);
  }
}

for (const secretLeak of [
  ".field(\"keys\"",
  ".field(\"cursor\"",
  "derive(Debug, Clone, PartialEq, Eq)\npub struct IndexSourceContinuationToken",
]) {
  if (production.includes(secretLeak)) {
    throw new Error(`continuation Debug boundary leaks secret material: ${secretLeak}`);
  }
}

requireMarkers("pageRuntime", [
  "next_cursor: Option<rustok_index::IndexSourceCursor>",
  "The continuation cursor remains server-owned and is not attached to GraphQL",
]);
for (const leaked of [
  "IndexSourceContinuationCodec",
  "IndexSourceContinuationToken",
  "IndexSourceCursor",
]) {
  if (content.graphql.includes(leaked)) {
    throw new Error(`source continuation capability leaked into exact GraphQL transport: ${leaked}`);
  }
}

requireMarkers("doc", [
  "Status: `source_complete_server_key_composition_pending`.",
  "AES-256-GCM",
  "fresh 96-bit operating-system nonce",
  "canonical owner module",
  "canonical source name",
  "between 1 second and 15 minutes",
  "A token naming a removed or otherwise unavailable key fails closed",
  "does not add or claim",
  "No tests, verifiers, formatting, Cargo checks",
]);
requireMarkers("plan", [
  "M6 authenticated and confidential source continuation codec",
  "source_complete_server_key_composition_pending",
]);
requireMarkers("aggregate", ["'verify-index-source-continuation.mjs'"]);

for (const claim of [
  "tests passed",
  "server key composition is complete",
  "source-page transport is complete",
  "retained evidence admitted",
]) {
  if (content.doc.toLowerCase().includes(claim.toLowerCase())) {
    throw new Error(`continuation documentation makes forbidden completion claim: ${claim}`);
  }
}

console.log("Index confidential source continuation contract verified");
