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

function coreSource({ includeLeptos = false, omitSaveCommand = false } = {}) {
  return `
${includeLeptos ? "use leptos::prelude::*;" : ""}
pub struct BlogPostFormInput;
pub fn build_blog_post_draft() {}
${omitSaveCommand ? "" : "pub enum BlogPostSaveOperation { Create }\npub struct BlogPostSaveCommand;\npub fn prepare_blog_post_save_command() {}"}
pub struct BlogPostEditorFormState;
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
pub struct BlogPostAdminBodyFormatSelectViewModel;
pub struct BlogPostAdminBodyFormatOptionViewModel;
pub fn blog_post_admin_body_format_select_view() {}
pub struct BlogPostAdminBodyFormatChangeViewModel;
pub fn blog_post_admin_body_format_change_view() {}
pub fn normalize_blog_post_body_format() {}
pub struct BlogPostAdminStatusBadgeViewModel;
pub fn blog_post_admin_status_badge_view() {}
pub struct BlogPostAdminEditBannerViewModel;
pub fn edit_banner_class() {}
pub fn blog_post_admin_edit_banner_view() {}
pub struct BlogPostAdminRawBodyWarningViewModel;
pub fn raw_body_warning_class() {}
pub fn blog_post_admin_raw_body_warning_view() {}
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

function uiSource({ rawApiCall = false, rawServiceCall = false, omitSaveCommand = false } = {}) {
  return `
use crate::{core, transport};

pub fn BlogAdmin() {
    let _posts = transport::fetch_posts;
    ${omitSaveCommand ? "" : "let _save = core::prepare_blog_post_save_command;\n    let _op = core::BlogPostSaveOperation::Create;"}
    let _load = core::blog_post_load_result_view;
    let _failure = core::blog_post_transport_failure_issue;
    let _saved = core::blog_post_save_result_view;
    let _edit_banner = core::blog_post_admin_edit_banner_view;
    let _raw_warning = core::blog_post_admin_raw_body_warning_view;
    let _posts_load = core::blog_post_admin_posts_load_view_from_list;
    let _status_badge = core::blog_post_admin_status_badge_view;
    let _form_copy = core::blog_post_admin_editor_form_copy_view;
    let _field_classes = core::blog_post_admin_editor_field_classes_view;
    let _title_input = core::blog_post_admin_title_input_view;
    let _body_format = core::blog_post_admin_body_format_select_view;
    let _body_format_change = core::blog_post_admin_body_format_change_view;
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

pub fn is_posts_contract_unavailable() { graphql_adapter::is_posts_contract_unavailable(); }
pub async fn fetch_posts() { graphql_adapter::fetch_posts().await; }
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

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-blog-boundary-"));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/lib.rs", libSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/core.rs", coreSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/ui/leptos.rs", uiSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/moderation.rs", moderationSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/transport/mod.rs", transportSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/transport/graphql_adapter.rs", graphqlAdapterSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/transport/moderation_adapter.rs", moderationAdapterSource(options));
  writeFixtureFile(root, "crates/rustok-blog/src/graphql/types.rs", options.omitModeration ? "pub struct GqlPost;" : "async fn moderation_comments() {} Permission::BLOG_POSTS_MANAGE GqlModerationCommentList");
  writeFixtureFile(root, "crates/rustok-blog/src/graphql/mutation.rs", options.omitModeration ? "pub struct BlogMutation;" : "async fn moderate_comment() {} Permission::BLOG_POSTS_MANAGE ModerateCommentInput");
  writeFixtureFile(root, "crates/rustok-blog/src/graphql/rate_limit.rs", options.omitModeration ? "enum Surface {}" : "ModerateComment moderateComment Permission::BLOG_POSTS_MANAGE");
  if (options.includeLegacyApiFile) {
    writeFixtureFile(root, "crates/rustok-blog/admin/src/api.rs", "pub async fn fetch_posts() {}");
  }
  writeFixtureFile(root, "crates/rustok-blog/docs/implementation-plan.md", `verify-blog-admin-boundary.mjs ${options.omitModeration ? "" : "moderation"}`);
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

test("blog admin boundary verifier rejects missing moderation slice", () => {
  withRoot({ omitModeration: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /moderation/);
  });
});
