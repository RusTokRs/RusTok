#!/usr/bin/env node
// RusTok blog admin FFA boundary guardrails.
// Fast source-level checks for the module-owned core/transport/ui split.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  return readFileSync(repoPath(relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertExists(relativePath, description) {
  if (!existsSync(repoPath(relativePath))) fail(description);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

const libPath = "crates/rustok-blog/admin/src/lib.rs";
const corePath = "crates/rustok-blog/admin/src/core.rs";
const uiPath = "crates/rustok-blog/admin/src/ui/leptos.rs";
const moderationPath = "crates/rustok-blog/admin/src/moderation.rs";
const transportPath = "crates/rustok-blog/admin/src/transport/mod.rs";
const graphqlAdapterPath = "crates/rustok-blog/admin/src/transport/graphql_adapter.rs";
const moderationAdapterPath = "crates/rustok-blog/admin/src/transport/moderation_adapter.rs";
const graphqlTypesPath = "crates/rustok-blog/src/graphql/types.rs";
const graphqlMutationPath = "crates/rustok-blog/src/graphql/mutation.rs";
const graphqlRateLimitPath = "crates/rustok-blog/src/graphql/rate_limit.rs";
const legacyApiPath = "crates/rustok-blog/admin/src/api.rs";
const implementationPlanPath = "crates/rustok-blog/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";

if (existsSync(repoPath(legacyApiPath))) {
  fail(`${legacyApiPath}: legacy GraphQL api adapter must live under transport/graphql_adapter.rs`);
}

for (const filePath of [
  libPath,
  corePath,
  uiPath,
  moderationPath,
  transportPath,
  graphqlAdapterPath,
  moderationAdapterPath,
  graphqlTypesPath,
  graphqlMutationPath,
  graphqlRateLimitPath,
  implementationPlanPath,
  registryPath,
]) {
  assertExists(filePath, `${filePath}: expected blog admin FFA boundary file`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const moderation = readRepo(moderationPath);
const transport = readRepo(transportPath);
const graphqlAdapter = readRepo(graphqlAdapterPath);
const moderationAdapter = readRepo(moderationAdapterPath);
const graphqlTypes = readRepo(graphqlTypesPath);
const graphqlMutation = readRepo(graphqlMutationPath);
const graphqlRateLimit = readRepo(graphqlRateLimitPath);
const implementationPlan = readRepo(implementationPlanPath);
const registry = readRepo(registryPath);

assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire legacy api.rs after GraphQL adapter moved under transport/`);
assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod moderation;", `${libPath}: crate root must wire the moderation UI slice`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub fn BlogAdmin()", `${libPath}: crate root must expose the composed BlogAdmin root`);
assertContains(lib, "<BlogEditor />", `${libPath}: composed root must preserve the existing CRUD editor`);
assertContains(lib, "<BlogModerationPanel />", `${libPath}: composed root must include moderation`);
for (const marker of [/pub async fn fetch_/, /pub async fn create_/, /pub async fn update_/, /pub async fn publish_/, /pub async fn archive_/, /pub async fn delete_/, /pub async fn moderate_/]) {
  assertNotContains(lib, marker, `${libPath}: crate root must not expose public transport passthroughs (${marker})`);
}

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
}
for (const marker of [
  "BlogPostFormInput",
  "build_blog_post_draft",
  "BlogPostSaveOperation",
  "BlogPostSaveCommand",
  "prepare_blog_post_save_command",
  "BlogPostLoadResultViewModel",
  "blog_post_load_result_view",
  "blog_post_transport_failure_issue",
  "BlogPostSaveResultViewModel",
  "blog_post_save_result_view",
  "BlogPostEditorFormState",
  "BlogPostAdminTableRowViewModel",
  "blog_post_admin_table_row_view",
  "BlogPostAdminTableViewModel",
  "blog_post_admin_table_view",
  "BlogPostAdminPostsTableViewModel",
  "BlogPostAdminPostsTableLabels",
  "blog_post_admin_posts_table_view_from_items",
  "BlogPostAdminFormViewModel",
  "blog_post_admin_form_view",
  "BlogPostAdminTableClassesViewModel",
  "blog_post_admin_table_classes_view",
  "BlogPostAdminShellClassesViewModel",
  "blog_post_admin_shell_classes_view",
  "BlogPostAdminEditorFormCopyViewModel",
  "BlogPostAdminEditorFormCopyLabels",
  "blog_post_admin_editor_form_copy_view",
  "BlogPostAdminEditorFieldClassesViewModel",
  "blog_post_admin_editor_field_classes_view",
  "BlogPostAdminTitleInputViewModel",
  "blog_post_admin_title_input_view",
  "BlogPostAdminBodyFormatSelectViewModel",
  "BlogPostAdminBodyFormatOptionViewModel",
  "blog_post_admin_body_format_select_view",
  "BlogPostAdminBodyFormatChangeViewModel",
  "blog_post_admin_body_format_change_view",
  "normalize_blog_post_body_format",
  "BlogPostAdminStatusBadgeViewModel",
  "blog_post_admin_status_badge_view",
  "BlogPostAdminEditBannerViewModel",
  "edit_banner_class",
  "blog_post_admin_edit_banner_view",
  "BlogPostAdminRawBodyWarningViewModel",
  "raw_body_warning_class",
  "blog_post_admin_raw_body_warning_view",
  "BlogPostAdminPostsLoadViewModel",
  "blog_post_admin_posts_load_view",
  "blog_post_admin_posts_load_view_from_list",
  "show_archive_action",
  "archive_label",
  "delete_label",
  "selected_post_request",
  "issue_banner_class_or_hidden",
  "BlogPostAdminIssueBannerViewModel",
  "blog_post_admin_issue_banner_view",
  "BlogPostStatusCommand",
  "prepare_blog_post_status_command",
  "BlogPostArchiveCommand",
  "prepare_blog_post_archive_command",
  "BlogPostDeleteCommand",
  "prepare_blog_post_delete_command",
  "BlogPostAdminRouteQueryIntent",
  "blog_post_admin_open_post_query_intent",
  "blog_post_admin_saved_post_query_intent",
  "blog_post_admin_clear_post_query_intent",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned FFA helper ${marker}`);
}

