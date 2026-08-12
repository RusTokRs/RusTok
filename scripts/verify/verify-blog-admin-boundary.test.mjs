#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-blog-admin-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function libSource({ publicTransportPassthrough = false, includeLegacyApiMod = false, includeApiLikeText = false, omitModeration = false } = {}) {
  return `
${includeLegacyApiMod ? "mod api;" : ""}
mod core;
mod i18n;
mod model;
${omitModeration ? "" : "mod moderation;"}
mod transport;
mod ui;

pub fn BlogAdmin() {
  <BlogEditor />;
  ${omitModeration ? "" : "<BlogModerationPanel />;"}
}
${publicTransportPassthrough ? "pub async fn fetch_posts() {}" : ""}
${includeApiLikeText ? "// harmless api; text must not be treated as module wiring" : ""}
`;
}

function coreSource({ includeLeptos = false, omitSaveCommand = false, legacyRichtextCore = false } = {}) {
  return `
${includeLeptos ? "use leptos::prelude::*;" : ""}
use rustok_api::RichTextDocument;
pub struct BlogPostFormInput<'a> { pub content: &'a RichTextDocument }
pub fn build_blog_post_draft() {}
pub fn has_required_draft_fields() {}
${omitSaveCommand ? "" : "pub enum BlogPostSaveOperation { Create }\npub struct BlogPostSaveCommand;\npub fn prepare_blog_post_save_command() {}"}
pub struct BlogPostEditorFormState { pub content: RichTextDocument }
pub struct BlogPostAdminTableRowViewModel;
pub fn blog_post_admin_table_row_view() {}
pub struct BlogPostAdminTableViewModel;
pub fn blog_post_admin_table_view() {}
pub struct BlogPostAdminPostsTableViewModel;
pub struct BlogPostAdminPostsTableLabels;
pub fn blog_post_admin_posts_table_view_from_items() {}
pub struct BlogPostAdminFormViewModel;
pub fn blog_post_admin_form_view() {}
pub struct BlogPostAdminTableClassesViewModel;
pub fn blog_post_admin_table_classes_view() {}
pub struct BlogPostAdminShellClassesViewModel;
pub fn blog_post_admin_shell_classes_view() {}
pub struct BlogPostAdminEditorFormCopyViewModel;
pub struct BlogPostAdminEditorFormCopyLabels;
pub fn blog_post_admin_editor_form_copy_view() {}
pub struct BlogPostAdminEditorFieldClassesViewModel;
pub fn blog_post_admin_editor_field_classes_view() {}
pub struct BlogPostAdminTitleInputViewModel;
pub fn blog_post_admin_title_input_view() {}
${legacyRichtextCore ? "pub struct BlogPostAdminBodyFormatSelectViewModel;\npub struct BlogPostAdminBodyFormatOptionViewModel;\npub fn blog_post_admin_body_format_select_view() {}\npub struct BlogPostAdminBodyFormatChangeViewModel;\npub fn blog_post_admin_body_format_change_view() {}\npub fn normalize_blog_post_body_format() {}\npub struct BlogPostAdminRawBodyWarningViewModel;\npub fn raw_body_warning_class() {}\npub fn blog_post_admin_raw_body_warning_view() {}" : ""}
pub struct BlogPostAdminStatusBadgeViewModel;
pub fn blog_post_admin_status_badge_view() {}
pub struct BlogPostAdminEditBannerViewModel;
pub fn edit_banner_class() {}
pub fn blog_post_admin_edit_banner_view() {}
pub enum BlogPostAdminPostsLoadViewModel {}
pub fn blog_post_admin_posts_load_view() {}
pub fn blog_post_admin_posts_load_view_from_list() {}
pub fn selected_post_request() {}
pub fn issue_banner_class_or_hidden() {}
pub fn show_archive_action() {}
pub fn archive_label() {}
pub fn delete_label() {}
pub struct BlogPostAdminIssueBannerViewModel;
pub fn blog_post_admin_issue_banner_view() {}
pub struct BlogPostLoadResultViewModel;
pub fn blog_post_load_result_view() {}
pub fn blog_post_transport_failure_issue() {}
pub struct BlogPostSaveResultViewModel;
pub fn blog_post_save_result_view() {}
pub struct BlogPostStatusCommand;
pub fn prepare_blog_post_status_command() {}
pub struct BlogPostArchiveCommand;
pub fn prepare_blog_post_archive_command() {}
pub struct BlogPostDeleteCommand;
pub fn prepare_blog_post_delete_command() {}
pub enum BlogPostAdminRouteQueryIntent {}
pub fn blog_post_admin_open_post_query_intent() {}
pub fn blog_post_admin_saved_post_query_intent() {}
pub fn blog_post_admin_clear_post_query_intent() {}
`;
}

