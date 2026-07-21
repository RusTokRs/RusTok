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
  const root = mkdtempSync(path.join(tmpdir(), "rustok-forum-presentation-"));
  const contract = `
pub struct CategoryCoverMediaCandidate {
  pub media_id: Uuid,
  pub tenant_id: Uuid,
  pub mime_type: String,
  pub size: i64,
  pub width: Option<i32>,
  pub height: Option<i32>,
  pub descriptor: Option<MediaImageDescriptor>,
}
pub fn normalize_category_icon_key() {}
pub fn validate() { should_emit_to_public_metadata(); }
// Quarantine/deletion state is not currently published
${options.rawMediaAccess ? "rustok_media::entities::media;" : ""}
${options.arbitraryUrl ? "cover_url: String" : ""}
`;
  writeFixture(root, "crates/rustok-forum/src/category_presentation.rs", contract);
  writeFixture(
    root,
    "crates/rustok-forum/src/entities/forum_category.rs",
    options.unvalidatedIcon ? "pub icon: Option<String>" : "normalize_category_icon_key(icon);",
  );
  for (const filePath of [
    "crates/rustok-forum/src/dto/category.rs",
    "crates/rustok-forum/src/dto/category_tree.rs",
    "crates/rustok-forum/src/services/category.rs",
    "crates/rustok-forum/src/services/category_owner.rs",
  ]) {
    writeFixture(root, filePath, "category boundary\n");
  }
  writeFixture(
    root,
    "crates/rustok-forum/docs/implementation-plan.md",
    "Delivered in `FORUM-13A`\nremaining quarantine/deletion owner state\n",
  );
  writeFixture(
    root,
    "crates/rustok-forum/CRATE_API.md",
    "CategoryCoverMediaCandidate\n",
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

test("category presentation verifier accepts canonical boundary", () => {
  withFixture({}, (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /verification passed/);
  });
});

test("category presentation verifier rejects raw Media access", () => {
  withFixture({ rawMediaAccess: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not access Media persistence/);
  });
});

test("category presentation verifier rejects arbitrary cover URL", () => {
  withFixture({ arbitraryUrl: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not store an arbitrary image URL/);
  });
});

test("category presentation verifier requires DB icon guard", () => {
  withFixture({ unvalidatedIcon: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /database write boundary/);
  });
});