assertContains(ui, "use crate::{core, transport};", `${uiPath}: Leptos adapter must consume core and transport layers`);
assertContains(ui, "core::prepare_blog_post_save_command", `${uiPath}: UI must use core-owned save command preparation`);
assertContains(ui, "core::BlogPostSaveOperation", `${uiPath}: UI must dispatch core-owned save operations`);
assertContains(ui, "core::blog_post_admin_edit_banner_view", `${uiPath}: UI must use core-owned edit-banner view policy`);
assertContains(ui, "core::blog_post_admin_raw_body_warning_view", `${uiPath}: UI must use core-owned raw-body warning view policy`);
assertContains(ui, "core::blog_post_admin_posts_load_view_from_list", `${uiPath}: UI must use core-owned posts load result view-list normalization policy`);
assertContains(ui, "core::blog_post_admin_status_badge_view", `${uiPath}: UI must use core-owned status badge presentation policy`);
assertContains(ui, "core::blog_post_admin_editor_form_copy_view", `${uiPath}: UI must use core-owned editor form copy presentation policy`);
assertContains(ui, "core::blog_post_admin_editor_field_classes_view", `${uiPath}: UI must use core-owned editor field class presentation policy`);
assertContains(ui, "core::blog_post_admin_title_input_view", `${uiPath}: UI must use core-owned title input/autoslug policy`);
assertContains(ui, "core::blog_post_admin_body_format_select_view", `${uiPath}: UI must use core-owned body-format select option policy`);
assertContains(ui, "core::blog_post_admin_body_format_change_view", `${uiPath}: UI must use core-owned body-format change normalization policy`);
assertContains(ui, "core::blog_post_admin_posts_table_view_from_items", `${uiPath}: UI must use core-owned posts-table normalization and row view-model policy`);
assertContains(ui, "core::blog_post_admin_table_classes_view", `${uiPath}: UI must use core-owned posts-table class presentation policy`);
assertContains(ui, "core::blog_post_admin_shell_classes_view", `${uiPath}: UI must use core-owned admin shell class presentation policy`);
assertContains(ui, "core::blog_post_load_result_view", `${uiPath}: UI must use core-owned load result policy`);
assertContains(ui, "core::blog_post_transport_failure_issue", `${uiPath}: UI must use core-owned transport failure issue mapping`);
assertContains(ui, "core::blog_post_save_result_view", `${uiPath}: UI must use core-owned save result policy`);
assertContains(ui, "apply_query_intent", `${uiPath}: UI must apply core-owned route/query intents through the Leptos writer adapter`);
assertContains(ui, "core::blog_post_admin_open_post_query_intent", `${uiPath}: UI must use core-owned open-post query intent`);
assertContains(ui, "core::blog_post_admin_clear_post_query_intent", `${uiPath}: UI must use core-owned clear-post query intent`);
assertContains(ui, "transport::is_posts_contract_unavailable", `${uiPath}: UI must use transport-owned posts contract-unavailable classification`);
assertContains(ui, "core::prepare_blog_post_status_command", `${uiPath}: UI must use core-owned status command preparation`);
assertContains(ui, "core::prepare_blog_post_archive_command", `${uiPath}: UI must use core-owned archive command preparation`);
assertContains(ui, "core::prepare_blog_post_delete_command", `${uiPath}: UI must use core-owned delete command preparation`);
assertContains(ui, "transport::fetch_posts", `${uiPath}: UI must call the module-owned transport facade`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "PostService", "CategoryService", "TagService", "CommentService"]) {
  assertNotContains(ui, marker, `${uiPath}: CRUD UI adapter must not call raw transport or services (${marker})`);
}

