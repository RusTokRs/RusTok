#!/usr/bin/env node
import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const script = path.resolve("scripts/verify/verify-region-storefront-boundary.mjs");
function put(root, file, content) {
  const target = path.join(root, file);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}
function fixture({ graphqlFirst = false, rawUi = false, leptosCore = false } = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "region-storefront-"));
  put(root, "crates/rustok-region/storefront/src/lib.rs", "mod core; mod transport; mod ui; pub use ui::RegionView;");
  put(root, "crates/rustok-region/storefront/src/core.rs", `${leptosCore ? "leptos::" : ""} RegionErrorEvidence RegionErrorViewModel RegionErrorDomEvidence selected_region_query_update`);
  put(root, "crates/rustok-region/storefront/src/ui/leptos.rs", `transport::fetch_regions data-region-error-status data-region-error-locale-key ${rawUi ? "graphql_adapter::" : ""}`);
  const native = "native_server_adapter::fetch_regions";
  const graphql = "graphql_adapter::fetch_regions";
  put(root, "crates/rustok-region/storefront/src/transport/mod.rs", `mod graphql_adapter; mod native_server_adapter; RegionFetchFallbackPolicy::NativeThenGraphql ${graphqlFirst ? `${graphql} ${native}` : `${native} ${graphql}`} RegionTransportError::fallback_failed`);
  put(root, "crates/rustok-region/storefront/src/transport/native_server_adapter.rs", "fetch_storefront_regions_server");
  put(root, "crates/rustok-region/storefront/src/transport/graphql_adapter.rs", "fetch_storefront_regions_graphql");
  put(root, "crates/rustok-region/docs/implementation-plan.md", "verify-region-storefront-boundary.mjs");
  put(root, "docs/modules/registry.md", "verify-region-storefront-boundary.mjs");
  put(root, "package.json", JSON.stringify({ scripts: {
    "verify:region:storefront-boundary": "node verifier",
    "test:verify:region:storefront-boundary": "node tests",
    "test:verify:ffa:ui:migration": "npm run test:verify:region:storefront-boundary",
  }}));
  return root;
}
function run(root) {
  return spawnSync("node", [script], { env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root }, encoding: "utf8" });
}
function verifyFailure(options, pattern) {
  const root = fixture(options);
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}
test("region storefront boundary verifier passes native-first fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
test("rejects GraphQL-first fallback", () => verifyFailure({ graphqlFirst: true }, /fallback order must remain native then GraphQL/));
test("rejects raw adapter calls from UI", () => verifyFailure({ rawUi: true }, /UI must not call raw adapter/));
test("rejects Leptos-specific core", () => verifyFailure({ leptosCore: true }, /core must stay Leptos\/runtime free/));
