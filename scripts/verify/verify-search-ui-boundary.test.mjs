#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-search-ui-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function adminCore({ leptos = false, omitPreviewRequest = false } = {}) {
  return `
${leptos ? "use leptos::prelude::*;" : ""}
${omitPreviewRequest ? "" : "pub fn build_search_preview_request() {}"}
pub fn build_search_preview_view_model() {}
pub fn build_search_analytics_summary_view_model() {}
pub fn build_lagging_search_document_row_view_models() {}
pub fn build_search_consistency_issue_row_view_models() {}
pub fn build_search_synonym_mutation_request() {}
pub fn build_search_pin_rule_mutation_request() {}
`;
}

function storefrontCore({ omitRouteIntent = false } = {}) {
  return `
pub fn build_search_results_view_model() {}
pub fn build_search_suggestion_view_models() {}
pub fn build_search_preset_chip_view_models() {}
pub fn build_search_facet_view_models() {}
pub fn build_search_result_action_view_model() {}
pub fn build_storefront_search_fetch_request() {}
${omitRouteIntent ? "" : "pub fn build_storefront_search_route_intent() {}"}
pub fn build_storefront_suggestion_fetch_request() {}
`;
}

function adminUi({ rawApi = false } = {}) {
  return `
use crate::{core, transport};
pub fn render() {
  let _ = core::build_search_preview_request;
  let _ = transport::fetch_search_preview;
  let _ = transport::fetch_dictionary_snapshot;
  ${rawApi ? "let _ = api::fetch_search_preview;" : ""}
}
`;
}

function storefrontUi({ rawAdapter = false } = {}) {
  return `
use crate::{core, transport};
pub fn render() {
  let _ = core::build_storefront_search_fetch_request;
  let _ = core::build_storefront_suggestion_fetch_request;
  let _ = transport::fetch_search;
  let _ = transport::track_search_click;
  ${rawAdapter ? "let _ = native_server_adapter::fetch_search;" : ""}
}
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-search-ui-boundary-"));
  writeFixtureFile(root, "crates/rustok-search/admin/src/lib.rs", "mod api;\nmod core;\nmod transport;\nmod ui;\npub use ui::leptos::SearchAdmin;\n");
  writeFixtureFile(root, "crates/rustok-search/admin/src/core.rs", adminCore(options));
  writeFixtureFile(root, "crates/rustok-search/admin/src/ui/leptos.rs", adminUi(options));
  writeFixtureFile(root, "crates/rustok-search/admin/src/transport/mod.rs", "use crate::api;\npub type TransportError = api::ApiError;\npub async fn fetch_bootstrap() {}\npub async fn fetch_search_preview() {}\npub async fn fetch_search_analytics() {}\npub async fn fetch_dictionary_snapshot() {}\npub async fn update_search_settings() {}\n");
  writeFixtureFile(root, "crates/rustok-search/admin/src/api.rs", "use leptos_graphql as graphql;\n#[server]\npub async fn endpoint() {}\n");

  writeFixtureFile(root, "crates/rustok-search/storefront/src/lib.rs", "mod core;\nmod transport;\nmod ui;\npub use ui::leptos::SearchView;\n");
  writeFixtureFile(root, "crates/rustok-search/storefront/src/core.rs", storefrontCore(options));
  writeFixtureFile(root, "crates/rustok-search/storefront/src/ui/leptos.rs", storefrontUi(options));
  writeFixtureFile(root, "crates/rustok-search/storefront/src/transport/mod.rs", "pub mod graphql_adapter;\npub mod native_server_adapter;\npub async fn fetch_search() { let _ = native_server_adapter::fetch_search; let _ = graphql_adapter::fetch_search; }\npub async fn fetch_suggestions() { let _ = native_server_adapter::fetch_suggestions; let _ = graphql_adapter::fetch_suggestions; }\n");
  writeFixtureFile(root, "crates/rustok-search/storefront/src/transport/native_server_adapter.rs", "pub fn fetch_storefront_search_server() {}\npub fn fetch_storefront_suggestions_server() {}\npub fn fetch_search() {}\npub fn fetch_suggestions() {}\n");
  writeFixtureFile(root, "crates/rustok-search/storefront/src/transport/graphql_adapter.rs", "pub fn fetch_storefront_search_graphql() {}\npub fn fetch_storefront_suggestions_graphql() {}\npub fn fetch_search() {}\npub fn fetch_suggestions() {}\n");
  return root;
}

function runVerifier(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

test("search UI boundary verifier passes canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /search UI boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects Leptos-specific admin core", () => {
  const root = withFixture({ leptos: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects missing admin preview request policy", () => {
  const root = withFixture({ omitPreviewRequest: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /build_search_preview_request/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects raw admin api calls from UI", () => {
  const root = withFixture({ rawApi: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /UI adapter must not call raw transport/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects missing storefront route intent policy", () => {
  const root = withFixture({ omitRouteIntent: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /build_storefront_search_route_intent/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects raw storefront adapter calls from UI", () => {
  const root = withFixture({ rawAdapter: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /UI adapter must not call raw transport/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
