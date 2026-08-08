#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  source: "crates/rustok-index/src/application/source_continuation.rs",
  applicationMod: "crates/rustok-index/src/application/mod.rs",
  cargo: "crates/rustok-index/Cargo.toml",
  pageRuntime: "apps/server/src/services/index_drift_source_page_diagnosis.rs",
  exactGraphql: "apps/server/src/graphql/index_drift_diagnosis.rs",
  pageGraphql: "apps/server/src/graphql/index_drift_source_page_diagnosis.rs",
  doc: "crates/rustok-index/docs/m6-source-continuation-codec.md",
  serverDoc: "crates/rustok-index/docs/m6-source-continuation-server-keyring.md",
  plan: "crates/rustok-index/docs/implementation-plan-current-2026-08-08.md",
  agents: "AGENTS.md",
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
  'b"rustok-index-source-continuation"',
  "const KEY_BYTES: usize = 32;",
  "const NONCE_BYTES: usize = 12;",
  "const MAX_KEYS: usize = 16;",
  "const MAX_LIFETIME_MILLIS: u128 = 15 * 60 * 1_000;",
  "const MAX_CLOCK_SKEW_MILLIS: i64 = 30 * 1_000;",
  "pub struct IndexSourceContinuationScope",
  "locale: Option<LocaleKey>",
  "pub fn from_registry(",
  "pub fn for_locale(",
  ".source_for_schema(&schema)",
  "descriptor.owner_module()",
  "descriptor.source_name()",
  "pub fn locale(&self) -> Option<&LocaleKey>",
  "pub struct IndexSourceContinuationToken(String);",
  "pub struct IndexSourceContinuationCodec",
  "keys: Arc<BTreeMap<String, [u8; KEY_BYTES]>>",
  "Aes256Gcm::new_from_slice",
  "OsRng.fill_bytes(&mut nonce);",
  "pub fn seal(",
  "pub fn open_encoded(",
  "validate_claims(&claims, expected_scope, now)?;",
  "claims.locale != expected_scope.locale",
  "IndexSourceContinuationError::LocaleScopeMismatch",
  "IndexSourceContinuationError::TenantMismatch",
  "IndexSourceContinuationError::SchemaMismatch",
  "IndexSourceContinuationError::SourceOwnerMismatch",
  "IndexSourceContinuationError::SourceNameMismatch",
  "IndexSourceContinuationError::IssuedAtInFuture",
  "IndexSourceContinuationError::Expired",
  "IndexSourceContinuationError::KeyUnavailable",
  "sealed_cursor_round_trips_only_under_exact_scope",
  "schema_wide_and_exact_locale_continuations_cannot_cross_scopes",
  "tampering_fails_authentication",
  "rotation_decodes_retained_old_key_and_rejects_removed_key",
  "token_and_codec_debug_do_not_expose_secret_material",
]);

const production = content.source.split("\n#[cfg(test)]")[0];
for (const forbidden of [
  "sea_orm",
  "DatabaseConnection",
  "tokio::spawn",
  "async_graphql",
  "std::env",
  "SecretResolverRegistry",
  "CONTINUATION_VERSION",
  "LEGACY_CONTINUATION",
  "ContinuationClaimsV1",
  "ContinuationClaimsV2",
  "UnsupportedVersion",
  "ContractVersionMismatch",
  "-continuation-v1",
  "-continuation-v2",
]) {
  if (production.includes(forbidden)) {
    throw new Error(`source continuation codec contains forbidden dependency/legacy marker: ${forbidden}`);
  }
}

const decode = production.indexOf("let decoded = URL_SAFE_NO_PAD.decode(token.as_str())?;");
const decodedBound = production.indexOf("if decoded.len() > MAX_DECODED_TOKEN_BYTES", decode);
const decrypt = production.indexOf(".decrypt(", decodedBound);
const claims = production.indexOf("let claims: ContinuationClaims", decrypt);
const validate = production.indexOf("validate_claims(&claims, expected_scope, now)?;", claims);
if (decode < 0 || decodedBound <= decode || decrypt <= decodedBound || claims <= decrypt || validate <= claims) {
  throw new Error("continuation opening must bound, authenticate, decode, then validate claims");
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
  "next_token: Option<rustok_index::IndexSourceContinuationToken>",
  "pub async fn diagnose_source_page_sealed(",
]);
for (const leaked of [
  "IndexSourceContinuationCodec",
  "IndexSourceContinuationToken",
  "IndexSourceCursor",
]) {
  if (content.exactGraphql.includes(leaked)) {
    throw new Error(`source continuation leaked into exact GraphQL transport: ${leaked}`);
  }
}

requireMarkers("pageGraphql", [
  "pub continuation: Option<String>",
  "const MAX_CONTINUATION_BYTES: usize = 16 * 1024;",
  ".diagnose_source_page_sealed(",
  "continuation.as_deref()",
]);
const pageGraphqlProduction = content.pageGraphql.split("\n#[cfg(test)]")[0];
for (const forbidden of [
  "IndexSourceCursor",
  "IndexSourceContinuationKeyringRuntime",
  "SecretRef",
  "IndexSourceContinuationCodec",
  ".diagnose_source_page(",
]) {
  if (pageGraphqlProduction.includes(forbidden)) {
    throw new Error(`sealed GraphQL transport contains ${forbidden}`);
  }
}

requireMarkers("doc", [
  "Status: `source_complete_locale_scope_owner_execution_pending`.",
  "one current unversioned continuation envelope",
  "AES-256-GCM",
  "fresh 96-bit operating-system nonce",
  "canonical owner module",
  "canonical source name",
  "exact canonical `LocaleKey`",
  "A schema-wide token cannot open",
  "There is no internal continuation version byte",
  "between 1 second and 15 minutes",
  "A token naming a removed key fails closed",
  "No tests, verifiers, formatting, Cargo checks",
]);
requireMarkers("serverDoc", [
  "Status: `source_complete_owner_execution_pending`.",
  "single current unversioned continuation envelope",
  "SecretRef",
  "exactly 32 bytes",
  "Key rotation preserves cryptographic key continuity only",
]);
requireMarkers("agents", [
  "Unreleased tokens, cursors, envelopes, and other repository-owned serialized",
  "Delete prior-format readers, writers, version bytes/tags, compatibility fixtures,",
  "Do not introduce `V1`/`V2` claim structs",
]);
requireMarkers("plan", [
  "Make Shadow continuation identity locale-safe before exposing exact-locale Shadow GraphQL transport.",
]);
requireMarkers("aggregate", ["'verify-index-source-continuation.mjs'"]);

for (const claim of [
  "tests passed",
  "retained evidence admitted",
]) {
  if (content.doc.toLowerCase().includes(claim.toLowerCase())) {
    throw new Error(`continuation documentation makes forbidden completion claim: ${claim}`);
  }
}

console.log("Index confidential source continuation contract verified with canonical schema-wide/exact-locale scope and no legacy format family");
