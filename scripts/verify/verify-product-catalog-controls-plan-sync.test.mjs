#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve(
  "scripts/verify/verify-product-catalog-controls-plan-sync.mjs",
);

const sourcePaths = [
  "crates/rustok-product/storefront/src/core.rs",
  "crates/rustok-product/storefront/src/ui/leptos.rs",
  "crates/rustok-product/storefront/src/transport/native_server_adapter.rs",
  "crates/rustok-product/storefront/src/transport/graphql_adapter.rs",
  "crates/rustok-product/admin/src/transport.rs",
  "crates/rustok-product/admin/src/ui/leptos.rs",
  "crates/rustok-product/admin/src/transport/graphql_adapter.rs",
  "crates/rustok-commerce/src/graphql/types.rs",
];

function writeFixture(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function plan({ complete = false, staleSourceLock = false, includeProvenance = true } = {}) {
  const checkbox = complete ? "[x]" : "[ ]";
  const sourceLock = staleSourceLock
    ? "The optional catalog filters/sorts, detached-value marker contract, and no-compile schema guardrail are source-locked."
    : "Catalog search-option discovery is source-locked; catalog filter and sort execution remains open.";
  const provenance = includeProvenance
    ? "Recheck on 2026-07-28 found the typed query contract incomplete."
    : "The typed query contract remains incomplete.";
  return `
# Implementation Plan for rustok-product

${sourceLock}
${provenance}

## Verification

- ${checkbox} Connect storefront/admin UI controls to optional catalog filters/sorts.
- node scripts/verify/verify-product-catalog-controls-plan-sync.mjs
- node scripts/verify/verify-product-catalog-controls-plan-sync.test.mjs
`;
}

function registry({ includeCatalogPriority = true } = {}) {
  const priority = includeCatalogPriority
    ? "Close the storefront/admin catalog filters/sorts contract before provider promotion."
    : "Execute the catalog read provider before provider promotion.";
  return `
| Module/Crate | Local plan | Status | Nearest priority |
| --- | --- | --- | --- |
| \`product\` | [plan](../../crates/rustok-product/docs/implementation-plan.md) | \`boundary_ready\` | ${priority} |
`;
}

function sourceFixture(complete) {
  if (!complete) return "// incomplete catalog controls source\n";
  return `
pub search: Option<String>
pub category_id: Option<String>
pub sort_by: Option<String>
pub sort_direction: Option<String>
pub attribute_filters: Vec<String>
category_id
sort_by
sort_direction
attribute_filters
`;
}

function run({
  planContent = plan(),
  registryContent = registry(),
  implementationComplete = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-plan-sync-"));
  try {
    writeFixture(
      root,
      "crates/rustok-product/docs/implementation-plan.md",
      planContent,
    );
    writeFixture(
      root,
      "docs/modules/implementation-plans-registry.md",
      registryContent,
    );
    for (const sourcePath of sourcePaths) {
      writeFixture(root, sourcePath, sourceFixture(implementationComplete));
    }
    return spawnSync("node", [scriptPath], {
      cwd: path.resolve("."),
      env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
      encoding: "utf8",
    });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("accepts an honest pending plan while catalog controls are incomplete", () => {
  const result = run();
  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /verification passed/);
});

test("rejects a completed checkbox while catalog controls are incomplete", () => {
  const result = run({ planContent: plan({ complete: true }) });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /must remain pending/);
});

test("rejects a pending checkbox after every source layer is complete", () => {
  const result = run({ implementationComplete: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /source-complete but the task is still pending/);
});

test("accepts a completed checkbox after every source layer is complete", () => {
  const result = run({
    planContent: plan({ complete: true }),
    implementationComplete: true,
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);
});

test("rejects the stale source-locked catalog-controls claim", () => {
  const result = run({ planContent: plan({ staleSourceLock: true }) });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /must not be described as source-locked/);
});

test("rejects missing source recheck provenance", () => {
  const result = run({ planContent: plan({ includeProvenance: false }) });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /source recheck provenance/);
});

test("rejects central registry priority drift", () => {
  const result = run({
    registryContent: registry({ includeCatalogPriority: false }),
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /nearest priority/);
});
