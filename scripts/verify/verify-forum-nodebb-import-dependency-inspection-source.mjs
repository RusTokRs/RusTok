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

const inspectionPath = "crates/rustok-forum/src/import_inspection.rs";
const libPath = "crates/rustok-forum/src/lib.rs";
const packetPath =
  "docs/modules/forum-34-nodebb-dependency-inspection-actualization-2026-08-09.md";

const inspection = read(inspectionPath);
const lib = read(libPath);
const packet = read(packetPath);

for (const marker of [
  "pub const MAX_FORUM_IMPORT_DEPENDENCY_ISSUES_PER_BATCH: usize =",
  "MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH * 3;",
  "pub enum ForumImportDependencyRelation",
  "CategoryParent",
  "TopicCategory",
  "TopicMainPost",
  "PostTopic",
  "AuthorUser",
  "pub enum ForumImportDependencyDisposition",
  "MissingBatchRecord",
  "MismatchedBatchRecord",
  "ExternalOwnerResolution",
  "pub struct ForumImportDependencyIssue",
  "pub struct NodebbForumImportInspection",
  "pub struct NodebbForumImportInspector;",
  "pub fn inspect_batch(",
  "let candidates = NodebbForumImportMapper.map_batch(batch)?;",
  ".collect::<BTreeSet<_>>();",
  ".collect::<BTreeMap<_, _>>();",
  "record.parent_cid.filter(|value| *value > 0)",
  "!category_ids.contains(&record.cid)",
  "post_topics.get(&main_pid)",
  "*post_topic_id != record.tid",
  "!topic_ids.contains(&record.tid)",
  "candidate.author_source.as_ref()",
  "unresolved_dependencies.len() <= MAX_FORUM_IMPORT_DEPENDENCY_ISSUES_PER_BATCH",
]) {
  requireText(inspection, marker, `${inspectionPath}: missing ${marker}`);
}

for (const marker of [
  "pub mod import_inspection;",
  "pub use import_inspection::*;",
  "pub mod import_mapping;",
  "pub use import_mapping::*;",
]) {
  requireText(lib, marker, `${libPath}: missing public import contract ${marker}`);
}

for (const forbidden of [
  "sea_orm",
  "DatabaseConnection",
  "DatabaseTransaction",
  "Entity::",
  "ActiveModel",
  "Uuid",
  "rustok_media",
  "rustok_profiles",
  "rustok_notifications",
  "rustok_search",
  "rustok_moderation",
  "async fn",
  ".await",
  "PortContext",
  "Service::new",
  "register_runtime_extensions",
  "INSERT ",
  "UPDATE ",
  "DELETE ",
  ".insert(",
  ".update(",
  ".delete(",
]) {
  requireAbsent(
    inspection,
    forbidden,
    `${inspectionPath}: dependency inspection must remain side-effect free: ${forbidden}`,
  );
}

for (const forbiddenIdentity of [
  "Uuid::new",
  "Uuid::parse",
  "user_id: Uuid",
  "tenant_id: Uuid",
  "category_id: Uuid",
  "topic_id: Uuid",
  "reply_id: Uuid",
]) {
  requireAbsent(
    inspection,
    forbiddenIdentity,
    `${inspectionPath}: dependency inspection must not manufacture owner identity: ${forbiddenIdentity}`,
  );
}

for (const marker of [
  "FORUM-34B",
  "shared-runner-blocked",
  "no neutral shared import runner/framework",
  "dry-runnable, resumable, cursor-based, idempotent and bounded",
  "missing_batch_record",
  "mismatched_batch_record",
  "external_owner_resolution",
  "does not treat a missing reference as proof",
  "is not a persistence-admission decision",
  "MAX_FORUM_IMPORT_DEPENDENCY_ISSUES_PER_BATCH = 1536",
  "Forum export adapter over stable bounded owner reads",
  "no test, Cargo command",
]) {
  requireText(packet, marker, `${packetPath}: missing ${marker}`);
}

console.log("Forum FORUM-34B NodeBB import dependency inspection source: ok");
