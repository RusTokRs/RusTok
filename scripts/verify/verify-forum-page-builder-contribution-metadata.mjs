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
const adminBuild = read("crates/rustok-forum/admin/build.rs");
const adminAdapter = read("crates/rustok-forum/admin/src/page_builder.rs");
const sharedTooling = read("crates/rustok-build/src/module_manifest_contribution.rs");
const actualization = read("docs/modules/forum-page-builder-fly-adapter-actualization-2026-08-08.md");

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
  'required_capabilities = ["tree", "properties"]',
  'role = "widget_preview"',
  'id = "rustok.forum.widget-preview"',
  'required_capabilities = ["preview"]',
  '"forum.topic_list"',
  '"forum.topic_detail"',
  '"forum.reply_stream"',
  "[[fba.builder_consumer.contribution_manifest.admin.renderers]]",
  "[[fba.builder_consumer.contribution_manifest.admin.property_editors]]",
  'presentations = ["full", "inline", "preview", "read_only"]',
  'format = "forum_widget_owner_schema_ref_v1"',
  'schema_id = "forum.topic_list.v1"',
  'schema_id = "forum.topic_detail.v1"',
  'schema_id = "forum.reply_stream.v1"',
  'catalog_endpoint = "/api/forum/widgets/catalog"',
  'validate_endpoint = "/api/forum/widgets/validate"',
  'adapter_state = "fly_contract_ready"',
  'preview_data_state = "owner_preview_transport_open"',
  'persistence_owner = "forum"',
  'authorization_owner = "forum"',
]) {
  requireMarker(moduleManifest, marker, "Forum canonical Page Builder metadata");
}
forbidMarker(moduleManifest, "[[fba.builder_consumer.contribution_manifest.storefront]]", "Forum admin-only contribution metadata");

for (const marker of [
  'FORUM_WIDGET_TYPE_TOPIC_LIST: &str = "forum.topic_list"',
  'FORUM_WIDGET_TYPE_TOPIC_DETAIL: &str = "forum.topic_detail"',
  'FORUM_WIDGET_TYPE_REPLY_STREAM: &str = "forum.reply_stream"',
  "fn topic_list_catalog_item()",
  "fn topic_detail_catalog_item()",
  "fn reply_stream_catalog_item()",
]) requireMarker(widgetContract, marker, "Forum owner widget contract");

for (const marker of [
  'path = "/api/forum/widgets/catalog"',
  'path = "/api/forum/widgets/validate"',
  "Permission::FORUM_TOPICS_READ",
  '"Permission denied: forum_topics:read required"',
]) requireMarker(widgetController, marker, "Forum widget authorization boundary");

for (const marker of [
  "pub fn normalize_module_contribution_manifest(",
  "required_permissions",
  "OWNER_PROVIDER_METADATA_KEY",
  "PROVIDER_VERSION_METADATA_KEY",
  "outside fba.builder_consumer.capabilities",
]) requireMarker(sharedTooling, marker, "shared contribution metadata tooling");

for (const marker of [
  'fly = { path = "../../fly" }',
  'fly-ui = { path = "../../fly-ui" }',
  "[build-dependencies]",
  "toml.workspace = true",
]) requireMarker(adminCargo, marker, "Forum admin Fly dependency boundary");
for (const forbidden of ["rustok-page-builder-admin", "rustok-page-builder ="]) forbidMarker(adminCargo, forbidden, "Forum owner-local Fly adapter dependency boundary");

for (const marker of [
  '#[path = "../../rustok-build/src/module_manifest_contribution.rs"]',
  "normalize_module_contribution_manifest",
  'WIDGET_CATALOG_ROLE',
  'WIDGET_PREVIEW_ROLE',
  "validate_widget_contracts",
  'FORUM_WIDGET_PREVIEW_CONTRIBUTION_ID',
  'FORUM_WIDGET_COMPONENT_TYPES',
  'GENERATED_FORUM_CONTRIBUTION_MANIFEST_JSON',
]) requireMarker(adminBuild, marker, "Forum build-generated contribution manifest");

for (const marker of [
  "pub struct ForumContributionAdapter",
  "impl ContributionAdapter for ForumContributionAdapter",
  "pub fn register_forum_fly_widgets",
  "pub fn build_forum_admin_contribution_registry",
  "pub fn forum_widget_preview_contribution",
  "pub fn forum_fly_registry_set",
  'COMPONENT_PROPS_FIELD: &str = "props"',
  'OWNER_SCHEMA_REF_FORMAT: &str = "forum_widget_owner_schema_ref_v1"',
  'owner_schema_for_component',
  'preview_off_keeps_authoring_contracts_but_filters_renderers',
]) requireMarker(adminAdapter, marker, "Forum Fly adapter source");
for (const forbidden of ["toml::", "ForumWidgetContractService", "TopicService", "ReplyService", "DatabaseConnection"]) forbidMarker(adminAdapter, forbidden, "Forum Fly adapter owner-data boundary");

for (const marker of [
  "Forum Fly component/block/adapter contracts: source-ready",
  "Forum owner-backed preview transport/host mount: open",
  "Cargo.lock refresh: maintainer-owned",
]) requireMarker(actualization, marker, "Forum Fly adapter actualization");

if (failures.length > 0) {
  console.error("forum Page Builder Fly contribution verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum Page Builder Fly contribution verification passed: generated_manifest=true split_capabilities=true owner_preview_transport=open");
