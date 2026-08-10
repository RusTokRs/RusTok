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

const contract = read("crates/rustok-forum/src/attachment_relation.rs");
const lib = read("crates/rustok-forum/src/lib.rs");
const mediaPorts = read("crates/rustok-media/src/ports.rs");
const mediaDto = read("crates/rustok-media/src/dto.rs");
const categoryPresentation = read("crates/rustok-forum/src/category_presentation.rs");
const packet = read(
  "docs/modules/forum-14-attachment-relation-admission-actualization-2026-08-10.md",
);

for (const marker of [
  "pub mod attachment_relation;",
  "pub use attachment_relation::*;",
]) need(lib, marker, "FORUM-14A crate wiring");

for (const marker of [
  "pub const MAX_FORUM_ATTACHMENTS_PER_REVISION: usize = 32;",
  "pub const MAX_FORUM_ATTACHMENT_CAPTION_BYTES: usize = 512;",
  "pub enum ForumAttachmentUsage",
  "Inline",
  "Attachment",
  "pub struct ForumAttachmentRelationAdmissionRequest",
  "pub tenant_id: Uuid",
  "pub target: ForumContentTarget",
  "pub source_revision: u64",
  "pub locale: String",
  "pub struct ForumAttachmentSourceRevision",
  "pub struct ForumPreparedAttachmentRelation",
  "pub struct ForumPreparedAttachmentRelationBatch",
  "pub struct ForumAttachmentRelationPreparer",
  "request.target.id().is_nil()",
  "request.source_revision == 0",
  "normalize_locale_tag(&request.locale)",
  "request.attachments.len() > MAX_FORUM_ATTACHMENTS_PER_REVISION",
  "relation.media_id.is_nil()",
  "DuplicatePosition",
  "NonContiguousPositions",
  "attachments.sort_by_key(|relation| relation.position)",
  "caption.trim().to_string()",
  "caption.chars().any(char::is_control)",
]) need(contract, marker, "FORUM-14A attachment admission contract");

for (const marker of [
  "DuplicateMediaId",
  "MediaAssetReadPort",
  "MediaAssetWritePort",
  "DatabaseConnection",
  "DatabaseTransaction",
  "sea_orm",
  "TransactionalEventBus",
  "Uuid::new_v4",
  "storage_path",
  "public_url",
]) forbid(contract, marker, "FORUM-14A admission-only boundary");

for (const marker of [
  "async fn get_asset",
  "async fn list_assets",
  "async fn get_image_descriptor",
  "async fn get_translations",
]) need(mediaPorts, marker, "Media public read baseline");
for (const marker of [
  "pub struct MediaItem",
  "pub tenant_id: Uuid",
  "pub metadata: serde_json::Value",
]) need(mediaDto, marker, "Media item baseline");

need(
  categoryPresentation,
  "Persistence remains disabled until Media publishes",
  "Forum category-cover Media lifecycle baseline",
);
need(
  categoryPresentation,
  "quarantine and deletion lifecycle state",
  "Forum category-cover Media lifecycle baseline",
);

for (const marker of [
  "FORUM-14A",
  "admission-only",
  "repeated use of the same Media asset",
  "does **not** call Media",
  "does not yet unblock attachment reconciliation",
  "FORUM-14B",
  "no Cargo command, test, Node verifier",
]) need(packet, marker, "FORUM-14A actualization");

console.log("Forum FORUM-14A attachment relation admission source: ok");
