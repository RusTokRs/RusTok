#!/usr/bin/env node

// Source-level guard for the generic Search audience mechanism.
//
// The mechanism is module-agnostic: `rustok-search` must never decide by itself which
// source module is gated, and a host that forgets to declare a module must fail closed.
// Runtime behavior is exercised by the Rust tests; this guard keeps the security
// invariants from being edited away silently.

import { readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");

const failures = [];

const paths = {
  audience: "crates/modules/rustok-search/src/audience_security.rs",
  engine: "crates/modules/rustok-search/src/pg_engine.rs",
  dictionaries: "crates/modules/rustok-search/src/dictionaries.rs",
  suggestions: "crates/modules/rustok-search/src/suggestions.rs",
  graphql: "crates/modules/rustok-search/src/graphql/query.rs",
  native:
    "crates/modules/rustok-search/storefront/src/transport/native_server_adapter.rs",
  serverServices: "apps/server/src/services/mod.rs",
  lib: "crates/modules/rustok-search/src/lib.rs",
  hostGraphqlSchema: "apps/server/src/graphql/schema.rs",
};

function read(relativePath) {
  return readFileSync(path.join(root, relativePath), "utf8");
}

function requireAll(relativePath, markers) {
  const text = read(relativePath);
  for (const marker of markers) {
    if (!text.includes(marker)) {
      failures.push(`${relativePath}: missing ${JSON.stringify(marker)}`);
    }
  }
  return text;
}

function forbid(relativePath, markers) {
  const text = read(relativePath);
  for (const marker of markers) {
    if (text.includes(marker)) {
      failures.push(`${relativePath}: forbidden ${JSON.stringify(marker)}`);
    }
  }
  return text;
}

// 1. Declared audience classes, no pass-through for undeclared modules, bounded batches.
requireAll(paths.audience, [
  "pub enum SearchAudienceClass",
  "pub fn declare_public(&mut self, source_module: &str)",
  "pub fn declared_source_modules(&self) -> Vec<String>",
  "pub const MAX_SEARCH_AUDIENCE_SCAN_DOCUMENTS: usize = 1_000;",
  "for batch in module_candidates.chunks(MAX_SEARCH_AUDIENCE_CANDIDATES)",
  // An undeclared module must be denied, never admitted.
  "Search audience registry has no declared audience class for a matching source module; denying its documents",
  // Policy infrastructure failures must not become caller validation errors.
  "PortErrorKind::Unavailable | PortErrorKind::Timeout => Error::External(message)",
  // A missing module (module disabled) excludes documents instead of failing the request.
  "PortErrorKind::NotFound | PortErrorKind::Forbidden",
]);
forbid(paths.audience, [
  "build_authorized_facets",
  "pub fn with_policy",
  ".expect(\"valid source module for search audience policy\")",
  // The generic SPI must not depend on the deprecated Forum pipeline.
  "forum_storefront_execution",
  "forum_visible_status",
  // No silent pass-through of unregistered modules.
  "Unconstrained public source modules",
]);

// 2. The engine carries only a neutral scope; module knowledge stays out of the fast path.
requireAll(paths.engine, [
  "pub struct SearchAudienceDocumentScope",
  "pub async fn search_storefront_scoped(",
  "sd.document_id AS document_id",
  "(source_module NOT IN ({gated_params}) OR document_id IN ({authorized_params}))",
]);
forbid(paths.engine, [
  "exclude_forum",
  "source_module <> 'forum'",
  "search_storefront_candidates_with_exclusions",
]);

// 3. Query rules cannot inject documents outside an authorized result.
requireAll(paths.dictionaries, [
  "allow_extra_pins: bool",
  "if !allow_extra_pins || has_catalog_filters(query)",
  "storefront_payload_visible_for_channel(&payload, &entity_type, channel)",
]);
forbid(paths.dictionaries, ["exclude_forum"]);

// 4. Suggestions obey the same declared corpus and the same candidate authorization.
requireAll(paths.suggestions, [
  "pub struct StorefrontSuggestionAudience",
  "fn suggestion_module_placeholders(",
  "source_module IN ({})",
  "async fn authorize_suggestion_documents(",
  "resolve_audience_eligible_candidates(",
]);
forbid(paths.suggestions, ["source_module <> 'forum'"]);

// 5. Storefront boundaries refuse to run without a composed registry.
// Search is a required module, so the manifest `runtime_data_factory` hook is not
// wired for it: the host must attach the composed registry to the GraphQL schema data
// explicitly. Without the attachment every storefront field fails closed.
requireAll(paths.hostGraphqlSchema, [
  ".shared_get::<rustok_search::SearchAudienceRegistry>()",
  "search audience registry must be composed before GraphQL schema construction",
  ".data(search_audience_registry)",
]);

requireAll(paths.graphql, [
  "SEARCH_AUDIENCE_REGISTRY_UNAVAILABLE_MESSAGE",
  "ctx\n            .data::<crate::SearchAudienceRegistry>()",
  "crate::StorefrontSuggestionAudience {",
]);
forbid(paths.graphql, ["unwrap_or(&default_registry)", "let default_registry"]);

requireAll(paths.native, [
  "Search audience registry is unavailable for this surface",
  "&registry,",
  "rustok_search::StorefrontSuggestionAudience {",
]);
forbid(paths.native, ["unwrap_or(&default_registry)", "let default_registry"]);

// 6. Host composition declares the corpus (including Content-owned CMS kinds) and
//    propagates registration errors.
const serverServices = requireAll(paths.serverServices, [
  "pub fn search_public_source_modules() -> Vec<&'static str>",
  "declare_public_modules(search_public_source_modules())",
  "audience_registry\n                .register(forum_eligibility)",
]);
if (serverServices.includes("let _ = audience_registry.register(")) {
  failures.push(
    `${paths.serverServices}: registration failures must not be discarded with let _ =`,
  );
}

// 7. The retired facet re-aggregation must not come back through the public API.
forbid(paths.lib, ["build_authorized_facets"]);

// 8. The corpus inventory must cover everything the Content projector can emit.
//    `projector_core` keys Content documents by node kind, so the write allowlist and the
//    public declaration must be the same closed list, plus the empty-kind fallback.
const contentValidation = "crates/modules/rustok-content/src/dto/validation.rs";
requireAll(contentValidation, [
  "pub const CONTENT_NODE_KINDS: &[&str]",
  "if CONTENT_NODE_KINDS.contains(&kind)",
]);
forbid(contentValidation, [
  // A second, drifting allowlist would let a kind be accepted but never declared.
  '"forum_category" | "forum_topic" | "forum_reply" => Ok(())',
]);
requireAll("crates/modules/rustok-search/src/projector_core.rs", [
  "COALESCE(NULLIF(n.kind::text, ''), 'content') AS source_module",
]);
requireAll(paths.serverServices, [
  "rustok_content::CONTENT_NODE_KINDS",
  '"content", "docs", "pages"',
]);

if (failures.length > 0) {
  console.error("Search audience class verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Search audience class verification passed");
