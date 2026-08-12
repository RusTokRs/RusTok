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

function readJson(relativePath) {
  return JSON.parse(readRepo(relativePath));
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
const modelPath = "crates/rustok-blog/admin/src/model.rs";
const uiPath = "crates/rustok-blog/admin/src/ui/leptos.rs";
const richtextAdapterPath = "crates/rustok-blog/admin/src/ui/richtext.rs";
const sharedRichtextAdapterPath = "crates/leptos-ui/src/richtext.rs";
const moderationPath = "crates/rustok-blog/admin/src/moderation.rs";
const transportPath = "crates/rustok-blog/admin/src/transport/mod.rs";
const graphqlAdapterPath = "crates/rustok-blog/admin/src/transport/graphql_adapter.rs";
const moderationAdapterPath = "crates/rustok-blog/admin/src/transport/moderation_adapter.rs";
const nativeAdapterPath = "crates/rustok-blog/admin/src/transport/native_server_adapter.rs";
const hostCargoPath = "apps/admin/Cargo.toml";
const graphqlTypesPath = "crates/rustok-blog/src/graphql/types.rs";
const graphqlMutationPath = "crates/rustok-blog/src/graphql/mutation.rs";
const graphqlRateLimitPath = "crates/rustok-blog/src/graphql/rate_limit.rs";
const legacyApiPath = "crates/rustok-blog/admin/src/api.rs";
const implementationPlanPath = "crates/rustok-blog/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const adminRichtextEvidencePath = "crates/rustok-blog/contracts/evidence/blog-admin-richtext-boundary.json";
const adminEnLocalePath = "crates/rustok-blog/admin/locales/en.json";
const adminRuLocalePath = "crates/rustok-blog/admin/locales/ru.json";

if (existsSync(repoPath(legacyApiPath))) {
  fail(`${legacyApiPath}: legacy GraphQL api adapter must live under transport/graphql_adapter.rs`);
}

for (const filePath of [
  libPath,
  corePath,
  modelPath,
  uiPath,
  richtextAdapterPath,
  sharedRichtextAdapterPath,
  moderationPath,
  transportPath,
  graphqlAdapterPath,
  moderationAdapterPath,
  nativeAdapterPath,
  hostCargoPath,
  graphqlTypesPath,
  graphqlMutationPath,
  graphqlRateLimitPath,
  implementationPlanPath,
  registryPath,
  adminRichtextEvidencePath,
  adminEnLocalePath,
  adminRuLocalePath,
]) {
  assertExists(filePath, `${filePath}: expected blog admin FFA boundary file`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const model = readRepo(modelPath);
const ui = readRepo(uiPath);
const richtextAdapter = readRepo(richtextAdapterPath);
const sharedRichtextAdapter = readRepo(sharedRichtextAdapterPath);
const moderation = readRepo(moderationPath);
const transport = readRepo(transportPath);
const graphqlAdapter = readRepo(graphqlAdapterPath);
const moderationAdapter = readRepo(moderationAdapterPath);
const nativeAdapter = readRepo(nativeAdapterPath);
const hostCargo = readRepo(hostCargoPath);
const graphqlTypes = readRepo(graphqlTypesPath);
const graphqlMutation = readRepo(graphqlMutationPath);
const graphqlRateLimit = readRepo(graphqlRateLimitPath);
const implementationPlan = readRepo(implementationPlanPath);
const registry = readRepo(registryPath);
const adminRichtextEvidence = readJson(adminRichtextEvidencePath);
const adminEnLocale = readJson(adminEnLocalePath);
const adminRuLocale = readJson(adminRuLocalePath);

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
  "RichTextDocument",
  "content: &'a RichTextDocument",
  "content: RichTextDocument",
  "has_required_draft_fields",
  "BlogPostAdminStatusBadgeViewModel",
  "blog_post_admin_status_badge_view",
  "BlogPostAdminEditBannerViewModel",
  "edit_banner_class",
  "blog_post_admin_edit_banner_view",
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

const legacyRichtextAdminMarkers = [
  "BlogPostAdminBodyFormatSelectViewModel",
  "BlogPostAdminBodyFormatOptionViewModel",
  "blog_post_admin_body_format_select_view",
  "BlogPostAdminBodyFormatChangeViewModel",
  "blog_post_admin_body_format_change_view",
  "normalize_blog_post_body_format",
  "BlogPostAdminRawBodyWarningViewModel",
  "raw_body_warning_class",
  "blog_post_admin_raw_body_warning_view",
];
for (const marker of legacyRichtextAdminMarkers) {
  assertNotContains(core, marker, `${corePath}: canonical richtext core must not reintroduce legacy admin helper ${marker}`);
  assertNotContains(ui, marker, `${uiPath}: canonical richtext UI must not reintroduce legacy admin helper ${marker}`);
}

const legacyRichtextLocaleKeys = ["blog.form.bodyFormat", "blog.form.rawWarning"];
for (const [localePath, catalog] of [
  [adminEnLocalePath, adminEnLocale],
  [adminRuLocalePath, adminRuLocale],
]) {
  for (const key of legacyRichtextLocaleKeys) {
    if (Object.prototype.hasOwnProperty.call(catalog, key)) {
      fail(`${localePath}: canonical richtext locale catalog must not expose legacy key ${key}`);
    }
  }
}

assertContains(ui, "use crate::{core, transport};", `${uiPath}: Leptos adapter must consume core and transport layers`);
assertContains(ui, "core::prepare_blog_post_save_command", `${uiPath}: UI must use core-owned save command preparation`);
assertContains(ui, "core::BlogPostSaveOperation", `${uiPath}: UI must dispatch core-owned save operations`);
assertContains(ui, "core::blog_post_admin_edit_banner_view", `${uiPath}: UI must use core-owned edit-banner view policy`);
assertContains(ui, "use super::richtext::BlogRichTextEditor;", `${uiPath}: UI must mount the owner richtext lifecycle adapter`);
assertContains(ui, "let (content, set_content) = signal(RichTextDocument::empty());", `${uiPath}: UI must keep canonical document state`);
assertContains(ui, "<BlogRichTextEditor", `${uiPath}: UI must render the owner richtext editor`);
assertContains(ui, "document=content", `${uiPath}: UI must pass canonical document state to the editor`);
assertContains(ui, "set_document=set_content", `${uiPath}: UI must receive canonical document updates from the editor`);
assertContains(ui, "content_locale=locale", `${uiPath}: UI must pass the owner-selected content locale`);
assertContains(ui, "disabled=Signal::derive", `${uiPath}: UI must pass dynamic busy/read-only state`);
for (const [marker, description] of [
  ["pub fn BlogRichTextEditor(", "owner editor component"],
  ["ReadSignal<RichTextDocument>", "typed controlled input"],
  ["WriteSignal<RichTextDocument>", "typed controlled output"],
  ["content_locale: ReadSignal<String>", "owner-selected content locale"],
  ["disabled: Signal<bool>", "dynamic read-only state"],
  ["t(ui_locale.as_deref(), key, fallback)", "host-provided UI locale"],
  ["RichTextEditorFrame", "shared editor frame component"],
  ['profile="article".to_string()', "fixed Article profile"],
  ["localized_richtext_frame_copy", "host-localized shared frame copy"],
]) {
  assertContains(richtextAdapter, marker, `${richtextAdapterPath}: missing ${description}`);
}
for (const [marker, description] of [
  ["pub fn RichTextEditorFrame(", "shared controlled editor component"],
  ["mount_richtext_frame", "shared frame mount"],
  ['"/richtext/frame"', "canonical frame route"],
  ["serde_json::from_str::<RichTextDocument>", "typed RichTextDocument deserialization"],
  ["set_document.set(document)", "typed document state update"],
  ["set_richtext_authoring_context", "dynamic content locale and spellcheck update"],
  ["set_richtext_editable", "dynamic editable/read-only update"],
  ['sandbox="allow-scripts"', "isolated script-only iframe sandbox"],
  ['referrerpolicy="no-referrer"', "no-referrer iframe policy"],
  ["on_cleanup", "frame cleanup hook"],
  ["dispose_richtext_frame", "frame disposal"],
]) {
  assertContains(sharedRichtextAdapter, marker, `${sharedRichtextAdapterPath}: missing ${description}`);
}
assertNotContains(
  sharedRichtextAdapter,
  /sandbox="[^"]*allow-same-origin/,
  `${sharedRichtextAdapterPath}: shared iframe must not grant allow-same-origin`,
);
for (const marker of ['"discussion"', "serde_json::from_str::<serde_json::Value>"]) {
  assertNotContains(richtextAdapter, marker, `${richtextAdapterPath}: owner Article adapter contains forbidden ${marker}`);
}
assertNotContains(
  richtextAdapter,
  'unwrap_or_else(|| "en".to_string())',
  `${richtextAdapterPath}: owner wrapper must not invent a UI locale fallback`,
);
for (const marker of ["mount_richtext_frame", "dispose_richtext_frame", 'sandbox="allow-scripts"', "serde_json::from_str"]) {
  assertNotContains(richtextAdapter, marker, `${richtextAdapterPath}: owner wrapper must not duplicate shared frame lifecycle ${marker}`);
}
assertContains(ui, "core::blog_post_admin_posts_load_view_from_list", `${uiPath}: UI must use core-owned posts load result view-list normalization policy`);
assertContains(ui, "core::blog_post_admin_status_badge_view", `${uiPath}: UI must use core-owned status badge presentation policy`);
assertContains(ui, "core::blog_post_admin_editor_form_copy_view", `${uiPath}: UI must use core-owned editor form copy presentation policy`);
assertContains(ui, "core::blog_post_admin_editor_field_classes_view", `${uiPath}: UI must use core-owned editor field class presentation policy`);
assertContains(ui, "core::blog_post_admin_title_input_view", `${uiPath}: UI must use core-owned title input/autoslug policy`);
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
assertContains(transport, "mod native_server_adapter;", `${transportPath}: transport facade must own the native server-function adapter module`);
assertContains(transport, "execute_selected_transport", `${transportPath}: transport facade must select one transport without fallback`);
assertContains(transport, "UiTransportPath::NativeServer", `${transportPath}: Leptos SSR/hydrate profiles must select native server functions`);
assertContains(transport, "UiTransportPath::Graphql", `${transportPath}: GraphQL must remain the parallel public/headless transport`);
assertContains(transport, "graphql_adapter::", `${transportPath}: transport facade must delegate CRUD through transport/graphql_adapter.rs`);
assertContains(transport, "moderation_adapter::", `${transportPath}: transport facade must delegate moderation through transport/moderation_adapter.rs`);
assertContains(transport, "native_server_adapter::", `${transportPath}: transport facade must delegate the Leptos path through native server functions`);
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
  "#[server(",
  "HostRuntimeContext",
  "TransactionalEventBus",
  "AuthContext",
  "TenantContext",
  "security_context_from_access_token",
  "PostService",
  "CommentService",
  "Permission::BLOG_POSTS_MANAGE",
  "blog_admin_posts_native",
  "blog_admin_create_post_native",
  "blog_admin_update_post_native",
  "blog_admin_moderation_comments_native",
  "blog_admin_moderate_comment_native",
]) {
  assertContains(nativeAdapter, marker, `${nativeAdapterPath}: missing native server-function boundary marker ${marker}`);
}
for (const marker of ["GraphqlRequest", "BLOG_POSTS_QUERY", "MODERATE_BLOG_COMMENT_MUTATION"]) {
  assertNotContains(nativeAdapter, marker, `${nativeAdapterPath}: native server-function adapter must not call GraphQL (${marker})`);
}
for (const marker of ["body_format", "bodyFormat", "raw_body", "rawWarning", "rt_json_v1", "rich_text_v1"]) {
  assertNotContains(core, marker, `${corePath}: canonical rich-text admin core must not expose legacy format state (${marker})`);
  assertNotContains(model, marker, `${modelPath}: canonical rich-text admin model must not expose legacy format state (${marker})`);
  assertNotContains(ui, marker, `${uiPath}: canonical rich-text admin UI must not expose legacy format state (${marker})`);
}
for (const feature of ["rustok-blog-admin/csr", "rustok-blog-admin/hydrate", "rustok-blog-admin/ssr"]) {
  assertContains(hostCargo, feature, `${hostCargoPath}: host must propagate the blog admin ${feature.split("/")[1]} feature`);
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

if (
  adminRichtextEvidence.schema_version !== 3 ||
  adminRichtextEvidence.module !== "blog" ||
  adminRichtextEvidence.surface !== "leptos_admin_article_richtext_boundary" ||
  adminRichtextEvidence.status !== "source_verified_no_compile" ||
  adminRichtextEvidence.compile_policy !== "not_run_by_request"
) {
  fail(`${adminRichtextEvidencePath}: evidence identity/status drift`);
}
if (
  adminRichtextEvidence.sources?.core !== corePath ||
  adminRichtextEvidence.sources?.ui !== uiPath ||
  adminRichtextEvidence.sources?.adapter !== richtextAdapterPath ||
  adminRichtextEvidence.sources?.shared_adapter !== sharedRichtextAdapterPath ||
  adminRichtextEvidence.sources?.locales?.en !== adminEnLocalePath ||
  adminRichtextEvidence.sources?.locales?.ru !== adminRuLocalePath ||
  adminRichtextEvidence.verifier !== "scripts/verify/verify-blog-admin-boundary.mjs" ||
  adminRichtextEvidence.self_test !== "scripts/verify/verify-blog-admin-boundary.test.mjs"
) {
  fail(`${adminRichtextEvidencePath}: evidence source/verifier path drift`);
}
for (const marker of adminRichtextEvidence.required_markers?.core ?? []) {
  assertContains(core, marker, `${corePath}: evidence-required canonical core marker ${marker}`);
}
for (const marker of adminRichtextEvidence.required_markers?.ui ?? []) {
  assertContains(ui, marker, `${uiPath}: evidence-required canonical UI marker ${marker}`);
}
for (const marker of adminRichtextEvidence.required_markers?.adapter ?? []) {
  assertContains(richtextAdapter, marker, `${richtextAdapterPath}: evidence-required owner adapter marker ${marker}`);
}
for (const marker of adminRichtextEvidence.required_markers?.shared_adapter ?? []) {
  assertContains(sharedRichtextAdapter, marker, `${sharedRichtextAdapterPath}: evidence-required shared adapter marker ${marker}`);
}
for (const marker of adminRichtextEvidence.forbidden_adapter_markers ?? []) {
  assertNotContains(richtextAdapter, marker, `${richtextAdapterPath}: evidence-forbidden owner adapter marker ${marker}`);
}
for (const marker of adminRichtextEvidence.forbidden_markers ?? []) {
  assertNotContains(core, marker, `${corePath}: evidence-forbidden legacy richtext marker ${marker}`);
  assertNotContains(ui, marker, `${uiPath}: evidence-forbidden legacy richtext marker ${marker}`);
}
const evidenceLocaleKeys = [...(adminRichtextEvidence.forbidden_locale_keys ?? [])].sort();
const expectedLocaleKeys = [...legacyRichtextLocaleKeys].sort();
if (JSON.stringify(evidenceLocaleKeys) !== JSON.stringify(expectedLocaleKeys)) {
  fail(`${adminRichtextEvidencePath}: forbidden locale key evidence drift`);
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
