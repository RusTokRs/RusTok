#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-region-admin-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function libSource({ omitTransport = false } = {}) {
  return `
mod core;
mod i18n;
mod model;
${omitTransport ? "" : "mod transport;"}
mod ui;

pub use ui::RegionAdmin;
`;
}

function coreSource({ includeLeptos = false, omitRouteWriter = false } = {}) {
  return `
${includeLeptos ? "use leptos::prelude::*;" : ""}
pub struct RegionAdminSubmitInput;
pub struct RegionAdminSubmitCommand;
pub enum RegionAdminSubmitError { HostLocaleUnavailable }
pub struct RegionAdminSubmitErrorLabels;
pub struct RegionAdminTransportErrorLabels;
pub fn prepare_region_admin_submit() {}
pub fn region_admin_submit_error_message() {}
pub fn region_admin_save_region_error_message() {}
pub enum RegionAdminRouteQueryUpdate { ClearSelected }
${omitRouteWriter ? "" : "pub fn region_admin_open_query_update() {}\npub fn region_admin_saved_query_update() {}\npub fn region_admin_new_query_update() {}"}
pub enum RegionAdminDetailPanelViewModel { Empty }
pub struct RegionAdminOpenDetailViewModel;
pub struct RegionAdminSaveSuccessViewModel;
pub fn region_admin_save_success() {}
pub fn region_admin_open_detail_success() {}
pub fn region_admin_open_detail_error() {}
`;
}

function uiSource({ rawApi = false, rawService = false } = {}) {
  return `
use crate::core::{prepare_region_admin_submit, RegionAdminSubmitError};

pub fn RegionAdmin() {
    let _submit = prepare_region_admin_submit;
    let _err = RegionAdminSubmitError::HostLocaleUnavailable;
    let _create = crate::transport::create_region;
    let _update = crate::transport::update_region;
    ${rawApi ? "let _raw = crate::api::fetch_regions;" : ""}
    ${rawService ? "let _service = RegionService;" : ""}
}
`;
}

function transportSource({ omitApiDelegation = false } = {}) {
  return `
${omitApiDelegation ? "" : "mod native_server_adapter;"}
pub async fn fetch_bootstrap() {}
pub async fn fetch_regions() { native_server_adapter::fetch_regions().await; }
pub async fn fetch_region_detail(region_id: String) {}
pub async fn create_region(payload: String) {}
pub async fn update_region(region_id: String, payload: String) {}
`;
}

function apiSource({ omitServer = false, omitService = false } = {}) {
  return `
${omitServer ? "" : '#[server(prefix = "/api/fn", endpoint = "region/list")]'}
async fn region_list_native() {}
${omitService ? "" : "fn load() { let _service = RegionService; }"}
fn runtime() { let _runtime = HostRuntimeContext; }
`;
}

function cargoSource({ includeLoco = false } = {}) {
  return `
[features]
ssr = ["leptos/ssr", "rustok-api/server"]

[dependencies]
${includeLoco ? 'loco-rs = { workspace = true, optional = true }' : ""}
rustok-api = { workspace = true, default-features = false }
`;
}

function implementationPlanSource({ omitGuardrail = false } = {}) {
  return `
# План реализации rustok-region
- FFA slice #31 добавила admin submit command preparation.
- FFA slice #36 добавила route/query writer operation.
- Loco-free native admin transport now consumes HostRuntimeContext.
${omitGuardrail ? "" : "- Fast guardrail: scripts/verify/verify-region-admin-boundary.mjs."}
`;
}

function registrySource({ staleSlice = false, omitGuardrail = false } = {}) {
  return `
| Module slug | UI surfaces | FFA status | FBA status | Structural shape | Source plan |
| \`region\` | admin + storefront | \`in_progress\` | \`not_started\` | \`core_transport_ui\` | ${staleSlice ? "" : "region-fba-registry.json"}; ${omitGuardrail ? "" : "scripts/verify/verify-region-admin-boundary.mjs"} |
`;
}

