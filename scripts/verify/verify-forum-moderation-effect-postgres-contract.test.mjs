import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { inspectEffects } from "./verify-forum-moderation-effect-postgres-contract.mjs";

const adapter = readFileSync(new URL("../../crates/modules/rustok-forum/src/moderation_subject.rs", import.meta.url), "utf8");
const publication = readFileSync(new URL("../../crates/modules/rustok-forum/src/services/reply_publication.rs", import.meta.url), "utf8");
function mutate(source, helper, before, after) {
  const start = source.indexOf(`async fn ${helper}(`);
  assert.ok(start >= 0);
  const at = source.indexOf(before, start);
  assert.ok(at >= 0, before);
  return source.slice(0, at) + after + source.slice(at + before.length);
}
test("trusted effects guard accepts the current adapter and shared owner", () => {
  assert.deepEqual(inspectEffects(adapter, publication), []);
});
for (const [name, helper, before, after, error] of [
  ["separate counter path", "apply_reply_non_public_status_effect_in_tx", "apply_publication_change_in_tx", "old_counter_path", "hide/reject effects"],
  ["missing topic invalidation", "apply_reply_non_public_status_effect_in_tx", "publish_forum_topic_projection_direct_in_tx", "missing_projection", "hide/reject effects"],
  ["missing removal invalidation", "apply_reply_removed_effect_in_tx", "publish_forum_topic_projection_direct_in_tx", "missing_projection", "removed effects"],
  ["missing category fence", "lock_active_subject_and_revision", "lock_category_tree_in_tx", "missing_fence", "adapter admission"],
  ["missing SQLite writer reservation", "lock_active_subject_and_revision", "SET revision = revision", "SET other = other", "adapter admission"],
  ["missing moved-parent check", "lock_active_subject_and_revision", "reply.topic_id != parent_topic_id", "false", "adapter admission"],
  ["missing solution scope", "lock_subject_parent_scopes_in_tx", "lock_topic_solution_scopes_in_tx", "missing_scope", "parent lock order"],
]) {
  test(`trusted effects guard rejects ${name}`, () => {
    const changed = mutate(adapter, helper, before, after);
    assert.ok(inspectEffects(changed, publication).some((failure) => failure.includes(error)));
  });
}
for (const [name, before] of [
  ["accepted solution cleanup", "forum_solution::Entity::delete_many()"],
  ["archived counter exemption", "return Ok(topic.category_id)"],
  ["combined member accounting", "adjust_reply_publication_counts_in_tx"],
]) {
  test(`trusted effects guard requires ${name}`, () => {
    assert.ok(publication.includes(before));
    assert.ok(inspectEffects(adapter, publication.replace(before, "missing_effect")).some((failure) => failure.includes("shared publication effects")));
  });
}
