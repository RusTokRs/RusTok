import fs from "node:fs";

const test = fs.readFileSync(
  "crates/rustok-forum/tests/moderation_revision_migration_contract.rs",
  "utf8",
);
const docs = fs.readFileSync(
  "crates/rustok-forum/docs/forum-moderation-revision-migration-contract.md",
  "utf8",
);
const migration = fs.readFileSync(
  "crates/rustok-forum/src/migrations/m20260807_000027_add_forum_moderation_subject_revisions.rs",
  "utf8",
);
const migrations = fs.readFileSync(
  "crates/rustok-forum/src/migrations/mod.rs",
  "utf8",
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  "sqlite_moderation_revision_migration_backfills_and_tracks_owner_changes",
  "sqlite_clean_install_initializes_moderation_revision_clocks",
  "postgres_moderation_revision_migration_clean_upgrade_and_trigger_contract",
  "install_forum_before_revision_migration",
  "ForumModule.migrations()",
  "assert_backfilled_revisions",
  "forum_topic_moderation_subject_revisions",
  "forum_reply_moderation_subject_revisions",
  "UPDATE forum_topics SET metadata",
  "UPDATE forum_topics SET is_locked = TRUE",
  "INSERT INTO forum_topic_translations",
  "UPDATE forum_topic_translations SET title",
  "DELETE FROM forum_topic_translations",
  "UPDATE forum_topics SET reply_count = reply_count",
  "UPDATE forum_replies SET status = 'hidden'",
  "INSERT INTO forum_reply_bodies",
  "UPDATE forum_reply_bodies SET body",
  "DELETE FROM forum_reply_bodies",
  "UPDATE forum_replies SET updated_at = updated_at",
  "assert_new_subject_initialization",
]) {
  requireText(test, marker, `Forum moderation revision migration contract is missing ${marker}`);
}

for (const marker of [
  "forum_topic_moderation_subject_revisions",
  "forum_reply_moderation_subject_revisions",
  "INSERT INTO forum_topic_moderation_subject_revisions",
  "INSERT INTO forum_reply_moderation_subject_revisions",
  "forum_initialize_topic_moderation_subject_revision",
  "forum_initialize_reply_moderation_subject_revision",
  "forum_bump_topic_moderation_subject_revision_on_owner_update",
  "forum_bump_reply_moderation_subject_revision_on_owner_update",
  "forum_bump_topic_moderation_subject_revision_on_translation_update",
  "forum_bump_reply_moderation_subject_revision_on_body_update",
]) {
  requireText(migration, marker, `Forum moderation revision migration source is missing ${marker}`);
}

requireText(
  migrations,
  "Box::new(m20260807_000027_add_forum_moderation_subject_revisions::Migration)",
  "Forum moderation revision migration must remain present in the production migration list",
);

for (const marker of [
  "Upgrade / backfill",
  "Clean install / new subjects",
  "Trigger parity",
  "current-state fencing clocks only",
  "never silent retargeting",
  "cargo test -p rustok-forum --test moderation_revision_migration_contract",
]) {
  requireText(docs, marker, `Forum moderation revision migration handoff is missing ${marker}`);
}

console.log("Forum moderation revision migration contract source guard passed");
