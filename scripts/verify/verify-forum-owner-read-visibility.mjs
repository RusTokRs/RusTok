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

function between(source, start, end, label) {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return "";
  }
  return source.slice(startIndex, endIndex);
}

const contractPath = "crates/rustok-forum/contracts/forum-owner-read-visibility.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.topic_visibility_owner_file ?? "");
const topicFacade = read(contract.topic_facade_file ?? "");
const topicSelector = read(contract.topic_selector_file ?? "");
const replyFacade = read(contract.reply_facade_file ?? "");
const testSource = read(contract.test_file ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("owner read visibility contract must use schema_version=1");
}
if (contract.task !== "FORUM-20D") {
  failures.push("owner read visibility contract must belong to FORUM-20D");
}
if (contract.canonical_plan_sync !== "included") {
  failures.push("owner read visibility contract must be synchronized into the canonical plan");
}
if (contract.category_tree_bound !== 512 || contract.category_depth_bound !== 16) {
  failures.push("owner read visibility category bounds must remain 512 nodes and depth 16");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("source publication must not claim unexecuted owner-read evidence");
}
for (const residual of [
  "role visibility",
  "trust-level visibility",
  "channel membership visibility",
  "group membership visibility",
  "explicit allow and deny",
  "create reply and moderate audience policy",
  "search notification SEO and deep-link migration to the owner scope",
  "visibility-scoped category and all-read mutations",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`owner read visibility contract must keep ${residual} explicitly open`);
  }
}

for (const marker of [
  "pub(crate) async fn hidden_category_ids_for_viewer(",
  "pub(crate) async fn is_topic_category_visible_to_viewer(",
  "Unlike storefront visibility",
  "forum_topic::Column::TenantId.eq(tenant_id)",
  "forum_topic::Column::Id.eq(topic_id)",
  "forum_topic::Column::CategoryId.is_not_in(hidden_category_ids)",
  "Ok(select.one(&self.db).await?.is_some())",
]) {
  requireText(owner, marker, `topic visibility owner is missing ${marker}`);
}
for (const forbidden of [
  "rustok_profiles",
  "rustok_channels",
  "rustok_groups",
  "forum_topic::Column::Metadata",
]) {
  rejectText(owner, forbidden, `owner category-floor evaluation must not depend on ${forbidden}`);
}

const topicExactRead = between(
  topicFacade,
  "pub async fn get_with_locale_fallback(",
  "pub async fn get_storefront_visible_with_locale_fallback(",
  "topic exact owner read",
);
for (const marker of [
  "enforce_scope(&security, Resource::ForumTopics, Action::Read)?",
  ".is_topic_category_visible_to_viewer(",
  "!security.is_public_read()",
  "return Err(ForumError::TopicNotFound(topic_id))",
  "let response = self",
]) {
  requireText(topicExactRead, marker, `topic exact owner read is missing ${marker}`);
}
const topicReadScopeIndex = topicExactRead.indexOf("enforce_scope(");
const topicVisibilityIndex = topicExactRead.indexOf(".is_topic_category_visible_to_viewer(");
const topicHydrationIndex = topicExactRead.indexOf("let response = self");
if (
  topicReadScopeIndex < 0 ||
  topicVisibilityIndex < 0 ||
  topicHydrationIndex < 0 ||
  topicReadScopeIndex > topicVisibilityIndex ||
  topicVisibilityIndex > topicHydrationIndex
) {
  failures.push("topic exact owner read must authorize, check category visibility, then hydrate");
}

const topicPageRead = between(
  topicFacade,
  "pub async fn list_with_locale_fallback(",
  "pub async fn list_storefront_visible_with_locale_fallback(",
  "topic owner page",
);
for (const marker of [
  "enforce_scope(&security, Resource::ForumTopics, Action::List)?",
  ".hidden_category_ids_for_viewer(tenant_id, !security.is_public_read())",
  ".list_with_locale_fallback_and_hidden_categories(",
]) {
  requireText(topicPageRead, marker, `topic owner page is missing ${marker}`);
}

