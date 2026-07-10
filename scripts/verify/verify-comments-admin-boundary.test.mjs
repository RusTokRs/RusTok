#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-comments-admin-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-comments-boundary-"));
  writeFixtureFile(root, "crates/rustok-comments/admin/src/lib.rs", `
mod core;
${options.includeApiModule ? "mod api;" : "mod transport;"}
mod ui;

pub use ui::leptos::CommentsAdmin;
`);
  writeFixtureFile(root, "crates/rustok-comments/admin/src/core.rs", `
${options.includeLeptos ? "use leptos::prelude::*;" : ""}
pub(crate) struct CommentThreadsRequest;
pub(crate) struct SetCommentStatusCommand;
pub(crate) const COMMENTS_ADMIN_THREAD_QUERY_KEY: &str = "thread_id";
pub(crate) const COMMENTS_ADMIN_LOCALE_QUERY_KEY: &str = "locale";
pub(crate) struct UiRouteQueryIntent;
pub(crate) fn comments_admin_select_thread_query_intent() -> UiRouteQueryIntent { UiRouteQueryIntent }
pub(crate) fn comments_admin_locale_query_intent() -> UiRouteQueryIntent { UiRouteQueryIntent }
`);
  writeFixtureFile(root, "crates/rustok-comments/admin/src/ui/leptos.rs", `
use crate::core::{comments_admin_locale_query_intent, comments_admin_select_thread_query_intent};
use crate::transport;

pub fn CommentsAdmin() {
    let _ = comments_admin_select_thread_query_intent;
    let _ = comments_admin_locale_query_intent;
    let _ = transport::fetch_threads;
    let _ = apply_query_intent;
    ${options.rawRoutePolicy ? 'let _ = AdminQueryKey::new("thread_id"); push_value("thread_id", "1");' : ""}
    ${options.rawTransport ? "let _ = native_server_adapter::fetch_threads;" : ""}
    ${options.rawApi ? "crate::api::fetch_threads().await;" : ""}
    ${options.serverInUi ? "#[server] async fn bad() {}" : ""}
}
fn apply_query_intent() {}
`);
  writeFixtureFile(root, "crates/rustok-comments/admin/src/transport/mod.rs", `
pub(crate) mod native_server_adapter;
pub(crate) enum CommentsAdminTransportPath { NativeServerFunction }
pub(crate) const ACTIVE_TRANSPORT_PATH: CommentsAdminTransportPath = CommentsAdminTransportPath::NativeServerFunction;
pub async fn fetch_threads() {
    native_server_adapter::fetch_threads().await;
}
${options.graphqlInTransport ? "fn graphql_fallback() {}" : ""}
${options.serverInFacade ? "#[server] async fn bad() {}" : ""}
`);
  writeFixtureFile(root, "crates/rustok-comments/admin/src/transport/native_server_adapter.rs", `
use leptos::prelude::*;
struct HostRuntimeContext;
struct CommentsService;
impl CommentsService { fn new() -> Self { Self } }
#[server]
pub async fn comments_threads_native() -> Result<(), ServerFnError> {
    let _runtime = HostRuntimeContext;
    let _ = CommentsService::new();
    Ok(())
}
#[server]
pub async fn comments_set_comment_status_native() -> Result<(), ServerFnError> {
    let _runtime = HostRuntimeContext;
    let _ = CommentsService::new();
    Ok(())
}
pub async fn fetch_threads() { let _ = comments_threads_native; }
`);
  writeFixtureFile(root, "crates/rustok-comments/admin/Cargo.toml", `
[features]
ssr = ["leptos/ssr", "rustok-api/server"]

[dependencies]
rustok-api = { workspace = true, default-features = false }
`);
  writeFixtureFile(root, "crates/rustok-comments/docs/implementation-plan.md", `
native-only comments admin exception
Loco-free native admin transport
UiRouteQueryIntent
verify-comments-admin-boundary.mjs
`);
  writeFixtureFile(root, "docs/modules/registry.md", "verify-comments-admin-boundary.mjs\n");
  if (options.legacyApiFile) {
    writeFixtureFile(root, "crates/rustok-comments/admin/src/api.rs", "pub async fn fetch_threads() {}\n");
  }
  return root;
}

function runVerifier(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function expectFailure(options, pattern) {
  const root = withFixture(options);
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected comments boundary fixture to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("comments admin boundary verifier passes canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /comments admin boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("comments admin boundary verifier rejects legacy api facade", () => {
  expectFailure({ legacyApiFile: true, includeApiModule: true }, /pre-FFA api facade must stay removed|must not wire the pre-FFA api facade/);
});

test("comments admin boundary verifier rejects Leptos-specific core", () => {
  expectFailure({ includeLeptos: true }, /core must stay Leptos\/server-function free/);
});

test("comments admin boundary verifier rejects UI-owned route policy", () => {
  expectFailure({ rawRoutePolicy: true }, /UI adapter must not own raw route\/transport policy/);
});

test("comments admin boundary verifier rejects raw transport calls from UI", () => {
  expectFailure({ rawTransport: true }, /UI adapter must not own raw route\/transport policy/);
});

test("comments admin boundary verifier rejects package-local GraphQL fallback", () => {
  expectFailure({ graphqlInTransport: true }, /must not invent a package-local GraphQL fallback/);
});

test("comments admin boundary verifier rejects server functions outside native adapter", () => {
  expectFailure({ serverInFacade: true }, /server-function endpoints belong in native_server_adapter.rs/);
});
