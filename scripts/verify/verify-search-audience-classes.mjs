#!/usr/bin/env node

// Source-level guard for the generic Search audience mechanism.
//
// The mechanism is module-agnostic: `rustok-search` must never decide by itself which
// source module is gated, and a host that forgets to declare a module must fail closed.
// Runtime behavior is exercised by the Rust tests; this guard keeps the security
// invariants from being edited away silently.

import { existsSync, readdirSync, readFileSync } from "node:fs";
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
  scopePort: "crates/modules/rustok-search/src/storefront_category_scope.rs",
  hostForumPolicy: "apps/server/src/services/forum_search_result_eligibility.rs",
  hostForumScope: "apps/server/src/services/forum_search_category_scope.rs",
  projectors: {
    blog: "crates/modules/rustok-blog/src/services/search_projection.rs",
    content: "crates/modules/rustok-search/src/projector_core.rs",
    forum: "crates/modules/rustok-forum/src/search_projection.rs",
  },
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

const RETIRED_SEARCH_SYMBOLS = [
  "execute_forum_storefront_search",
  "forumStorefrontSearch",
  "StorefrontSearchResultEligibilityPort",
  "forum_storefront_execution",
  "storefront_result_eligibility",
  "forum_document_filters",
  "forum_current_channel_filter",
  "forum_graphql_adapter",
  "forum_native_server_adapter",
];

/** Flatten to `dotted.path: string` so a finding names the field it came from. */
function flattenStrings(value, prefix = "") {
  const out = {};
  if (typeof value === "string") {
    out[prefix || "(root)"] = value;
  } else if (Array.isArray(value)) {
    value.forEach((item, index) => Object.assign(out, flattenStrings(item, `${prefix}[${index}]`)));
  } else if (value && typeof value === "object") {
    for (const [key, item] of Object.entries(value)) {
      Object.assign(out, flattenStrings(item, prefix ? `${prefix}.${key}` : key));
    }
  }
  return out;
}

function forbidMissing(relativePaths, reason) {
  for (const relativePath of relativePaths) {
    if (existsSync(path.join(root, relativePath))) {
      failures.push(`${relativePath}: retired file must not exist (${reason})`);
    }
  }
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

// 7. The retired facet re-aggregation and the retired Forum storefront exports must not
//    come back through the public API.
forbid(paths.lib, [
  "build_authorized_facets",
  // The Forum module slug belongs to the owner: `rustok-search` must not export a
  // constant that names one module's corpus entry.
  "FORUM_SEARCH_SOURCE_MODULE",
  "StorefrontSearchResultEligibilityPort",
  "execute_forum_storefront_search",
  "forum_storefront_execution",
  "forum_document_filters",
  "storefront_result_eligibility",
  // The Forum-only storage driver is gone: one module-agnostic driver owns the
  // `search_documents` writes and the owner only declares its projection source.
  "ForumSearchProjector",
  "forum_projector",
]);

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

// 9. The legacy Forum storefront pipeline is retired, not wrapped. It had its own
//    executor, its own eligibility port and its own GraphQL field; the generic
//    audience executor and the host policy replace all three.
forbidMissing(
  [
    "crates/modules/rustok-search/src/forum_storefront_execution.rs",
    "crates/modules/rustok-search/src/forum_storefront_execution_public.rs",
    "crates/modules/rustok-search/src/storefront_result_eligibility.rs",
    "crates/modules/rustok-search/src/forum_document_filters.rs",
    "crates/modules/rustok-search/src/forum_current_channel_filter.rs",
    "crates/modules/rustok-search/src/graphql/forum_storefront.rs",
    "crates/modules/rustok-search/storefront/src/transport/forum_graphql_adapter.rs",
    "crates/modules/rustok-search/storefront/src/transport/forum_native_server_adapter.rs",
    "crates/modules/rustok-search/src/forum_projector.rs",
  ],
  "replaced by execute_audience_eligible_search, SearchAudiencePolicyPort and SearchProjectionDriver",
);
forbid(paths.audience, [
  "StorefrontSearchResultEligibilityPort",
  "resolve_storefront_search_result_candidates",
]);
forbid(paths.graphql, ["forum_storefront_search", "ForumStorefrontSearchQuery"]);
forbid(paths.engine, ["search_forum_storefront_candidates"]);

// 10. The category-scope capability is owner-declared and the engine stays neutral.
requireAll(paths.scopePort, [
  "pub trait StorefrontSearchCategoryScopePort",
  "fn source_module(&self) -> &'static str;",
  "async fn expand_category_scope(",
  "if !scope_is_exactly(&request.source_modules, port.source_module())",
  "pub fn scope_is_exactly(source_modules: &[String], module: &str) -> bool",
]);
forbid(paths.scopePort, [
  "is_explicit_forum_only",
  "expand_forum_category_scope",
]);
requireAll(paths.engine, [
  "OR id IN ({category_params})",
  "OR facets ->> 'category_id' IN ({category_facet_params})",
]);
forbid(paths.engine, ["source_module = 'forum'", "entity_type = 'forum_category'"]);

// 11. The host Forum policy implements the generic SPI directly.
requireAll(paths.hostForumScope, [
  "fn source_module(&self) -> &'static str",
  "async fn expand_category_scope(",
  "if !scope_is_exactly(&request.source_modules, self.source_module())",
]);
forbid(paths.hostForumScope, [
  "expand_forum_category_scope",
  "is_explicit_forum_only_source_scope",
]);
requireAll(paths.hostForumPolicy, [
  "impl SearchAudiencePolicyPort for ServerForumSearchResultEligibilityPort",
  "fn source_module(&self) -> &'static str",
  "async fn filter_eligible_candidates(",
]);
forbid(paths.hostForumPolicy, [
  "StorefrontSearchResultEligibilityPort",
  "StorefrontSearchResultCandidate",
]);

// 12. Forum Search contracts are documentation of live owners, so every repo path they
//     name must exist and none of them may still point at a retired symbol. This is one
//     rule for the whole directory instead of one hand-maintained list per contract.
const forumContractsDir = path.join(root, "crates/modules/rustok-forum/contracts");
for (const entry of readdirSync(forumContractsDir).sort()) {
  if (!entry.startsWith("forum-search-") || !entry.endsWith(".json")) continue;
  const relativePath = path.posix.join(
    "crates/modules/rustok-forum/contracts",
    entry,
  );
  const raw = readFileSync(path.join(root, relativePath), "utf8");
  let contract;
  try {
    contract = JSON.parse(raw);
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    continue;
  }
  for (const [key, value] of Object.entries(flattenStrings(contract))) {
    if (RETIRED_SEARCH_SYMBOLS.some((symbol) => value.includes(symbol))) {
      failures.push(`${relativePath}: ${key} still names a retired symbol: ${value}`);
    }
    const candidate = value.split(/\s+/)[0];
    if (!/^(crates|apps|scripts|DECISIONS|docs)\//.test(candidate)) continue;
    if (!/\.(rs|md|mjs|json|ts|tsx)$/.test(candidate)) continue;
    // A Rust module also lives as `foo/mod.rs`, so `src/foo.rs` is a valid spelling of it.
    const moduleDirectory = candidate.endsWith(".rs")
      ? `${candidate.slice(0, -3)}/mod.rs`
      : null;
    if (
      !existsSync(path.join(root, candidate)) &&
      !(moduleDirectory && existsSync(path.join(root, moduleDirectory)))
    ) {
      failures.push(`${relativePath}: ${key} points at a missing file: ${candidate}`);
    }
  }
}

if (failures.length > 0) {
  console.error("Search audience class verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Search audience class verification passed");
