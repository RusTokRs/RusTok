#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-pricing-storefront-boundary.mjs");
const repoRoot = path.resolve(".");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-pricing-storefront-boundary-"));
  writeFixtureFile(root, "crates/rustok-pricing/storefront/src/lib.rs", `
${options.legacyModApi ? "mod api;" : ""}
mod core;
mod transport;
mod ui;
pub use ui::leptos::PricingView;
`);
  const realCore = readFileSync(path.join(repoRoot, "crates/rustok-pricing/storefront/src/core.rs"), "utf8");
  writeFixtureFile(root, "crates/rustok-pricing/storefront/src/core.rs", `${options.includeLeptosCore ? "use leptos::prelude::*;\n" : ""}${realCore}`);
  writeFixtureFile(root, "crates/rustok-pricing/storefront/src/ui/leptos.rs", `
use crate::core;
use crate::transport;
pub fn PricingView() {
  let _ = transport::fetch_storefront_pricing;
  ${options.rawApiCall ? "let _ = api::fetch_storefront_pricing_graphql;" : ""}
}
`);
  writeFixtureFile(root, "crates/rustok-pricing/storefront/src/transport/mod.rs", readFileSync(path.join(repoRoot, "crates/rustok-pricing/storefront/src/transport/mod.rs"), "utf8"));
  writeFixtureFile(root, "crates/rustok-pricing/storefront/src/transport/graphql_adapter.rs", readFileSync(path.join(repoRoot, "crates/rustok-pricing/storefront/src/transport/graphql_adapter.rs"), "utf8"));
  writeFixtureFile(root, "crates/rustok-pricing/storefront/src/transport/graphql_error_safety.rs", readFileSync(path.join(repoRoot, "crates/rustok-pricing/storefront/src/transport/graphql_error_safety.rs"), "utf8"));
  writeFixtureFile(root, "crates/rustok-pricing/storefront/src/transport/native_server_adapter.rs", readFileSync(path.join(repoRoot, "crates/rustok-pricing/storefront/src/transport/native_server_adapter.rs"), "utf8"));
  writeFixtureFile(root, "crates/rustok-graphql/src/lib.rs", readFileSync(path.join(repoRoot, "crates/rustok-graphql/src/lib.rs"), "utf8"));
  writeFixtureFile(root, "crates/rustok-pricing/storefront/Cargo.toml", readFileSync(path.join(repoRoot, "crates/rustok-pricing/storefront/Cargo.toml"), "utf8"));
  writeFixtureFile(root, "crates/rustok-pricing/contracts/evidence/storefront-graphql-error-safety-source.json", readFileSync(path.join(repoRoot, "crates/rustok-pricing/contracts/evidence/storefront-graphql-error-safety-source.json"), "utf8"));
  writeFixtureFile(root, "crates/rustok-pricing/contracts/evidence/storefront-graphql-error-safety-source-review.json", readFileSync(path.join(repoRoot, "crates/rustok-pricing/contracts/evidence/storefront-graphql-error-safety-source-review.json"), "utf8"));
  writeFixtureFile(root, "crates/rustok-pricing/contracts/evidence/storefront-native-error-safety-source.json", readFileSync(path.join(repoRoot, "crates/rustok-pricing/contracts/evidence/storefront-native-error-safety-source.json"), "utf8"));
  writeFixtureFile(root, "crates/rustok-pricing/docs/storefront-graphql-error-safety.md", readFileSync(path.join(repoRoot, "crates/rustok-pricing/docs/storefront-graphql-error-safety.md"), "utf8"));
  writeFixtureFile(root, "crates/rustok-pricing/docs/storefront-native-error-safety.md", readFileSync(path.join(repoRoot, "crates/rustok-pricing/docs/storefront-native-error-safety.md"), "utf8"));
  writeFixtureFile(root, "crates/rustok-commerce/docs/implementation-plan.md", readFileSync(path.join(repoRoot, "crates/rustok-commerce/docs/implementation-plan.md"), "utf8"));
  if (options.legacyApi) writeFixtureFile(root, "crates/rustok-pricing/storefront/src/api.rs", "pub async fn fetch_storefront_pricing_graphql() {}\n");
  writeFixtureFile(root, "crates/rustok-pricing/docs/implementation-plan.md", "verify-pricing-storefront-boundary.mjs");
  writeFixtureFile(root, "docs/modules/registry.md", "verify-pricing-storefront-boundary.mjs");
  writeFixtureFile(root, "package.json", JSON.stringify({
    scripts: {
      "verify:pricing:storefront-boundary": "node scripts/verify/verify-pricing-storefront-boundary.mjs",
      "test:verify:pricing:storefront-boundary": "node scripts/verify/verify-pricing-storefront-boundary.test.mjs",
      "verify:ffa:ui:migration": "npm run verify:pricing:storefront-boundary",
      "test:verify:ffa:ui:migration": "npm run test:verify:pricing:storefront-boundary",
    },
  }));
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

test("pricing storefront boundary verifier passes canonical fixture", () => {
  withTempFixture({}, (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /pricing storefront boundary verification passed/);
  });
});

test("pricing storefront boundary verifier rejects legacy api file", () => {
  withTempFixture({ legacyApi: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /legacy api\.rs/);
  });
});

test("pricing storefront boundary verifier rejects legacy api module wiring", () => {
  withTempFixture({ legacyModApi: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not wire legacy api adapter/);
  });
});

test("pricing storefront boundary verifier rejects Leptos-specific core", () => {
  withTempFixture({ includeLeptosCore: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  });
});

test("pricing storefront boundary verifier rejects raw api calls from UI", () => {
  withTempFixture({ rawApiCall: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /UI adapter must not call raw transport or services/);
  });
});
