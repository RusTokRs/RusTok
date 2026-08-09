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
  inspection: "crates/rustok-forum/src/import_inspection.rs",
  mapping: "crates/rustok-forum/src/import_mapping.rs",
  packet: "docs/modules/forum-34-import-application-resolution-actualization-2026-08-09.md",
};

const source = Object.fromEntries(
  Object.entries(files).map(([key, path]) => [key, read(path)]),
);

for (const marker of [
  "pub mod import_resolution;",
  "pub use import_resolution::*;",
]) need(source.lib, marker, "forum import resolution export");

for (const marker of [
  "pub const MAX_FORUM_IMPORT_RESOLUTION_BINDINGS_PER_BATCH: usize =",
  "MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH * 2",
  "pub enum ForumImportTargetIdentityKind",
  "Category,",
  "Topic,",
  "Reply,",
  "User,",
  "pub struct ForumImportIdentityBinding",
  "pub struct ForumImportApplicationResolutionRequest",
  "pub struct ForumResolvedImportApplicationBatch",
  "pub struct ForumImportApplicationResolver",
  "pub fn resolve_batch(",
  "normalize_locale_code(&request.locale)",
  "request.tenant_id.is_nil()",
  "TooManyCandidates",
  "TooManyBindings",
  "DuplicateCandidateSource",
  "DuplicateBinding",
  "NilBindingTarget",
  "TargetIdentityCollision",
  "MissingBinding",
  "BindingKindMismatch",
  "UnexpectedBinding",
  "SourceNamespaceMismatch",
  "SourceKindMismatch",
  "UnresolvedDependency",
  "CrossBatchDependency",
  "CategoryCycle",
  "UnresolvedPostRole",
  "MissingTopicBodySource",
  "DeletedTopicBody",
  "TopicBodyAuthorMismatch",
  "ForumImportDependencyRelation::AuthorUser",
  "ForumImportDependencyDisposition::ExternalOwnerResolution",
  "ForumImportTargetIdentityKind::Category",
  "ForumImportTargetIdentityKind::Topic",
  "ForumImportTargetIdentityKind::Reply",
  "ForumImportTargetIdentityKind::User",
  "ForumImportPostRole::TopicBody => continue",
  "body_source: body_post.source.clone()",
  "body: body_post.body.clone()",
  "deleted: candidate.deleted",
]) need(source.resolution, marker, "FORUM-34K resolution source");

for (const marker of [
  "sea_orm",
  "DatabaseConnection",
  "DatabaseTransaction",
  "Entity::",
  "ActiveModel",
  "TransactionalEventBus",
  "CategoryService",
  "TopicService",
  "ReplyService",
  "Uuid::new_v4",
  "Serialize",
  "Deserialize",
  "#[serde(",
]) forbid(source.resolution, marker, "resolution side-effect/non-wire boundary");

for (const marker of [
  "MissingBatchRecord",
  "MismatchedBatchRecord",
  "CyclicBatchRelation",
  "ExternalOwnerResolution",
  "AuthorUser",
]) need(source.inspection, marker, "34B/34C inspection baseline");

for (const marker of [
  "MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH: usize = 512",
  "ForumImportPostRole",
  "TopicBody",
  "Reply",
  "Unresolved",
  "FORUM_IMPORT_SOURCE_NODEBB",
]) need(source.mapping, marker, "34A mapping baseline");

for (const marker of [
  "FORUM-34K",
  "FORUM-34A through FORUM-34J",
  "still labels `FORUM-34` as `planned`",
  "no neutral shared `ImportRunner` / `ImportJob`",
  "generate fresh UUIDs internally",
  "derived from `SecurityContext`",
  "never calls `Uuid::new_v4`",
  "A NodeBB `post` classified as `TopicBody` does **not** receive a `Reply` UUID",
  "structurally self-contained bounded application batch",
  "Cross-batch category/topic/post resolution remains runner-owned",
  "topic author and its main-post author must resolve to the same optional owner user ID",
  "imports no SeaORM/database type",
  "34A NodeBB mapping -> 34B/34C inspection -> 34K explicit owner identity + application fact resolution",
  "no tests, Cargo commands",
]) need(source.packet, marker, "FORUM-34K packet");

const newUuidCalls = source.resolution.split("Uuid::new_v4").length - 1;
if (newUuidCalls !== 0) {
  throw new Error(`resolution boundary generated UUIDs: ${newUuidCalls}`);
}

console.log("Forum FORUM-34K bounded import application resolution source: ok");
