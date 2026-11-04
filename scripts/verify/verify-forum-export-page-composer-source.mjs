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
  planner: "crates/modules/rustok-forum/src/export_planner.rs",
  page: "crates/modules/rustok-forum/src/export_page.rs",
  inventory: "crates/modules/rustok-forum/src/export_inventory.rs",
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

const inventoryCalls = source.page.split(".list_page(").length - 1;
const plannerCalls = source.page.split(".plan_fragment(").length - 1;
const readerCalls = source.page.split(".read_fragment(").length - 1;
if (inventoryCalls !== 1 || plannerCalls !== 1 || readerCalls !== 1) {
  throw new Error(
    `export page composer: expected one inventory/planner/reader call, found ${inventoryCalls}/${plannerCalls}/${readerCalls}`,
  );
}

console.log("Forum FORUM-34J bounded export page composer source: ok");