function uiSource({ rawApiCall = false, rawServiceCall = false, omitSaveCommand = false, legacyRichtextUi = false } = {}) {
  return `
use crate::{core, transport};
use rustok_api::RichTextDocument;
use super::richtext::BlogRichTextEditor;

pub fn BlogAdmin() {
    let (content, set_content) = signal(RichTextDocument::empty());
    let (locale, _set_locale) = signal(String::new());
    <BlogRichTextEditor
        document=content
        set_document=set_content
        content_locale=locale
        disabled=Signal::derive(move || false)
    />;
    let _posts = transport::fetch_posts;
    ${omitSaveCommand ? "" : "let _save = core::prepare_blog_post_save_command;\n    let _op = core::BlogPostSaveOperation::Create;"}
    let _load = core::blog_post_load_result_view;
    let _failure = core::blog_post_transport_failure_issue;
    let _saved = core::blog_post_save_result_view;
    let _edit_banner = core::blog_post_admin_edit_banner_view;
    ${legacyRichtextUi ? "let _raw_warning = core::blog_post_admin_raw_body_warning_view;\n    let _body_format = core::blog_post_admin_body_format_select_view;\n    let _body_format_change = core::blog_post_admin_body_format_change_view;" : ""}
    let _posts_load = core::blog_post_admin_posts_load_view_from_list;
    let _status_badge = core::blog_post_admin_status_badge_view;
    let _form_copy = core::blog_post_admin_editor_form_copy_view;
    let _field_classes = core::blog_post_admin_editor_field_classes_view;
    let _title_input = core::blog_post_admin_title_input_view;
    let _posts_table = core::blog_post_admin_posts_table_view_from_items;
    let _table_classes = core::blog_post_admin_table_classes_view;
    let _shell_classes = core::blog_post_admin_shell_classes_view;
    let _apply = apply_query_intent;
    let _open = core::blog_post_admin_open_post_query_intent;
    let _clear = core::blog_post_admin_clear_post_query_intent;
    let _status = core::prepare_blog_post_status_command;
    let _archive = core::prepare_blog_post_archive_command;
    let _delete = core::prepare_blog_post_delete_command;
    let _contract = transport::is_posts_contract_unavailable;
    ${rawApiCall ? "let _raw = api::fetch_posts;" : ""}
    ${rawServiceCall ? "let _service = PostService::new;" : ""}
}
`;
}

function richtextAdapterSource({
  wrongProfile = false,
} = {}) {
  return `
use leptos_ui::{RichTextEditorFrame, localized_richtext_frame_copy};
use rustok_api::RichTextDocument;
pub fn BlogRichTextEditor(
    document: ReadSignal<RichTextDocument>,
    set_document: WriteSignal<RichTextDocument>,
    content_locale: ReadSignal<String>,
    disabled: Signal<bool>,
) {
    let ui_locale = use_ui_route_context().map(|context| context.locale);
    let copy = localized_richtext_frame_copy(|key, fallback| {
        t(ui_locale.as_deref(), key, fallback)
    });
    <RichTextEditorFrame
        document=document
        set_document=set_document
        content_locale=Signal::from(content_locale)
        disabled=disabled
        profile="${wrongProfile ? "discussion" : "article"}".to_string()
        copy=copy
    />;
}
`;
}

function sharedRichtextAdapterSource({
  unsafeSandbox = false,
  untypedPayload = false,
  missingCleanup = false,
} = {}) {
  return `
use rustok_api::RichTextDocument;
pub fn RichTextEditorFrame() {
    mount_richtext_frame("/richtext/frame");
    ${untypedPayload ? "serde_json::from_str::<serde_json::Value>(document_json);" : "serde_json::from_str::<RichTextDocument>(document_json);"}
    set_document.set(document);
    set_richtext_authoring_context(&mounted_handle, content_locale.get(), spellcheck.get());
    set_richtext_editable(&mounted_handle, !disabled.get());
    sandbox="${unsafeSandbox ? "allow-scripts allow-same-origin" : "allow-scripts"}";
    referrerpolicy="no-referrer";
    ${missingCleanup ? "" : "on_cleanup(move || { dispose_richtext_frame(&mounted_handle); });"}
}
`;
}

