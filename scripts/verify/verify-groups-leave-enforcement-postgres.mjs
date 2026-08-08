import fs from "node:fs";

const testPath = "apps/server/tests/groups_leave_enforcement_postgres.rs";
const sqliteTestPath = "apps/server/tests/groups_leave_enforcement_sqlite.rs";
const contractPath = "crates/rustok-groups/contracts/groups-effective-membership-access.json";

const test = fs.readFileSync(testPath, "utf8");
const contract = JSON.parse(fs.readFileSync(contractPath, "utf8"));

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
  "GroupCommandPort::leave_group",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "GroupMembershipEnforcementCommandPort::revoke_membership_suspension",
  "GroupMembershipEnforcementReadPort::read_membership_enforcement",
  '"groups.membership_banned"',
  '"groups.membership_enforcement_revision_conflict"',
  "GroupMembershipEffectiveStatus::Suspended",
  "GroupMembershipEffectiveStatus::Inactive",
  "GroupMembershipStatus::Left",
  "const ROUNDS: usize = 8",
  "Barrier::new(3)",
  "tokio::time::timeout(PAIR_TIMEOUT",
  '("banned".to_string(), 1)',
  '("left".to_string(), 3)',
  "active_enforcement_count",
]) {
  requireText(test, marker, `Groups leave/enforcement PostgreSQL evidence is missing ${marker}`);
}

for (const forbidden of [
  "SET search_path",
  "groups.persistence_unavailable",
  "INSERT INTO group_membership_enforcements",
  "UPDATE group_memberships SET",
  "rustok_moderation::",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups leave/enforcement PostgreSQL evidence contains shortcut ${forbidden}`);
  }
}

if (contract?.evidence?.leave_sqlite_source !== sqliteTestPath) {
  throw new Error("SQLite leave/enforcement source must remain registered");
}
if (contract?.evidence?.leave_postgresql_source !== testPath) {
  throw new Error("PostgreSQL leave/enforcement source is not registered");
}
if (contract?.evidence?.leave_runtime !== null) {
  throw new Error("unexecuted leave runtime evidence must remain null");
}
if (contract?.access_semantics?.leave_during_active_suspension !== "allowed_preserves_enforcement_projection") {
  throw new Error("leave-during-suspension contract drifted");
}
if (contract?.access_semantics?.leave_for_legacy_banned_status !== "denied_preserves_ban") {
  throw new Error("legacy-ban leave contract drifted");
}

console.log("Groups leave/enforcement PostgreSQL source evidence guard passed");
