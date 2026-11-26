import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const base = "crates/modules/rustok-forum/src/services";
function fixture(mutate) {
  const temporary = mkdtempSync(path.join(tmpdir(), "forum-delete-admission-"));
  try {
    for (const file of ["topic_owner.rs", "reply_owner.rs"]) {
      const source = readFileSync(path.join(root, base, file), "utf8");
      const target = path.join(temporary, base, file);
      mkdirSync(path.dirname(target), { recursive: true });
      writeFileSync(target, mutate(file, source));
    }
    return spawnSync(process.execPath, [path.join(root, "scripts/verify/verify-forum-delete-policy.mjs")], {
      cwd: temporary, env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: temporary }, encoding: "utf8",
    });
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}
function replaceAfter(source, signature, before, after) {
  const start = source.indexOf(signature);
  assert.ok(start >= 0, signature);
  const index = source.indexOf(before, start);
  assert.ok(index >= 0, `mutation must apply: ${before}`);
  return source.slice(0, index) + after + source.slice(index + before.length);
}
function rejects(target, mutate, pattern) {
  const result = fixture((file, source) => file === target ? mutate(source) : source);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, pattern);
}

test("delete admission guard accepts current topic/reply modules", () => {
  const result = fixture((_file, source) => source);
  assert.equal(result.status, 0, result.stderr);
});
for (const kind of ["topic", "reply"]) {
  test(`static preflight cannot satisfy a missing ${kind} transactional settings read`, () => {
    rejects(`${kind}_owner.rs`, (source) => replaceAfter(source, "pub async fn delete(",
      "module_settings_in_tx", "module_settings"), new RegExp(`${kind} delete transactional admission.*module_settings_in_tx`));
  });
}
test("topic delete must recheck ownership after claiming its source", () => {
  rejects("topic_owner.rs", (source) => replaceAfter(source, "claim_topic_delete_in_tx(&txn",
    "enforce_owned_scope", "removed_owner_check"), /topic delete transactional admission.*enforce_owned_scope/);
});
test("reply delete cannot use only the preflight policy decision", () => {
  rejects("reply_owner.rs", (source) => replaceAfter(source, "let (topic, reply) = Self::lock_reply_removal_in_tx(&txn",
    "enforce_author_deletion_policy", "removed_policy_check"), /reply delete transactional admission.*enforce_author_deletion_policy/);
});
test("internal moderation removal must stay separate from author self-deletion policy", () => {
  rejects("reply_owner.rs", (source) => replaceAfter(source, "pub(crate) async fn remove_in_tx(",
    "let (topic, reply)", "enforce_author_deletion_policy();\n        let (topic, reply)"), /internal moderation removal must not acquire author/);
});
test("shared removal locks the solution scope before the reply row", () => {
  rejects("reply_owner.rs", (source) => replaceAfter(source, "async fn lock_reply_removal_in_tx(",
    "lock_topic_solution_scopes_in_tx", "removed_solution_scope"), /reply removal lock order.*lock_topic_solution_scopes_in_tx/);
});
test("shared removal persists the deleted source before applying publication effects", () => {
  rejects("reply_owner.rs", (source) => replaceAfter(source, "async fn remove_locked_in_tx(",
    "mark_reply_deleted_in_tx", "removed_tombstone_write"), /shared reply removal effects.*mark_reply_deleted_in_tx/);
});