function moderationSource({ rawServiceCall = false, omitModeration = false } = {}) {
  if (omitModeration) return "pub fn placeholder() {}";
  return `
use_route_query_value(AdminQueryKey::PostId.as_str());
transport::fetch_moderation_comments;
transport::moderate_comment;
transport::is_moderation_contract_unavailable;
BlogModerationStatus::Approved;
BlogModerationStatus::Spam;
BlogModerationStatus::Trash;
${rawServiceCall ? "CommentService::new;" : ""}
`;
}

function transportSource({ includeServerEndpoint = false, omitModeration = false } = {}) {
  return `
mod graphql_adapter;
${omitModeration ? "" : "mod moderation_adapter;"}
mod native_server_adapter;

use rustok_ui_transport::{execute_selected_transport, UiTransportPath};

fn selected_transport_path() -> UiTransportPath { UiTransportPath::Graphql }
pub fn is_posts_contract_unavailable() { graphql_adapter::is_posts_contract_unavailable(); }
pub async fn fetch_posts() { execute_selected_transport(UiTransportPath::NativeServer, native_server_adapter::fetch_posts, graphql_adapter::fetch_posts).await; }
pub async fn fetch_post() { graphql_adapter::fetch_post().await; }
pub async fn create_post() { graphql_adapter::create_post().await; }
pub async fn update_post() { graphql_adapter::update_post().await; }
pub async fn publish_post() { graphql_adapter::publish_post().await; }
pub async fn unpublish_post() { graphql_adapter::unpublish_post().await; }
pub async fn archive_post() { graphql_adapter::archive_post().await; }
pub async fn delete_post() { graphql_adapter::delete_post().await; }
${omitModeration ? "" : "pub async fn fetch_moderation_comments() { moderation_adapter::fetch_comments().await; }\npub async fn moderate_comment() { moderation_adapter::moderate_comment().await; }\npub fn is_moderation_contract_unavailable() { moderation_adapter::is_contract_unavailable(); }"}
${includeServerEndpoint ? '#[server(prefix = "/api/fn", endpoint = "bad")] async fn bad() {}' : ""}
`;
}

function graphqlAdapterSource({ swallowPostsContractUnavailable = false } = {}) {
  return `
use rustok_graphql::GraphqlRequest;
const BLOG_POSTS_QUERY: &str = "query BlogPostsAdmin { posts { total } }";
pub fn is_posts_contract_unavailable() {}
pub async fn fetch_posts() {
${swallowPostsContractUnavailable ? "    Err(error) if is_posts_contract_unavailable(&error) => return Ok(());" : ""}
}
pub async fn fetch_post() {}
pub async fn create_post() {}
pub async fn update_post() {}
pub async fn publish_post() {}
pub async fn unpublish_post() {}
pub async fn archive_post() {}
pub async fn delete_post() {}
`;
}

function moderationAdapterSource({ omitModeration = false } = {}) {
  if (omitModeration) return "pub fn placeholder() {}";
  return `
const BLOG_MODERATION_COMMENTS_QUERY: &str = "moderationComments";
const MODERATE_BLOG_COMMENT_MUTATION: &str = "moderateComment BlogCommentModerationStatus!";
`;
}

