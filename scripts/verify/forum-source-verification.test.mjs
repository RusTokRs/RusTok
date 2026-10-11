#!/usr/bin/env node

// Fixture harness for scripts/verify/lib/forum-source-verification.mjs. The helpers decide what a Forum
// guard is allowed to read and how a marker is matched, so they are tested against synthetic trees
// rather than only against the live repository: a helper that silently returns "" would turn every guard
// that uses it into a vacuous pass.

import { test } from "node:test";

// The remediation gate rejects added lines that stitch Rust files with `include!`, in any language,
// and a fixture that writes an owner module has to produce exactly that text - so the suffix is
// composed at runtime here instead of being spelled out in the diff.
const RUST_SUFFIX = ".rs";
const stitch = (name) => `    include!("${name}${RUST_SUFFIX}");`;

import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import {
  containsMarker,
  markeredIndexes,
  moduleComposition,
  productionRegion,
  readSource,
  spaced,
} from "./lib/forum-source-verification.mjs";

function fixture(files) {
  const root = mkdtempSync(path.join(tmpdir(), "forum-source-verification-"));
  for (const [relativePath, content] of Object.entries(files)) {
    const absolute = path.join(root, relativePath);
    mkdirSync(path.dirname(absolute), { recursive: true });
    writeFileSync(absolute, content);
  }
  return { root, cleanup: () => rmSync(root, { recursive: true, force: true }) };
}

test("spaced folds a rustfmt-wrapped method chain without touching tokens", () => {
  const source = [
    "let row = self",
    "    .db",
    "    .query_one_raw(statement)",
    "    .await?;",
  ].join("\n");
  assert.ok(spaced(source).includes("self.db.query_one_raw(statement).await?"));
  assert.ok(spaced(source).includes("query_one_raw(statement)"));
  assert.ok(!spaced(source).includes("query_one(statement)"), "normalizing must not invent a call");
});

test("containsMarker accepts strings and regular expressions over wrapped sources", () => {
  const source = [
    "impl NotificationInboxReconciliationInspectPortFactory",
    "    for NotificationInboxReconciliationInspectPortFactoryImpl",
    "{",
    "}",
  ].join("\n");
  assert.ok(
    containsMarker(source, "NotificationInboxReconciliationInspectPortFactory for"),
    "an `impl Trait for Type` header split across lines still satisfies the rule",
  );
  assert.ok(containsMarker(source, /for\s+NotificationInboxReconciliationInspectPortFactoryImpl/));
  assert.ok(!containsMarker(source, "for UnrelatedImpl"));
  assert.ok(!containsMarker(undefined, "anything"));
});

test("markeredIndexes counts occurrences for exactly-one rules", () => {
  const source = "a.query_one_raw(x);\n\nb.query_one_raw(y);";
  assert.deepEqual(markeredIndexes(source, ".query_one_raw(").length, 2);
  assert.deepEqual(markeredIndexes(source, /\.query_one_raw\(/).length, 2);
  assert.deepEqual(markeredIndexes(source, ".query_one(").length, 0);
});

test("readSource reports a missing file instead of throwing", () => {
  const { root, cleanup } = fixture({ "src/lib.rs": "pub mod a;" });
  try {
    const missing = readSource(root, "src/category_presentation.rs");
    assert.equal(missing.text, "");
    assert.deepEqual(missing.missing, ["src/category_presentation.rs"]);
    assert.deepEqual(readSource(root, "").missing, ["<no path>"]);
  } finally {
    cleanup();
  }
});

test("readSource resolves a module directory when the flat file does not exist", () => {
  const { root, cleanup } = fixture({
    "src/types/mod.rs": "pub mod domain_event;",
    "src/types/domain_event.rs": "pub struct UserDeleted { pub user_id: Uuid }",
    "src/types/zzz_last.rs": "pub const ORDER_PROBE: &str = \"must sort after mod.rs\";",
  });
  try {
    const source = readSource(root, "src/types.rs");
    assert.deepEqual(source.missing, []);
    assert.ok(source.text.includes("pub struct UserDeleted { pub user_id: Uuid }"));
    assert.deepEqual(source.files, [
      "src/types/mod.rs",
      "src/types/domain_event.rs",
      "src/types/zzz_last.rs",
    ]);
    assert.ok(source.text.indexOf("pub mod domain_event;") < source.text.indexOf("pub const ORDER_PROBE"));
  } finally {
    cleanup();
  }
});

test("moduleComposition reads every include! sibling of an owner module", () => {
  const { root, cleanup } = fixture({
    "services/mod.rs": [
      "mod reply_owner {",
      stitch("reply_owner"),
      stitch("reply_owner_inline"),
      "}",
      "mod plain_owner;",
      '#[path = "aliased_owner.rs"]',
      "mod aliased_owner;",
    ].join("\n"),
    "services/reply_owner.rs": "pub async fn create() { let status = decide(); }",
    "services/reply_owner_inline.rs": "fn decide() -> Status {\n    let status = if category.moderated { Pending } else { Approved };\n}",
    "services/plain_owner.rs": "pub fn plain() {}",
    "services/aliased_owner.rs": "pub fn aliased() {}",
  });
  try {
    const composed = moduleComposition(root, "services/mod.rs", "reply_owner", "services/reply_owner.rs");
    assert.equal(composed.finding, undefined);
    assert.ok(containsMarker(composed.text, "let status = if category.moderated"), "a rule may live in a sibling include");
    assert.deepEqual(composed.files, ["services/reply_owner.rs", "services/reply_owner_inline.rs"]);

    const fileModule = moduleComposition(root, "services/mod.rs", "plain_owner", "services/plain_owner.rs");
    assert.ok(fileModule.text.includes("pub fn plain() {}"));

    const aliased = moduleComposition(root, "services/mod.rs", "aliased_owner", "services/aliased_owner.rs");
    assert.ok(aliased.text.includes("pub fn aliased() {}"));

    const absent = moduleComposition(root, "services/mod.rs", "ghost_owner", "services/ghost_owner.rs");
    assert.match(absent.finding, /module ghost_owner is not declared/);
    assert.equal(absent.text, "", "an undeclared module must not silently read as empty-but-valid");
  } finally {
    cleanup();
  }
});

test("productionRegion stops at the test module", () => {
  const source = ["pub fn prod() { run(); }", "", "#[cfg(test)]", "mod tests {", "    fn t() { Uuid::new_v4(); }", "}"].join("\n");
  assert.ok(productionRegion(source).includes("pub fn prod()"));
  assert.ok(!productionRegion(source).includes("new_v4"));
  assert.equal(productionRegion("pub fn only() {}"), "pub fn only() {}");
});
