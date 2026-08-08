#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const read = (relativePath) => fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const moduleManifest = read("crates/rustok-forum/rustok-module.toml");
const widgetContract = read("crates/rustok-forum/src/services/widget_contract.rs");
const widgetController = read("crates/rustok-forum/src/controllers/widgets.rs");
const adminCargo = read("crates/rustok-forum/admin/Cargo.toml");
const sharedTooling = read("crates/rustok-build/src/module_manifest_contribution.rs");
const forumPlan = read("crates/rustok-forum/docs/implementation-plan.md");
const pageBuilderPlan = read("docs/modules/page-builder-implementation-plan.md");

const failures = [];
const requireMarker = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label} is missing ${marker}`);
};
const forbidMarker = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label} still contains ${marker}`);
};

for (const marker of [
  "[fba.builder_consumer.contribution_manifest]",
  'owner_provider = "rustok.forum"',
  'required_permissions = ["forum_topics:read"]',
  'role = "widget_catalog"',
  'id = "rustok.forum.widget-catalog"',
  'provider = "rustok.forum"',
  'required_capabilities = ["preview"]',
  "blocks = []",
  'format = "forum_widget_catalog_v1"',
  'catalog_version = "v1"',
  'catalog_endpoint = "/api/forum/widgets/catalog"',
  'validate_endpoint = "/api/forum/widgets/validate"',
  'catalog_source = "fba.builder_consumer.widgets"',
  'adapter_state = "pending"',
  'persistence_owner = "forum"',
  'authorization_owner = "forum"',
  'props_schema = "forum.topic_list.v1"',
  'props_schema = "forum.topic_detail.v1"',
  'props_schema = "forum.reply_stream.v1"',
]) {
  requireMarker(moduleManifest, marker, "Forum canonical Page Builder metadata");
}

for (const forbidden of [
  "[[fba.builder_consumer.contribution_manifest.admin.renderers]]",
  "[[fba.builder_consumer.contribution_manifest.admin.property_editors]]",
  "[[fba.builder_consumer.contribution_manifest.storefront]]",
]) {
  forbidMarker(moduleManifest, forbidden, "Forum adapter-pending contribution metadata");
}

for (const marker of [
  'FORUM_WIDGET_TYPE_TOPIC_LIST: &str = "forum.topic_list"',
  'FORUM_WIDGET_TYPE_TOPIC_DETAIL: &str = "forum.topic_detail"',
  'FORUM_WIDGET_TYPE_REPLY_STREAM: &str = "forum.reply_stream"',
  "fn topic_list_catalog_item()",
  "fn topic_detail_catalog_item()",
  "fn reply_stream_catalog_item()",
]) {
  requireMarker(widgetContract, marker, "Forum owner widget contract");
}

for (const marker of [
  'path = "/api/forum/widgets/catalog"',
  'path = "/api/forum/widgets/validate"',
  "Permission::FORUM_TOPICS_READ",
  '"Permission denied: forum_topics:read required"',
]) {
  requireMarker(widgetController, marker, "Forum widget authorization boundary");
}

for (const marker of [
  "pub fn normalize_module_contribution_manifest(",
  "required_permissions",
  "OWNER_PROVIDER_METADATA_KEY",
  "PROVIDER_VERSION_METADATA_KEY",
  "outside fba.builder_consumer.capabilities",
]) {
  requireMarker(sharedTooling, marker, "shared contribution metadata tooling");
}

for (const forbidden of ["fly-ui", "rustok-page-builder-admin", "rustok-page-builder ="] ) {
  forbidMarker(adminCargo, forbidden, "Forum admin adapter-pending dependency boundary");
}

for (const marker of [
  "Forum Page Builder contribution discovery metadata: source-ready",
  "Fly block/renderer/property-editor adapter remains open",
]) {
  requireMarker(forumPlan, marker, "Forum canonical implementation plan");
}
for (const marker of [
  "Forum second-consumer contribution discovery: source-ready",
  "Forum Fly adapter/component registry: open",
]) {
  requireMarker(pageBuilderPlan, marker, "Page Builder canonical implementation plan");
}

if (failures.length > 0) {
  console.error("forum Page Builder contribution metadata verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "forum Page Builder contribution metadata verification passed: shared_metadata=true adapter_state=pending runtime_claim=false",
);
