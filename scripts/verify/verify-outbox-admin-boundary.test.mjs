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

function createFixture(overrides = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "outbox-admin-boundary-"));
  const files = {
    "crates/rustok-outbox/admin/src/lib.rs": `
mod core;
mod i18n;
mod transport;
pub mod ui;
pub use ui::leptos::OutboxAdmin;
`,
    "crates/rustok-outbox/admin/src/core.rs": `
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxAdminBootstrap { pub tenant_slug: Option<String>, pub health: String, pub counters: Vec<OutboxCounterSnapshot>, pub relay_notes: Vec<String> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxCounterSnapshot { pub key: String, pub label: String, pub value: u64 }
pub struct OutboxAdminShellText { pub health_label: String, pub tenant_context_label: String, pub global_tenant_label: String }
pub fn outbox_info_cards(bootstrap: &OutboxAdminBootstrap, text: &OutboxAdminShellText) -> Vec<String> {
  vec![bootstrap.tenant_slug.clone().unwrap_or_else(|| text.global_tenant_label.clone())]
}
`,
    "crates/rustok-outbox/admin/src/transport/mod.rs": `
mod native_server_adapter;
use crate::core::OutboxAdminBootstrap;
pub enum OutboxTransportError { ServerFn(String) }
pub async fn fetch_bootstrap() -> Result<OutboxAdminBootstrap, OutboxTransportError> {
  native_server_adapter::fetch_bootstrap_native().await.map_err(|error| OutboxTransportError::ServerFn(error.to_string()))
}
`,
    "crates/rustok-outbox/admin/src/transport/native_server_adapter.rs": `
use leptos::prelude::*;
use crate::core::OutboxAdminBootstrap;
pub async fn fetch_bootstrap_native() -> Result<OutboxAdminBootstrap, ServerFnError> { outbox_bootstrap_native().await }
#[server(prefix = "/api/fn", endpoint = "outbox/bootstrap")]
async fn outbox_bootstrap_native() -> Result<OutboxAdminBootstrap, ServerFnError> { todo!() }
`,
    "crates/rustok-outbox/admin/src/ui/leptos.rs": `
use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_api::UiRouteContext;
use crate::core::{outbox_info_cards, OutboxAdminShellText};
use crate::transport;
#[component]
pub fn OutboxAdmin() -> impl IntoView {
  let _token = use_token();
  let _tenant = use_tenant();
  let _locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
  view! { <div>{move || async move { let bootstrap = transport::fetch_bootstrap().await.unwrap(); outbox_info_cards(&bootstrap, &text); }}</div> }
}
`,
    "crates/rustok-outbox/docs/implementation-plan.md": "npm run verify:outbox:admin-boundary\n",
    "docs/modules/registry.md": "| `outbox` | npm run verify:outbox:admin-boundary |\n",
  };

  for (const [relativePath, content] of Object.entries({ ...files, ...overrides })) {
    writeFixtureFile(root, relativePath, content);
  }
  return root;
}

function runVerifier(root) {
  return spawnSync(process.execPath, [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

test("accepts the module-owned outbox admin core/transport/ui boundary", () => {
  const root = createFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Outbox admin core\/transport\/ui boundary is consistent/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects UI calls that bypass the transport facade", () => {
  const root = createFixture({
    "crates/rustok-outbox/admin/src/ui/leptos.rs": `
use leptos::prelude::*;
use crate::transport::native_server_adapter;
#[component]
pub fn OutboxAdmin() -> impl IntoView {
  view! { <div>{move || async move { native_server_adapter::fetch_bootstrap_native().await; }}</div> }
}
`,
  });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /UI adapter must not reach through the module-owned transport boundary/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects Leptos/server-function leakage into the core layer", () => {
  const root = createFixture({
    "crates/rustok-outbox/admin/src/core.rs": `
use leptos::prelude::*;
pub struct OutboxAdminBootstrap;
pub fn outbox_info_cards() { let _ = use_context::<String>(); }
`,
  });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /core DTO\/view-model layer must remain Leptos\/server-function free/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
