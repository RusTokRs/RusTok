#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-forum-mention-contract.mjs");

function writeFixture(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-forum-mentions-"));
  const contract = `
pub const FORUM_MAX_MENTION_TARGETS_PER_REVISION: usize = 32;
pub const FORUM_MAX_QUOTE_REFERENCES_PER_REVISION: usize = 32;
use serde::Serialize;
normalize_content_format();
validate_and_sanitize_rt_json();
ProfileService::normalize_handle();
ProfilesReader;
ProfileVisibility::Public | ProfileVisibility::Authenticated;
ForumRevisionIdentity;
impl ForumQuoteReference {}
pub fn new(target: ForumContentTarget, revision_id: i64) {}
resolved: ForumResolvedMentions;
diff_forum_mentions();
Forum mention replay changed an existing revision projection;
source: current.source().clone();
pub fn event_candidates(&self) {}
pub fn added_users(&self) {}
${options.notificationCall ? "rustok_notifications::send();" : ""}
${options.profilePersistence ? "rustok_profiles::entities::profile::Entity;" : ""}
${options.forumPersistence ? "ActiveModel.insert();" : ""}
${options.uncheckedDeserialize ? "#[derive(Deserialize)]" : ""}
${options.publicDiff ? "pub added_users: Vec<User>" : ""}
${options.publicValidatedField ? "pub revision_id: i64" : ""}
${options.missingCap ? "// FORUM_MAX_MENTION_TARGETS_PER_REVISION removed" : ""}
`;
  writeFixture(
    root,
    "crates/rustok-forum/src/mentions.rs",
    options.missingCap
      ? contract.replace("pub const FORUM_MAX_MENTION_TARGETS_PER_REVISION: usize = 32;", "")
      : contract,
  );
  writeFixture(
    root,
    "crates/rustok-forum/src/error.rs",
    options.errorCarriesHandle
      ? 'MentionTargetUnavailable { handle: String }\n"FORUM_MENTION_TARGET_UNAVAILABLE"\n'
      : 'MentionTargetUnavailable,\n"FORUM_MENTION_TARGET_UNAVAILABLE"\n',
  );
  writeFixture(
    root,
    "crates/rustok-forum/src/lib.rs",
    "pub mod mentions;\npub use mentions::*;\n",
  );
  writeFixture(
    root,
    "crates/rustok-forum/tests/mention_contract.rs",
    [
      "markdown_extraction_ignores_code_escaping_and_email_addresses",
      "rt_json_extraction_ignores_code_blocks_and_code_marks",
      "profile_resolution_is_tenant_scoped_and_fail_closed",
      "manual candidate construction must enforce audience permission",
      "revision_diff_emits_only_new_targets_and_replay_is_immutable",
      "identical revision replay must be idempotent",
      "quote_references_are_revision_bound_deduplicated_and_non_recursive",
    ].join("\n"),
  );
  writeFixture(
    root,
    "crates/rustok-forum/docs/implementation-plan.md",
    "Delivered in `FORUM-12A`\nFORUM-12 remains `in_progress`\n",
  );
  writeFixture(
    root,
    "crates/rustok-forum/CRATE_API.md",
    "ForumMentionRevisionProjection\nForumQuoteReference\n",
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

test("mention verifier accepts canonical owner boundary", () => {
  withFixture({}, (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /verification passed/);
  });
});

test("mention verifier rejects synchronous notification delivery", () => {
  withFixture({ notificationCall: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not call notification/);
  });
});

test("mention verifier rejects profile internals", () => {
  withFixture({ profilePersistence: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /public ProfilesReader boundary/);
  });
});

test("mention verifier rejects premature forum persistence", () => {
  withFixture({ forumPersistence: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not add persistence/);
  });
});

test("mention verifier rejects unchecked deserialization", () => {
  withFixture({ uncheckedDeserialize: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not bypass constructors/);
  });
});

test("mention verifier rejects mutable diff collections", () => {
  withFixture({ publicDiff: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must remain immutable/);
  });
});

test("mention verifier rejects public validated DTO fields", () => {
  withFixture({ publicValidatedField: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must remain constructor-only/);
  });
});

test("mention verifier rejects identifiers in safe target errors", () => {
  withFixture({ errorCarriesHandle: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not retain requested identifiers/);
  });
});

test("mention verifier requires the hard mention cap", () => {
  withFixture({ missingCap: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /missing required contract marker/);
  });
});
