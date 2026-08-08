#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const read = (relativePath) => fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const moduleManifest = read("crates/rustok-forum/rustok-module.toml");
const widgetContract = read("crates/rustok-forum/src/services/widget_contract.rs");
const widgetPreview = read("crates/rustok-forum/src/services/widget_preview.rs");
const topicWidgetPreview = read("crates/rustok-forum/src/services/topic_widget_preview.rs");
const widgetController = read("crates/rustok-forum/src/controllers/widgets.rs");
const forumAdminCargo = read("crates/rustok-forum/admin/Cargo.toml");
const forumAdminBuild = read("crates/rustok-forum/admin/build.rs");
const forumAdminAdapter = read("crates/rustok-forum/admin/src/page_builder.rs");
const forumAdminPreviewTransport = read("crates/rustok-forum/admin/src/widget_preview_transport.rs");
const sharedTooling = read("crates/rustok-build/src/module_manifest_contribution.rs");
const pageBuilderHost = read("crates/rustok-page-builder/admin/src/contribution_host.rs");
const pageBuilderUi = read("crates/rustok-page-builder/admin/src/ui/leptos.rs");
const pageBuilderPreviewPanel = read("crates/rustok-page-builder/admin/src/editor/contribution_preview.rs");
const adminComposition = read("apps/admin/src/app/page_builder_contributions.rs");
const moduleAdmin = read("apps/admin/src/pages/module_admin.rs");
const actualization = read("docs/modules/forum-page-builder-owner-preview-actualization-2026-08-08.md");

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
  'format = "forum_widget_owner_schema_ref_v1"',
  'preview_endpoint = "/api/forum/widgets/preview"',
  'preview_data_state = "owner_preview_transport_ready"',
  'persistence_owner = "forum"',
  'authorization_owner = "forum"',
]) requireMarker(moduleManifest, marker, "Forum canonical Page Builder metadata");
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
  "ForumWidgetContractService::validate_props",
  "ForumTopicVisibilityService::new",
  "hidden_category_ids_for_viewer",
  "list_widget_preview_with_locale_fallback_and_hidden_categories",
  "PermissionScope::None",
  "Action::Moderate",
  "MODERATOR_PREVIEW_REPLY_STATUSES",
  "ReplyStatus::Approved",
]) requireMarker(widgetPreview, marker, "Forum owner widget preview service");
for (const forbidden of ["rustok_page_builder", "fly_ui", "PageBuilderContribution"])
  forbidMarker(widgetPreview, forbidden, "Forum owner preview independence");

for (const marker of [
  '"activity" =>',
  '"newest" =>',
  '"top" =>',
  "forum_topic_votes.value",
  "IsPinned.eq(false)",
  "paginate(&self.db",
  "hidden_category_ids",
]) requireMarker(topicWidgetPreview, marker, "Forum widget topic-list owner query");

for (const marker of [
  'path = "/api/forum/widgets/catalog"',
  'path = "/api/forum/widgets/validate"',
  'path = "/api/forum/widgets/preview"',
  "ForumWidgetPreviewService::new",
  "Permission::FORUM_TOPICS_READ",
  '"Permission denied: forum_topics:read required"',
]) requireMarker(widgetController, marker, "Forum widget HTTP authorization boundary");

for (const marker of [
  "pub fn normalize_module_contribution_manifest(",
  "required_permissions",
  "OWNER_PROVIDER_METADATA_KEY",
  "PROVIDER_VERSION_METADATA_KEY",
]) requireMarker(sharedTooling, marker, "shared contribution metadata tooling");

for (const marker of [
  'fly = { path = "../../fly" }',
  'fly-ui = { path = "../../fly-ui" }',
  "[build-dependencies]",
  "toml.workspace = true",
]) requireMarker(forumAdminCargo, marker, "Forum admin Fly dependency boundary");
for (const forbidden of ["rustok-page-builder-admin", "rustok-page-builder ="])
  forbidMarker(forumAdminCargo, forbidden, "Forum owner-local adapter dependency boundary");