function packageJsonSource({ omitPackageScript = false, omitAggregateRegionTest = false } = {}) {
  const scripts = omitPackageScript
    ? {}
    : {
        "test:verify:region:admin-boundary":
          "node scripts/verify/verify-region-admin-boundary.test.mjs",
        "test:verify:ffa:ui:migration": omitAggregateRegionTest
          ? "node scripts/verify/verify-ffa-ui-migration-contract.test.mjs"
          : "node scripts/verify/verify-ffa-ui-migration-contract.test.mjs && npm run test:verify:region:admin-boundary",
      };
  return JSON.stringify({ scripts }, null, 2);
}

function verifierTestSource() {
  return `
test("region admin boundary verifier passes canonical fixture", () => {});
test("region admin boundary verifier rejects stale central readiness board", () => {});
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-region-boundary-"));
  writeFixtureFile(root, "crates/rustok-region/admin/src/lib.rs", libSource(options));
  writeFixtureFile(root, "crates/rustok-region/admin/src/core.rs", coreSource(options));
  writeFixtureFile(root, "crates/rustok-region/admin/src/ui/leptos.rs", uiSource(options));
  writeFixtureFile(root, "crates/rustok-region/admin/src/transport/mod.rs", transportSource(options));
  writeFixtureFile(root, "crates/rustok-region/admin/src/transport/native_server_adapter.rs", apiSource(options));
  writeFixtureFile(root, "crates/rustok-region/admin/Cargo.toml", cargoSource(options));
  if (options.legacyApi) writeFixtureFile(root, "crates/rustok-region/admin/src/api.rs", apiSource(options));
  writeFixtureFile(root, "crates/rustok-region/docs/implementation-plan.md", implementationPlanSource(options));
  writeFixtureFile(root, "docs/modules/registry.md", registrySource(options));
  writeFixtureFile(root, "package.json", packageJsonSource(options));
  writeFixtureFile(root, "scripts/verify/verify-region-admin-boundary.test.mjs", verifierTestSource());
  return root;
}

function runVerifier(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function withTempFixture(options, assertion) {
  const root = withFixture(options);
  try {
    assertion(runVerifier(root));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("region admin boundary verifier passes canonical fixture", () => {
  withTempFixture({}, (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /region admin boundary verification passed/);
  });
});

test("region admin boundary verifier rejects Leptos-specific core", () => {
  withTempFixture({ includeLeptos: true }, (result) => {
    assert.notEqual(result.status, 0, "Expected Leptos core fixture to fail");
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  });
});

test("region admin boundary verifier rejects raw api calls from UI", () => {
  withTempFixture({ rawApi: true }, (result) => {
    assert.notEqual(result.status, 0, "Expected raw api fixture to fail");
    assert.match(result.stderr, /UI adapter must not call raw\/native transport or service/);
  });
});

test("region admin boundary verifier rejects legacy admin api module", () => {
  withTempFixture({ legacyApi: true }, (result) => {
    assert.notEqual(result.status, 0, "Expected legacy api fixture to fail");
    assert.match(result.stderr, /legacy api\.rs/);
  });
});

test("region admin boundary verifier rejects missing route writer core helper", () => {
  withTempFixture({ omitRouteWriter: true }, (result) => {
    assert.notEqual(result.status, 0, "Expected missing route writer fixture to fail");
    assert.match(result.stderr, /expected core-owned FFA helper region_admin_open_query_update/);
  });
});

test("region admin boundary verifier rejects stale central readiness board", () => {
  withTempFixture({ staleSlice: true }, (result) => {
    assert.notEqual(result.status, 0, "Expected stale registry fixture to fail");
    assert.match(result.stderr, /central readiness board must record region provider evidence/);
  });
});

test("region admin boundary verifier rejects missing package fixture script", () => {
  withTempFixture({ omitPackageScript: true }, (result) => {
    assert.notEqual(result.status, 0, "Expected missing package script fixture to fail");
    assert.match(result.stderr, /package scripts must expose region boundary fixture tests/);
  });
});


test("region admin boundary verifier rejects aggregate test script without region fixtures", () => {
  withTempFixture({ omitAggregateRegionTest: true }, (result) => {
    assert.notEqual(result.status, 0, "Expected aggregate script fixture to fail");
    assert.match(result.stderr, /aggregate FFA fixture tests must include region boundary fixtures/);
  });
});
