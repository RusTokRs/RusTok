#!/usr/bin/env node

import fs from "node:fs";

import { requirePublicReexport } from "./lib/forum-source-verification.mjs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

const failures = [];

function requireText(text, marker, message) {
  if (!text.includes(marker)) failures.push(message);
}

function requireAbsent(text, marker, message) {
  if (text.includes(marker)) failures.push(message);
}

const inspectionPath = "crates/modules/rustok-forum/src/import_inspection.rs";
const libPath = "crates/modules/rustok-forum/src/lib.rs";
const inspection = read(inspectionPath);
const lib = read(libPath);

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

// Crate-root re-exports are explicit lists, so a glob string pin cannot prove the public surface:
// require the module to be public *and* actually re-exported from the root.
for (const moduleName of ["import_inspection", "import_mapping"]) {
  requirePublicReexport(failures, lib, moduleName, `${libPath}: missing public import contract`);
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
  // Collection inserts (`BTreeMap`/`BTreeSet`) are pure and legitimate here; pin the database write
  // shapes instead so the guard forbids persistence, not in-memory bookkeeping.
  ".exec(",
  ".execute(",
  ".execute_unprepared(",
  ".insert_one(",
  ".update_many(",
  ".delete_many(",
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

if (failures.length > 0) {
  console.error("Forum source guard failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum FORUM-34B NodeBB import dependency inspection source: ok");
