import fs from "node:fs";

const testPath = "apps/server/tests/groups_join_enforcement_postgres.rs";
const sqliteTestPath = "apps/server/tests/groups_join_enforcement_sqlite.rs";
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
  '#[ignore = "requires RUSTOK_GROUPS_TEST_POSTGRES_URL"]',
  "RUSTOK_GROUPS_TEST_POSTGRES_URL",
  "options=-csearch_path%3D",
  "CREATE SCHEMA",
  "DROP SCHEMA",
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
  "tokio::time::timeout(PAIR_TIMEOUT",
  '("left".to_string(), 2)',
  '("active".to_string(), 2)',
  '("active".to_string(), 3)',
  "race_base_version + 1",
]) {
  requireText(test, marker, `Groups join/enforcement PostgreSQL evidence is missing ${marker}`);
}

for (const forbidden of [
  "SET search_path",
  "groups.persistence_unavailable",
  "INSERT INTO group_membership_enforcements",
  "UPDATE group_memberships SET",
  "rustok_moderation::",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups join/enforcement PostgreSQL evidence contains shortcut ${forbidden}`);
  }
}

if (contract?.converted_source_paths?.join_and_rejoin !== "transaction_aware_effective_membership") {
  throw new Error("join/rejoin contract must retain transaction-aware effective membership");
}
if (contract?.evidence?.join_rejoin_sqlite_source !== sqliteTestPath) {
  throw new Error("SQLite join/rejoin evidence source must remain registered");
}
if (contract?.evidence?.join_rejoin_postgresql_source !== testPath) {
  throw new Error("PostgreSQL join/rejoin evidence source is not registered in the existing access contract");
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

console.log("Groups join/enforcement PostgreSQL source evidence guard passed");
