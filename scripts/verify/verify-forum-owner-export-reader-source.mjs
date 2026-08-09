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
const readerPath = "crates/rustok-forum/src/export_reader.rs";
const packetPath = "docs/modules/forum-34-owner-export-reader-actualization-2026-08-09.md";

const mapping = read(mappingPath);
const reader = read(readerPath);
const packet = read(packetPath);

for (const marker of [
  '#[path = "export_reader.rs"]',
  "mod reader;",
  "pub use reader::*;",
]) {
  requireText(mapping, marker, `${mappingPath}: missing reader composition marker ${marker}`);
}

for (const marker of [
  "pub const MAX_FORUM_EXPORT_READ_TARGETS_PER_FRAGMENT: usize =",
  "MAX_FORUM_EXPORT_OWNER_VIEWS_PER_FRAGMENT;",
  "pub enum ForumExportReadTargetKind",
  "pub struct ForumExportReadTarget",
  "pub struct ForumExportReadBatch",
  "pub enum ForumExportReadError",
  "pub struct ForumOwnerExportReader;",
  "pub async fn read_fragment(",
  "if security.is_public_read()",
  "ForumExportReadError::OperatorContextRequired",
  "ForumExportReadError::EmptyTargets",
  "security.get_scope(kind.resource(), Action::Manage)",
  'Self::Category => "forum_categories"',
  'Self::Topic => "forum_topics"',
  'Self::Reply => "forum_replies"',
  "MAX_FORUM_EXPORT_READ_TARGETS_PER_FRAGMENT",
  "normalize_locale_code(&target.locale)",
  "!seen.insert((target.kind, target.id, locale.clone()))",
  ".get_with_locale_fallback(",
  "response.id",
  "response.effective_locale",
  "OwnerIdentityChanged",
  "LocaleNotStored",
  "ForumExportOwnerViewBatch",
  "ForumOwnerExportMapper.map_fragment(&owner_views)",
]) {
  requireText(reader, marker, `${readerPath}: missing ${marker}`);
}

const targetStart = reader.indexOf(
  "#[derive(Clone, Debug, Eq, PartialEq)]\npub struct ForumExportReadTarget",
);
const errorStart = reader.indexOf("#[derive(Debug, Error)]", targetStart);
if (targetStart < 0 || errorStart <= targetStart) {
  throw new Error(`${readerPath}: read-batch non-wire boundary is invalid`);
}
const batchContract = reader.slice(targetStart, errorStart);
for (const forbidden of ["Serialize", "Deserialize", "#[serde("]) {
  requireAbsent(
    batchContract,
    forbidden,
    `${readerPath}: export read targets must remain non-wire in-process types: ${forbidden}`,
  );
}

for (const forbidden of [
  "sea_orm",
  "DatabaseConnection",
  "DatabaseTransaction",
  "crate::entities",
  "Entity::",
  "QueryFilter",
  ".all(&",
  ".one(&",
  ".insert(",
  ".update(",
  ".delete(",
  "ForumExportCategoryRecord {",
  "ForumExportTopicRecord {",
  "ForumExportReplyRecord {",
]) {
  requireAbsent(
    reader,
    forbidden,
    `${readerPath}: export reader must compose owner services and mapper without direct storage/schema duplication: ${forbidden}`,
  );
}

const ownerReadCalls = reader.split(".get_with_locale_fallback(").length - 1;
if (ownerReadCalls !== 3) {
  throw new Error(
    `${readerPath}: expected exactly three typed owner read branches, found ${ownerReadCalls}`,
  );
}

const mapperCalls = reader.split("ForumOwnerExportMapper.map_fragment(&owner_views)").length - 1;
if (mapperCalls !== 1) {
  throw new Error(`${readerPath}: reader must delegate export field mapping exactly once`);
}

for (const marker of [
  "FORUM-34F",
  "FORUM-34A through FORUM-34E",
  "still finds no generic `ImportRunner`, `ExportRunner`, `ImportAdapter`, `ExportAdapter` or `rustok-import`",
  "canonical Forum ledger still labels `FORUM-34` as `planned`",
  "do not derive `Serialize` or `Deserialize`",
  "at most 512 localized targets",
  "category targets require `forum_categories:manage`",
  "topic targets require `forum_topics:manage`",
  "reply targets require `forum_replies:manage`",
  "has no SeaORM/database/entity dependency",
  "normalized `effective_locale` equals the normalized requested locale",
  "prevents a fallback read from silently fabricating multilingual export completeness",
  "prevents Topic canonical resolution from silently exporting a different topic identity",
  "ForumOwnerExportMapper",
  "at most 512 localized owner calls per fragment",
  "no test, Cargo command",
]) {
  requireText(packet, marker, `${packetPath}: missing ${marker}`);
}

console.log("Forum FORUM-34F bounded owner export reader source: ok");