for (const marker of [
  '#[path = "../../rustok-build/src/module_manifest_contribution.rs"]',
  "normalize_module_contribution_manifest",
  "WIDGET_CATALOG_ROLE",
  "WIDGET_PREVIEW_ROLE",
  "FORUM_WIDGET_PREVIEW_CONTRIBUTION_ID",
  "GENERATED_FORUM_CONTRIBUTION_MANIFEST_JSON",
]) requireMarker(forumAdminBuild, marker, "Forum build-generated contribution manifest");

for (const marker of [
  "pub struct ForumContributionAdapter",
  "impl ContributionAdapter for ForumContributionAdapter",
  "pub fn register_forum_fly_widgets",
  "pub fn forum_widget_preview_contribution",
  'COMPONENT_PROPS_FIELD: &str = "props"',
]) requireMarker(forumAdminAdapter, marker, "Forum Fly adapter source");
for (const forbidden of ["ForumWidgetContractService", "TopicService", "ReplyService", "DatabaseConnection"])
  forbidMarker(forumAdminAdapter, forbidden, "Forum Fly adapter owner-data boundary");

for (const marker of [
  'endpoint = "forum/page-builder-widget-preview"',
  "require_tenant_scope",
  "require_forum_module_enabled",
  "Permission::FORUM_TOPICS_READ",
  "ForumWidgetPreviewService::new",
  "SecurityContext::from_permission_snapshot",
]) requireMarker(forumAdminPreviewTransport, marker, "Forum admin owner preview transport");

for (const marker of [
  "pub trait PageBuilderContributionPreviewPort",
  "pub struct PageBuilderContributionHostExtension",
  "pub struct PageBuilderContributionHostContext",
  "install_registries",
  "merge_admin_assembly",
  "build_admin_contribution_registry_from_manifests",
  "contribution_host_registry_conflict",
]) requireMarker(pageBuilderHost, marker, "Page Builder provider-neutral contribution host");
forbidMarker(pageBuilderHost, "rustok_forum", "provider-neutral Page Builder contribution host");

for (const marker of [
  "PageBuilderContributionHostContext",
  "install_contribution_registries",
  "merge_admin_assembly",
  "contribution_capabilities",
  '"preview".to_string()',
]) requireMarker(pageBuilderUi, marker, "Page Builder host composition");
forbidMarker(pageBuilderUi, "rustok_forum", "provider-neutral Page Builder UI composition");

for (const marker of [
  "pub fn ContributionPreviewPanel",
  "selected_preview_request",
  "Presentation::Preview",
  "preview_port",
  "MAX_PREVIEW_JSON_BYTES",
  '"Refresh"',
  'extensions\n            .get("props")',
]) requireMarker(pageBuilderPreviewPanel, marker, "Page Builder owner preview panel");
forbidMarker(pageBuilderPreviewPanel, "rustok_forum", "provider-neutral owner preview panel");

for (const marker of [
  "ForumPageBuilderPreviewPort",
  "preview_forum_page_builder_widget",
  "required_contribution_permissions",
  "Permission::from_str",
  "has_effective_permission",
  'enabled_modules.contains("forum")',
  "forum_contribution_manifest",
  "register_forum_fly_widgets",
]) requireMarker(adminComposition, marker, "admin contribution composition root");

for (const marker of [
  "PageBuilderContributionScope",
  'page.module_slug == "pages"',
  "enabled_modules.modules.get()",
]) requireMarker(moduleAdmin, marker, "Pages host contribution mount");

for (const marker of [
  "Status: `source-ready / owner-preview-host-composed / property-editing-open / execution-pending`",
  "ForumWidgetPreviewService",
  "Provider-neutral Page Builder host",
  "effective `manage -> read` contribution admission",
  "owner-backed Forum widget property-editor transport/UI",
]) requireMarker(actualization, marker, "Forum owner preview actualization");

if (failures.length > 0) {
  console.error("forum Page Builder owner preview verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum Page Builder owner preview verification passed: owner_preview=true host_composed=true property_editor=open runtime_evidence=pending");
