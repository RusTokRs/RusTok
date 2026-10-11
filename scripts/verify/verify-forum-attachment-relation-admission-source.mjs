#!/usr/bin/env node

import fs from "node:fs";

// A missing input is a finding, not a crash: an uncaught ENOENT means none of the rules below ever
// run, and the exit code looks like tooling noise rather than a verdict.
function read(path) {
  if (!fs.existsSync(path)) {
    throw new Error(`Forum attachment relation admission source: missing required input ${path}`);
  }
  return fs.readFileSync(path, "utf8");
}

function need(text, marker, label) {
  if (!text.includes(marker)) throw new Error(`${label}: missing ${marker}`);
}

function forbid(text, marker, label) {
  if (text.includes(marker)) throw new Error(`${label}: forbidden ${marker}`);
}

// The admission-only boundary is a rule about production code. A file's own `#[cfg(test)]` module
// legitimately imports SeaORM and mints identifiers to exercise a pure function, so scanning it
// would report a violation that does not exist.
function production(text) {
  const tests = text.indexOf("#[cfg(test)]");
  return tests < 0 ? text : text.slice(0, tests);
}

const contract = read("crates/modules/rustok-forum/src/attachment_relation.rs");
const lib = read("crates/modules/rustok-forum/src/lib.rs");
const mediaPorts = read("crates/modules/rustok-media/src/ports.rs");
const mediaDto = read("crates/modules/rustok-media/src/dto.rs");
// `src/category_presentation.rs` was removed when Taxonomy took canonical Category presentation
// (CAT-5 cutover); the guard asked for a "persistence remains disabled" note inside that file, so
// the check could only pass by resurrecting a retired path. The contract is now the negative one.
const categoryPresentationPath = "crates/modules/rustok-forum/src/category_presentation.rs";
if (fs.existsSync(categoryPresentationPath)) {
  throw new Error(
    `Forum attachment relation admission source: retired Forum presentation owner ${categoryPresentationPath} must stay removed`,
  );
}
const packet = read(
  "docs/modules/forum-14-attachment-relation-admission-actualization-2026-08-10.md",
);
const forumPlan = read("crates/modules/rustok-forum/docs/implementation-plan.md");

for (const marker of [
  "pub mod attachment_relation;",
  "pub use attachment_relation::*;",
]) need(lib, marker, "FORUM-14A crate wiring");

for (const marker of [
  "pub const MAX_FORUM_ATTACHMENTS_PER_SET: usize = 32;",
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
  "request.attachments.len() > MAX_FORUM_ATTACHMENTS_PER_SET",
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
]) forbid(production(contract), marker, "FORUM-14A admission-only boundary");

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

// The boundary the retired file used to document is stated by the canonical plan and enforced by
// the Forum Media-facing code: category-cover quarantine/deletion lifecycle stays with Media, so
// Forum must not own a category-cover lifecycle service.
need(
  forumPlan,
  "Forum must not reintroduce a duplicate",
  "Forum category-cover Media lifecycle boundary in the canonical plan",
);
for (const forbidden of ["CategoryCoverService", "category_cover_media"]) {
  forbid(lib, forbidden, "Forum category-cover owner command must stay absent");
}

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
