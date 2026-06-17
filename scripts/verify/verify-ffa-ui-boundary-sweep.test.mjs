#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-ffa-ui-boundary-sweep.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function withFixture({
  boardFfa = "in_progress",
  boardFba = "not_started",
  boardShape = "core_transport_ui",
  localFfa = "in_progress",
  localFba = "not_started",
  localShape = "core_transport_ui",
  includeUi = true,
  coreSource = "pub fn view_model() {}\n",
  uiSource = "use crate::core;\nuse crate::transport;\npub fn DemoAdmin() {}\n",
  aggregate = "npm run verify:ffa:ui:migration:boundary-sweep && npm run verify:inventory:admin-boundary && npm run verify:workflow:admin-boundary",
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-ffa-sweep-"));

  writeFixtureFile(
    root,
    "docs/modules/registry.md",
    [
      "| Module slug | UI surfaces | FFA status | FBA status | Structural shape | Source plan |",
      "|---|---|---|---|---|---|",
      `| \`demo\` | admin | \`${boardFfa}\` | \`${boardFba}\` | \`${boardShape}\` | \`crates/rustok-demo/docs/implementation-plan.md\` fixture |`,
    ].join("\n"),
  );
  writeFixtureFile(
    root,
    "crates/rustok-demo/docs/implementation-plan.md",
    [
      "## FFA/FBA status",
      `- FFA status: \`${localFfa}\``,
      `- FBA status: \`${localFba}\``,
      `- Structural shape: \`${localShape}\``,
    ].join("\n"),
  );
  writeFixtureFile(root, "crates/rustok-demo/admin/src/core.rs", coreSource);
  writeFixtureFile(root, "crates/rustok-demo/admin/src/transport/mod.rs", "pub async fn fetch_demo() {}\n");
  if (includeUi) {
    writeFixtureFile(root, "crates/rustok-demo/admin/src/ui/leptos.rs", uiSource);
  }
  writeFixtureFile(
    root,
    "package.json",
    JSON.stringify(
      {
        scripts: {
          "verify:ffa:ui:migration": aggregate,
          "verify:ffa:ui:migration:boundary-sweep": "node scripts/verify/verify-ffa-ui-boundary-sweep.mjs",
          "verify:inventory:admin-boundary": "node scripts/verify/verify-inventory-admin-boundary.mjs",
          "verify:workflow:admin-boundary": "node scripts/verify/verify-workflow-admin-boundary.mjs",
        },
      },
      null,
      2,
    ),
  );

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

test("passes canonical core_transport_ui fixture", () => {
  const fixture = withFixture();
  try {
    const result = runVerifier(fixture.root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /FFA UI boundary sweep passed/);
  } finally {
    fixture.cleanup();
  }
});

test("rejects central/local status drift", () => {
  const fixture = withFixture({ boardFba: "boundary_ready", localFba: "in_progress" });
  try {
    const result = runVerifier(fixture.root);
    assert.notEqual(result.status, 0, "Expected status drift fixture to fail");
    assert.match(result.stderr, /central fbaStatus=boundary_ready/);
  } finally {
    fixture.cleanup();
  }
});

test("rejects missing UI adapter for core_transport_ui surface", () => {
  const fixture = withFixture({ includeUi: false });
  try {
    const result = runVerifier(fixture.root);
    assert.notEqual(result.status, 0, "Expected missing ui fixture to fail");
    assert.match(result.stderr, /expected core_transport_ui structure/);
  } finally {
    fixture.cleanup();
  }
});

test("rejects Leptos imports inside core", () => {
  const fixture = withFixture({ coreSource: "use leptos::prelude::*;\npub fn view_model() {}\n" });
  try {
    const result = runVerifier(fixture.root);
    assert.notEqual(result.status, 0, "Expected Leptos core fixture to fail");
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  } finally {
    fixture.cleanup();
  }
});

test("rejects raw api calls from UI adapter", () => {
  const fixture = withFixture({ uiSource: "use crate::api;\npub fn DemoAdmin() { let _ = api::fetch_demo; }\n" });
  try {
    const result = runVerifier(fixture.root);
    assert.notEqual(result.status, 0, "Expected raw api fixture to fail");
    assert.match(result.stderr, /UI adapter must not call raw api::\* directly/);
  } finally {
    fixture.cleanup();
  }
});

test("rejects aggregate pipeline without sweep", () => {
  const fixture = withFixture({ aggregate: "npm run verify:inventory:admin-boundary && npm run verify:workflow:admin-boundary" });
  try {
    const result = runVerifier(fixture.root);
    assert.notEqual(result.status, 0, "Expected missing aggregate sweep fixture to fail");
    assert.match(result.stderr, /verify:ffa:ui:migration must include npm run verify:ffa:ui:migration:boundary-sweep/);
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
