#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-channel-admin-boundary.mjs");

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

pub use ui::leptos::ChannelAdmin;
`;
}

function coreSource({ includeLeptos = false } = {}) {
  return `
${includeLeptos ? "use leptos::prelude::*;" : ""}
pub(crate) fn channel_selection_exists(bootstrap: &ChannelAdminBootstrap, channel_id: &str) -> bool {
    bootstrap.channels.iter().any(|channel| channel.channel.id == channel_id)
}
struct ChannelAdminBootstrap { channels: Vec<ChannelDetail> }
struct ChannelDetail { channel: ChannelRecord }
struct ChannelRecord { id: String }
`;
}

function uiSource({ rawTransport = false, includeApiCall = false } = {}) {
  return `
use crate::core::channel_selection_exists;
use crate::transport;

pub fn ChannelAdmin() {
    let _policy = channel_selection_exists;
    let _facade = transport::fetch_bootstrap;
    ${rawTransport ? "let _native = native_server_adapter::channel_bootstrap_native;" : ""}
    ${includeApiCall ? "api::fetch_bootstrap().await;" : ""}
}
`;
}

function transportModSource({ includeServerEndpoint = false, includeRawRest = false } = {}) {
  return `
mod native_server_adapter;
mod rest_adapter;

pub async fn fetch_bootstrap() {
    native_server_adapter::channel_bootstrap_native().await;
    rest_adapter::get_json("/api/channels/bootstrap", None, None).await;
}
${includeServerEndpoint ? '#[server(prefix = "/api/fn", endpoint = "bad")] async fn bad() {}' : ""}
${includeRawRest ? "fn bad_rest() { reqwest::Client::new(); }" : ""}
`;
}

function nativeAdapterSource({ includeRest = false } = {}) {
  return `
use leptos::prelude::*;
#[server(prefix = "/api/fn", endpoint = "channel/bootstrap")]
pub(super) async fn channel_bootstrap_native() -> Result<(), ServerFnError> {
    ${includeRest ? "reqwest::Client::new();" : ""}
    Ok(())
}
`;
}

function restAdapterSource({ includeServerEndpoint = false } = {}) {
  return `
fn api_url(path: &str) -> String {
    let base = std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
    format!("{base}{path}")
}
pub(super) async fn get_json<T>(path: &str, token: Option<String>, tenant_slug: Option<String>) -> Result<T, ApiError> {
    let client = reqwest::Client::new();
    todo!()
}
${includeServerEndpoint ? '#[server(prefix = "/api/fn", endpoint = "bad")] async fn bad() {}' : ""}
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-channel-boundary-"));
  writeFixtureFile(root, "crates/rustok-channel/admin/src/lib.rs", libSource(options));
  writeFixtureFile(root, "crates/rustok-channel/admin/src/core.rs", coreSource(options));
  writeFixtureFile(root, "crates/rustok-channel/admin/src/ui/leptos.rs", uiSource(options));
  writeFixtureFile(root, "crates/rustok-channel/admin/src/transport/mod.rs", transportModSource(options));
  writeFixtureFile(root, "crates/rustok-channel/admin/src/transport/native_server_adapter.rs", nativeAdapterSource(options));
  writeFixtureFile(root, "crates/rustok-channel/admin/src/transport/rest_adapter.rs", restAdapterSource({ includeServerEndpoint: options.restServerEndpoint }));
  if (options.includeLegacyApiFile) {
    writeFixtureFile(root, "crates/rustok-channel/admin/src/api.rs", "pub async fn fetch_bootstrap() {}");
  }
  if (options.includeLegacyTransportFile) {
    writeFixtureFile(root, "crates/rustok-channel/admin/src/transport.rs", "pub async fn fetch_bootstrap() {}");
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

test("channel admin boundary verifier passes canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /channel admin boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects legacy api facade", () => {
  const root = withFixture({ includeLegacyApiFile: true, includeApiModule: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected legacy api fixture to fail");
    assert.match(result.stderr, /pre-FFA api facade must stay removed|must not wire the pre-FFA api facade/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects legacy flat transport file", () => {
  const root = withFixture({ includeLegacyTransportFile: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected legacy transport fixture to fail");
    assert.match(result.stderr, /transport must remain split into transport\/ adapters/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects raw adapter calls from UI", () => {
  const root = withFixture({ rawTransport: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected raw UI transport fixture to fail");
    assert.match(result.stderr, /UI adapter must not call raw\/pre-FFA transport/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects Leptos-specific core", () => {
  const root = withFixture({ includeLeptos: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected Leptos core fixture to fail");
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});


test("channel admin boundary verifier rejects server functions in transport facade", () => {
  const root = withFixture({ includeServerEndpoint: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected facade server-function fixture to fail");
    assert.match(result.stderr, /server-function endpoints belong in native_server_adapter\.rs/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects raw REST calls in transport facade", () => {
  const root = withFixture({ includeRawRest: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected facade raw REST fixture to fail");
    assert.match(result.stderr, /raw REST client belongs in rest_adapter\.rs/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects REST calls in native adapter", () => {
  const root = withFixture({ includeRest: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected native adapter REST fixture to fail");
    assert.match(result.stderr, /native adapter must not own REST fallback HTTP calls/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("channel admin boundary verifier rejects server functions in REST adapter", () => {
  const root = withFixture({ restServerEndpoint: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected REST adapter server-function fixture to fail");
    assert.match(result.stderr, /REST adapter must not contain server-function endpoints/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
