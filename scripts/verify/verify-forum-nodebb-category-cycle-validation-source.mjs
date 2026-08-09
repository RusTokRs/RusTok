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
const packetPath =
  "docs/modules/forum-34-nodebb-category-cycle-validation-actualization-2026-08-09.md";

const inspection = read(inspectionPath);
const packet = read(packetPath);

for (const marker of [
  "CyclicBatchRelation",
  "let cyclic_category_ids = category_cycle_members(batch, &category_ids);",
  "cyclic_category_ids.contains(&record.cid)",
  "ForumImportDependencyDisposition::CyclicBatchRelation",
  "fn category_cycle_members(",
  ".collect::<BTreeMap<_, _>>();",
  "position_by_category.get(&current).copied()",
  "cyclic.extend(path[position..].iter().copied());",
  "if !category_ids.contains(&parent_id)",
  "completed.extend(path.into_iter());",
  "reports_self_and_multi_node_category_cycles_in_source_order",
  "acyclic_in_batch_category_chain_is_dependency_complete",
  'assert_eq!(issues[0].target.key, "category:1");',
  "ForumImportDependencyDisposition::MissingBatchRecord",
  "MAX_FORUM_IMPORT_DEPENDENCY_ISSUES_PER_BATCH",
]) {
  requireText(inspection, marker, `${inspectionPath}: missing ${marker}`);
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
    `${inspectionPath}: category cycle validation must remain side-effect free: ${forbidden}`,
  );
}

const cycleStart = inspection.indexOf("fn category_cycle_members(");
const testsStart = inspection.indexOf("#[cfg(test)]", cycleStart);
if (cycleStart < 0 || testsStart <= cycleStart) {
  throw new Error(`${inspectionPath}: cycle helper source boundary is invalid`);
}
const cycleHelper = inspection.slice(cycleStart, testsStart);
for (const forbidden of [
  "NodebbForumImportMapper",
  "ForumImportExternalRef",
  "std::fs",
  "serde_json",
  "loop { loop",
]) {
  requireAbsent(
    cycleHelper,
    forbidden,
    `${inspectionPath}: cycle helper must stay bounded to category ids/parents: ${forbidden}`,
  );
}

for (const marker of [
  "FORUM-34C",
  "shared-runner-blocked",
  "self-parent and multi-node category cycles",
  "cyclic_batch_relation",
  "missing external parent remaining `missing_batch_record` rather than cyclic",
  "does not infer that such a reference is cyclic or invalid globally",
  "MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH = 512",
  "MAX_FORUM_IMPORT_DEPENDENCY_ISSUES_PER_BATCH = 1536",
  "no test, Cargo command",
]) {
  requireText(packet, marker, `${packetPath}: missing ${marker}`);
}

console.log("Forum FORUM-34C NodeBB category cycle validation source: ok");
