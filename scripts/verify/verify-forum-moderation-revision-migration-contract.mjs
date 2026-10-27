import fs from "node:fs";

const test = fs.readFileSync(
  "crates/modules/rustok-forum/tests/moderation_revision_migration_contract.rs",
  "utf8",
);
const docs = fs.readFileSync(
  "crates/modules/rustok-forum/docs/forum-moderation-revision-migration-contract.md",
  "utf8",
);
const migration = fs.readFileSync(
  "crates/modules/rustok-forum/src/migrations/m20260807_000027_add_forum_moderation_subject_revisions.rs",
  "utf8",
);
const migrations = fs.readFileSync(
  "crates/modules/rustok-forum/src/migrations/mod.rs",
  "utf8",
);

// Collect every drift instead of aborting on the first one: a guard that throws hides the rest of
// the contract behind whichever marker happened to move first.
const failures = [];

function requireText(source, needle, message) {
  if (!source.includes(needle)) failures.push(message);
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

// The upgrade harness resolves the revision migration by name and installs every migration listed
// before it, so the contract is "listed exactly once and reachable by name" — not "last in the
// list", which stopped holding as soon as later forum migrations were appended after it.
const migrationList = migrations.match(/pub fn migrations\(\)[\s\S]*?\n}\n/)?.[0];
if (!migrationList) {
  failures.push("Forum production migration list could not be located");
} else {
  const revisionEntries =
    migrationList.match(
      /Box::new\(m20260807_000027_add_forum_moderation_subject_revisions::Migration\)/g,
    ) ?? [];
  if (revisionEntries.length !== 1) {
    failures.push(
      `Forum moderation revision migration must be registered exactly once, found ${revisionEntries.length}`,
    );
  }
  for (const needle of [
    'position(|m| m.name().contains("add_forum_moderation_subject_revisions"))',
    "migrations.drain(..index)",
    "migrations.remove(0)",
  ]) {
    requireText(
      test,
      needle,
      `Forum moderation upgrade harness must resolve the revision migration by name: missing ${needle}`,
    );
  }
}

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

if (failures.length > 0) {
  console.error("Forum moderation revision migration contract source guard failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum moderation revision migration contract source guard passed");
