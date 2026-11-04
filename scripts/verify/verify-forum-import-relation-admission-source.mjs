#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

// Collect every drift instead of aborting on the first one: a guard that throws hides the rest of
// the contract behind whichever marker happened to move first.
const failures = [];

function need(text, marker, label) {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function forbid(text, marker, label) {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const files = {
  lib: "crates/modules/rustok-forum/src/lib.rs",
  mapping: "crates/modules/rustok-forum/src/import_mapping.rs",
  writePreparation: "crates/modules/rustok-forum/src/import_write_preparation.rs",
  relationPreparation: "crates/modules/rustok-forum/src/import_relation_preparation.rs",
  mentions: "crates/modules/rustok-forum/src/mentions.rs",
  relationOwner: "crates/modules/rustok-forum/src/services/mention_relation.rs",
  categoryProjectionOwner: "crates/modules/rustok-forum/src/services/category_projection_owner.rs",
};

const source = Object.fromEntries(
  Object.entries(files).map(([key, path]) => [key, read(path)]),
);

for (const marker of [
  "pub mod import_relation_preparation;",
  "pub use import_relation_preparation::{",
  "ForumImportRelationPreparer",
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
  ".insert(&",
  ".update(&",
  ".delete(&",
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


const libWiringCount = source.lib.split("import_relation_preparation").length - 1;
if (libWiringCount !== 2) {
  throw new Error(`forum import relation preparation wiring count: ${libWiringCount}`);
}

if (failures.length > 0) {
  console.error("Forum source guard failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum FORUM-34M bounded import relation admission source: ok");
