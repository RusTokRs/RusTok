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
  page: "crates/rustok-forum/src/export_page.rs",
  inventory: "crates/rustok-forum/src/export_inventory.rs",
  packet: "docs/modules/forum-34-export-page-composer-actualization-2026-08-09.md",
};

const source = Object.fromEntries(
  Object.entries(files).map(([key, path]) => [key, read(path)]),
);

for (const marker of [
  '#[path = "export_page.rs"]',
  "mod page;",
  "pub use page::*;",
]) {
  need(source.planner, marker, "export page wiring");
}

for (const marker of [
  "pub struct ForumExportPage",
  "pub source: ForumExportSourceInventoryPage",
  "pub fragment: Option<ForumExportFragment>",
  "pub enum ForumExportPageComposeError",
  "pub struct ForumExportPageComposer",
  "pub async fn compose_page(",
  "inventory.list_page(security, request).await?",
  "source.target_plan_request()",
  "if source.has_more",
  "EmptyPageHasMore",
  "fragment: None",
  "ForumExportTargetPlanner",
  ".plan_fragment(",
  "ForumOwnerExportReader",
  ".read_fragment(",
  "validate_fragment(&source, &fragment)?",
  "fragment: Some(fragment)",
  "FragmentTenantChanged",
  "FragmentKindContaminated",
  "FragmentSourceIdentityChanged",
  "unique_ids(fragment.categories.iter().map(|record| record.id))",
  "unique_ids(fragment.topics.iter().map(|record| record.id))",
  "unique_ids(fragment.replies.iter().map(|record| record.id))",
  "if actual_ids != source.ids",
]) {
  need(source.page, marker, "export page composer");
}

for (const marker of [
  "sea_orm",
  "DatabaseConnection",
  "ConnectionTrait",
  "Entity::",
  "crate::entities",
  "Serialize",
  "Deserialize",
  "#[serde(",
  "ForumOwnerExportMapper",
  "get_with_locale_fallback",
  "available_locales_for_categories",
  "available_locales_for_topics",
  "available_locales_for_replies",
]) {
  forbid(source.page, marker, "composer orchestration-only boundary");
}

for (const marker of [
  "PermissionScope::All",
  "Action::Manage",
  "request.limit == 0",
  "MAX_FORUM_EXPORT_SOURCE_INVENTORY_LIMIT",
]) {
  need(source.inventory, marker, "34I authorization/bound baseline");
}

for (const marker of [
  "FORUM-34J",
  "FORUM-34A through FORUM-34I",
  "still labels `FORUM-34` as `planned`",
  "34I list_page -> 34H target_plan_request/plan_fragment -> 34F read_fragment -> 34D mapping",
  "fragment` is `None`",
  "PermissionScope::All",
  "TooManyTargets",
  "retry the same `after_id` with a smaller source limit",
  "ordered unique source IDs",
  "does **not** claim a frozen database snapshot",
  "imports no SeaORM/database types",
  "does not call `ForumOwnerExportMapper` directly",
  "34J page composer -> 34I bounded candidate IDs -> 34H exact-locale targets -> 34F exact owner reads -> 34D export mapping",
  "no tests, Cargo commands",
]) {
  need(source.packet, marker, "FORUM-34J packet");
}

const inventoryCalls = source.page.split(".list_page(").length - 1;
const plannerCalls = source.page.split(".plan_fragment(").length - 1;
const readerCalls = source.page.split(".read_fragment(").length - 1;
if (inventoryCalls !== 1 || plannerCalls !== 1 || readerCalls !== 1) {
  throw new Error(
    `export page composer: expected one inventory/planner/reader call, found ${inventoryCalls}/${plannerCalls}/${readerCalls}`,
  );
}

console.log("Forum FORUM-34J bounded export page composer source: ok");
