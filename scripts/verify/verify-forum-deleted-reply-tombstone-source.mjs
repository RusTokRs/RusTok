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
  lib: "crates/rustok-forum/src/lib.rs",
  tombstone: "crates/rustok-forum/src/import_tombstone_preparation.rs",
  mapping: "crates/rustok-forum/src/import_mapping.rs",
  relation: "crates/rustok-forum/src/import_relation_preparation.rs",
  write: "crates/rustok-forum/src/services/import_write.rs",
  replyOwner: "crates/rustok-forum/src/services/reply_owner.rs",
  replyImport: "crates/rustok-forum/src/services/reply_owner_import.rs",
  pgSoftDelete: "crates/rustok-forum/src/migrations/m20260713_000009_add_forum_soft_delete_revisions/postgres_up.rs",
  sqliteRevisions: "crates/rustok-forum/src/migrations/m20260713_000009_add_forum_soft_delete_revisions/sqlite_revisions.rs",
  packet: "docs/modules/forum-34-deleted-reply-tombstone-actualization-2026-08-09.md",
};

const source = Object.fromEntries(
  Object.entries(files).map(([key, path]) => [key, read(path)]),
);

for (const marker of [
  "pub mod import_tombstone_preparation;",
  "pub use import_tombstone_preparation::*;",
]) need(source.lib, marker, "FORUM-34P root wiring");

for (const marker of [
  "pub const MAX_FORUM_IMPORT_REPLY_TOMBSTONES_PER_BATCH: usize =",
  "pub struct NodebbReplyTombstoneRecord",
  'serde(rename = "deletedTimestamp", alias = "deleted_timestamp")',
  "pub struct ForumImportReplyTombstoneFact",
  "pub struct ForumPreparedDeletedReplyTombstone",
  "pub struct ForumImportTombstonePreparationRequest",
  "pub struct ForumPreparedImportTombstoneBatch",
  "pub struct NodebbForumReplyTombstoneMapper",
  "pub struct ForumImportTombstonePreparer",
  "ForumImportEntityKind::Post",
  'format!("post:{}", record.pid)',
  "DateTime::<Utc>::from_timestamp_millis",
  "MissingDeletedReplyTombstone",
  "LiveReplyHasTombstone",
  "UnexpectedTombstone",
  "TombstonePredatesCreation",
  "relation.target != crate::mentions::ForumContentTarget::reply(reply.id)",
]) need(source.tombstone, marker, "FORUM-34P tombstone admission");

for (const marker of [
  "pub struct NodebbPostRecord",
  "pub deleted: bool",
]) need(source.mapping, marker, "FORUM-34A mapping baseline");
forbid(
  source.mapping,
  "deletedTimestamp",
  "canonical NodeBB post mapping must not pretend the optional sidecar is a core post field",
);

for (const marker of [
  "pub struct ForumPreparedImportRelationBatch",
  "pub writes: ForumPreparedImportWriteBatch",
]) need(source.relation, marker, "FORUM-34M relation batch baseline");

for (const marker of [
  "reply.status == ReplyStatus::Deleted",
  "tombstone timestamp is admitted",
]) need(source.write, marker, "FORUM-34O deleted-reply fail-closed baseline");
for (const marker of [
  "SET status = 'deleted', deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP",
  "remove_in_tx",
]) need(source.replyOwner, marker, "interactive reply owner delete baseline");
for (const marker of [
  "record.status == ReplyStatus::Deleted",
  "requires an admitted tombstone timestamp",
]) need(source.replyImport, marker, "FORUM-34O reply import boundary baseline");
for (const marker of [
  "forum_guard_deleted_reply_update",
  "revision_reason",
  "'delete'",
]) need(source.pgSoftDelete, marker, "Postgres delete-revision baseline");
for (const marker of [
  "forum_reply_delete_revision_update",
  "OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL",
  "'delete'",
]) need(source.sqliteRevisions, marker, "SQLite delete-revision baseline");

for (const marker of [
  "FORUM-34P",
  "does not guarantee a historical deletion timestamp",
  "does **not** change `NodebbPostRecord`",
  "separate explicit exporter/audit enrichment contract",
  "every `ReplyStatus::Deleted` reply to have exactly one tombstone",
  "34O atomic writer remains unchanged",
  "FORUM-34Q",
  "no tests, Cargo commands",
]) need(source.packet, marker, "FORUM-34P actualization");

for (const marker of [
  "sea_orm",
  "DatabaseConnection",
  "DatabaseTransaction",
  "TransactionTrait",
  "SecurityContext",
  "TransactionalEventBus",
  "Uuid::new_v4",
  ".insert(",
  ".update(",
  ".delete(",
  ".commit(",
]) forbid(source.tombstone, marker, "side-effect-free tombstone admission");

console.log("Forum FORUM-34P deleted reply tombstone admission source: ok");