function nativeAdapterSource({ nativeGraphqlLeak = false } = {}) {
  return `
use rustok_api::{AuthContext, HostRuntimeContext, Permission, TenantContext};
use rustok_outbox::TransactionalEventBus;
use rustok_blog::{CommentService, PostService};
use rustok_core::security_context_from_access_token;
${nativeGraphqlLeak ? "use rustok_graphql::GraphqlRequest;" : ""}

#[server(prefix = "/api/fn", endpoint = "blog/admin/posts")]
async fn blog_admin_posts_native() {}
#[server(prefix = "/api/fn", endpoint = "blog/admin/create-post")]
async fn blog_admin_create_post_native() {}
#[server(prefix = "/api/fn", endpoint = "blog/admin/update-post")]
async fn blog_admin_update_post_native() {}
#[server(prefix = "/api/fn", endpoint = "blog/admin/moderation-comments")]
async fn blog_admin_moderation_comments_native() {}
#[server(prefix = "/api/fn", endpoint = "blog/admin/moderate-comment")]
async fn blog_admin_moderate_comment_native() {
  let _ = Permission::BLOG_POSTS_MANAGE;
}
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-blog-boundary-"));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/lib.rs", libSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/core.rs", coreSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/model.rs", "pub struct BlogPostDraft; pub struct BlogPostDetail;");
  writeFixtureFile(root, "crates/rustok-blog/admin/src/ui/leptos.rs", uiSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/ui/richtext.rs", richtextAdapterSource(options));
  writeFixtureFile(root, "crates/leptos-ui/src/richtext.rs", sharedRichtextAdapterSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/moderation.rs", moderationSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/transport/mod.rs", transportSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/transport/graphql_adapter.rs", graphqlAdapterSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/transport/moderation_adapter.rs", moderationAdapterSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/transport/native_server_adapter.rs", nativeAdapterSource(options));
  writeFixtureFile(root, "apps/admin/Cargo.toml", 'csr = ["rustok-blog-admin/csr"]\nhydrate = ["rustok-blog-admin/hydrate"]\nssr = ["rustok-blog-admin/ssr"]');
  writeFixtureFile(root, "crates/rustok-blog/src/graphql/types.rs", options.omitModeration ? "pub struct GqlPost;" : "async fn moderation_comments() {} Permission::BLOG_POSTS_MANAGE GqlModerationCommentList");
  writeFixtureFile(root, "crates/rustok-blog/src/graphql/mutation.rs", options.omitModeration ? "pub struct BlogMutation;" : "async fn moderate_comment() {} Permission::BLOG_POSTS_MANAGE ModerateCommentInput");
  writeFixtureFile(root, "crates/rustok-blog/src/graphql/rate_limit.rs", options.omitModeration ? "enum Surface {}" : "ModerateComment moderateComment Permission::BLOG_POSTS_MANAGE");
  if (options.includeLegacyApiFile) {
    writeFixtureFile(root, "crates/rustok-blog/admin/src/api.rs", "pub async fn fetch_posts() {}");
  }
  writeFixtureFile(root, "crates/rustok-blog/docs/implementation-plan.md", `verify-blog-admin-boundary.mjs ${options.omitModeration ? "" : "moderation"}`);
  const localeCatalog = { "blog.form.body": "Body" };
  if (options.legacyLocaleKeys) {
    localeCatalog["blog.form.bodyFormat"] = "Body format";
    localeCatalog["blog.form.rawWarning"] = "Raw payload warning";
  }
  writeFixtureFile(root, "crates/rustok-blog/admin/locales/en.json", JSON.stringify(localeCatalog));
  writeFixtureFile(root, "crates/rustok-blog/admin/locales/ru.json", JSON.stringify(localeCatalog));
  writeFixtureFile(root, "crates/rustok-blog/contracts/evidence/blog-admin-richtext-boundary.json", JSON.stringify({
    schema_version: 3,
    module: "blog",
    surface: "leptos_admin_article_richtext_boundary",
    status: "source_verified_no_compile",
    compile_policy: "not_run_by_request",
    sources: {
      core: "crates/rustok-blog/admin/src/core.rs",
      ui: "crates/rustok-blog/admin/src/ui/leptos.rs",
      adapter: "crates/rustok-blog/admin/src/ui/richtext.rs",
      shared_adapter: "crates/leptos-ui/src/richtext.rs",
      locales: {
        en: "crates/rustok-blog/admin/locales/en.json",
        ru: "crates/rustok-blog/admin/locales/ru.json"
      }
    },
    required_markers: {
      core: ["RichTextDocument", "content: &'a RichTextDocument", "content: RichTextDocument", "has_required_draft_fields"],
      ui: ["use super::richtext::BlogRichTextEditor;", "let (content, set_content) = signal(RichTextDocument::empty());", "<BlogRichTextEditor", "document=content", "set_document=set_content"],
      adapter: ["pub fn BlogRichTextEditor(", "ReadSignal<RichTextDocument>", "WriteSignal<RichTextDocument>", "RichTextEditorFrame", "profile=\"article\".to_string()", "localized_richtext_frame_copy"],
      shared_adapter: ["pub fn RichTextEditorFrame(", "mount_richtext_frame", "\"/richtext/frame\"", "serde_json::from_str::<RichTextDocument>", "set_document.set(document)", "sandbox=\"allow-scripts\"", "referrerpolicy=\"no-referrer\"", "on_cleanup", "dispose_richtext_frame"]
    },
    forbidden_adapter_markers: ["\"discussion\"", "allow-same-origin", "serde_json::from_str::<serde_json::Value>", "mount_richtext_frame", "dispose_richtext_frame", "sandbox=\"allow-scripts\"", "serde_json::from_str"],
    forbidden_markers: ["blog_post_admin_body_format_select_view", "blog_post_admin_body_format_change_view", "normalize_blog_post_body_format", "blog_post_admin_raw_body_warning_view"],
    forbidden_locale_keys: ["blog.form.bodyFormat", "blog.form.rawWarning"],
    verifier: "scripts/verify/verify-blog-admin-boundary.mjs",
    self_test: "scripts/verify/verify-blog-admin-boundary.test.mjs"
  }, null, 2));
  writeFixtureFile(root, "docs/modules/registry.md", "verify-blog-admin-boundary.mjs");
  return root;
}

function runVerifier(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function withRoot(options, callback) {
  const root = withFixture(options);
  try {
    callback(runVerifier(root));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("blog admin boundary verifier passes canonical fixture", () => {
  withRoot({}, (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /blog admin boundary verification passed/);
  });
});

test("blog admin boundary verifier rejects legacy richtext helpers in core", () => {
  withRoot({ legacyRichtextCore: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /canonical richtext core must not reintroduce legacy admin helper/);
  });
});

test("blog admin boundary verifier rejects legacy richtext helpers in UI", () => {
  withRoot({ legacyRichtextUi: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /canonical richtext UI must not reintroduce legacy admin helper/);
  });
});

test("blog admin boundary verifier rejects legacy richtext locale keys", () => {
  withRoot({ legacyLocaleKeys: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /canonical richtext locale catalog must not expose legacy key/);
  });
});

test("blog admin boundary verifier rejects a non-Article owner editor profile", () => {
  withRoot({ wrongProfile: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /fixed Article profile|forbidden.*discussion/);
  });
});

test("blog admin boundary verifier rejects allow-same-origin on the editor iframe", () => {
  withRoot({ unsafeSandbox: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not grant allow-same-origin|evidence-forbidden owner adapter marker/);
  });
});

test("blog admin boundary verifier rejects untyped editor payload deserialization", () => {
  withRoot({ untypedPayload: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /typed RichTextDocument deserialization|evidence-required owner adapter marker/);
  });
});

test("blog admin boundary verifier rejects missing frame cleanup", () => {
  withRoot({ missingCleanup: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /frame cleanup hook|frame disposal|evidence-required owner adapter marker/);
  });
});

test("blog admin boundary verifier rejects Leptos-specific core", () => {
  withRoot({ includeLeptos: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  });
});

test("blog admin boundary verifier allows non-module api text in crate root", () => {
  withRoot({ includeApiLikeText: true }, (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
  });
});

test("blog admin boundary verifier rejects legacy api module wiring", () => {
  withRoot({ includeLegacyApiMod: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not wire legacy api.rs/);
  });
});

test("blog admin boundary verifier rejects legacy api file", () => {
  withRoot({ includeLegacyApiFile: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /legacy GraphQL api adapter must live under transport\/graphql_adapter.rs/);
  });
});

test("blog admin boundary verifier rejects raw api calls from CRUD UI", () => {
  withRoot({ rawApiCall: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /CRUD UI adapter must not call raw transport or services/);
  });
});

test("blog admin boundary verifier rejects raw service calls from moderation UI", () => {
  withRoot({ rawServiceCall: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /moderation UI must use only the transport facade/);
  });
});

test("blog admin boundary verifier rejects public crate-root transport passthroughs", () => {
  withRoot({ publicTransportPassthrough: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /crate root must not expose public transport passthroughs/);
  });
});

test("blog admin boundary verifier rejects missing save command helper", () => {
  withRoot({ omitSaveCommand: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /prepare_blog_post_save_command/);
  });
});

test("blog admin boundary verifier rejects server functions in transport facade", () => {
  withRoot({ includeServerEndpoint: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /server\/native endpoints must not live in the blog admin transport facade/);
  });
});

test("blog admin boundary verifier rejects swallowed posts contract-unavailable errors", () => {
  withRoot({ swallowPostsContractUnavailable: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not swallow posts contract-unavailable errors/);
  });
});

test("blog admin boundary verifier rejects GraphQL calls from the native adapter", () => {
  withRoot({ nativeGraphqlLeak: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /native server-function adapter must not call GraphQL/);
  });
});

test("blog admin boundary verifier rejects missing moderation slice", () => {
  withRoot({ omitModeration: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /moderation/);
  });
});
