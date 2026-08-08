import fs from "node:fs";

const testPath = "apps/server/tests/groups_membership_enforcement_concurrency_sqlite.rs";
const docsPath = "crates/rustok-groups/docs/membership-enforcement-concurrency-sqlite-contract.md";
const registryPath = "crates/rustok-groups/contracts/groups-fba-registry.json";
const test = fs.readFileSync(testPath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");
const registry = JSON.parse(fs.readFileSync(registryPath, "utf8"));

function requireText(source, marker, message) {
  if (!source.includes(marker)) throw new Error(message);
}

for (const marker of [
  '#![cfg(feature = "mod-groups")]',
  "tempfile::tempdir()",
  "mode=rwc",
  ".max_connections(1)",
  "PRAGMA busy_timeout",
  "PRAGMA journal_mode = WAL",
  "Barrier::new(3)",
  "tokio::time::timeout(PAIR_TIMEOUT",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "GroupMembershipEnforcementCommandPort::revoke_membership_suspension",
  '"same-key-suspend"',
  "u8::from(left.replayed) + u8::from(right.replayed), 1",
  "assert_same_material_result",
  '"distinct-suspend-left"',
  '"distinct-suspend-right"',
  '"groups.membership_enforcement_revision_conflict"',
  '"baseline-suspend"',
  '"revoke-left"',
  '"revoke-right"',
  "group_snapshot(&fixture_db, fixture).await, (2, 2)",
  "group_snapshot(&fixture_db, fixture).await, (3, 2)",
  "enforcement_snapshot(&fixture_db, fixture).await, (1, 0)",
  "enforcement_snapshot(&fixture_db, fixture).await, (2, 1)",
  "ledger_counts(&fixture_db, fixture).await, (1, 1, 1)",
  "ledger_counts(&fixture_db, fixture).await, (2, 2, 2)",
]) {
  requireText(test, marker, `Groups direct enforcement SQLite concurrency source is missing ${marker}`);
}

for (const forbidden of [
  "groups.persistence_unavailable",
  "database is locked",
  "rustok_moderation::",
  "INSERT INTO group_membership_enforcements",
  "UPDATE group_membership_enforcements",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups direct enforcement SQLite concurrency source contains shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "Same-key concurrent suspension",
  "Distict-key concurrent suspension",
  "Distinct-key concurrent revoke",
  "groups.membership_enforcement_revision_conflict",
  "SQLite serialization",
  "membership_enforcement_command_concurrency",
]) {
  requireText(docs, marker, `Groups direct enforcement SQLite concurrency handoff is missing ${marker}`);
}

if (registry?.evidence?.membership_enforcement_command_concurrency !== null) {
  throw new Error("unexecuted membership enforcement command concurrency evidence must remain null");
}

console.log("Groups direct membership-enforcement SQLite concurrency source guard passed");