for (const marker of [
  "pub(crate) async fn list_with_locale_fallback_and_hidden_categories(",
  "forum_topic::Column::CategoryId.is_not_in(hidden_category_ids.to_vec())",
  "let paginator = select",
  "let total = paginator.num_items().await?",
  "let topics = paginator.fetch_page",
]) {
  requireText(topicSelector, marker, `topic owner selector is missing ${marker}`);
}
const ownerSelector = between(
  topicSelector,
  "pub(crate) async fn list_with_locale_fallback_and_hidden_categories(",
  "#[instrument(skip(self, security, hidden_category_ids))]",
  "topic owner hidden-category selector",
);
const ownerCategoryFilterIndex = ownerSelector.indexOf(
  "forum_topic::Column::CategoryId.is_not_in(hidden_category_ids.to_vec())",
);
const ownerPaginatorIndex = ownerSelector.indexOf("let paginator = select");
if (
  ownerCategoryFilterIndex < 0 ||
  ownerPaginatorIndex < 0 ||
  ownerCategoryFilterIndex > ownerPaginatorIndex
) {
  failures.push("topic category visibility must be applied before owner count and pagination");
}

const replyExactRead = between(
  replyFacade,
  "pub async fn get_with_locale_fallback(",
  "pub async fn update(",
  "reply exact owner read",
);
for (const marker of [
  "enforce_scope(&security, Resource::ForumReplies, Action::Read)?",
  "let reply = self.inner.find_reply(tenant_id, reply_id).await?",
  ".topic_category_is_visible(tenant_id, reply.topic_id, &security)",
  "return Err(ForumError::ReplyNotFound(reply_id))",
  "let response = self",
]) {
  requireText(replyExactRead, marker, `reply exact owner read is missing ${marker}`);
}
const replyLookupIndex = replyExactRead.indexOf("let reply = self.inner.find_reply");
const replyVisibilityIndex = replyExactRead.indexOf(".topic_category_is_visible(");
const replyHydrationIndex = replyExactRead.indexOf("let response = self");
if (
  replyLookupIndex < 0 ||
  replyVisibilityIndex < 0 ||
  replyHydrationIndex < 0 ||
  replyLookupIndex > replyVisibilityIndex ||
  replyVisibilityIndex > replyHydrationIndex
) {
  failures.push("reply exact owner read must resolve the parent, hide it, then hydrate the body");
}

for (const marker of [
  "enforce_scope(&security, Resource::ForumReplies, Action::List)?",
  ".topic_category_is_visible(tenant_id, topic_id, &security)",
  "return Err(ForumError::TopicNotFound(topic_id))",
  "if !security.is_public_read()",
  "return Ok(true)",
  "ForumTopicVisibilityService::new(self.db.clone())",
  ".is_topic_category_visible_to_viewer(tenant_id, topic_id, false)",
]) {
  requireText(replyFacade, marker, `reply owner facade is missing ${marker}`);
}
const replyVisibilityUses =
  replyFacade.match(/\.topic_category_is_visible\(tenant_id, topic_id, &security\)/g) ?? [];
if (replyVisibilityUses.length !== 2) {
  failures.push(`expected two guarded reply page paths, found ${replyVisibilityUses.length}`);
}

for (const marker of [
  "inherited_authenticated_floor_guards_topic_and_reply_owner_reads",
  "ForumCategoryVisibility::Authenticated",
  "SecurityContext::public_read()",
  "assert_eq!(public_total, 1)",
  "assert_eq!(authenticated_total, 2)",
  "Err(ForumError::TopicNotFound(id)) if id == restricted_topic",
  "Err(ForumError::ReplyNotFound(id)) if id == restricted_reply",
  "public reply page must fail as an absent hidden topic before pagination",
  "assert_eq!(authenticated_reply_total, 1)",
]) {
  requireText(testSource, marker, `owner visibility SQLite scenario is missing ${marker}`);
}

for (const marker of [
  "## `FORUM-20` — ACL and visibility inheritance",
  "Delivered in `FORUM-20C`",
  "Delivered in `FORUM-20D`",
  "topic_reply_owner_visibility_sqlite",
  "verify-forum-owner-read-visibility.mjs",
]) {
  requireText(plan, marker, `canonical FORUM-20 plan is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum owner read visibility verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum owner read visibility contract is source-ready.");
