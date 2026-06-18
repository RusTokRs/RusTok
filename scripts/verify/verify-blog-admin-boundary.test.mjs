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

function libSource({ publicTransportPassthrough = false, includeLegacyApiMod = false, includeApiLikeText = false } = {}) {
  return `
${includeLegacyApiMod ? "mod api;" : ""}
mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::BlogAdmin;
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
pub struct BlogPostAdminFormViewModel;
pub fn blog_post_admin_form_view() {}
pub struct BlogPostStatusBadgeViewModel;
pub fn blog_post_status_badge_view() {}
pub struct BlogPostAdminEditBannerViewModel;
pub fn blog_post_admin_edit_banner_view() {}
pub struct BlogPostAdminRawBodyWarningViewModel;
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
    let _status_badge = core::blog_post_status_badge_view;
    let form_view = Memo::new;
    let _posts_load = core::blog_post_admin_posts_load_view_from_list;
    let _apply = apply_blog_post_admin_route_query_intent;
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

function transportSource({ includeServerEndpoint = false } = {}) {
  return `
mod graphql_adapter;

pub fn is_posts_contract_unavailable() { graphql_adapter::is_posts_contract_unavailable(); }
pub async fn fetch_posts() { graphql_adapter::fetch_posts().await; }
pub async fn fetch_post() { graphql_adapter::fetch_post().await; }
pub async fn create_post() { graphql_adapter::create_post().await; }
pub async fn update_post() { graphql_adapter::update_post().await; }
pub async fn publish_post() { graphql_adapter::publish_post().await; }
pub async fn unpublish_post() { graphql_adapter::unpublish_post().await; }
pub async fn archive_post() { graphql_adapter::archive_post().await; }
pub async fn delete_post() { graphql_adapter::delete_post().await; }
${includeServerEndpoint ? '#[server(prefix = "/api/fn", endpoint = "bad")] async fn bad() {}' : ""}
`;
}

function graphqlAdapterSource({ swallowPostsContractUnavailable = false } = {}) {
  return `
use leptos_graphql::GraphqlRequest;
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

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-blog-boundary-"));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/lib.rs", libSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/core.rs", coreSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/ui/leptos.rs", uiSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/transport/mod.rs", transportSource(options));
  writeFixtureFile(root, "crates/rustok-blog/admin/src/transport/graphql_adapter.rs", graphqlAdapterSource(options));
  if (options.includeLegacyApiFile) {
    writeFixtureFile(root, "crates/rustok-blog/admin/src/api.rs", "pub async fn fetch_posts() {}");
  }
  writeFixtureFile(root, "crates/rustok-blog/docs/implementation-plan.md", "verify-blog-admin-boundary.mjs");
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

test("blog admin boundary verifier passes canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /blog admin boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("blog admin boundary verifier rejects Leptos-specific core", () => {
  const root = withFixture({ includeLeptos: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected Leptos core fixture to fail");
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});



test("blog admin boundary verifier allows non-module api text in crate root", () => {
  const root = withFixture({ includeApiLikeText: true });
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("blog admin boundary verifier rejects legacy api module wiring", () => {
  const root = withFixture({ includeLegacyApiMod: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected legacy api module wiring fixture to fail");
    assert.match(result.stderr, /must not wire legacy api.rs/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("blog admin boundary verifier rejects legacy api file", () => {
  const root = withFixture({ includeLegacyApiFile: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected legacy api file fixture to fail");
    assert.match(result.stderr, /legacy GraphQL api adapter must live under transport\/graphql_adapter.rs/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("blog admin boundary verifier rejects raw api calls from UI", () => {
  const root = withFixture({ rawApiCall: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected raw UI api fixture to fail");
    assert.match(result.stderr, /UI adapter must not call raw transport or services/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("blog admin boundary verifier rejects public crate-root transport passthroughs", () => {
  const root = withFixture({ publicTransportPassthrough: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected public transport passthrough fixture to fail");
    assert.match(result.stderr, /crate root must not expose public transport passthroughs/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("blog admin boundary verifier rejects missing save command helper", () => {
  const root = withFixture({ omitSaveCommand: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected missing save-command fixture to fail");
    assert.match(result.stderr, /prepare_blog_post_save_command/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("blog admin boundary verifier rejects server functions in transport facade", () => {
  const root = withFixture({ includeServerEndpoint: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected transport server-function fixture to fail");
    assert.match(result.stderr, /server\/native endpoints must not live in the blog admin transport facade/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("blog admin boundary verifier rejects swallowed posts contract-unavailable errors", () => {
  const root = withFixture({ swallowPostsContractUnavailable: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected swallowed contract-unavailable fixture to fail");
    assert.match(result.stderr, /must not swallow posts contract-unavailable errors/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
