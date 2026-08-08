import fs from "node:fs";

const test = fs.readFileSync(
  "crates/rustok-forum/tests/moderation_terminal_concurrency_postgres.rs",
  "utf8",
);
const docs = fs.readFileSync(
  "crates/rustok-forum/docs/forum-moderation-terminal-concurrency-contract.md",
  "utf8",
);
const adapter = fs.readFileSync(
  "crates/rustok-forum/src/moderation_subject.rs",
  "utf8",
);
const revisionMigration = fs.readFileSync(
  "crates/rustok-forum/src/migrations/m20260807_000027_add_forum_moderation_subject_revisions.rs",
  "utf8",
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  "postgres_concurrent_reply_edits_fence_reject_and_remove",
  "reply_body_edit_fences_reject_publication",
  "reply_body_edit_fences_remove_with_accepted_solution",
  "ForumModerationSubjectAdapterFactory::reply()",
  "UPDATE forum_reply_bodies SET body",
  "wait_for_processing_receipt",
  "owner_operation_receipts",
  'status == "processing"',
  "assert_application_waits_while_edit_owns_revision",
  "ModerationDecisionKind::RejectPublication",
  "ModerationDecisionEffectAction::RejectPublication",
  "ModerationDecisionKind::Remove",
  "ModerationVisibilityState::Removed",
  "forum.moderation_subject_revision_conflict",
  "forum.moderation_database_unavailable",
  "expected_solution_count",
  "solution_rows",
  "status_events",
]) {
  requireText(test, marker, `Forum terminal concurrency contract is missing ${marker}`);
}

for (const marker of [
  "IsolationLevel::Serializable",
  "FOR UPDATE",
  "apply_reply_rejected_effect_in_tx",
  "apply_reply_removed_effect_in_tx",
  "forum.moderation_subject_revision_conflict",
  "reviewed_revision != command.subject.revision",
]) {
  requireText(adapter, marker, `Forum adapter terminal concurrency invariant is missing ${marker}`);
}

for (const marker of [
  "forum_reply_moderation_subject_revision_body_update",
  "forum_bump_reply_moderation_subject_revision_on_body_update",
]) {
  requireText(
    revisionMigration,
    marker,
    `Forum reply-body revision trigger invariant is missing ${marker}`,
  );
}

for (const marker of [
  "Deterministic overlap",
  "RejectPublication race",
  "Remove + accepted solution race",
  "no-partial-effect guarantee",
  "forum.moderation_subject_revision_conflict",
  "cargo test -p rustok-forum --test moderation_terminal_concurrency_postgres",
]) {
  requireText(docs, marker, `Forum terminal concurrency handoff is missing ${marker}`);
}

console.log("Forum moderation reject/remove PostgreSQL concurrency source guard passed");
