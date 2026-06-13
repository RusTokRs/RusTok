#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-fulfillment-admin-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function libSource({ publicTransportPassthrough = false } = {}) {
  return `
mod api;
mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::FulfillmentAdmin;
${publicTransportPassthrough ? "pub async fn fetch_bootstrap() {}" : ""}
`;
}

function coreSource({ includeLeptos = false, omitRequestHelper = false } = {}) {
  return `
${includeLeptos ? "use leptos::prelude::*;" : ""}
pub struct ShippingOptionListRequest;
pub struct ShippingProfileListRequest;
${omitRequestHelper ? "" : "pub fn shipping_option_list_request() {}"}
pub fn shipping_profile_list_request() {}
pub fn text_or_none() {}
`;
}

function uiSource({ rawApiCall = false, rawServiceCall = false, omitRequestHelper = false } = {}) {
  return `
use crate::core::{${omitRequestHelper ? "shipping_profile_list_request" : "shipping_option_list_request, shipping_profile_list_request"}};
use crate::transport;

pub fn FulfillmentAdmin() {
    let _bootstrap = transport::fetch_bootstrap;
    let _options = transport::fetch_shipping_options;
    let _option = transport::fetch_shipping_option;
    let _profiles = transport::fetch_shipping_profiles;
    let _create = transport::create_shipping_option;
    let _update = transport::update_shipping_option;
    let _deactivate = transport::deactivate_shipping_option;
    let _reactivate = transport::reactivate_shipping_option;
    ${omitRequestHelper ? "" : "let _request = shipping_option_list_request;"}
    let _profiles_request = shipping_profile_list_request;
    ${rawApiCall ? "let _raw = api::fetch_bootstrap;" : ""}
    ${rawServiceCall ? "let _service = FulfillmentService::new;" : ""}
}
`;
}

function transportSource({ includeServerEndpoint = false } = {}) {
  return `
use crate::api;

pub async fn fetch_bootstrap() { api::fetch_bootstrap().await; }
pub async fn fetch_shipping_options() { api::fetch_shipping_options().await; }
pub async fn fetch_shipping_option() { api::fetch_shipping_option().await; }
pub async fn fetch_shipping_profiles() { api::fetch_shipping_profiles().await; }
pub async fn create_shipping_option() { api::create_shipping_option().await; }
pub async fn update_shipping_option() { api::update_shipping_option().await; }
pub async fn deactivate_shipping_option() { api::deactivate_shipping_option().await; }
pub async fn reactivate_shipping_option() { api::reactivate_shipping_option().await; }
${includeServerEndpoint ? '#[server(prefix = "/api/fn", endpoint = "bad")] async fn bad() {}' : ""}
`;
}

function apiSource() {
  return `
use leptos_graphql::GraphqlRequest;
pub async fn fetch_bootstrap() {}
pub async fn fetch_shipping_options() {}
pub async fn fetch_shipping_option() {}
pub async fn fetch_shipping_profiles() {}
pub async fn create_shipping_option() {}
pub async fn update_shipping_option() {}
pub async fn deactivate_shipping_option() {}
pub async fn reactivate_shipping_option() {}
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-fulfillment-boundary-"));
  writeFixtureFile(root, "crates/rustok-fulfillment/admin/src/lib.rs", libSource(options));
  writeFixtureFile(root, "crates/rustok-fulfillment/admin/src/core.rs", coreSource(options));
  writeFixtureFile(root, "crates/rustok-fulfillment/admin/src/ui/leptos.rs", uiSource(options));
  writeFixtureFile(root, "crates/rustok-fulfillment/admin/src/transport.rs", transportSource(options));
  writeFixtureFile(root, "crates/rustok-fulfillment/admin/src/api.rs", apiSource());
  writeFixtureFile(root, "crates/rustok-fulfillment/docs/implementation-plan.md", "verify-fulfillment-admin-boundary.mjs");
  writeFixtureFile(root, "docs/modules/registry.md", "verify-fulfillment-admin-boundary.mjs");
  return root;
}

function runVerifier(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

test("fulfillment admin boundary verifier passes canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /fulfillment admin boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("fulfillment admin boundary verifier rejects Leptos-specific core", () => {
  const root = withFixture({ includeLeptos: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected Leptos core fixture to fail");
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("fulfillment admin boundary verifier rejects raw api calls from UI", () => {
  const root = withFixture({ rawApiCall: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected raw UI api fixture to fail");
    assert.match(result.stderr, /UI adapter must not call raw transport or services/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("fulfillment admin boundary verifier rejects public crate-root transport passthroughs", () => {
  const root = withFixture({ publicTransportPassthrough: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected public transport passthrough fixture to fail");
    assert.match(result.stderr, /crate root must not expose public transport passthroughs/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("fulfillment admin boundary verifier rejects missing list request helper", () => {
  const root = withFixture({ omitRequestHelper: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected missing request helper fixture to fail");
    assert.match(result.stderr, /shipping_option_list_request/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("fulfillment admin boundary verifier rejects server functions in transport facade", () => {
  const root = withFixture({ includeServerEndpoint: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected transport server-function fixture to fail");
    assert.match(result.stderr, /server\/native endpoints must not live in the fulfillment admin transport facade/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
