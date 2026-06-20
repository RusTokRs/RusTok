#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-outbox-admin-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-outbox-boundary-"));
  writeFixtureFile(root, "crates/rustok-outbox/admin/src/lib.rs", `
mod core;
${options.includeApiModule ? "mod api;" : "mod transport;"}
mod ui;
pub use ui::leptos::OutboxAdmin;
`);
  writeFixtureFile(root, "crates/rustok-outbox/admin/src/core.rs", `
${options.includeLeptos ? "use leptos::prelude::*;" : ""}
pub struct OutboxAdminBootstrap;
pub struct OutboxAdminShellText;
pub struct OutboxInfoCardViewModel;
pub fn outbox_info_cards(_: &OutboxAdminBootstrap, _: &OutboxAdminShellText) -> Vec<OutboxInfoCardViewModel> { vec![] }
`);
  writeFixtureFile(root, "crates/rustok-outbox/admin/src/ui/leptos.rs", `
use crate::core::{outbox_info_cards, OutboxAdminShellText};
use crate::transport;
pub fn OutboxAdmin() {
  let _ = transport::fetch_bootstrap;
  let _ = outbox_info_cards;
  ${options.rawNative ? "let _ = native_server_adapter::fetch_bootstrap_native;" : ""}
  ${options.rawApi ? "api::fetch_bootstrap().await;" : ""}
  ${options.serverInUi ? "#[server] async fn bad() {}" : ""}
}
`);
  writeFixtureFile(root, "crates/rustok-outbox/admin/src/transport/mod.rs", `
mod native_server_adapter;
pub enum OutboxTransportError { ServerFn(String) }
pub async fn fetch_bootstrap() { native_server_adapter::fetch_bootstrap_native().await; }
${options.graphqlInTransport ? "fn graphql_fallback() {}" : ""}
${options.serverInFacade ? "#[server] async fn bad() {}" : ""}
`);
  writeFixtureFile(root, "crates/rustok-outbox/admin/src/transport/native_server_adapter.rs", `
use leptos::prelude::*;
#[server]
pub async fn outbox_bootstrap_native() {}
pub async fn fetch_bootstrap_native() { outbox_bootstrap_native().await; }
fn module() { let _ = OutboxModule; let _ = relay_notes; }
`);
  writeFixtureFile(root, "crates/rustok-outbox/docs/implementation-plan.md", "verify-outbox-admin-boundary.mjs\n");
  writeFixtureFile(root, "crates/rustok-outbox/docs/README.md", "verify-outbox-admin-boundary.mjs\n");
  writeFixtureFile(root, "docs/modules/registry.md", "verify-outbox-admin-boundary.mjs\n");
  if (options.legacyApiFile) writeFixtureFile(root, "crates/rustok-outbox/admin/src/api.rs", "pub async fn fetch_bootstrap() {}\n");
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
    assert.notEqual(result.status, 0, "Expected outbox boundary fixture to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("outbox admin boundary verifier passes canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /outbox admin boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("outbox admin boundary verifier rejects legacy api facade", () => {
  expectFailure({ legacyApiFile: true, includeApiModule: true }, /pre-FFA api facade must stay removed|must not wire the pre-FFA api facade/);
});

test("outbox admin boundary verifier rejects Leptos-specific core", () => {
  expectFailure({ includeLeptos: true }, /core must stay Leptos\/server-function free/);
});

test("outbox admin boundary verifier rejects raw transport calls from UI", () => {
  expectFailure({ rawNative: true }, /UI adapter must not call raw\/pre-FFA transport/);
});

test("outbox admin boundary verifier rejects package-local GraphQL fallback", () => {
  expectFailure({ graphqlInTransport: true }, /must not invent a package-local GraphQL fallback/);
});

test("outbox admin boundary verifier rejects server functions outside native adapter", () => {
  expectFailure({ serverInFacade: true }, /server-function endpoint belongs in native_server_adapter.rs/);
});
