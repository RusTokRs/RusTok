import fs from "node:fs";

const testPath = "apps/server/tests/groups_localization_enforcement_concurrency_sqlite.rs";
const docsPath = "crates/rustok-groups/docs/localization-enforcement-concurrency-sqlite-contract.md";
const registryPath = "crates/rustok-groups/contracts/groups-fba-registry.json";
const test = fs.readFileSync(testPath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");
const registry = JSON.parse(fs.readFileSync(registryPath, "utf8"));

function requireText(source, marker, message) {
  if (!source.includes(marker)) throw new Error(message);
}

for (const marker of [
  '#![cfg(feature = "mod-groups")]',
  "const ROUNDS: usize = 12",
  "const SQLITE_BUSY_TIMEOUT_MS: u64 = 5_000",
  "mode=rwc",
  ".max_connections(1)",
  "PRAGMA busy_timeout",
  "PRAGMA journal_mode = WAL",
  "rustok_groups::migrations::migrations()",
  "tokio::sync::Barrier",
  "Barrier::new(3)",
  "connect(&url).await",
  "GroupLocalizationCommandPort::upsert_group_translation",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "owner suspension must serialize to a successful SQLite commit",
  "match localization_result",
  'assert_eq!(error.code, "groups.membership_suspended")',
  "result.group_version < suspension.group_version as u64",
  "a SQLite localization command denied after suspension must not write translation state",
  "GroupMembershipEnforcementReadPort::read_membership_enforcement",
  "GroupMembershipEffectiveStatus::Suspended",
  "effective.membership_revision, Some(2)",
]) {
  requireText(test, marker, `Groups SQLite localization concurrency source is missing ${marker}`);
}

for (const forbidden of [
  "groups.persistence_unavailable",
  "database is locked",
  "UPDATE group_membership_enforcements",
  "DELETE FROM group_membership_enforcements",
  "UPDATE group_memberships SET status",
  "groups:manage",
  "rustok_moderation::",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups SQLite localization concurrency source contains shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "twelve unique fixtures",
  "Localization wins the writer reservation first",
  "Suspension wins the writer reservation first",
  "groups.membership_suspended",
  "SQLite lock acquisition errors are also forbidden outcomes",
  "PRAGMA journal_mode = WAL",
  "PRAGMA busy_timeout = 5000",
  "localization_concurrency",
]) {
  requireText(docs, marker, `Groups SQLite localization concurrency handoff is missing ${marker}`);
}

if (registry?.evidence?.localization_concurrency !== null) {
  throw new Error("unexecuted localization concurrency evidence must remain null");
}

console.log("Groups localization SQLite enforcement-vs-write concurrency source guard passed");
