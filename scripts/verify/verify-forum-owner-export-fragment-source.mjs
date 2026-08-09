#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function requireText(text, marker, message) {
  if (!text.includes(marker)) throw new Error(message);
}

function requireAbsent(text, marker, message) {
  if (text.includes(marker)) throw new Error(message);
}

const mappingPath = "crates/rustok-forum/src/export_mapping.rs";
const libPath = "crates/rustok-forum/src/lib.rs";
const packetPath = "docs/modules/forum-34-owner-export-fragment-actualization-2026-08-09.md";

const mapping = read(mappingPath);
const lib = read(libPath);
const packet = read(packetPath);

for (const marker of [
  'pub const FORUM_EXPORT_SCHEMA_V1: &str = "rustok.forum.export.v1";',
  "pub const MAX_FORUM_EXPORT_OWNER_VIEWS_PER_FRAGMENT: usize = 512;",
  "pub struct ForumExportOwnerViewBatch",
  "pub struct ForumExportUserRef",
  "pub struct ForumExportCategoryRecord",
  "pub struct ForumExportTopicRecord",
  "pub struct ForumExportReplyRecord",
  "pub struct ForumExportFragment",
  "pub enum ForumExportMappingError",
  "pub struct ForumOwnerExportMapper;",
  "pub fn map_fragment(",
  "view.effective_locale",
  "view.body.document.clone()",
  "view.content.document.clone()",
  "export_user_ref(view.author_id)",
  "view.solution_reply_id",
  "DuplicateLocalizedView",
  "BTreeSet<(Uuid, String)>",
]) {
  requireText(mapping, marker, `${mappingPath}: missing ${marker}`);
}

for (const marker of ["pub mod export_mapping;", "pub use export_mapping::*;"]) {
  requireText(lib, marker, `${libPath}: missing public export mapping marker ${marker}`);
}

for (const forbidden of [
  "sea_orm",
  "DatabaseConnection",
  "DatabaseTransaction",
  "Entity::",
  "ActiveModel",
  "async fn",
  ".await",
  "Service::new",
  "PortContext",
  "SecurityContext",
  "register_runtime_extensions",
  "std::fs",
  "reqwest",
  "INSERT ",
  "UPDATE ",
  "DELETE ",
  ".insert(",
  ".update(",
  ".delete(",
]) {
  requireAbsent(
    mapping,
    forbidden,
    `${mappingPath}: export fragment mapping must remain side-effect free: ${forbidden}`,
  );
}

for (const forbiddenOutputField of [
  "pub html:",
  "pub body_plain_text:",
  "pub content_plain_text:",
  "pub content_preview:",
  "pub vote_score:",
  "pub current_user_vote:",
  "pub is_subscribed:",
  "pub topic_count:",
  "pub reply_count:",
  "pub is_solution:",
]) {
  requireAbsent(
    mapping,
    forbiddenOutputField,
    `${mappingPath}: export records must not promote viewer/derived field ${forbiddenOutputField}`,
  );
}

for (const marker of [
  "FORUM-34D",
  "shared-runner-blocked",
  "locale-enumeration-open",
  "ReplyReadModel` contains only `content_preview`",
  "already-authorized full owner responses",
  "uses `effective_locale`, never `requested_locale`",
  "does **not** claim multilingual export completeness",
  "`ReplyResponse` currently does not",
  "canonical `RichTextDocument`",
  "does not declare that a source RusTok user UUID can be reused",
  "no test, Cargo command",
]) {
  requireText(packet, marker, `${packetPath}: missing ${marker}`);
}

console.log("Forum FORUM-34D owner export fragment source: ok");
