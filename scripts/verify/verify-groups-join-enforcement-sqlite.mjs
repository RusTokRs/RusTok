import fs from "node:fs";

const testPath = "apps/server/tests/groups_join_enforcement_sqlite.rs";
const contractPath = "crates/rustok-groups/contracts/groups-effective-membership-access.json";
const planPath = "crates/rustok-groups/docs/implementation-plan.md";

const test = fs.readFileSync(testPath, "utf8");
const contract = JSON.parse(fs.readFileSync(contractPath, "utf8"));
const plan = fs.readFileSync(planPath, "utf8");

function requireText(source, marker, message) {
  if (!source.includes(marker)) throw new Error(message);
}

for (const marker of [
  '#![cfg(feature = "mod-groups")]',
  "tempfile::tempdir()",
  "mode=rwc",
  "PRAGMA busy_timeout",
  "PRAGMA journal_mode = WAL",
  ".max_connections(1)",
  "rustok_groups::migrations::migrations()",
  "GroupCommandPort::join_group",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "GroupMembershipEnforcementReadPort::read_membership_enforcement",
  '"groups.membership_suspended"',
  '"groups.membership_enforcement_revision_conflict"',
  "GroupMembershipEffectiveStatus::Suspended",
  "GroupMembershipEffectiveStatus::Inactive",
  "GroupMembershipStatus::Active",
  "const ROUNDS: usize = 8",
  "Barrier::new(3)",
  '("left".to_string(), 2)',
  '("active".to_string(), 2)',
  '("active".to_string(), 3)',
  "base_version + 1",
]) {
  requireText(test, marker, `Groups join/enforcement SQLite evidence is missing ${marker}`);
}

for (const forbidden of [
  "sqlite::memory:",
  "database is locked",
  "groups.persistence_unavailable",
  "INSERT INTO group_membership_enforcements",
  "UPDATE group_memberships SET",
  "rustok_moderation::",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups join/enforcement SQLite evidence contains shortcut ${forbidden}`);
  }
}

if (contract?.converted_source_paths?.join_and_rejoin !== "transaction_aware_effective_membership") {
  throw new Error("join/rejoin contract must retain transaction-aware effective membership");
}
if (contract?.evidence?.join_rejoin_sqlite_source !== testPath) {
  throw new Error("join/rejoin SQLite evidence source is not registered in the existing access contract");
}
if (contract?.evidence?.join_rejoin_postgresql_source !== null) {
  throw new Error("PostgreSQL join/rejoin evidence must remain open until its source is added");
}
if (contract?.evidence?.join_rejoin_runtime !== null) {
  throw new Error("unexecuted join/rejoin runtime evidence must remain null");
}

for (const marker of [
  "Source-complete join/rejoin effective authorization",
  "executed join/rejoin suspension and enforcement-vs-join serialization evidence",
]) {
  requireText(plan, marker, `canonical Groups plan is missing open join evidence gate ${marker}`);
}

console.log("Groups join/enforcement SQLite source evidence guard passed");
