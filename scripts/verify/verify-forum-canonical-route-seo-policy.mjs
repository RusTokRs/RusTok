#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

const paths = {
  seo: "crates/rustok-forum/src/seo_audience_targets.rs",
  legacySeo: "crates/rustok-forum/src/seo_targets.rs",
  categoryHost: "apps/storefront/src/forum_category_route.rs",
  topicHost: "apps/storefront/src/forum_topic_route.rs",
  contract: "crates/rustok-forum/contracts/forum-canonical-route-seo-policy.json",
  contractTest: "crates/rustok-forum/tests/canonical_route_seo_policy_contract.rs",
  docs: "crates/rustok-forum/docs/forum-24p-canonical-route-seo-policy.md",
  discoveryVerifier: "scripts/verify/verify-forum-public-discovery-seo.mjs",
};

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireText(content, marker, label) {
  if (!content.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function forbidText(content, marker, label) {
  if (content.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const source = Object.fromEntries(
  Object.entries(paths).map(([key, value]) => [key, read(value)]),
);

let contract = null;
try {
  contract = JSON.parse(source.contract);
} catch (error) {
  failures.push(`${paths.contract}: invalid JSON (${error.message})`);
}

for (const marker of [
  "const MAX_FORUM_SEO_ALTERNATE_ROUTES: usize = 64",
  "ForumCategoryRouteService",
  "ForumTopicRouteService",
  "rewrite_category_target",
  "rewrite_topic_target",
  "parse_canonical_forum_route",
  "ForumTopicRouteDisposition::Gone",
  "BTreeMap<String, String>",
  "BTreeSet<String>",
  "record.template_fields.insert(\"locale\", effective_locale)",
  "record.template_fields.insert(\"route\", canonical_route)",
  "return category_provider().load_target(runtime, request).await",
  "return topic_provider().load_target(runtime, request).await",
]) {
  requireText(source.seo, marker, paths.seo);
}
for (const forbidden of [
  "CategoryService::new",
  "TopicService::new",
  "SecurityContext::system()",
  "format!(\"/modules/forum?category=",
]) {
  forbidText(source.seo, forbidden, paths.seo);
}

for (const marker of [
  "map_category_response",
  "map_topic_response",
  "schema::collection_page_with_image",
  "schema::discussion_forum_posting_with_image",
]) {
  requireText(source.legacySeo, marker, paths.legacySeo);
}

for (const [content, label, markers] of [
  [
    source.categoryHost,
    paths.categoryHost,
    [
      "resolve_storefront_category_route",
      "query_params.remove(\"topic\")",
      "fetch_seo_page_context(",
      "seo_context.as_ref()",
      "failed to resolve Forum category SEO context",
      "ForumCategoryHostAction::Redirect",
    ],
  ],
  [
    source.topicHost,
    paths.topicHost,
    [
      "resolve_storefront_topic_route",
      "query_params.remove(\"category\")",
      "fetch_seo_page_context(",
      "seo_context.as_ref()",
      "failed to resolve Forum topic SEO context",
      "ForumTopicHostAction::Gone",
      "fn valid_topic_descriptor",
      "!path.starts_with(\"//\")",
      "!path.chars().any(char::is_control)",
    ],
  ],
]) {
  for (const marker of markers) requireText(content, marker, label);
}

for (const marker of [
  "seo_wrapper_uses_route_owners_for_canonical_and_alternate_paths",
  "canonical_route_handlers_compose_head_without_replacing_route_authority",
  "contract_preserves_visibility_schema_and_compatibility_boundaries",
  "stale_public_discovery_guard_tracks_current_canonical_card_routes",
]) {
  requireText(source.contractTest, marker, paths.contractTest);
}

for (const marker of [
  "source-ready / maintainer execution pending",
  "Legacy UUID module routes remain accepted",
  "deduplicated and sorted",
  "An owner-authorized topic `gone` decision produces no SEO target",
  "optional SEO transport failure",
  "This slice does not emit `QAPage`",
  "No tests, Node verifiers, formatting, Cargo commands",
]) {
  requireText(source.docs, marker, paths.docs);
}

for (const marker of [
  "category_provider().load_target",
  "topic_provider().load_target",
  "ForumCategoryRouteService",
  "ForumTopicRouteService",
  "retired category UUID card route",
  "retired topic UUID card route",
]) {
  requireText(source.discoveryVerifier, marker, paths.discoveryVerifier);
}

if (contract) {
  if (contract.task !== "FORUM-24P") {
    failures.push(`${paths.contract}: task must be FORUM-24P`);
  }
  if (contract.status !== "source_ready_maintainer_execution_pending") {
    failures.push(`${paths.contract}: unexpected source status`);
  }
  if (
    contract.canonical_routes?.legacy_uuid_module_routes_emitted_as_public_canonical !==
    false
  ) {
    failures.push(`${paths.contract}: UUID module routes must not remain public canonical URLs`);
  }
  if (
    contract.canonical_routes?.legacy_uuid_module_routes_emitted_as_public_alternates !==
    false
  ) {
    failures.push(`${paths.contract}: UUID module routes must not remain public alternates`);
  }
  if (contract.canonical_routes?.managed_authoring_legacy_load_preserved !== true) {
    failures.push(`${paths.contract}: managed authoring load must remain unchanged`);
  }
  if (contract.hreflang?.maximum_alternates !== 64) {
    failures.push(`${paths.contract}: alternate routes must remain bounded to 64`);
  }
  if (contract.hreflang?.available_locale_fallback_not_emitted_as_exact_alternate !== true) {
    failures.push(`${paths.contract}: fallback locales must not masquerade as exact alternates`);
  }
  if (contract.route_resolution?.authorized_topic_gone_excluded_from_seo !== true) {
    failures.push(`${paths.contract}: topic gone must remain absent from SEO`);
  }
  if (contract.public_discovery?.private_pending_archived_or_hidden_targets_absent !== true) {
    failures.push(`${paths.contract}: private and hidden targets must remain absent`);
  }
  if (contract.document?.topic_schema !== "DiscussionForumPosting") {
    failures.push(`${paths.contract}: topic schema must remain DiscussionForumPosting`);
  }
  if (contract.document?.question_answer_schema_added !== false) {
    failures.push(`${paths.contract}: QAPage semantics must remain deferred`);
  }
  if (contract.rust_storefront?.seo_failure_turns_public_route_into_outage !== false) {
    failures.push(`${paths.contract}: optional SEO failure must not become route outage`);
  }
  if (contract.compatibility?.search_result_routes_changed !== false) {
    failures.push(`${paths.contract}: Search route cutover is outside this slice`);
  }
  if (contract.compatibility?.next_storefront_changed !== false) {
    failures.push(`${paths.contract}: Next storefront parity is outside this slice`);
  }
  if (contract.compatibility?.new_migration !== false) {
    failures.push(`${paths.contract}: no migration is allowed`);
  }
  if (contract.verification?.executed_by_implementation_agent !== false) {
    failures.push(`${paths.contract}: execution must not be claimed`);
  }
}

if (failures.length > 0) {
  console.error("forum canonical route SEO policy verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum canonical route SEO policy verification passed");
