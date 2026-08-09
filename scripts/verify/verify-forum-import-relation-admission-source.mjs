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
  mapping: "crates/rustok-forum/src/import_mapping.rs",
  writePreparation: "crates/rustok-forum/src/import_write_preparation.rs",
  relationPreparation: "crates/rustok-forum/src/import_relation_preparation.rs",
  mentions: "crates/rustok-forum/src/mentions.rs",
  relationOwner: "crates/rustok-forum/src/services/mention_relation.rs",
  categoryProjectionOwner: "crates/rustok-forum/src/services/category_projection_owner.rs",
  packet: "docs/modules/forum-34-import-relation-admission-actualization-2026-08-09.md",
};

const source = Object.fromEntries(
  Object.entries(files).map(([key, path]) => [key, read(path)]),
);

for (const marker of [
  "pub mod import_relation_preparation;",
  "pub use import_relation_preparation::*;",
]) need(source.lib, marker, "forum import relation preparation export");

for (const marker of [
  "pub const MAX_FORUM_IMPORT_RELATION_TARGETS_PER_BATCH: usize =",
  "MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH",
  "pub enum ForumImportRelationMode",
  "SuppressRelations",
  "MaterializeRelations",
  "pub enum ForumImportRelationEventMode",
  "SuppressAddedTargetEvents",
  "EmitAddedTargetEvents",
  "pub struct ForumImportMentionBinding",
  "pub struct ForumImportContentRelationDecision",
  "pub struct ForumImportRelationPreparationRequest",
  "pub struct ForumPreparedImportMention",
  "pub struct ForumPreparedImportContentRelations",
  "pub struct ForumPreparedImportRelationBatch",
  "pub enum ForumImportRelationPreparationError",
  "pub struct ForumImportRelationPreparer",
  "pub fn prepare(",
  "normalize_locale_code(&batch.locale)",
  "extract_forum_mention_candidates(document, policy)",
  "ProfileService::normalize_handle",
  "FORUM_MAX_MENTION_TARGETS_PER_REVISION",
  "FORUM_MAX_QUOTE_REFERENCES_PER_REVISION",
  "QuoteRelationsUnsupported",
  "SuppressedRelationsContainFacts",
  "MentionHandleMismatch",
  "MentionAudienceMismatch",
  "EventModeRequiresMaterialization",
  "ForumImportWriteEventMode::SuppressInteractiveEvents",
  "ForumImportWriteEventMode::EmitDomainEvents",
  "ForumContentTarget::topic(record.id)",
  "ForumContentTarget::reply(record.id)",
]) need(source.relationPreparation, marker, "FORUM-34M relation admission source");

for (const marker of [
  "sea_orm",
  "DatabaseConnection",
  "DatabaseTransaction",
  "TransactionalEventBus",
  "SecurityContext",
  "MentionRelationService",
  ".insert(",
  ".update(",
  ".delete(",
  "Uuid::new_v4",
  "Serialize",
  "Deserialize",
  "#[serde(",
]) forbid(source.relationPreparation, marker, "relation admission side-effect/non-wire boundary");

for (const marker of [
  "pub enum ForumImportWriteEventMode",
  "SuppressInteractiveEvents",
  "EmitDomainEvents",
  "pub struct ForumPreparedImportWriteBatch",
  "pub struct ForumPreparedImportTopic",
  "pub struct ForumPreparedImportReply",
]) need(source.writePreparation, marker, "34L write preparation baseline");

for (const marker of [
  "FORUM_MAX_MENTION_TARGETS_PER_REVISION: usize = 32",
  "FORUM_MAX_QUOTE_REFERENCES_PER_REVISION: usize = 32",
  "pub struct ForumMentionCandidates",
  "pub fn extract_forum_mention_candidates",
  "ForumMentionAudience",
]) need(source.mentions, marker, "Forum mention baseline");

for (const marker of [
  "pub(crate) async fn prepare(",
  "security: &SecurityContext",
  "publish_added_target_events_in_tx",
  "persist_in_tx",
]) need(source.relationOwner, marker, "current owner relation baseline");

for (const marker of [
  "publish_forum_projection_scope_direct_in_tx",
  "ensure_current_route_key_available_in_tx",
]) need(source.categoryProjectionOwner, marker, "category owner projection baseline");

for (const marker of [
  "FORUM_IMPORT_SOURCE_NODEBB",
  "MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH: usize = 512",
]) need(source.mapping, marker, "NodeBB mapping baseline");

for (const marker of [
  "FORUM-34M",
  "FORUM-34A through FORUM-34L",
  "relation-admission gap",
  "does not call `ProfilesReader`",
  "does not use `SecurityContext`",
  "rejects every non-empty quote set",
  "SuppressInteractiveEvents` -> `SuppressAddedTargetEvents",
  "Projection invalidation is not an interactive event",
  "34A NodeBB mapping -> 34B/34C inspection -> 34K identity/application resolution -> 34L owner-write preparation -> 34M exact relation admission",
  "FORUM-34N",
  "no tests, Cargo commands",
]) need(source.packet, marker, "FORUM-34M packet");

const libWiringCount = source.lib.split("import_relation_preparation").length - 1;
if (libWiringCount !== 2) {
  throw new Error(`forum import relation preparation wiring count: ${libWiringCount}`);
}

console.log("Forum FORUM-34M bounded import relation admission source: ok");
