#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function need(text, marker, label) {
  if (!text.includes(marker)) throw new Error(`${label}: missing ${marker}`);
}

function forbid(text, marker, label) {
  if (text.includes(marker)) throw new Error(`${label}: forbidden ${marker}`);
}

const files = {
  services: "crates/rustok-forum/src/services/mod.rs",
  tombstoneAdmission: "crates/rustok-forum/src/import_tombstone_preparation.rs",
  writer: "crates/rustok-forum/src/services/import_tombstone_write.rs",
  legacyWriter: "crates/rustok-forum/src/services/import_write.rs",
  replyImport: "crates/rustok-forum/src/services/reply_owner_tombstone_import.rs",
  legacyReplyImport: "crates/rustok-forum/src/services/reply_owner_import.rs",
  replyOwner: "crates/rustok-forum/src/services/reply_owner.rs",
  relationImport: "crates/rustok-forum/src/services/mention_relation_import.rs",
  relationOwner: "crates/rustok-forum/src/services/mention_relation.rs",
  postgres: "crates/rustok-forum/src/migrations/m20260713_000009_add_forum_soft_delete_revisions/postgres_up.rs",
  sqlite: "crates/rustok-forum/src/migrations/m20260713_000009_add_forum_soft_delete_revisions/sqlite_revisions.rs",
  packet: "docs/modules/forum-34-deleted-reply-persistence-actualization-2026-08-10.md",
};

const source = Object.fromEntries(
  Object.entries(files).map(([key, path]) => [key, read(path)]),
);

for (const marker of [
  'include!("import_tombstone_write.rs");',
  'include!("import_write.rs");',
  'include!("reply_owner_tombstone_import.rs");',
  'include!("reply_owner_import.rs");',
]) need(source.services, marker, "FORUM-34Q include wiring");

for (const marker of [
  "pub async fn apply_prepared_tombstone_batch",
  "ForumPreparedImportTombstoneBatch",
  "revalidate_prepared_tombstone_batch",
  "ForumImportTombstonePreparer",
  "validate_tombstone_apply_shape",
  "prepare_import_reply_with_tombstone",
  "insert_import_reply_with_tombstone_in_tx",
  "approved_reply_aggregates",
  "publish_forum_projection_scope_direct_in_tx",
  "txn.commit().await?",
]) need(source.writer, marker, "FORUM-34Q atomic tombstone writer");

for (const marker of [
  "pub async fn apply_prepared_batch",
  "Forum import deleted replies remain blocked until a tombstone timestamp is admitted",
]) need(source.legacyWriter, marker, "FORUM-34O legacy fail-closed entrypoint");

for (const marker of [
  "pub struct ForumPreparedImportTombstoneBatch",
  "pub struct ForumPreparedDeletedReplyTombstone",
  "MissingDeletedReplyTombstone",
  "LiveReplyHasTombstone",
  "TombstonePredatesCreation",
]) need(source.tombstoneAdmission, marker, "FORUM-34P tombstone admission baseline");

for (const marker of [
  "prepare_import_reply_with_tombstone",
  "insert_import_reply_with_tombstone_in_tx",
  "ReplyStatus::Deleted",
  "SuppressAddedTargetEvents",
  "persist_import_reply_tombstone_in_tx",
  "count_import_delete_revisions_in_tx",
  "SET deleted_at = $1, updated_at = $1",
  "SET deleted_at = ?, updated_at = ?",
  "status = 'deleted' AND deleted_at IS NULL",
  "revision_reason = 'delete'",
  "SET created_at = $1",
  "SET created_at = ?",
  "Forum import reply tombstone must create exactly one delete revision",
  "Historical reconstruction differs from interactive create",
]) need(source.replyImport, marker, "FORUM-34Q deleted reply owner primitive");

for (const marker of [
  "pub(crate) fn prepare_import_reply",
  "pub(crate) async fn insert_import_reply_in_tx",
  "record.status == ReplyStatus::Deleted",
  "requires an admitted tombstone timestamp",
]) need(source.legacyReplyImport, marker, "FORUM-34O live reply primitive baseline");

for (const marker of [
  "persist_import_admitted_in_tx",
  "SuppressAddedTargetEvents",
]) need(source.relationImport, marker, "FORUM-34N relation bridge baseline");

for (const marker of [
  "lock_source_in_tx",
  "ensure_prepared_matches_source_in_tx",
]) need(source.relationOwner, marker, "Forum relation owner persistence baseline");

for (const marker of [
  "forum_guard_deleted_reply_update",
  "forum_reply_revisions",
  "revision_reason",
  "'delete'",
]) need(source.postgres, marker, "Postgres delete revision trigger baseline");
for (const marker of [
  "forum_reply_delete_revision_update",
  "OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL",
  "forum_reply_revisions",
  "'delete'",
]) need(source.sqlite, marker, "SQLite delete revision trigger baseline");

for (const marker of [
  "FORUM-34Q",
  "apply_prepared_tombstone_batch",
  "exactly one delete revision",
  "no `ForumTopicReplied` event",
  "no `ForumReplyStatusChanged` event",
  "Historical child -> deleted-parent relationships are allowed",
  "relation revisions/mention rows",
  "shared owner-data migration runner",
  "no tests, Cargo commands",
]) need(source.packet, marker, "FORUM-34Q actualization");

for (const marker of [
  "CURRENT_TIMESTAMP",
  "Utc::now()",
  "ForumTopicReplied",
  "ForumReplyStatusChanged",
  "adjust_reply_count_in_tx",
  "adjust_counters_in_tx",
  "UserStatsService::adjust_reply_count_in_tx",
  ".begin(",
  ".commit(",
]) forbid(source.replyImport, marker, "deleted reply historical owner primitive");

forbid(
  source.replyImport,
  "parent.status == ReplyStatus::Deleted",
  "historical deleted-parent reconstruction",
);

for (const marker of [
  "deletedTimestamp",
  "deleted_at_ms",
]) forbid(
  read("crates/rustok-forum/src/import_mapping.rs"),
  marker,
  "canonical NodeBB mapping must remain tombstone-sidecar free",
);

console.log("Forum FORUM-34Q deleted reply persistence source: ok");
