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
  resolution: "crates/rustok-forum/src/import_resolution.rs",
  preparation: "crates/rustok-forum/src/import_write_preparation.rs",
  packet: "docs/modules/forum-34-import-write-preparation-actualization-2026-08-09.md",
};

const source = Object.fromEntries(
  Object.entries(files).map(([key, path]) => [key, read(path)]),
);

for (const marker of [
  "pub mod import_write_preparation;",
  "pub use import_write_preparation::*;",
]) need(source.lib, marker, "forum import write preparation export");

for (const marker of [
  "pub const MAX_FORUM_IMPORT_WRITE_RECORDS_PER_BATCH: usize =",
  "MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH",
  "pub enum ForumImportWriteEventMode",
  "SuppressInteractiveEvents",
  "EmitDomainEvents",
  "pub struct ForumImportCategoryWriteDecision",
  "pub struct ForumImportTopicWriteDecision",
  "pub struct ForumImportReplyWriteDecision",
  "pub struct ForumImportWritePreparationRequest",
  "pub struct ForumPreparedImportCategory",
  "pub struct ForumPreparedImportTopic",
  "pub struct ForumPreparedImportReply",
  "pub struct ForumPreparedImportWriteBatch",
  "pub enum ForumImportWritePreparationError",
  "pub struct ForumImportWritePreparer",
  "pub fn prepare(",
  "normalize_locale_code(&batch.locale)",
  "EmptyBatch",
  "TooManyRecords",
  "LocaleNotNormalized",
  "DuplicateTargetId",
  "NilAuthorId",
  "DuplicateDecision",
  "MissingDecision",
  "UnexpectedDecision",
  "EmptyCategorySlug",
  "SourceCategoryPositionOutOfRange",
  "CategoryPositionChanged",
  "TimestampChanged",
  "DeletedReplyStatusRequired",
  "LiveReplyCannotBeDeleted",
  "CategoryParentOutsideBatch",
  "CategoryCycle",
  "TopicCategoryOutsideBatch",
  "ReplyTopicOutsideBatch",
  "ReplyParentOutsideBatch",
  "ReplyParentTopicMismatch",
  "ReplySelfParent",
  "ReplyStatus::Deleted",
  "body: decision.body.clone()",
  "content: decision.content.clone()",
  "event_mode: request.event_mode",
]) need(source.preparation, marker, "FORUM-34L preparation source");

for (const marker of [
  "sea_orm",
  "DatabaseConnection",
  "DatabaseTransaction",
  "Entity::",
  "ActiveModel",
  "TransactionalEventBus",
  "SecurityContext",
  "CategoryService",
  "TopicService",
  "ReplyService",
  "Uuid::new_v4",
  "Serialize",
  "Deserialize",
  "#[serde(",
  ".insert(",
  ".update(",
  ".delete(",
]) forbid(source.preparation, marker, "preparation side-effect/non-wire boundary");

for (const marker of [
  "pub struct ForumResolvedImportApplicationBatch",
  "pub struct ForumResolvedImportCategory",
  "pub struct ForumResolvedImportTopic",
  "pub struct ForumResolvedImportReply",
  "ForumImportTargetIdentityKind",
]) need(source.resolution, marker, "34K resolution baseline");

for (const marker of [
  "FORUM-34L",
  "FORUM-34A through FORUM-34K",
  "owner-write preparation",
  "NodeBB category mapping has no owner-required category slug",
  "raw source text, not an admitted `RichTextDocument`",
  "at most `MAX_FORUM_IMPORT_WRITE_RECORDS_PER_BATCH = 512`",
  "does not generate any entity UUID",
  "Cross-batch dependency assembly remains a shared-runner concern",
  "`ForumImportWriteEventMode` is explicit and has no default",
  "imports no SeaORM/database types",
  "34A NodeBB mapping -> 34B/34C inspection -> 34K identity/application resolution -> 34L owner-write preparation",
  "FORUM-34M",
  "no tests, Cargo commands",
]) need(source.packet, marker, "FORUM-34L packet");

const libWiringCount = source.lib.split("import_write_preparation").length - 1;
if (libWiringCount !== 2) {
  throw new Error(`forum import write preparation wiring count: ${libWiringCount}`);
}

console.log("Forum FORUM-34L bounded import write preparation source: ok");