for (const marker of [
  "use_route_query_value(AdminQueryKey::PostId.as_str())",
  "transport::fetch_moderation_comments",
  "transport::moderate_comment",
  "transport::is_moderation_contract_unavailable",
  "BlogModerationStatus::Approved",
  "BlogModerationStatus::Spam",
  "BlogModerationStatus::Trash",
]) {
  assertContains(moderation, marker, `${moderationPath}: missing moderation UI boundary marker ${marker}`);
}
for (const marker of ["crate::api", "#[server", "PostService", "CommentService", "DatabaseConnection"]) {
  assertNotContains(moderation, marker, `${moderationPath}: moderation UI must use only the transport facade (${marker})`);
}

for (const marker of [
  "fetch_posts",
  "is_posts_contract_unavailable",
  "fetch_post",
  "create_post",
  "update_post",
  "publish_post",
  "unpublish_post",
  "archive_post",
  "delete_post",
  "fetch_moderation_comments",
  "moderate_comment",
  "is_moderation_contract_unavailable",
]) {
  assertContains(transport, marker, `${transportPath}: transport facade must expose ${marker}`);
}
assertContains(transport, "mod graphql_adapter;", `${transportPath}: transport facade must own the CRUD GraphQL adapter module`);
assertContains(transport, "mod moderation_adapter;", `${transportPath}: transport facade must own the moderation adapter module`);
assertContains(transport, "graphql_adapter::", `${transportPath}: transport facade must delegate CRUD through transport/graphql_adapter.rs`);
assertContains(transport, "moderation_adapter::", `${transportPath}: transport facade must delegate moderation through transport/moderation_adapter.rs`);
assertNotContains(transport, "#[server", `${transportPath}: server/native endpoints must not live in the blog admin transport facade`);
assertContains(graphqlAdapter, "GraphqlRequest", `${graphqlAdapterPath}: blog admin GraphQL adapter must keep the GraphQL transport contract`);
assertContains(graphqlAdapter, "BLOG_POSTS_QUERY", `${graphqlAdapterPath}: GraphQL adapter must own blog posts query text`);
assertNotContains(graphqlAdapter, "Err(error) if is_posts_contract_unavailable", `${graphqlAdapterPath}: GraphQL adapter must not swallow posts contract-unavailable errors before the UI parity branch can classify them`);
for (const marker of [
  "BLOG_MODERATION_COMMENTS_QUERY",
  "MODERATE_BLOG_COMMENT_MUTATION",
  "moderationComments",
  "moderateComment",
  "BlogCommentModerationStatus!",
]) {
  assertContains(moderationAdapter, marker, `${moderationAdapterPath}: missing moderation GraphQL marker ${marker}`);
}

for (const marker of [
  "async fn moderation_comments(",
  "Permission::BLOG_POSTS_MANAGE",
  "GqlModerationCommentList",
]) {
  assertContains(graphqlTypes, marker, `${graphqlTypesPath}: missing authenticated moderation queue marker ${marker}`);
}
for (const marker of [
  "async fn moderate_comment(",
  "Permission::BLOG_POSTS_MANAGE",
  "ModerateCommentInput",
]) {
  assertContains(graphqlMutation, marker, `${graphqlMutationPath}: missing comment moderation mutation marker ${marker}`);
}
for (const marker of [
  "ModerateComment",
  "moderateComment",
  "Permission::BLOG_POSTS_MANAGE",
]) {
  assertContains(graphqlRateLimit, marker, `${graphqlRateLimitPath}: missing moderation rate-limit marker ${marker}`);
}

assertContains(implementationPlan, "verify-blog-admin-boundary.mjs", `${implementationPlanPath}: local plan must mention the blog fast boundary guardrail`);
assertContains(implementationPlan, "moderation", `${implementationPlanPath}: local plan must record moderation parity`);
assertContains(registry, "verify-blog-admin-boundary.mjs", `${registryPath}: central readiness board must mention the blog fast boundary guardrail`);

if (failures.length > 0) {
  console.error("blog admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("blog admin boundary verification passed");
