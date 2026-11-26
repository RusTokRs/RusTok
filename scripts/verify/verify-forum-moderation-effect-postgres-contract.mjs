import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spaced } from "./lib/forum-source-verification.mjs";

export function inspectEffects(adapter, publication) {
  const failures = [];
  function body(name) {
    const start = adapter.indexOf(`async fn ${name}(`);
    if (start < 0) { failures.push(`missing helper ${name}`); return ""; }
    const tail = adapter.slice(start);
    const end = tail.slice(1).search(/\n(?:async )?fn /);
    return spaced(end < 0 ? tail : tail.slice(0, end + 1));
  }
  function ordered(source, markers, label) {
    let cursor = -1;
    for (const marker of markers) {
      cursor = source.indexOf(spaced(marker), cursor + 1);
      if (cursor < 0) { failures.push(`${label}: missing/out-of-order ${marker}`); return; }
    }
  }
  ordered(body("apply_reply_non_public_status_effect_in_tx"), [
    "if reply.status == target", "return Ok(false)", "set_status_in_tx",
    "apply_publication_change_in_tx", "publish_root_in_tx",
    "publish_forum_category_projection_direct_in_tx", "publish_forum_topic_projection_direct_in_tx",
  ], "hide/reject effects");
  ordered(body("apply_reply_removed_effect_in_tx"), [
    "ReplyService::remove_in_tx", "publish_root_in_tx",
    "publish_forum_category_projection_direct_in_tx", "outcome.was_public || outcome.solution_removed",
    "publish_forum_topic_projection_direct_in_tx",
  ], "removed effects");
  ordered(body("lock_active_subject_and_revision"), [
    "lock_category_tree_in_tx", "SET revision = revision", "lock_subject_parent_scopes_in_tx",
    "FOR UPDATE", "reply.topic_id != parent_topic_id",
  ], "adapter admission");
  ordered(body("lock_subject_parent_scopes_in_tx"), [
    "SELECT topic_id FROM forum_replies", "SELECT id FROM forum_topics",
    "FOR UPDATE", "lock_topic_solution_scopes_in_tx",
  ], "parent lock order");
  ordered(spaced(publication), [
    "forum_solution::Entity::delete_many()", "adjust_reply_count_in_tx",
    "TopicStatus::Archived", "return Ok(topic.category_id)", "adjust_counters_in_tx",
    "adjust_reply_publication_counts_in_tx",
  ], "shared publication effects");
  return failures;
}

function verify() {

  const test = fs.readFileSync(
    "crates/modules/rustok-forum/tests/moderation_effect_contract_postgres.rs",
    "utf8",
  );
  const docs = fs.readFileSync(
    "crates/modules/rustok-forum/docs/forum-moderation-effect-postgres-contract.md",
    "utf8",
  );
  const adapter = fs.readFileSync(
    "crates/modules/rustok-forum/src/moderation_subject.rs",
    "utf8",
  );
  const publication = fs.readFileSync(
    "crates/modules/rustok-forum/src/services/reply_publication.rs", "utf8",
  );
  const matrix = fs.readFileSync(
    "crates/modules/rustok-forum/tests/support/moderation_effects.rs", "utf8",
  );
  const replyOwner = fs.readFileSync(
    "crates/modules/rustok-forum/src/services/reply_owner.rs",
    "utf8",
  );

  function requireText(source, needle, message) {
    if (!spaced(source).includes(spaced(needle))) throw new Error(message);
  }

  for (const marker of [
    "postgres_moderation_effects_preserve_accounting_tombstones_and_unpublished_boundary",
    "reject_publication_accounts_once_and_replays",
    "removed_solution_reply_tombstones_once_and_replays",
    "unpublished_visibility_fails_closed_without_forum_mutation",
    "ForumModerationSubjectAdapterFactory::reply()",
    "ModerationDecisionKind::RejectPublication",
    "ModerationDecisionEffectAction::RejectPublication",
    "ModerationDecisionKind::Remove",
    "ModerationVisibilityState::Removed",
    "ModerationDecisionKind::Unpublish",
    "ModerationVisibilityState::Unpublished",
    "forum.moderation_effect_unsupported",
    "assert_rejected_state",
    "assert_removed_solution_tombstone",
    "status_event_count",
    "owner_operation_receipts",
    "expected_solution_count",
  ]) {
    requireText(test, marker, `Forum moderation effect contract is missing ${marker}`);
  }

  for (const marker of [
    "apply_reply_rejected_effect_in_tx",
    "apply_reply_removed_effect_in_tx",
    "apply_reply_non_public_status_effect_in_tx",
    "if reply.status == target",
    "return Ok(false)",
    "forum.moderation_effect_unsupported",
    "idempotency::admit",
    "APPLY_MODERATION_DECISION_OPERATION",
  ]) {
    requireText(adapter, marker, `Forum moderation adapter invariant is missing ${marker}`);
  }

  for (const marker of [
    "pub(crate) async fn remove_in_tx",
    "forum_solution::Entity::delete_many()",
    "mark_reply_deleted_in_tx",
    "adjust_reply_count_in_tx",
    "adjust_solution_count_in_tx",
    "status = 'deleted'",
  ]) {
    requireText(replyOwner, marker, `Forum reply-removal owner invariant is missing ${marker}`);
  }

  for (const marker of [
    "RejectPublication",
    "Removed accepted solution",
    "Unpublished remains distinct",
    "status=deleted",
    "failed owner-operation receipt",
    "fail closed instead of being silently remapped",
    "cargo test -p rustok-forum --test moderation_effect_contract_postgres",
  ]) {
    requireText(docs, marker, `Forum moderation effect handoff is missing ${marker}`);
  }

  for (const marker of [
    "trusted_moderation_effects_are_atomic_on_sqlite", "trusted_moderation_effects_are_atomic_on_postgres",
    "for archived in [false, true]", "for solution in [false, true]",
    "ModerationDecisionKind::Hide", "ModerationDecisionKind::RejectPublication", "ModerationDecisionKind::Remove",
    "fail_status_publication", "reply_restore_checkpoint", "projection_count", "an exact decision retry returns its immutable receipt",
  ]) {
    requireText(matrix, marker, `Forum shared effects matrix is missing ${marker}`);
  }
  const failures = inspectEffects(adapter, publication);
  if (failures.length) throw new Error(failures.join("\n"));
  console.log("Forum moderation effects source guard passed; Rust execution remains required");
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) verify();
