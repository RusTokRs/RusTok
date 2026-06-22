#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-pages-ui-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function libSource(entrypoint, { publicTransportPassthrough = false } = {}) {
  return `
mod api;
mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::leptos::${entrypoint};
${publicTransportPassthrough ? "pub async fn fetch_pages() {}" : ""}
`;
}

function adminCoreSource({ includeLeptos = false, omitDraftHelper = false } = {}) {
  return `
${includeLeptos ? "use leptos::prelude::*;" : ""}
pub struct PageDraftFormInput;
${omitDraftHelper ? "" : "pub fn build_create_page_draft() {}"}
pub fn missing_required_page_field() {}
pub fn write_path_issue_with_context() {}
pub fn builder_host_fallback_surface() {}
pub fn publish_state_view() {}
pub fn channel_count_label() {}
pub fn legacy_block_snapshot_label() {}
pub fn is_save_action_busy() {}
pub fn is_publish_action_disabled() {}
pub fn admin_page_list_item_view() {}
pub fn admin_page_row_action_state() {}
pub fn admin_page_row_action_labels() {}
`;
}

function storefrontCoreSource({ includeLeptos = false } = {}) {
  return `
${includeLeptos ? "use leptos::prelude::*;" : ""}
pub fn selected_page_title() {}
pub fn selected_page_slug() {}
pub fn summarize_page_content() {}
pub fn storefront_builder_fallback_read_contract() {}
pub fn count_label() {}
pub fn page_link_href() {}
pub fn page_status_label() {}
pub fn selected_page_empty_state() {}
pub fn load_error_message() {}
pub fn storefront_page_list_item_view() {}
`;
}

function adminUiSource({ rawApiCall = false, rawServiceCall = false, omitDraftHelper = false } = {}) {
  return `
use crate::core;
use crate::transport;

pub fn PagesAdmin() {
    let _pages = transport::fetch_pages;
    ${omitDraftHelper ? "" : "let _draft = core::build_create_page_draft;"}
    let _publish_state = core::publish_state_view;
    let _legacy_block_label = core::legacy_block_snapshot_label;
    let _save_busy = core::is_save_action_busy;
    let _publish_disabled = core::is_publish_action_disabled;
    let _item_view = core::admin_page_list_item_view;
    let _action_state = core::admin_page_row_action_state;
    let _action_labels = core::admin_page_row_action_labels;
    ${rawApiCall ? "let _raw = api::fetch_pages;" : ""}
    ${rawServiceCall ? "let _service = PageService::new;" : ""}
}
`;
}

function storefrontUiSource({ rawApiCall = false } = {}) {
  return `
use crate::core;
use crate::transport;

pub fn PagesView() {
    let _pages = transport::fetch_pages;
    let _title = core::selected_page_title;
    let _empty = core::selected_page_empty_state;
    let _load_error = core::load_error_message;
    let _item_view = core::storefront_page_list_item_view;
    ${rawApiCall ? "let _raw = api::fetch_storefront_pages;" : ""}
}
`;
}

function adminTransportSource({ includeServerEndpoint = false } = {}) {
  return `
use crate::api;

pub async fn fetch_pages() { api::fetch_pages().await; }
pub async fn fetch_page() { api::fetch_page().await; }
pub async fn create_page() { api::create_page().await; }
pub async fn update_page() { api::update_page().await; }
pub async fn publish_page() { api::publish_page().await; }
pub async fn unpublish_page() { api::unpublish_page().await; }
pub async fn delete_page() { api::delete_page().await; }
${includeServerEndpoint ? '#[server(prefix = "/api/fn", endpoint = "bad")] async fn bad() {}' : ""}
`;
}

function storefrontTransportSource({ includeServerEndpoint = false } = {}) {
  return `
use crate::api;

pub async fn fetch_pages() { api::fetch_storefront_pages().await; }
${includeServerEndpoint ? '#[server(prefix = "/api/fn", endpoint = "bad")] async fn bad() {}' : ""}
`;
}

function adminApiSource() {
  return `
use leptos_graphql::GraphqlRequest;
pub async fn fetch_pages() {}
pub async fn fetch_page() {}
pub async fn create_page() {}
pub async fn update_page() {}
pub async fn publish_page() {}
pub async fn unpublish_page() {}
pub async fn delete_page() {}
`;
}

function storefrontApiSource() {
  return `
use leptos_graphql::GraphqlRequest;
#[server(prefix = "/api/fn", endpoint = "pages/storefront-data")]
pub async fn fetch_storefront_pages_server() {}
pub async fn fetch_storefront_pages() {}
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-pages-boundary-"));
  writeFixtureFile(root, "crates/rustok-pages/admin/src/lib.rs", libSource("PagesAdmin", options));
  writeFixtureFile(root, "crates/rustok-pages/admin/src/core.rs", adminCoreSource(options));
  writeFixtureFile(root, "crates/rustok-pages/admin/src/ui/leptos.rs", adminUiSource(options));
  writeFixtureFile(root, "crates/rustok-pages/admin/src/transport/mod.rs", adminTransportSource(options));
  writeFixtureFile(root, "crates/rustok-pages/admin/src/api.rs", adminApiSource());
  writeFixtureFile(root, "crates/rustok-pages/storefront/src/lib.rs", libSource("PagesView", options));
  writeFixtureFile(root, "crates/rustok-pages/storefront/src/core.rs", storefrontCoreSource(options));
  writeFixtureFile(root, "crates/rustok-pages/storefront/src/ui/leptos.rs", storefrontUiSource(options));
  writeFixtureFile(root, "crates/rustok-pages/storefront/src/transport.rs", storefrontTransportSource(options));
  writeFixtureFile(root, "crates/rustok-pages/storefront/src/api.rs", storefrontApiSource());
  writeFixtureFile(root, "crates/rustok-pages/docs/implementation-plan.md", "verify-pages-ui-boundary.mjs");
  writeFixtureFile(root, "docs/modules/registry.md", "verify-pages-ui-boundary.mjs");
  return root;
}

function runVerifier(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

test("pages UI boundary verifier passes canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /pages UI boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("pages UI boundary verifier rejects Leptos-specific core", () => {
  const root = withFixture({ includeLeptos: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected Leptos core fixture to fail");
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("pages UI boundary verifier rejects raw admin api calls", () => {
  const root = withFixture({ rawApiCall: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected raw UI api fixture to fail");
    assert.match(result.stderr, /UI adapter must not call raw transport or services/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("pages UI boundary verifier rejects public crate-root transport passthroughs", () => {
  const root = withFixture({ publicTransportPassthrough: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected public transport passthrough fixture to fail");
    assert.match(result.stderr, /crate root must not expose public transport passthroughs/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("pages UI boundary verifier rejects missing admin draft helper", () => {
  const root = withFixture({ omitDraftHelper: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected missing draft helper fixture to fail");
    assert.match(result.stderr, /build_create_page_draft/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("pages UI boundary verifier rejects server functions in transport facades", () => {
  const root = withFixture({ includeServerEndpoint: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected transport server-function fixture to fail");
    assert.match(result.stderr, /server\/native endpoints must not live in the transport facade/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
