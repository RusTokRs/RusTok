#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-forum-mention-persistence.mjs");

function writeFixture(root, relativePath, content) {
  const absolute = path.join(root, relativePath);
  mkdirSync(path.dirname(absolute), { recursive: true });
  writeFileSync(absolute, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-forum-mention-persistence-"));
  const migration = `
forum_relation_revisions
forum_user_mentions
forum_audience_mentions
forum_quotes
projection_fingerprint VARCHAR(64)
DatabaseBackend::Postgres
DatabaseBackend::Sqlite
forum_relation_revision_source_guard
forum_relation_revision_immutable_guard
forum_user_mentions_immutable_guard
forum_audience_mentions_immutable_guard
forum_quotes_immutable_guard
forum_quotes_target_guard
forum relation projections are immutable
'legacy'
${options.missingImmutable ? "" : "append_only"}
`;
  writeFixture(
    root,
    "crates/rustok-forum/src/migrations/m20260722_000004_add_forum_mention_quote_relations.rs",
    options.missingImmutable
      ? migration.replace("forum_user_mentions_immutable_guard", "")
      : migration,
  );
  writeFixture(
    root,
    "crates/rustok-forum/src/services/mention_relation.rs",
    `
ProfilesReader
DatabaseTransaction
pub(crate) struct MentionRelationService;
pub(crate) async fn prepare() {}
pub(crate) async fn persist_in_tx() {}
lock_source_in_tx
ensure_prepared_matches_source_in_tx
latest_revision_in_tx
load_snapshot_in_tx
validate_quote_targets_in_tx
Sha256
projection_fingerprint
added_user_ids
replayed: true
ForumError::quote_target_unavailable()
${options.notificationCall ? "rustok_notifications::deliver();" : ""}
${options.profileInternals ? "rustok_profiles::entities::profile::Entity;" : ""}
${options.publicService ? "pub struct MentionRelationService;" : ""}
`,
  );
  writeFixture(
    root,
    "crates/rustok-forum/src/services/mention_relation_tests.rs",
    [
      "relation_revision_replay_diff_quotes_and_guards_are_atomic",
      "identical replay should persist idempotently",
      "cross-tenant quote revision must fail closed",
      "persisted mention rows must be immutable",
    ].join("\n"),
  );
  writeFixture(
    root,
    "crates/rustok-forum/src/migrations/mod.rs",
    "m20260722_000004_add_forum_mention_quote_relations",
  );
  writeFixture(
    root,
    "crates/rustok-forum/src/services/mod.rs",
    "mod mention_relation;",
  );
  writeFixture(
    root,
    "crates/rustok-forum/src/entities/mod.rs",
    "forum_relation_revision\nforum_user_mention\nforum_audience_mention\nforum_quote\n",
  );
  writeFixture(
    root,
    "crates/rustok-forum/src/error.rs",
    '"FORUM_QUOTE_TARGET_UNAVAILABLE"',
  );
  writeFixture(
    root,
    "crates/rustok-forum/docs/implementation-plan.md",
    "Delivered in `FORUM-12B1`\nFORUM-12B2\n",
  );
  writeFixture(
    root,
    "crates/rustok-forum/CRATE_API.md",
    "forum_relation_revisions\nMentionRelationService\nFORUM_QUOTE_TARGET_UNAVAILABLE\n",
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

test("mention persistence verifier accepts owner-only contract", () => {
  withFixture({}, (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /verification passed/);
  });
});

test("mention persistence verifier rejects notification delivery", () => {
  withFixture({ notificationCall: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not publish events or call Notifications/);
  });
});

test("mention persistence verifier rejects profile internals", () => {
  withFixture({ profileInternals: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /resolve identity through ProfilesReader/);
  });
});

test("mention persistence verifier rejects public persistence service", () => {
  withFixture({ publicService: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must remain crate-private/);
  });
});

test("mention persistence verifier requires immutable child guards", () => {
  withFixture({ missingImmutable: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /missing schema marker/);
  });
});
