#!/usr/bin/env node
// Supplemental source contract; runtime policy/rollback proof belongs to the
// shared SQLite/PostgreSQL service matrix in tests/support/delete_policy.rs.
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spaced } from "./lib/forum-source-verification.mjs";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const base = "crates/modules/rustok-forum/src/services";
const failures = [];
function read(file) {
  try { return readFileSync(path.join(root, file), "utf8"); }
  catch (error) { failures.push(`${file}: ${error.message}`); return ""; }
}
function method(source, signature) {
  const start = source.indexOf(signature);
  if (start < 0) { failures.push(`missing method: ${signature}`); return ""; }
  const end = source.indexOf("\n    }", start);
  if (end < 0) { failures.push(`missing method boundary: ${signature}`); return ""; }
  return source.slice(start, end);
}
function ordered(source, markers, label) {
  const normalized = spaced(source);
  let previous = -1;
  for (const marker of markers) {
    const index = normalized.indexOf(spaced(marker), previous + 1);
    if (index < 0) { failures.push(`${label}: missing/out-of-order ${marker}`); return; }
    previous = index;
  }
}
const topic = read(`${base}/topic_owner.rs`);
const reply = read(`${base}/reply_owner.rs`);
ordered(method(topic, "pub async fn delete("), [
  "let txn = self.db.begin().await?;", "lock_category_tree_in_tx",
  "claim_topic_delete_in_tx", "find_topic_in_tx",
  "enforce_owned_scope", "module_settings_in_tx", "enforce_author_deletion_policy",
  "record_topic_delete_snapshot_in_tx", "mark_topic_thread_deleted_in_tx", "txn.commit()",
], "topic delete transactional admission");
ordered(method(reply, "pub async fn delete("), [
  "let txn = self.db.begin().await?;", "lock_reply_removal_in_tx",
  "enforce_owned_scope", "module_settings_in_tx", "enforce_author_deletion_policy",
  "remove_locked_in_tx", "publish_in_tx", "txn.commit()",
], "reply delete transactional admission");
const internal = method(reply, "pub(crate) async fn remove_in_tx(");
ordered(internal, ["lock_reply_removal_in_tx", "remove_locked_in_tx"], "internal moderation removal");
if (/enforce_author_deletion_policy|module_settings/.test(internal)) {
  failures.push("internal moderation removal must not acquire author self-deletion policy");
}
ordered(method(reply, "async fn lock_reply_removal_in_tx("), [
  "lock_category_tree_in_tx", "find_reply_in_tx", "find_topic_for_update_in_tx",
  "lock_topic_solution_scopes_in_tx", "find_reply_for_update_in_tx",
], "reply removal lock order");
ordered(method(reply, "async fn remove_locked_in_tx("), [
  "validate_transition", "record_reply_delete_snapshot_in_tx", "mark_reply_deleted_in_tx",
  "apply_publication_change_in_tx",
], "shared reply removal effects");
if (failures.length) {
  console.error(failures.join("\n"));
  process.exit(1);
}
console.log("Forum delete-policy source contract passed; Rust service execution remains required.");
