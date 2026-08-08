import fs from "node:fs";

const test = fs.readFileSync(
  "crates/rustok-forum/tests/moderation_effect_contract_postgres.rs",
  "utf8",
);
const docs = fs.readFileSync(
  "crates/rustok-forum/docs/forum-moderation-effect-postgres-contract.md",
  "utf8",
);
const adapter = fs.readFileSync(
  "crates/rustok-forum/src/moderation_subject.rs",
  "utf8",
);
const replyOwner = fs.readFileSync(
  "crates/rustok-forum/src/services/reply_owner.rs",
  "utf8",
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
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

console.log("Forum moderation effect PostgreSQL source guard passed");
