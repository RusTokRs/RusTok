import fs from "node:fs";

const test = fs.readFileSync(
  "crates/rustok-forum/tests/moderation_revision_concurrency_postgres.rs",
  "utf8",
);
const docs = fs.readFileSync(
  "crates/rustok-forum/docs/forum-moderation-revision-concurrency-contract.md",
  "utf8",
);
const adapter = fs.readFileSync(
  "crates/rustok-forum/src/moderation_subject.rs",
  "utf8",
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  "postgres_concurrent_content_edits_fence_topic_lock_and_reply_hide",
  "topic_translation_edit_fences_permanent_lock",
  "reply_body_edit_fences_hide_application",
  "PostgresForumTestDb::setup",
  "ForumModerationSubjectAdapterFactory::topic()",
  "ForumModerationSubjectAdapterFactory::reply()",
  "UPDATE forum_topic_translations SET title",
  "UPDATE forum_reply_bodies SET body",
  "wait_for_processing_receipt",
  "owner_operation_receipts",
  "status == \"processing\"",
  "assert_application_waits_while_edit_owns_revision",
  "forum.moderation_subject_revision_conflict",
  "forum.moderation_database_unavailable",
  "ModerationDecisionEffectAction::Lock",
  "ModerationVisibilityState::Hidden",
  "reviewed_revision + 1",
  "assert_public_reply_accounting",
  "count_reply_status_events",
]) {
  requireText(test, marker, `Forum moderation concurrency contract is missing ${marker}`);
}

for (const marker of [
  "IsolationLevel::Serializable",
  "FOR UPDATE",
  "forum.moderation_subject_revision_conflict",
  "reviewed_revision != command.subject.revision",
  "idempotency::admit",
  "APPLY_MODERATION_DECISION_OPERATION",
]) {
  requireText(adapter, marker, `Forum moderation adapter concurrency invariant is missing ${marker}`);
}

for (const marker of [
  "Overlap construction",
  "Safe PostgreSQL outcomes",
  "owner_operation_receipts",
  "same stale reviewed revision",
  "Topic lock assertions",
  "Reply hide assertions",
  "fail-closed",
  "cargo test -p rustok-forum --test moderation_revision_concurrency_postgres",
]) {
  requireText(docs, marker, `Forum moderation concurrency handoff is missing ${marker}`);
}

console.log("Forum moderation revision PostgreSQL concurrency source guard passed");
