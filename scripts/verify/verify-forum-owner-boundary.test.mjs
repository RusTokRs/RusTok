#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-forum-owner-boundary.mjs");

function writeFixture(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function topicOwner({ deref = false, omitList = false } = {}) {
  return `
pub struct TopicService;
impl TopicService {
  pub async fn create() {}
  pub async fn get() {}
  pub async fn get_with_locale_fallback() {}
  pub async fn update() {}
  pub async fn delete() {}
  ${omitList ? "" : "pub async fn list() {}"}
  pub async fn list_with_locale_fallback() {}
  pub async fn list_storefront_visible_with_locale_fallback() {}
}
${deref ? "use std::ops::Deref; impl Deref for TopicService { type Target = (); fn deref(&self) -> &() { &() } }" : ""}
`;
}

function replyOwner({ deref = false } = {}) {
  return `
pub struct ReplyService;
impl ReplyService {
  pub async fn create() {}
  pub async fn get() {}
  pub async fn get_with_locale_fallback() {}
  pub async fn update() {}
  pub async fn delete() {}
  pub async fn list_for_topic() {}
  pub async fn list_for_topic_with_locale_fallback() {}
  pub async fn list_response_for_topic_with_locale_fallback() {}
  pub async fn list_response_for_topic_by_statuses_with_locale_fallback() {}
}
${deref ? "use std::ops::Deref; impl Deref for ReplyService { type Target = (); fn deref(&self) -> &() { &() } }" : ""}
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-forum-owner-boundary-"));
  writeFixture(
    root,
    "crates/rustok-forum/src/services/mod.rs",
    options.publicRawModules
      ? "pub mod topic;\npub mod reply;\npub use topic_owner::TopicService;\npub use reply_owner::ReplyService;\n"
      : "mod topic;\nmod reply;\npub use topic_owner::TopicService;\npub use reply_owner::ReplyService;\n",
  );
  writeFixture(root, "crates/rustok-forum/src/services/topic_owner.rs", topicOwner(options));
  writeFixture(root, "crates/rustok-forum/src/services/reply_owner.rs", replyOwner(options));
  writeFixture(root, "crates/rustok-forum/src/lib.rs", "pub use services::{ReplyService, TopicService};\n");
  writeFixture(root, "crates/rustok-forum/CRATE_API.md", "verify-forum-owner-boundary.mjs\n");
  writeFixture(root, "crates/rustok-forum/docs/implementation-plan.md", "verify-forum-owner-boundary.mjs\n");
  if (options.externalRawImport) {
    writeFixture(
      root,
      "crates/example/src/lib.rs",
      "use rustok_forum::services::topic::TopicService;\n",
    );
  } else {
    writeFixture(root, "crates/example/src/lib.rs", "use rustok_forum::TopicService;\n");
  }
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

test("forum owner boundary verifier passes canonical owner facade", () => {
  withTempFixture({}, (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /forum owner boundary verification passed/);
  });
});

test("forum owner boundary verifier rejects public raw modules", () => {
  withTempFixture({ publicRawModules: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /raw lifecycle modules must not be public/);
  });
});

test("forum owner boundary verifier rejects persistence deref", () => {
  withTempFixture({ deref: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not dereference into persistence service/);
  });
});

test("forum owner boundary verifier rejects external raw imports", () => {
  withTempFixture({ externalRawImport: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /imports a raw forum topic\/reply persistence service/);
  });
});

test("forum owner boundary verifier rejects missing explicit owner method", () => {
  withTempFixture({ omitList: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /explicit owner method missing/);
  });
});
