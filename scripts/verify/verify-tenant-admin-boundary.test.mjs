#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-tenant-admin-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-tenant-boundary-"));
  writeFixtureFile(root, "crates/rustok-tenant/admin/src/lib.rs", `
mod core;
mod i18n;
${options.includeApiModule ? "mod api;" : "mod transport;"}
mod ui;

pub use ui::leptos::TenantAdmin;
`);
  writeFixtureFile(root, "crates/rustok-tenant/admin/src/core.rs", `
${options.includeLeptos ? "use leptos::prelude::*;" : ""}
pub(crate) struct TenantAdminInfoCards;
pub(crate) fn load_bootstrap_error_message() -> String { String::new() }
`);
  writeFixtureFile(root, "crates/rustok-tenant/admin/src/ui/leptos.rs", `
use crate::{core, i18n::t, transport};
pub fn TenantAdmin() {
    let _ = core::load_bootstrap_error_message;
    let _ = t;
    let _ = transport::fetch_bootstrap;
    ${options.rawTransport ? "let _ = native_server_adapter::tenant_bootstrap_native;" : ""}
    ${options.apiCall ? "api::fetch_bootstrap().await;" : ""}
    ${options.serverInUi ? "#[server] async fn bad() {}" : ""}
}
`);
  writeFixtureFile(root, "crates/rustok-tenant/admin/src/transport/mod.rs", `
pub mod native_server_adapter;
pub async fn fetch_bootstrap() {
    native_server_adapter::tenant_bootstrap_native().await;
}
${options.serverInFacade ? "#[server] async fn bad() {}" : ""}
`);
  writeFixtureFile(root, "crates/rustok-tenant/admin/src/transport/native_server_adapter.rs", `
use leptos::prelude::*;
use rustok_api::HostRuntimeContext;
#[server]
pub async fn tenant_bootstrap_native() -> Result<(), ServerFnError> {
    let _runtime: Option<HostRuntimeContext> = None;
    Ok(())
}
`);
  writeFixtureFile(root, "crates/rustok-tenant/admin/Cargo.toml", `
[package]
name = "rustok-tenant-admin-fixture"
version = "0.1.0"
`);
  if (options.legacyApiFile) {
    writeFixtureFile(root, "crates/rustok-tenant/admin/src/api.rs", "pub async fn fetch_bootstrap() {}\n");
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
    assert.notEqual(result.status, 0, "Expected tenant boundary fixture to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("tenant admin boundary verifier passes canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /tenant admin boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("tenant admin boundary verifier rejects legacy api facade", () => {
  expectFailure({ legacyApiFile: true, includeApiModule: true }, /pre-FFA api facade must stay removed|must not wire the pre-FFA api facade/);
});

test("tenant admin boundary verifier rejects Leptos-specific core", () => {
  expectFailure({ includeLeptos: true }, /core must stay Leptos\/server-function free/);
});

test("tenant admin boundary verifier rejects raw transport calls from UI", () => {
  expectFailure({ rawTransport: true }, /UI adapter must not call raw\/pre-FFA transport/);
});

test("tenant admin boundary verifier rejects server functions outside native adapter", () => {
  expectFailure({ serverInFacade: true }, /server-function endpoint belongs in native_server_adapter.rs/);
});
