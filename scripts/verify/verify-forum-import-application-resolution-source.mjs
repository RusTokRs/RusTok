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
  resolution: "crates/modules/rustok-forum/src/import_resolution.rs",
  inspection: "crates/modules/rustok-forum/src/import_inspection.rs",
  mapping: "crates/modules/rustok-forum/src/import_mapping.rs",
};

const source = Object.fromEntries(
  Object.entries(files).map(([key, path]) => [key, read(path)]),
);

for (const marker of [
  "pub mod import_resolution;",
  "pub use import_resolution::{",
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


const newUuidCalls = source.resolution.split("Uuid::new_v4").length - 1;
if (newUuidCalls !== 0) {
  failures.push(`resolution boundary generated UUIDs: ${newUuidCalls}`);
}

if (failures.length > 0) {
  console.error("Forum FORUM-34K bounded import application resolution source failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum FORUM-34K bounded import application resolution source: ok");
