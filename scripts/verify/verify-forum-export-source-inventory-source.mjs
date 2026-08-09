#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function need(text, marker, label) {
  if (!text.includes(marker)) throw new Error(`${label}: missing ${marker}`);
}

function forbid(text, marker, label) {
  if (text.includes(marker)) throw new Error(`${label}: forbidden ${marker}`);
}

const files = {
  planner: "crates/rustok-forum/src/export_planner.rs",
  inventory: "crates/rustok-forum/src/export_inventory.rs",
  packet: "docs/modules/forum-34-export-source-inventory-actualization-2026-08-09.md",
};

const source = Object.fromEntries(
  Object.entries(files).map(([key, path]) => [key, read(path)]),
);

for (const marker of [
  '#[path = "export_inventory.rs"]',
  "mod inventory;",
  "pub use inventory::*;",
]) need(source.planner, marker, "export planner composition");

for (const marker of [
  "pub const MAX_FORUM_EXPORT_SOURCE_INVENTORY_LIMIT: u64 =",
  "MAX_FORUM_EXPORT_PLAN_SOURCE_IDS_PER_FRAGMENT as u64",
  "pub struct ForumExportSourceInventoryRequest",
  "pub struct ForumExportSourceInventoryPage",
  "pub fn target_plan_request(&self) -> Option<ForumExportTargetPlanRequest>",
  "if self.ids.is_empty()",
  "return None;",
  "Some(request)",
  "pub enum ForumExportSourceInventoryError",
  "pub struct ForumExportSourceInventoryService",
  "pub async fn list_page(",
  "if security.is_public_read()",
  "PermissionScope::All",
  "AllManagePermissionRequired",
  "Resource::ForumCategories",
  "Resource::ForumTopics",
  "Resource::ForumReplies",
  "Action::Manage",
  "request.limit == 0",
  "request.after_id.is_some_and(|id| id.is_nil())",
  "request.limit.saturating_add(1)",
  "rows.len() > request.limit as usize",
  "ids.last().copied().or(request.after_id)",
  "inventory_statement(",
]) need(source.inventory, marker, "export inventory");

const requestStart = source.inventory.indexOf(
  "#[derive(Clone, Debug)]\npub struct ForumExportSourceInventoryRequest",
);
const errorStart = source.inventory.indexOf("#[derive(Debug, Error)]", requestStart);
if (requestStart < 0 || errorStart <= requestStart) {
  throw new Error("export inventory: non-wire request/page boundary is invalid");
}
const publicContract = source.inventory.slice(requestStart, errorStart);
for (const marker of ["Serialize", "Deserialize", "#[serde("]) {
  forbid(publicContract, marker, "export inventory contract must remain non-wire");
}

for (const marker of [
  "forum_category_lifecycle",
  "topic.deleted_at IS NULL",
  "forum_topic_merge_operations",
  "merge_operation.source_topic_id = topic.id",
  "JOIN forum_categories category",
  "JOIN forum_topics topic",
  "ORDER BY c.id",
  "ORDER BY topic.id",
  "ORDER BY reply.id",
  "c.id > ?2",
  "c.id > $2",
  "topic.id > ?2",
  "topic.id > $2",
  "reply.id > ?2",
  "reply.id > $2",
]) need(source.inventory, marker, "inventory SQL");

for (const marker of [
  "reply.deleted_at IS NULL",
  "reply.status <> 'deleted'",
  "reply.status != 'deleted'",
]) forbid(source.inventory, marker, "reply tombstone preservation");

for (const marker of [
  "forum_category_translation",
  "forum_topic_translation",
  "forum_reply_bodies",
  "forum_reply_body",
  "VoteService",
  "SubscriptionService",
  "ForumOwnerExportMapper",
  ".get_with_locale_fallback(",
  ".available_locales_for_categories(",
  ".available_locales_for_topics(",
  ".available_locales_for_replies(",
  ".read_fragment(",
  "Serialize",
  "Deserialize",
]) forbid(source.inventory, marker, "inventory storage-only boundary");

for (const marker of [
  "FORUM-34I",
  "FORUM-34A through FORUM-34H",
  "still labels `FORUM-34` as `planned`",
  "presentation projections",
  "returns `None` for an empty terminal page",
  "PermissionScope::All",
  "strict `id > after_id` continuation",
  "does **not** claim a transactionally frozen tenant snapshot",
  "Archived categories are excluded",
  "normal `TopicStatus::Archived` topic remains eligible",
  "Soft-deleted topics are excluded",
  "Merge-source topics are excluded",
  "Reply rows are **not** filtered by their own `deleted_at`/`deleted` status",
  "permanently missing `parent_reply_id`",
  "Locale/body presence is **not** silently filtered",
  "34I bounded candidate IDs -> 34H bounded exact-locale target planning -> 34F exact owner reads -> 34D export mapping",
  "no tests, Cargo commands",
]) need(source.packet, marker, "FORUM-34I packet");

const queryCalls = source.inventory.split(".query_all(").length - 1;
if (queryCalls !== 1) {
  throw new Error(`export inventory: expected one query execution path, found ${queryCalls}`);
}

console.log("Forum FORUM-34I bounded export source inventory source: ok");
