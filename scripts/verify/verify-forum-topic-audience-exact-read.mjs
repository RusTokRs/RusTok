#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath ?? "");
  if (!relativePath || !existsSync(absolute)) {
    failures.push(`${relativePath || "<missing path>"}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function rejectText(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

function requireOrder(source, first, second, message) {
  const firstIndex = source.indexOf(first);
  const secondIndex = source.indexOf(second);
  if (firstIndex < 0 || secondIndex < 0 || firstIndex >= secondIndex) {
    failures.push(message);
  }
}

const contractPath =
  "crates/rustok-forum/contracts/forum-topic-audience-exact-read.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.owner_service);
const sourceTest = read(contract.source_test);
const ownerNote = read(contract.owner_note);
const crateApi = read(contract.crate_api);
const servicesMod = read("crates/rustok-forum/src/services/mod.rs");
const crateRoot = read("crates/rustok-forum/src/lib.rs");
const upstream = read(
  "crates/rustok-forum/contracts/forum-audience-plan-sync.json",
);

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20BB" ||
  contract.upstream_task !== "FORUM-20BA" ||
  contract.downstream_task !== "FORUM-20BC"
) {
  failures.push("FORUM-20BB contract identity is invalid");
}

for (const marker of [
  "pub struct ForumTopicAudienceReadService",
  "pub fn with_audience_facts(",
  "get_public_storefront_visible_with_locale_fallback",
  "get_authenticated_storefront_visible_with_audience_context",
  "ForumTopicAudienceViewer::public()",
  "ForumTopicAudienceViewer::authenticated(security.clone(), context)",
  "let locale = context.locale.trim().to_string();",
  "let channel_slug = context.channel.clone();",
  ".is_topic_visible(tenant_id, topic_id, channel_slug, viewer)",
  ".get_with_locale_fallback(tenant_id, security, topic_id, locale, fallback_locale)",
  "Err(ForumError::TopicNotFound(_)) => Ok(None)",
]) {
  requireText(owner, marker, `exact topic audience read owner is missing ${marker}`);
}

requireOrder(
  owner,
  "ForumTopicAudienceViewer::authenticated(security.clone(), context)",
  ".is_topic_visible(tenant_id, topic_id, channel_slug, viewer)",
  "authenticated context must validate before exact visibility evaluation",
);
requireOrder(
  owner,
  ".is_topic_visible(tenant_id, topic_id, channel_slug, viewer)",
  ".get_with_locale_fallback(tenant_id, security, topic_id, locale, fallback_locale)",
  "topic hydration must occur only after exact richer visibility succeeds",
);

for (const forbidden of [
  "crate::entities::",
  "forum_topic::Entity",
  "forum_category_audience_",
  "forum_topic_audience_",
  "crate::controllers",
  "crate::graphql",
  "HostRuntimeContext",
  "AuthContext",
  "RequestContext",
]) {
  rejectText(owner, forbidden, `owner service crosses its boundary through ${forbidden}`);
}

for (const marker of [
  "mod topic_audience_read;",
  "pub use topic_audience_read::ForumTopicAudienceReadService;",
]) {
  requireText(servicesMod, marker, `services module is missing ${marker}`);
}
requireText(
  crateRoot,
  "ForumTopicAudienceReadService",
  "crate root does not publish ForumTopicAudienceReadService",
);

for (const marker of [
  "CREATE TABLE users",
  "roles_any: vec![UserRole::Customer]",
  "minimum_trust_level: Some(5)",
  "deny_user_ids: vec![explicitly_denied_user_id]",
  "public rejection must not call optional owner facts",
  "trusted exact read should hydrate the topic",
  "route-channel miss should resolve as absent",
  "Err(ForumError::CapabilityUnavailable { .. })",
  "tenant does not match",
  "actor does not match",
  "missing topic should resolve as absent",
]) {
  requireText(sourceTest, marker, `source-ready SQLite proof is missing ${marker}`);
}

for (const marker of [
  "ForumTopicAudienceReadService",
  "FORUM-20BB",
  "effective locale and route channel come only from that context",
  "No existing `TopicService` method or transport call site changes",
  "FORUM-20BC",
  "canonical implementation plan",
  "did not run tests",
]) {
  requireText(ownerNote, marker, `FORUM-20BB owner note is missing ${marker}`);
}

for (const marker of [
  "pub struct ForumTopicAudienceReadService",
  "get_public_storefront_visible_with_locale_fallback",
  "get_authenticated_storefront_visible_with_audience_context",
  "### Exact storefront topic audience read",
  "FORUM-20BC",
]) {
  requireText(crateApi, marker, `CRATE_API is missing ${marker}`);
}

for (const marker of [
  '"downstream_task": "FORUM-20BB"',
  '"downstream_contract": "crates/rustok-forum/contracts/forum-topic-audience-exact-read.json"',
]) {
  requireText(upstream, marker, `FORUM-20BA handoff is missing ${marker}`);
}

for (const [key, expected] of [
  ["public_exact_read_published", true],
  ["authenticated_exact_read_published", true],
  ["read_scope_required_before_visibility", true],
  ["authenticated_tenant_validated_before_topic_lookup", true],
  ["authenticated_actor_validated_before_topic_lookup", true],
  ["effective_locale_from_port_context", true],
  ["route_channel_from_port_context", true],
  ["base_visibility_before_richer_facts", true],
  ["inherited_category_layers_enforced", true],
  ["topic_local_layer_enforced", true],
  ["explicit_deny_precedence_preserved", true],
  ["optional_owner_facts_exact_and_bounded", true],
  ["missing_required_facts_fail_closed", true],
  ["denied_and_missing_topic_non_oracular", true],
  ["topic_hydration_after_authorization", true],
  ["public_request_does_not_call_optional_facts", true],
  ["transport_composition_changed", false],
  ["list_read_changed", false],
  ["reply_read_changed", false],
  ["search_index_changed", false],
  ["seo_changed", false],
  ["deep_link_changed", false],
  ["migration_added", false],
  ["dependency_changed", false],
  ["public_dto_changed", false],
]) {
  if (contract.owner_boundary?.[key] !== expected) {
    failures.push(`FORUM-20BB owner_boundary.${key} must be ${expected}`);
  }
}

if (
  contract.documentation?.crate_api_updated !== true ||
  contract.documentation?.canonical_plan_updated !== false ||
  !contract.documentation?.canonical_plan_debt
) {
  failures.push("FORUM-20BB documentation handoff is incomplete");
}

if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-20BB must not claim maintainer runtime execution");
}

if (failures.length > 0) {
  console.error("Forum exact topic audience read verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum exact topic audience read is source-ready.");
