#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-ffa-ui-transport-profile-sweep.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function withFixture({
  planExtra = "",
  registryExtra = "",
  transportSource = "pub async fn fetch_demo() { native_server_adapter::fetch_demo().await }\nmod native_server_adapter;\n",
  nativeAdapterSource = "#[server(prefix = \"/api/fn\", endpoint = \"demo/fetch\")] async fn fetch_demo_server() {}\n",
  graphqlAdapterSource = "use rustok_graphql::GraphqlRequest;\npub async fn fetch_demo_graphql() {}\n",
  includeNativeAdapter = true,
  includeGraphqlAdapter = true,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-ffa-transport-"));
  writeFixtureFile(
    root,
    "docs/modules/registry.md",
    [
      "| Module slug | UI surfaces | FFA status | FBA status | Structural shape | Source plan |",
      "|---|---|---|---|---|---|",
      `| \`demo\` | admin | \`in_progress\` | \`not_started\` | \`core_transport_ui\` | \`crates/rustok-demo/docs/implementation-plan.md\` ${registryExtra} |`,
    ].join("\n"),
  );
  writeFixtureFile(
    root,
    "crates/rustok-demo/docs/implementation-plan.md",
    [
      "## FFA/FBA status",
      "- FFA status: `in_progress`",
      "- FBA status: `not_started`",
      "- Structural shape: `core_transport_ui`",
      planExtra,
    ].join("\n"),
  );
  writeFixtureFile(root, "crates/rustok-demo/admin/src/core.rs", "pub fn view_model() {}\n");
  writeFixtureFile(root, "crates/rustok-demo/admin/src/transport/mod.rs", transportSource);
  if (includeNativeAdapter) {
    writeFixtureFile(root, "crates/rustok-demo/admin/src/transport/native_server_adapter.rs", nativeAdapterSource);
  }
  if (includeGraphqlAdapter) {
    writeFixtureFile(root, "crates/rustok-demo/admin/src/transport/graphql_adapter.rs", graphqlAdapterSource);
  }
  writeFixtureFile(root, "crates/rustok-demo/admin/src/ui/leptos.rs", "use crate::transport;\npub fn DemoAdmin() {}\n");
  return {
    root,
    cleanup() {
      rmSync(root, { recursive: true, force: true });
    },
  };
}

function runVerifier(root, args = []) {
  return spawnSync("node", [scriptPath, "--root", root, ...args], {
    cwd: path.resolve("."),
    encoding: "utf8",
  });
}

test("passes dual native and GraphQL adapter fixture", () => {
  const fixture = withFixture();
  try {
    const result = runVerifier(fixture.root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /FFA UI transport profile sweep passed/);
  } finally {
    fixture.cleanup();
  }
});

test("rejects undocumented single adapter fixture", () => {
  const fixture = withFixture({ includeGraphqlAdapter: false });
  try {
    const result = runVerifier(fixture.root);
    assert.notEqual(result.status, 0, "Expected undocumented single-adapter fixture to fail");
    assert.match(result.stderr, /less than two adapters/);
  } finally {
    fixture.cleanup();
  }
});

test("passes documented native-only single adapter fixture", () => {
  const fixture = withFixture({
    includeGraphqlAdapter: false,
    planExtra: "admin transport profile is temporary native-only single-adapter state.",
  });
  try {
    const result = runVerifier(fixture.root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    fixture.cleanup();
  }
});

test("passes documented in-progress transport parity gap fixture", () => {
  const fixture = withFixture({
    includeGraphqlAdapter: false,
    includeNativeAdapter: false,
    transportSource: "use reqwest::Method;\npub async fn fetch_demo() {}\n",
    planExtra: "admin transport profile is a transport parity gap: current REST adapter is not a target state; next step is GraphQL admin parity.",
  });
  try {
    const result = runVerifier(fixture.root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    fixture.cleanup();
  }
});

test("rejects transport parity gap outside in-progress FFA status", () => {
  const fixture = withFixture({
    registryExtra: "",
    includeGraphqlAdapter: false,
    includeNativeAdapter: false,
    transportSource: "use reqwest::Method;\npub async fn fetch_demo() {}\n",
    planExtra: "admin transport profile is a transport parity gap: current REST adapter is not a target state; next step is GraphQL admin parity.",
  });
  try {
    writeFixtureFile(
      fixture.root,
      "docs/modules/registry.md",
      [
        "| Module slug | UI surfaces | FFA status | FBA status | Structural shape | Source plan |",
        "|---|---|---|---|---|---|",
        "| `demo` | admin | `phase_b_ready` | `not_started` | `core_transport_ui` | `crates/rustok-demo/docs/implementation-plan.md` |",
      ].join("\n"),
    );
    const result = runVerifier(fixture.root);
    assert.notEqual(result.status, 0, "Expected parity gap outside in_progress to fail");
    assert.match(result.stderr, /less than two adapters/);
  } finally {
    fixture.cleanup();
  }
});

test("passes documented owner fragment fixture", () => {
  const fixture = withFixture({
    includeNativeAdapter: false,
    includeGraphqlAdapter: false,
    transportSource: "pub struct DemoRequest { pub id: String }\n",
    planExtra: "This surface is an owner-module UI fragments handoff slice consumed by commerce checkout orchestration before owner transport cutover.",
  });
  try {
    const result = runVerifier(fixture.root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    fixture.cleanup();
  }
});

test("prints usage for help", () => {
  const result = spawnSync("node", [scriptPath, "--help"], {
    cwd: path.resolve("."),
    encoding: "utf8",
  });
  assert.equal(result.status, 0);
  assert.match(result.stdout, /Usage:/);
});
