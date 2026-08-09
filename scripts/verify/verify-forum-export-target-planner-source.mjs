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
  mapping: "crates/rustok-forum/src/export_mapping.rs",
  planner: "crates/rustok-forum/src/export_planner.rs",
  categoryLocales: "crates/rustok-forum/src/services/category_owner_locale_enumeration.rs",
  topicLocales: "crates/rustok-forum/src/services/topic_facade_locale_enumeration.rs",
  replyLocales: "crates/rustok-forum/src/services/reply_facade.rs",
  packet: "docs/modules/forum-34-export-target-planner-actualization-2026-08-09.md",
};

const source = Object.fromEntries(
  Object.entries(files).map(([key, path]) => [key, read(path)]),
);

for (const marker of [
  '#[path = "export_planner.rs"]',
  "mod planner;",
  "pub use planner::*;",
  '#[path = "export_reader.rs"]',
  "pub use reader::*;",
]) {
  need(source.mapping, marker, "export mapping composition");
}

for (const marker of [
  "pub const MAX_FORUM_EXPORT_PLAN_SOURCE_IDS_PER_FRAGMENT: usize =",
  "MAX_FORUM_EXPORT_READ_TARGETS_PER_FRAGMENT;",
  "pub struct ForumExportTargetPlanRequest",
  "pub enum ForumExportTargetPlanError",
  "pub struct ForumExportTargetPlanner;",
  "pub async fn plan_fragment(",
  "if security.is_public_read()",
  "ForumExportTargetPlanError::OperatorContextRequired",
  "ForumExportTargetPlanError::EmptySources",
  "ForumExportTargetPlanError::TooManySourceIds",
  "validate_source_ids(\"category\"",
  "validate_source_ids(\"topic\"",
  "validate_source_ids(\"reply\"",
  "require_requested_manage_scopes(security, request)?;",
  "Resource::ForumCategories",
  "Resource::ForumTopics",
  "Resource::ForumReplies",
  "Action::Manage",
  "PermissionScope::None",
  ".available_locales_for_categories(",
  ".available_locales_for_topics(",
  ".available_locales_for_replies(",
  "LocaleFactCountChanged",
  "LocaleFactIdentityChanged",
  "EmptyLocaleFacts",
  "normalize_locale_code(&locale)",
  "DuplicateLocaleFact",
  "ForumExportTargetPlanError::TooManyTargets",
  "ForumExportReadTarget {",
  "ForumExportReadBatch {",
]) {
  need(source.planner, marker, "export planner");
}

for (const [method, label] of [
  [".available_locales_for_categories(", "category locale call"],
  [".available_locales_for_topics(", "topic locale call"],
  [".available_locales_for_replies(", "reply locale call"],
]) {
  const count = source.planner.split(method).length - 1;
  if (count !== 1) throw new Error(`export planner: expected exactly one ${label}, found ${count}`);
}

const requestStart = source.planner.indexOf("#[derive(Clone, Debug)]\npub struct ForumExportTargetPlanRequest");
const errorStart = source.planner.indexOf("#[derive(Debug, Error)]", requestStart);
if (requestStart < 0 || errorStart <= requestStart) {
  throw new Error("export planner: request non-wire boundary is invalid");
}
const requestContract = source.planner.slice(requestStart, errorStart);
for (const marker of ["Serialize", "Deserialize", "#[serde("]) {
  forbid(requestContract, marker, "export planner request must remain non-wire");
}

for (const marker of [
  "sea_orm",
  "DatabaseConnection",
  "DatabaseTransaction",
  "crate::entities",
  "Entity::",
  "QueryFilter",
  ".get_with_locale_fallback(",
  ".read_fragment(",
  "ForumOwnerExportMapper",
  "ForumExportCategoryRecord {",
  "ForumExportTopicRecord {",
  "ForumExportReplyRecord {",
  "Serialize",
  "Deserialize",
]) {
  forbid(source.planner, marker, "export planner ownership boundary");
}

for (const [text, method, label] of [
  [source.categoryLocales, "pub async fn available_locales_for_categories(", "category locale API"],
  [source.topicLocales, "pub async fn available_locales_for_topics(", "topic locale API"],
  [source.replyLocales, "pub async fn available_locales_for_replies(", "reply locale API"],
]) {
  need(text, method, label);
  need(text, "Action::Manage", label);
}

for (const marker of [
  "FORUM-34H",
  "FORUM-34A through FORUM-34G",
  "still labels `FORUM-34` as `planned`",
  "combined category + topic + reply source-ID count exceeds 512",
  "preflighted for every requested kind before the first owner locale query",
  "category -> topic -> reply",
  "normalize_locale_code",
  "target 513",
  "contains no SeaORM/database/entity access",
  "does not call `ForumOwnerExportReader::read_fragment`",
  "Export eligibility remains a 34F concern",
  "neutral shared migration runner remains absent",
  "no tests, Cargo commands",
]) {
  need(source.packet, marker, "FORUM-34H packet");
}

console.log("Forum FORUM-34H bounded export target planner source: ok");
