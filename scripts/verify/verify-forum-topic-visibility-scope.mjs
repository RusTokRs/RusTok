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
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
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

const contractPath = "crates/rustok-forum/contracts/forum-topic-visibility-scope.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.owner_file ?? "");
const facade = read(contract.facade_file ?? "");
const storefrontReadState = read(contract.storefront_read_state_file ?? "");
const compatibilitySelector = read(contract.compatibility_selector_file ?? "");
const testSource = read(contract.test_file ?? "");
const plan = read(contract.canonical_plan ?? "");
const services = read("crates/rustok-forum/src/services/mod.rs");
const lib = read("crates/rustok-forum/src/lib.rs");

if (contract.schema_version !== 1) {
  failures.push("topic visibility scope contract must use schema_version=1");
}
if (contract.task !== "FORUM-20A") {
  failures.push("topic visibility scope contract must belong to FORUM-20A");
}
if (contract.candidate_bound !== 100) {
  failures.push("topic visibility candidate bound must remain 100");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("source publication must not claim unexecuted visibility evidence");
}
for (const residual of [
  "category inheritance",
  "role visibility",
  "group membership visibility",
  "explicit allow and deny",
  "visibility-scoped category and all-read mutations",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`topic visibility contract must keep ${residual} explicitly open`);
  }
}

for (const marker of [
  "pub const MAX_FORUM_TOPIC_VISIBILITY_CANDIDATES: usize = 100",
  "pub struct ForumTopicVisibilityScope",
  "channel_slug: Option<String>",
  "pub fn storefront(channel_slug: Option<&str>) -> ForumResult<Self>",
  "pub struct ForumTopicVisibilityService",
  "pub async fn is_topic_visible",
  "pub async fn filter_visible_topic_ids",
  "topic_ids.len() > MAX_FORUM_TOPIC_VISIBILITY_CANDIDATES",
  "if seen.insert(*topic_id)",
  ".filter(|topic_id| visible.contains(topic_id))",
  "forum_topic::Column::TenantId.eq(tenant_id)",
  "forum_topic::Column::Status.eq(TopicStatus::Open)",
  "all_topic_channel_access_subquery(tenant_id)",
  "matching_topic_channel_access_subquery(tenant_id, channel_slug)",
  "forum_topic_channel_access::Column::TenantId",
  "MAX_FORUM_CHANNEL_SLUG_LEN: usize = 128",
  "contains unsupported characters",
]) {
  requireText(owner, marker, `topic visibility owner is missing ${marker}`);
}
for (const forbidden of [
  "SecurityContext",
  "rustok_profiles",
  "rustok_channels",
  "rustok_groups",
  "forum_topic::Column::Metadata",
]) {
  rejectText(owner, forbidden, `topic visibility owner must not depend on premature policy input ${forbidden}`);
}

for (const marker of [
  "ForumTopicVisibilityScope::storefront(channel_slug)?",
  "ForumTopicVisibilityService::new(self.db.clone())",
  ".is_topic_visible(tenant_id, topic_id, &scope)",
  ".filter_visible_topic_ids(tenant_id, &candidate_ids, &scope)",
  "if visible_ids != candidate_ids",
  "diverged from the owner visibility scope",
]) {
  requireText(facade, marker, `topic facade visibility guard is missing ${marker}`);
}
rejectText(
  facade,
  "fn is_storefront_visible(",
  "topic facade must not keep a second in-memory storefront visibility policy",
);

for (const marker of [
  "list_storefront_visible_with_locale_fallback",
  "apply_public_topic_channel_filter(select, channel_slug)",
  "forum_topic::Column::Status.eq(TopicStatus::Open)",
]) {
  requireText(
    compatibilitySelector,
    marker,
    `compatibility topic selector is missing existing prefilter ${marker}`,
  );
}
for (const marker of [
  ".list_storefront_visible_with_locale_fallback(",
  ".get_storefront_visible_with_locale_fallback(",
]) {
  requireText(
    storefrontReadState,
    marker,
    `storefront read-state composition must consume the guarded topic facade: ${marker}`,
  );
}

requireText(services, "pub mod topic_visibility;", "services must declare topic_visibility");
for (const marker of [
  "ForumTopicVisibilityScope",
  "ForumTopicVisibilityService",
  "MAX_FORUM_TOPIC_VISIBILITY_CANDIDATES",
]) {
  requireText(services, marker, `services export is missing ${marker}`);
  requireText(lib, marker, `crate export is missing ${marker}`);
}

for (const marker of [
  "exact_visibility_scope_is_bounded_ordered_and_non_oracular",
  "storefront_topic_facade_is_guarded_by_the_exact_owner_scope",
  "Some(\"  MOBILE  \")",
  "assert_eq!(public_ids, vec![public_topic])",
  "assert_eq!(mobile_ids, vec![mobile_topic, public_topic])",
  "vec![public_topic; 101]",
  "not/a/channel",
  ".close_topic(",
]) {
  requireText(testSource, marker, `topic visibility SQLite scenario is missing ${marker}`);
}

for (const marker of [
  "Delivered in `FORUM-20A`",
  "ForumTopicVisibilityService",
  "forum-topic-visibility-scope.json",
  "topic_visibility_sqlite",
  "verify-forum-topic-visibility-scope.mjs",
]) {
  requireText(plan, marker, `canonical FORUM-20 plan is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum topic visibility scope verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum topic visibility scope contract is source-ready.");
