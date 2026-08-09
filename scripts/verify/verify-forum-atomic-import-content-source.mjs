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
  categoryImport: "crates/rustok-forum/src/services/category_import.rs",
  topicImport: "crates/rustok-forum/src/services/topic_import.rs",
  replyImport: "crates/rustok-forum/src/services/reply_owner_import.rs",
  importWrite: "crates/rustok-forum/src/services/import_write.rs",
  relationImport: "crates/rustok-forum/src/services/mention_relation_import.rs",
  categoryOwner: "crates/rustok-forum/src/services/category_projection_owner.rs",
  topicOwner: "crates/rustok-forum/src/services/topic_inline.rs",
  replyOwner: "crates/rustok-forum/src/services/reply_owner_inline.rs",
  packet: "docs/modules/forum-34-atomic-import-content-actualization-2026-08-09.md",
};

const source = Object.fromEntries(
  Object.entries(files).map(([key, path]) => [key, read(path)]),
);

for (const marker of [
  'include!("category_import.rs");',
  'mod import_write;',
  'include!("reply_owner_import.rs");',
  'include!("topic_import.rs");',
  'ForumImportWriteResult, ForumImportWriteService, MAX_FORUM_IMPORT_APPLY_RECORDS_PER_BATCH',
]) need(source.services, marker, "FORUM-34O service wiring");

for (const marker of [
  "pub const MAX_FORUM_IMPORT_APPLY_RECORDS_PER_BATCH: usize =",
  "pub struct ForumImportWriteResult",
  "pub struct ForumImportWriteService",
  "pub async fn apply_prepared_batch(",
  "PermissionScope::All",
  "Action::Manage",
  "normalize_locale_code(&batch.writes.locale)",
  "FORUM_IMPORT_SOURCE_NODEBB",
  "ForumImportEntityKind::Category",
  "ForumImportEntityKind::Topic",
  "ForumImportEntityKind::Post",
  "ForumImportEntityKind::User",
  "ForumImportRelationEventMode::SuppressAddedTargetEvents",
  "ForumImportRelationEventMode::EmitAddedTargetEvents",
  "ordered_category_indices",
  "ordered_reply_indices",
  "ensure_target_ids_absent_in_tx",
  "reply.status == ReplyStatus::Deleted",
  "let txn = self.db.begin().await?;",
  "publish_forum_projection_scope_direct_in_tx(",
  "txn.commit().await?;",
]) need(source.importWrite, marker, "FORUM-34O atomic import adapter");

const beginCount = source.importWrite.split("self.db.begin().await?").length - 1;
const commitCount = source.importWrite.split("txn.commit().await?").length - 1;
if (beginCount !== 1 || commitCount !== 1) {
  throw new Error(`atomic adapter transaction count begin=${beginCount} commit=${commitCount}`);
}

for (const marker of [
  "insert_import_category_in_tx(",
  "id: Set(record.id)",
  "lock_category_tree_in_tx(txn, tenant_id)",
  "shift_siblings_for_insert_in_tx(",
  "ensure_current_route_key_available_in_tx(",
]) need(source.categoryImport, marker, "FORUM-34O category owner primitive");

for (const marker of [
  "prepare_import_topic(",
  "validate_topic_title(&record.title)?;",
  "normalize_discussion(record.body.clone())?",
  "prepare_topic_custom_fields_for_create",
  "validate_normalized_topic_tags",
  "insert_import_topic_in_tx(",
  "id: Set(prepared.record.id)",
  "status: Set(TopicStatus::Open)",
  "is_locked: Set(false)",
  "persist_import_admitted_in_tx(",
  "sync_channel_access_in_tx(",
  "sync_topic_tags_in_tx(",
  "adjust_topic_count_in_tx",
  "DomainEvent::ForumTopicCreated",
  "finalize_import_topic_in_tx(",
]) need(source.topicImport, marker, "FORUM-34O topic owner primitive");

for (const marker of [
  "prepare_import_reply(",
  "record.status == ReplyStatus::Deleted",
  "requires an admitted tombstone timestamp",
  "insert_import_reply_in_tx(",
  "allocate_reply_position_in_tx(",
  "id: Set(prepared.record.id)",
  "persist_import_admitted_in_tx(",
  "prepared.record.status == ReplyStatus::Approved",
  "adjust_reply_count_in_tx",
  "DomainEvent::ForumTopicReplied",
]) need(source.replyImport, marker, "FORUM-34O reply owner primitive");

for (const marker of [
  "persist_import_admitted_in_tx(",
  "self.persist_in_tx(txn, prepared).await?",
]) need(source.relationImport, marker, "FORUM-34N relation bridge baseline");

for (const marker of [
  "ensure_current_route_key_available_in_tx",
  "publish_forum_projection_scope_direct_in_tx",
]) need(source.categoryOwner, marker, "category owner baseline");
for (const marker of [
  "validate_normalized_topic_tags",
  "sync_channel_access_in_tx",
  "sync_topic_tags_in_tx",
  "ForumTopicCreated",
]) need(source.topicOwner, marker, "topic owner baseline");
for (const marker of [
  "allocate_reply_position_in_tx",
  "status == ReplyStatus::Approved",
  "ForumTopicReplied",
]) need(source.replyOwner, marker, "reply owner baseline");

for (const marker of [
  "FORUM-34O",
  "FORUM-34A through FORUM-34N",
  "exact `PermissionScope::All`",
  "Provisional topic state",
  "only `ReplyStatus::Approved`",
  "always calls `publish_forum_projection_scope_direct_in_tx",
  "rejects every prepared `ReplyStatus::Deleted` before opening the write transaction",
  "one content transaction",
  "FORUM-34P",
  "no tests, Cargo commands",
]) need(source.packet, marker, "FORUM-34O packet");

for (const marker of [
  "checkpoint table",
  "receipt table",
  "replay journal table",
]) forbid(source.importWrite, marker, "Forum-local durable runner boundary");

// Imported entity UUIDs must come from admitted records. Internal owner rows
// intentionally retain their existing UUID allocation, so Uuid::new_v4 is not
// globally forbidden in the owner primitive files.
for (const [label, text] of [
  ["category", source.categoryImport],
  ["topic", source.topicImport],
  ["reply", source.replyImport],
]) {
  if (!text.includes("id: Set(record.id)") && !text.includes("id: Set(prepared.record.id)")) {
    throw new Error(`${label} imported entity ID is not visibly caller-admitted`);
  }
}

console.log("Forum FORUM-34O atomic prepared import content source: ok");
