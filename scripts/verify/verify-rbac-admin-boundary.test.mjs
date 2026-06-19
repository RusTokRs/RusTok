#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-rbac-admin-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-rbac-boundary-"));
  writeFixtureFile(root, "crates/rustok-rbac/admin/src/lib.rs", `
mod core;
${options.includeApiModule ? "mod api;" : "mod transport;"}
mod ui;

pub use ui::leptos::RbacAdmin;
`);
  writeFixtureFile(root, "crates/rustok-rbac/admin/src/core.rs", `
${options.includeLeptos ? "use leptos::prelude::*;" : ""}
pub(crate) struct RbacAdminOverviewViewModel;
pub(crate) fn build_rbac_admin_overview_view_model() -> RbacAdminOverviewViewModel { RbacAdminOverviewViewModel }
pub(crate) fn format_rbac_admin_bootstrap_error() -> String { String::new() }
#[cfg(test)]
mod tests {
    #[test]
    fn overview_view_model_formats_bootstrap_without_framework_runtime() {}
}
`);
  writeFixtureFile(root, "crates/rustok-rbac/admin/src/ui/leptos.rs", `
use crate::core::{build_rbac_admin_overview_view_model, format_rbac_admin_bootstrap_error};
use crate::transport;

pub fn RbacAdmin() {
    let _ = build_rbac_admin_overview_view_model;
    let _ = format_rbac_admin_bootstrap_error;
    let _ = transport::fetch_bootstrap;
    ${options.rawTransport ? "let _ = native_server_adapter::fetch_bootstrap_native;" : ""}
    ${options.rawApi ? "api::fetch_bootstrap().await;" : ""}
    ${options.serverInUi ? "#[server] async fn bad() {}" : ""}
}
`);
  writeFixtureFile(root, "crates/rustok-rbac/admin/src/transport/mod.rs", `
mod native_server_adapter;
pub enum RbacAdminTransportError { NativeServer(String) }
pub async fn fetch_bootstrap() {
    native_server_adapter::fetch_bootstrap_native().await;
}
${options.graphqlInTransport ? "fn graphql_fallback() {}" : ""}
${options.serverInFacade ? "#[server] async fn bad() {}" : ""}
`);
  writeFixtureFile(root, "crates/rustok-rbac/admin/src/transport/native_server_adapter.rs", `
use leptos::prelude::*;
use rustok_core::ModuleRegistry;
use rustok_api::infer_user_role_from_permissions;
#[server]
pub async fn fetch_bootstrap_native() -> Result<(), ServerFnError> { Ok(()) }
`);
  writeFixtureFile(root, "crates/rustok-rbac/docs/implementation-plan.md", "native-only\nverify-rbac-admin-boundary.mjs\n");
  writeFixtureFile(root, "docs/modules/registry.md", "verify-rbac-admin-boundary.mjs\n");
  if (options.legacyApiFile) {
    writeFixtureFile(root, "crates/rustok-rbac/admin/src/api.rs", "pub async fn fetch_bootstrap() {}\n");
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
    assert.notEqual(result.status, 0, "Expected RBAC boundary fixture to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("RBAC admin boundary verifier passes canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /RBAC admin boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("RBAC admin boundary verifier rejects legacy api facade", () => {
  expectFailure({ legacyApiFile: true, includeApiModule: true }, /pre-FFA api facade must stay removed|must not wire the pre-FFA api facade/);
});

test("RBAC admin boundary verifier rejects Leptos-specific core", () => {
  expectFailure({ includeLeptos: true }, /core must stay Leptos\/server-function free/);
});

test("RBAC admin boundary verifier rejects raw transport calls from UI", () => {
  expectFailure({ rawTransport: true }, /UI adapter must not call raw\/pre-FFA transport/);
});

test("RBAC admin boundary verifier rejects package-local GraphQL fallback", () => {
  expectFailure({ graphqlInTransport: true }, /must not invent a package-local GraphQL fallback/);
});

test("RBAC admin boundary verifier rejects server functions outside native adapter", () => {
  expectFailure({ serverInFacade: true }, /server-function endpoint belongs in native_server_adapter.rs/);
});
