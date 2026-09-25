#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-forum-category-presentation.mjs");

function writeFixture(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-forum-category-"));
  writeFixture(
    root,
    "crates/modules/rustok-forum/src/lib.rs",
    options.legacy ? "pub mod category_presentation; ["content", "media", "taxonomy", "tenant"]" : "dependencies ["content", "media", "taxonomy"]",
  );
  writeFixture(
    root,
    "crates/modules/rustok-forum/src/services/category_projection_owner.rs",
    options.directTaxonomy ? "rustok_taxonomy::entities::taxonomy_category_hierarchy" : "taxonomy_sync::load_category_owner_snapshot_in_tx rustok_taxonomy::lock_category_hierarchy_writer_in_tx",
  );
  writeFixture(
    root,
    "crates/modules/rustok-forum/src/services/category_command_owner.rs",
    options.directTaxonomy ? "rustok_taxonomy::entities::taxonomy_category_hierarchy" : "taxonomy_sync::move_category_in_tx taxonomy_sync::reorder_category_siblings_in_tx",
  );
  writeFixture(
    root,
    "crates/modules/rustok-forum/src/services/category_taxonomy_sync.rs",
    "rustok_taxonomy::move_module_category_in_tx rustok_taxonomy::shift_module_category_siblings_for_insert_in_tx TaxonomyOwnerCategoryReader::load_scoped_categories_in_strict",
  );
  writeFixture(
    root,
    "crates/modules/rustok-forum/src/services/category_import.rs",
    "taxonomy_sync::shift_category_siblings_for_insert_in_tx",
  );
  writeFixture(
    root,
    "crates/modules/rustok-forum/src/services/category_lifecycle.rs",
    "TaxonomyOwnerCategoryReader::load_scoped_categories_in_strict",
  );
  writeFixture(
    root,
    "crates/modules/rustok-forum/CRATE_API.md",
    options.staleApi ? "CategoryCoverMediaCandidate resolve_category_cover_for_write" : "### Category presentation ownership",
  );
  writeFixture(
    root,
    "crates/modules/rustok-forum/docs/implementation-plan.md",
    options.stalePlan ? "#### Delivered in \`FORUM-13A\` CategoryCoverMediaCandidate" : "Forum-specific Category presentation ownership was superseded",
  );
  writeFixture(
    root,
    "crates/modules/rustok-taxonomy/src/owner_category_hierarchy_mutation.rs",
    "pub async fn move_module_category_in_tx pub async fn shift_module_category_siblings_for_insert_in_tx",
  );
  writeFixture(
    root,
    "crates/modules/rustok-taxonomy/src/owner_category_read.rs",
    "pub async fn load_module_category_sibling_ids_in",
  );
  writeFixture(
    root,
    "crates/modules/rustok-taxonomy/src/lib.rs",
    "move_module_category_in_tx shift_module_category_siblings_for_insert_in_tx",
  );
  return root;
}

function run(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function withFixture(options, assertion) {
  const root = fixture(options);
  try {
    assertion(run(root));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("category presentation verifier accepts the Taxonomy owner boundary", () => {
  withFixture({}, (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /verification passed/);
  });
});

test("category presentation verifier rejects direct Taxonomy persistence access", () => {
  withFixture({ directTaxonomy: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not access Taxonomy persistence entities/);
  });
});

test("category presentation verifier rejects the legacy tenant dependency", () => {
  withFixture({ legacy: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /runtime dependency contract is stale/);
  });
});

test("category presentation verifier rejects retired Forum presentation APIs", () => {
  withFixture({ staleApi: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /retired Forum-local Category presentation APIs/);
  });
});

test("category presentation verifier rejects retired roadmap implementation details", () => {
  withFixture({ stalePlan: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /retired Category presentation implementation details/);
  });
});
