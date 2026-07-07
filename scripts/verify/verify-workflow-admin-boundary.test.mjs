#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-workflow-admin-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function libSource({ includeApiModule = false } = {}) {
  return `
mod core;
mod i18n;
mod model;
${includeApiModule ? "mod api;" : "mod transport;"}
mod ui;

pub use ui::leptos::WorkflowAdmin;
`;
}

function coreModSource({ includeLeptos = false } = {}) {
  return `
${includeLeptos ? "use leptos::prelude::*;" : ""}
pub struct WorkflowAdminTransportContext;
pub struct WorkflowTemplateCreateCommand;
pub fn workflow_admin_nav_view_model() {}
pub fn workflow_error_view_model() {}
pub fn workflow_template_create_command() {}
`;
}

function coreLeafSource() {
  return "pub fn marker() {}\n";
}

function uiSource({ rawAdapterCall = false } = {}) {
  return `
use crate::core::{workflow_admin_nav_view_model, workflow_admin_transport_context};
use crate::transport;

pub fn WorkflowAdmin() {
    let _nav = workflow_admin_nav_view_model;
    let _ctx = workflow_admin_transport_context;
    let _transport = transport::fetch_workflows;
    ${rawAdapterCall ? "let _raw = graphql_adapter::fetch_workflows;" : ""}
}
`;
}

function transportModSource({ includeServerEndpoint = false, includeRawGraphql = false } = {}) {
  return `
mod graphql_adapter;
mod native_server_adapter;

use crate::core::{WorkflowAdminTransportContext, WorkflowTemplateCreateCommand};

pub async fn fetch_workflows(context: WorkflowAdminTransportContext) {
    native_server_adapter::fetch_workflows_native().await;
    graphql_adapter::fetch_workflows().await;
}

pub async fn create_from_template(context: WorkflowAdminTransportContext, command: WorkflowTemplateCreateCommand) {}
${includeServerEndpoint ? '#[server(prefix = "/api/fn", endpoint = "bad")] async fn bad() {}' : ""}
${includeRawGraphql ? "fn bad_graphql() { execute_graphql(); }" : ""}
`;
}

function nativeAdapterSource() {
  return `
struct HostRuntimeContext;
#[server(prefix = "/api/fn", endpoint = "workflow/list")]
pub(super) async fn fetch_workflows_native() { let _runtime = HostRuntimeContext; }
#[server(prefix = "/api/fn", endpoint = "workflow/templates")]
pub(super) async fn fetch_templates_native() {}
#[server(prefix = "/api/fn", endpoint = "workflow/create-from-template")]
pub(super) async fn create_from_template_native() { let _runtime = HostRuntimeContext; }
`;
}

function cargoSource() {
  return `
[features]
ssr = ["leptos/ssr", "rustok-api/server"]

[dependencies]
rustok-api = { workspace = true, default-features = false }
`;
}

function graphqlAdapterSource({ includeServerEndpoint = false } = {}) {
  return `
use rustok_graphql::execute as execute_graphql;
pub async fn fetch_workflows() {}
pub async fn fetch_templates() {}
pub async fn create_from_template() {}
${includeServerEndpoint ? '#[server(prefix = "/api/fn", endpoint = "bad")] async fn bad() {}' : ""}
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-workflow-boundary-"));
  writeFixtureFile(root, "crates/rustok-workflow/admin/src/lib.rs", libSource(options));
  writeFixtureFile(root, "crates/rustok-workflow/admin/src/core/mod.rs", coreModSource(options));
  for (const leaf of ["presentation", "navigation", "transport_context", "error", "command"]) {
    writeFixtureFile(root, `crates/rustok-workflow/admin/src/core/${leaf}.rs`, coreLeafSource());
  }
  writeFixtureFile(root, "crates/rustok-workflow/admin/src/ui/leptos.rs", uiSource(options));
  writeFixtureFile(root, "crates/rustok-workflow/admin/src/transport/mod.rs", transportModSource(options));
  writeFixtureFile(root, "crates/rustok-workflow/admin/src/transport/native_server_adapter.rs", nativeAdapterSource());
  writeFixtureFile(root, "crates/rustok-workflow/admin/src/transport/graphql_adapter.rs", graphqlAdapterSource(options));
  writeFixtureFile(root, "crates/rustok-workflow/admin/Cargo.toml", cargoSource());
  if (options.includeLegacyApiFile) {
    writeFixtureFile(root, "crates/rustok-workflow/admin/src/api.rs", "pub async fn fetch_workflows() {}");
  }
  if (options.includeLegacyTransportFile) {
    writeFixtureFile(root, "crates/rustok-workflow/admin/src/transport.rs", "pub async fn fetch_workflows() {}");
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

test("workflow admin boundary verifier passes canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /workflow admin boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("workflow admin boundary verifier rejects legacy api facade", () => {
  const root = withFixture({ includeLegacyApiFile: true, includeApiModule: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected legacy api fixture to fail");
    assert.match(result.stderr, /pre-FFA api facade must stay absent|must not wire a pre-FFA api facade/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("workflow admin boundary verifier rejects raw adapter calls from UI", () => {
  const root = withFixture({ rawAdapterCall: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected raw UI adapter fixture to fail");
    assert.match(result.stderr, /UI adapter must not call raw\/pre-FFA transport/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("workflow admin boundary verifier rejects Leptos-specific core", () => {
  const root = withFixture({ includeLeptos: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected Leptos core fixture to fail");
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("workflow admin boundary verifier rejects server functions in transport facade", () => {
  const root = withFixture({ includeServerEndpoint: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected facade server-function fixture to fail");
    assert.match(result.stderr, /server-function endpoints belong in native_server_adapter\.rs/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
