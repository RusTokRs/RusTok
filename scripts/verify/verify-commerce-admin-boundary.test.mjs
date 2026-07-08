#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-commerce-admin-boundary.mjs");

function put(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-commerce-admin-boundary-"));
  put(root, "crates/rustok-commerce/admin/src/lib.rs", `${options.legacyModApi ? "mod api;\n" : ""}mod core;\nmod transport;\nmod ui;\npub use ui::CommerceAdmin;\n`);
  put(root, "crates/rustok-commerce/admin/src/core/mod.rs", `${options.leptosCore ? "use leptos::prelude::*;" : ""}\npub fn build_shipping_profile_form_snapshot() {}\n`);
  put(root, "crates/rustok-commerce/admin/src/ui/leptos.rs", `use crate::transport;\npub fn render() { let _ = transport::fetch_bootstrap; ${options.rawUi ? "let _ = api::fetch_bootstrap;" : ""} }\n`);
  put(root, "crates/rustok-commerce/admin/src/transport/mod.rs", "mod order_change;\nmod promotion;\nmod native_server_adapter;\nmod shipping_profile;\npub use shipping_profile::fetch_bootstrap;\n");
  put(root, "crates/rustok-commerce/admin/src/transport/shipping_profile.rs", "use super::native_server_adapter::{self, ApiError};\npub async fn fetch_bootstrap() -> Result<(), ApiError> { native_server_adapter::fetch_bootstrap().await }\n");
  put(root, "crates/rustok-commerce/admin/src/transport/promotion.rs", "use super::native_server_adapter::{self, ApiError};\npub async fn preview_cart_promotion() -> Result<(), ApiError> { native_server_adapter::fetch_bootstrap().await }\n");
  put(root, "crates/rustok-commerce/admin/src/transport/order_change.rs", "use super::native_server_adapter::{self, ApiError};\npub async fn fetch_order_changes() -> Result<(), ApiError> { native_server_adapter::fetch_bootstrap().await }\n");
  put(root, "crates/rustok-commerce/admin/src/transport/native_server_adapter.rs", "use \npub enum ApiError { ServerFn(String) }\n#[server]\npub async fn fetch_bootstrap() -> Result<(), ApiError> { Ok(()) }\n");
  put(root, "crates/rustok-commerce/src/lib.rs", "pub mod graphql;\npub mod state_machine;\n");
  if (options.legacyApi) put(root, "crates/rustok-commerce/admin/src/api.rs", "pub async fn fetch_bootstrap() {}\n");
  put(root, "crates/rustok-commerce/docs/implementation-plan.md", "verify-commerce-admin-boundary.mjs admin/src/transport/native_server_adapter.rs root GraphQL and state-machine aliases");
  put(root, "docs/modules/registry.md", "verify-commerce-admin-boundary.mjs root GraphQL/state-machine aliases");
  put(root, "package.json", JSON.stringify({
    scripts: {
      "verify:commerce:admin-boundary": "node scripts/verify/verify-commerce-admin-boundary.mjs",
      "test:verify:commerce:admin-boundary": "node scripts/verify/verify-commerce-admin-boundary.test.mjs",
      "verify:ffa:ui:migration": "npm run verify:commerce:admin-boundary",
      "test:verify:ffa:ui:migration": "npm run test:verify:commerce:admin-boundary",
    },
  }));
  return root;
}

function run(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function expectFailure(options, pattern) {
  const root = fixture(options);
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("commerce admin boundary verifier passes canonical fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /commerce admin boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("commerce admin boundary verifier rejects legacy api file", () => {
  expectFailure({ legacyApi: true }, /legacy api\.rs/);
});

test("commerce admin boundary verifier rejects legacy api module", () => {
  expectFailure({ legacyModApi: true }, /must not wire legacy api module/);
});

test("commerce admin boundary verifier rejects Leptos-specific core", () => {
  expectFailure({ leptosCore: true }, /core must stay Leptos\/server-function free/);
});

test("commerce admin boundary verifier rejects raw api calls from UI", () => {
  expectFailure({ rawUi: true }, /UI adapter must not call raw transport/);
});
