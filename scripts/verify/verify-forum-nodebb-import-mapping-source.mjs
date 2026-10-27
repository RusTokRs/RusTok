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

const mappingPath = "crates/modules/rustok-forum/src/import_mapping.rs";
const libPath = "crates/modules/rustok-forum/src/lib.rs";
const packetPath = "docs/modules/forum-34-nodebb-mapping-actualization-2026-08-09.md";

const mapping = read(mappingPath);
const lib = read(libPath);
const packet = read(packetPath);

for (const marker of [
  'pub const FORUM_IMPORT_SOURCE_NODEBB: &str = "nodebb";',
  "pub const MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH: usize = 512;",
  "pub struct NodebbExportBatch",
  "pub struct NodebbCategoryRecord",
  "pub struct NodebbTopicRecord",
  "pub struct NodebbPostRecord",
  "pub struct ForumImportExternalRef",
  "pub enum ForumImportPostRole",
  "TopicBody",
  "Reply",
  "Unresolved",
  "pub struct NodebbForumImportMapper;",
  "pub fn map_batch(",
  "ensure_batch_bound(batch)?;",
  "ensure_unique_positive_ids(",
  'key: format!("{kind_name}:{id}"),',
  "positive_optional(record.parent_cid)",
  "author_ref(record.uid)",
  "positive_optional(record.main_pid)",
  "topic_main_posts.get(&record.tid)",
  "Some(Some(_)) => ForumImportPostRole::Reply",
  "Some(None) | None => ForumImportPostRole::Unresolved",
  "post_role_stays_unresolved_when_topic_has_no_main_post",
]) {
  requireText(mapping, marker, `${mappingPath}: missing ${marker}`);
}

// Crate-root re-exports are explicit lists, so a glob string pin cannot prove the public surface:
// require the module to be public *and* actually re-exported from the root.
requirePublicReexport(
  failures,
  lib,
  "import_mapping",
  `${libPath}: missing public mapping export`,
);

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
  "register_runtime_extensions",
]) {
  requireAbsent(
    mapping,
    forbidden,
    `${mappingPath}: runner-neutral mapping must not contain ${forbidden}`,
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
    mapping,
    forbiddenIdentity,
    `${mappingPath}: external NodeBB ids must not become RusTok identities: ${forbiddenIdentity}`,
  );
}

const mapperStart = mapping.indexOf("impl NodebbForumImportMapper");
const testsStart = mapping.indexOf("#[cfg(test)]", mapperStart);
if (mapperStart < 0 || testsStart <= mapperStart) {
  failures.push(`${mappingPath}: mapper source boundary is invalid`);
}
const mapper = mapping.slice(mapperStart, testsStart);
for (const forbidden of [
  "async fn",
  ".await",
  "Transaction",
  "Service::new",
  "PortContext",
  "receipt",
  "checkpoint",
  "scheduler",
]) {
  requireAbsent(mapper, forbidden, `${mappingPath}: mapper must stay side-effect free: ${forbidden}`);
}

for (const marker of [
  "FORUM-34A",
  "shared-runner-blocked",
  "FORUM-33 has no additional truthful source slice",
  "no neutral shared import runner/framework contract",
  "does **not** create a Forum-only runner",
  "NodeBB numeric identities are preserved only as external source references",
  "never manufactures RusTok UUIDs",
  "or the topic has no positive `mainPid`",
  "role is `Unresolved`",
  "final Forum owner validation",
  "Forum export adapter",
  "no tests, Cargo command",
]) {
  requireText(packet, marker, `${packetPath}: missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum source guard failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum FORUM-34A NodeBB import mapping source: ok");
